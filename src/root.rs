//! The root winio component.
//!
//! `PyRoot` is the only component that knows about the Python-defined tree. It
//! creates the window and every native widget, wires all child event streams
//! generically (`start` + `select_all`), re-runs layout whenever a child asks
//! for a repaint, and dispatches widget events to Python callbacks.
//!
//! None of this is visible from Python.

use std::collections::HashMap;
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use futures_util::StreamExt;
use pyo3::prelude::*;
use winio::prelude::*;

use crate::AppError;
use crate::layout;
use crate::model::{AppRuntime, LayoutSpec, Shared, WidgetNode};
use crate::widget::{AnyInit, AnyWidget, Evt};

/// Messages accepted by the root component.
pub enum Msg {
    Event(Evt),
    Quit,
}

/// Everything the root component needs to materialize the Python tree.
pub struct RootInit {
    pub shared: Arc<Shared>,
    pub layout: Arc<LayoutSpec>,
    pub widgets: Vec<Arc<WidgetNode>>,
    pub title: String,
    pub size: Size,
}

pub struct PyRoot {
    window: Child<Window>,
    layout: Arc<LayoutSpec>,
    widgets: HashMap<u64, Child<AnyWidget>>,
    nodes: HashMap<u64, Arc<WidgetNode>>,
}

impl Component for PyRoot {
    type Error = AppError;
    type Event = Evt;
    type Init<'a> = RootInit;
    type Message = Msg;

    async fn init(
        init: Self::Init<'_>,
        _sender: &ComponentSender<Self>,
    ) -> std::result::Result<Self, AppError> {
        let RootInit {
            shared,
            layout,
            widgets: node_specs,
            title,
            size,
        } = init;

        let mut window = Child::<Window>::init(()).await?;
        window.set_text(&title)?;
        window.set_size(size)?;

        let mut widgets = HashMap::with_capacity(node_specs.len());
        for node in &node_specs {
            let child = Child::<AnyWidget>::init(AnyInit {
                node: node.clone(),
                parent: &window,
            })
            .await?;
            widgets.insert(node.key, child);
        }

        // Publish the per-widget senders so Python handles can post commands.
        {
            let mut guard = shared.runtime.lock().unwrap();
            if let Some(runtime) = guard.as_mut() {
                for (key, child) in &widgets {
                    runtime.senders.insert(*key, child.sender().clone());
                }
            }
        }

        window.show()?;

        let nodes = node_specs
            .into_iter()
            .map(|node| (node.key, node))
            .collect();

        Ok(Self {
            window,
            layout,
            widgets,
            nodes,
        })
    }

    async fn start(&mut self, sender: &ComponentSender<Self>) -> ! {
        let mut futures: Vec<Pin<Box<dyn Future<Output = ()> + '_>>> = Vec::new();

        // The window itself: close and resize are forwarded to the root.
        let window_start = self.window.start(sender, |event| match event {
            WindowEvent::Close => Some(Msg::Event(Evt::Closed)),
            WindowEvent::Resize => Some(Msg::Event(Evt::Resized)),
            _ => None,
        });
        futures.push(Box::pin(async move {
            let _: Infallible = window_start.await;
        }));

        for child in self.widgets.values_mut() {
            let child_start = child.start(sender, |event| Some(Msg::Event(event)));
            futures.push(Box::pin(async move {
                let _: Infallible = child_start.await;
            }));
        }

        match futures_util::future::select_all(futures).await {
            ((), _, _) => unreachable!("component start futures never complete"),
        }
    }

    async fn update_children(&mut self) -> std::result::Result<bool, AppError> {
        let mut need_render = false;
        for child in self.widgets.values_mut() {
            need_render |= child.update().await?;
        }
        Ok(need_render)
    }

    async fn update(
        &mut self,
        message: Self::Message,
        sender: &ComponentSender<Self>,
    ) -> std::result::Result<bool, AppError> {
        match message {
            Msg::Quit | Msg::Event(Evt::Closed) => {
                self.window.hide()?;
                sender.output(Evt::Closed);
                Ok(false)
            }
            Msg::Event(Evt::Resized) => Ok(true),
            Msg::Event(Evt::Clicked(key)) => {
                self.invoke_click(key);
                Ok(false)
            }
        }
    }

    fn render_children(&mut self) -> std::result::Result<(), AppError> {
        // Any child repaint can change a preferred size (e.g. a label whose
        // text changed), so re-run layout before painting the children.
        let size = self.window.client_size()?;
        self.relayout(size)?;
        for child in self.widgets.values_mut() {
            child.render()?;
        }
        Ok(())
    }
}

impl PyRoot {
    fn relayout(&mut self, size: Size) -> std::result::Result<(), AppError> {
        let measured = layout::measure(&self.layout, |key| {
            let widget = self
                .widgets
                .get(&key)
                .ok_or_else(|| AppError::layout(format!("unknown widget node {key}")))?;
            Ok(widget.preferred_size()?)
        })?;

        let rects = layout::arrange(&self.layout, Rect::new(Point::zero(), size), &measured)?;

        for (key, rect) in rects {
            if let Some(widget) = self.widgets.get_mut(&key) {
                widget.set_rect(rect)?;
            }
        }
        Ok(())
    }

    /// Run the Python callback registered for `key`, if any.
    ///
    /// A Python exception is printed and does not tear down the UI loop.
    fn invoke_click(&self, key: u64) {
        let Some(node) = self.nodes.get(&key) else {
            return;
        };
        Python::attach(|py| {
            let callback = node
                .on_click
                .lock()
                .unwrap()
                .as_ref()
                .map(|callback| callback.clone_ref(py));
            if let Some(callback) = callback
                && let Err(error) = callback.bind(py).call0()
            {
                error.print(py);
            }
        });
    }
}

/// Build the winio application, drive the component tree, and return when the
/// window is closed.
///
/// Runs on the Python thread with the GIL released (see `PyApp::run`).
pub fn run_app(
    shared: Arc<Shared>,
    layout: Arc<LayoutSpec>,
    widgets: Vec<Arc<WidgetNode>>,
    title: String,
    size: (f64, f64),
) -> std::result::Result<(), String> {
    let app = winio::ui::App::builder()
        .name(&title)
        .build()
        .map_err(|error| format!("cannot create winio application: {error:?}"))?;

    app.block_on(async move {
        *shared.runtime.lock().unwrap() = Some(AppRuntime::default());

        let mut root = Root::<PyRoot>::init(RootInit {
            shared: shared.clone(),
            layout,
            widgets,
            title,
            size: Size::new(size.0, size.1),
        })
        .await
        .map_err(|error| format!("cannot initialize the window: {error}"))?;

        if let Some(runtime) = shared.runtime.lock().unwrap().as_mut() {
            runtime.root = Some(root.sender().clone());
        }

        let mut stream = std::pin::pin!(root.run());
        while let Some(event) = stream.next().await {
            match event {
                RunEvent::Event(Evt::Closed) => break,
                RunEvent::Event(_) => {}
                RunEvent::UpdateErr(error) => eprintln!("pywinio: update error: {error}"),
                RunEvent::RenderErr(error) => eprintln!("pywinio: render error: {error}"),
                _ => {}
            }
        }

        *shared.runtime.lock().unwrap() = None;
        Ok::<(), String>(())
    })
}
