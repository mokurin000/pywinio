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
use tracing_subscriber::filter::LevelFilter;
use winio::prelude::*;

use crate::AppError;
use crate::layout;
use crate::model::{AppRuntime, LayoutSpec, Shared, WidgetNode, WidgetPlan};
use crate::widget::{AnyInit, AnyWidget, Evt, TimerInit, TimerWidget};

/// Messages accepted by the root component.
pub enum Msg {
    Event(Evt),
    Quit,
}

/// Everything the root component needs to materialize the Python tree.
/// Everything the root component needs to materialize the Python tree.
#[derive(Clone, Copy)]
enum Callback {
    Click,
    Change,
    Select,
    Tick,
}

pub struct RootInit {
    pub shared: Arc<Shared>,
    pub layout: Arc<LayoutSpec>,
    pub widgets: Vec<WidgetPlan>,
    pub timers: Vec<Arc<WidgetNode>>,
    pub title: String,
    pub size: Size,
}

pub struct PyRoot {
    window: Child<Window>,
    layout: Arc<LayoutSpec>,
    widgets: HashMap<u64, Child<AnyWidget>>,
    timers: HashMap<u64, Child<TimerWidget>>,
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
            widgets: plans,
            timers: timer_specs,
            title,
            size,
        } = init;

        let mut window = Child::<Window>::init(()).await?;
        window.set_text(&title)?;
        window.set_size(size)?;

        let mut widgets: HashMap<u64, Child<AnyWidget>> = HashMap::with_capacity(plans.len());
        let tab_selections: Vec<(u64, usize)> = plans
            .iter()
            .filter(|plan| plan.node.kind == crate::model::WidgetKind::TabView)
            .filter_map(|plan| {
                plan.node
                    .props()
                    .selected
                    .map(|selection| (plan.node.key, selection))
            })
            .collect();

        // Create widgets in dependency order: a TabViewItem needs its TabView,
        // and tab-page content needs its TabViewItem.
        let mut pending: Vec<Option<WidgetPlan>> = plans.iter().cloned().map(Some).collect();
        let mut created = 0;
        while created < pending.len() {
            let mut progress = false;
            for slot in &mut pending {
                let Some(plan) = slot.take() else {
                    continue;
                };
                let node = plan.node.clone();
                let child = {
                    let mut tab_view: Option<&TabView> = None;
                    let parent: BorrowedContainer<'_> = if let Some(key) = plan.tab_view_parent {
                        let Some(tab) = widgets.get(&key).and_then(|widget| widget.as_tab_view())
                        else {
                            *slot = Some(plan);
                            continue;
                        };
                        tab_view = Some(tab);
                        (&window).into()
                    } else if let Some(key) = plan.tab_item_parent {
                        let Some(parent) = widgets
                            .get(&key)
                            .and_then(|widget| widget.as_tab_item_container())
                        else {
                            *slot = Some(plan);
                            continue;
                        };
                        parent
                    } else if let Some(key) = plan.scroll_parent {
                        let Some(parent) = widgets
                            .get(&key)
                            .and_then(|widget| widget.as_scroll_container())
                        else {
                            *slot = Some(plan);
                            continue;
                        };
                        parent
                    } else {
                        (&window).into()
                    };
                    Child::<AnyWidget>::init(AnyInit {
                        node: node.clone(),
                        parent,
                        tab_view,
                    })
                    .await?
                };
                widgets.insert(node.key, child);
                created += 1;
                progress = true;
            }
            if !progress {
                return Err(AppError::layout(
                    "cannot resolve widget parent hierarchy".to_string(),
                ));
            }
        }

        for (key, selection) in tab_selections {
            if let Some(widget) = widgets.get_mut(&key) {
                widget.select_tab(selection)?;
            }
        }

        let mut timers: HashMap<u64, Child<TimerWidget>> =
            HashMap::with_capacity(timer_specs.len());
        for node in &timer_specs {
            let child = Child::<TimerWidget>::init(TimerInit { node: node.clone() }).await?;
            timers.insert(node.key, child);
        }

        // Publish the per-widget senders so Python handles can post commands.
        {
            let mut guard = shared.runtime.lock().unwrap();
            if let Some(runtime) = guard.as_mut() {
                for (key, child) in &widgets {
                    runtime.senders.insert(*key, child.sender().clone());
                }
                for (key, child) in &timers {
                    runtime.timer_senders.insert(*key, child.sender().clone());
                }
            }
        }

        let _ = &shared;

        window.show()?;

        let mut nodes: HashMap<u64, Arc<WidgetNode>> = plans
            .iter()
            .map(|plan| (plan.node.key, plan.node.clone()))
            .collect();
        nodes.extend(timer_specs.iter().map(|node| (node.key, node.clone())));

        Ok(Self {
            window,
            layout,
            widgets,
            timers,
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

        for timer in self.timers.values_mut() {
            let timer_start = timer.start(sender, |event| Some(Msg::Event(event)));
            futures.push(Box::pin(async move {
                let _: Infallible = timer_start.await;
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
        for timer in self.timers.values_mut() {
            need_render |= timer.update().await?;
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
                self.invoke(key, Callback::Click);
                Ok(false)
            }
            Msg::Event(Evt::Changed(key)) => {
                self.invoke(key, Callback::Change);
                Ok(false)
            }
            Msg::Event(Evt::Selected(key)) => {
                self.invoke(key, Callback::Select);
                Ok(false)
            }
            Msg::Event(Evt::Ticked(key)) => {
                self.invoke(key, Callback::Tick);
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
        self.layout_tab_contents()?;
        self.layout_scroll_contents()?;
        Ok(())
    }

    /// Arrange the content panel of every `TabViewItem` inside the page's
    /// available client area.
    fn layout_tab_contents(&mut self) -> std::result::Result<(), AppError> {
        let tab_keys: Vec<u64> = self
            .widgets
            .iter()
            .filter(|(_, widget)| widget.as_tab_item_container().is_some())
            .map(|(key, _)| *key)
            .collect();

        for key in tab_keys {
            let content = self
                .nodes
                .get(&key)
                .and_then(|node| node.tab_content.lock().unwrap().clone());
            let Some(content) = content else {
                continue;
            };
            let size = self
                .widgets
                .get(&key)
                .ok_or_else(|| AppError::layout(format!("unknown TabViewItem {key}")))?
                .size()?;
            let measured = layout::measure(&content, |child_key| {
                let widget = self
                    .widgets
                    .get(&child_key)
                    .ok_or_else(|| AppError::layout(format!("unknown widget node {child_key}")))?;
                Ok(widget.preferred_size()?)
            })?;
            let rects = layout::arrange(&content, Rect::new(Point::zero(), size), &measured)?;
            for (child_key, rect) in rects {
                if let Some(widget) = self.widgets.get_mut(&child_key) {
                    widget.set_rect(rect)?;
                }
            }
        }
        Ok(())
    }

    /// Arrange the content panel of every `ScrollView` in its own coordinate
    /// space, then refresh the native scroll ranges.
    fn layout_scroll_contents(&mut self) -> std::result::Result<(), AppError> {
        let scroll_keys: Vec<u64> = self
            .widgets
            .iter()
            .filter(|(_, widget)| widget.as_scroll_container().is_some())
            .map(|(key, _)| *key)
            .collect();

        for key in scroll_keys {
            let content = self
                .nodes
                .get(&key)
                .and_then(|node| node.scroll_content.lock().unwrap().clone());
            let Some(content) = content else {
                continue;
            };

            let viewport = self
                .widgets
                .get(&key)
                .ok_or_else(|| AppError::layout(format!("unknown ScrollView {key}")))?
                .rect()?;

            let measured = layout::measure(&content, |child_key| {
                let widget = self
                    .widgets
                    .get(&child_key)
                    .ok_or_else(|| AppError::layout(format!("unknown widget node {child_key}")))?;
                Ok(widget.preferred_size()?)
            })?;
            let preferred = measured
                .panels
                .get(&content.uid)
                .copied()
                .unwrap_or_else(Size::zero);

            let props = self.nodes[&key].props();
            let content_size = Size::new(
                if props.hscroll {
                    preferred.width.max(viewport.size.width)
                } else {
                    viewport.size.width
                },
                if props.vscroll {
                    preferred.height.max(viewport.size.height)
                } else {
                    viewport.size.height
                },
            );
            let rects =
                layout::arrange(&content, Rect::new(Point::zero(), content_size), &measured)?;
            for (child_key, rect) in rects {
                if let Some(widget) = self.widgets.get_mut(&child_key) {
                    widget.set_rect(rect)?;
                }
            }
            // ScrollView computes its scroll range by enumerating child
            // windows, so resize it again after placing the content.
            if let Some(widget) = self.widgets.get_mut(&key) {
                widget.set_size(viewport.size)?;
            }
        }
        Ok(())
    }

    /// Run the Python callback registered for `key`, if any.
    ///
    /// A Python exception is printed and does not tear down the UI loop.
    fn invoke(&self, key: u64, which: Callback) {
        let Some(node) = self.nodes.get(&key) else {
            return;
        };
        Python::attach(|py| {
            let slot = match which {
                Callback::Click => &node.on_click,
                Callback::Change => &node.on_change,
                Callback::Select => &node.on_select,
                Callback::Tick => &node.on_tick,
            };
            let callback = slot
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
    widgets: Vec<WidgetPlan>,
    timers: Vec<Arc<WidgetNode>>,
    title: String,
    size: (f64, f64),
) -> std::result::Result<(), String> {
    tracing_subscriber::fmt()
        .with_max_level(LevelFilter::INFO)
        .init();

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
            timers,
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
