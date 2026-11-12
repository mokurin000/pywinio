//! Declarative model shared by the Python side and the winio component side.
//!
//! Python builds a tree of [`WidgetNode`]s and [`LayoutSpec`]s before the
//! window exists. The tree is plain data (plus Python callables), so it can be
//! created and inspected from Python and then handed to the winio root
//! component when `App.run()` is called.

use std::sync::atomic::{AtomicIsize, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use pyo3::prelude::*;
use winio::prelude::{ComponentSender, HAlign, Orient, VAlign};

use crate::root::{Msg, PyRoot};
use crate::widget::{AnyWidget, Cmd};

/// Identifier of a widget node, unique inside one [`Shared`].
pub type NodeKey = u64;

/// Identifier of a layout container, unique inside one [`Shared`].
pub type LayoutKey = u64;

/// Which native widget a [`WidgetNode`] stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetKind {
    Button,
    Label,
}

/// Properties mirrored on the Python side.
///
/// The widget keeps the authoritative values once it is created; this struct
/// is the initial value used by `AnyWidget::init` and the shadow value
/// returned by Python getters.
#[derive(Debug, Clone)]
pub struct Props {
    pub text: String,
    pub enabled: bool,
    pub visible: bool,
}

/// A pre-registered widget.
///
/// Python holds one `Arc<WidgetNode>` per `Button`/`Label`, and so does the
/// layout tree. The node itself is immutable except for its property shadow
/// and its callback, both behind mutexes.
pub struct WidgetNode {
    pub key: NodeKey,
    pub id: String,
    pub kind: WidgetKind,
    pub props: Mutex<Props>,
    pub on_click: Mutex<Option<Py<PyAny>>>,
}

impl WidgetNode {
    pub fn new(key: NodeKey, id: String, kind: WidgetKind, text: String) -> Self {
        Self {
            key,
            id,
            kind,
            props: Mutex::new(Props {
                text,
                enabled: true,
                visible: true,
            }),
            on_click: Mutex::new(None),
        }
    }

    pub fn props(&self) -> Props {
        self.props.lock().unwrap().clone()
    }
}

/// Four-sided spacing, in logical pixels.
#[derive(Debug, Clone, Copy, Default)]
pub struct Edges {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl Edges {
    pub fn all(v: f64) -> Self {
        Self {
            left: v,
            top: v,
            right: v,
            bottom: v,
        }
    }

    pub fn new(left: f64, top: f64, right: f64, bottom: f64) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub fn horizontal(self) -> f64 {
        self.left + self.right
    }

    pub fn vertical(self) -> f64 {
        self.top + self.bottom
    }
}

/// A grid track definition, mirroring winio's `Grid::from_str` notation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Track {
    Fixed(f64),
    Auto,
    /// Weighted share of the remaining space (`"1*"`, `"2*"`, `"*"`).
    Weight(f64),
}

impl Track {
    pub fn parse(s: &str) -> Result<Self, String> {
        let s = s.trim();
        if s.is_empty() {
            return Err("empty grid track".into());
        }
        if s.eq_ignore_ascii_case("auto") {
            return Ok(Self::Auto);
        }
        if let Some(weight) = s.strip_suffix('*') {
            let weight = weight.trim();
            let weight = if weight.is_empty() {
                1.0
            } else {
                weight
                    .parse::<f64>()
                    .map_err(|_| format!("invalid grid weight {s:?}"))?
            };
            if !(weight.is_finite() && weight >= 0.0) {
                return Err(format!("invalid grid weight {s:?}"));
            }
            return Ok(Self::Weight(weight));
        }
        let v = s
            .parse::<f64>()
            .map_err(|_| format!("invalid grid track {s:?}"))?;
        if !(v.is_finite() && v >= 0.0) {
            return Err(format!("invalid grid track {s:?}"));
        }
        Ok(Self::Fixed(v))
    }

    pub fn parse_list(spec: &str) -> Result<Vec<Self>, String> {
        let mut out = Vec::new();
        for part in spec.split(',') {
            if part.trim().is_empty() {
                continue;
            }
            out.push(Self::parse(part)?);
        }
        if out.is_empty() {
            out.push(Self::Weight(1.0));
        }
        Ok(out)
    }
}

/// Layout hint for one child of a [`LayoutSpec`].
#[derive(Debug, Clone)]
pub struct ItemOpts {
    pub grow: bool,
    pub margin: Edges,
    pub halign: HAlign,
    pub valign: VAlign,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub row: usize,
    pub col: usize,
    pub row_span: usize,
    pub col_span: usize,
}

impl Default for ItemOpts {
    fn default() -> Self {
        Self {
            grow: false,
            margin: Edges::default(),
            halign: HAlign::Stretch,
            valign: VAlign::Stretch,
            width: None,
            height: None,
            row: 0,
            col: 0,
            row_span: 1,
            col_span: 1,
        }
    }
}

/// What a layout item points at.
#[derive(Clone)]
pub enum Elem {
    Widget(Arc<WidgetNode>),
    Panel(Arc<LayoutSpec>),
}

/// One entry of a container's child list.
#[derive(Clone)]
pub struct LayoutItem {
    pub elem: Elem,
    pub opts: ItemOpts,
}

/// The kind-specific data of a container.
#[derive(Debug, Clone)]
pub enum LayoutKind {
    Stack {
        orient: Orient,
        spacing: f64,
        padding: Edges,
    },
    Grid {
        columns: Vec<Track>,
        rows: Vec<Track>,
        col_gap: f64,
        row_gap: f64,
        padding: Edges,
    },
}

/// A container in the layout tree.
///
/// Children live behind a mutex so Python can keep building a container after
/// it has been nested into another one.
pub struct LayoutSpec {
    pub uid: LayoutKey,
    pub kind: LayoutKind,
    pub items: Mutex<Vec<LayoutItem>>,
}

impl LayoutSpec {
    pub fn stack(uid: LayoutKey, orient: Orient, spacing: f64, padding: Edges) -> Self {
        Self {
            uid,
            kind: LayoutKind::Stack {
                orient,
                spacing,
                padding,
            },
            items: Mutex::new(Vec::new()),
        }
    }

    pub fn grid(
        uid: LayoutKey,
        columns: Vec<Track>,
        rows: Vec<Track>,
        col_gap: f64,
        row_gap: f64,
        padding: Edges,
    ) -> Self {
        Self {
            uid,
            kind: LayoutKind::Grid {
                columns,
                rows,
                col_gap,
                row_gap,
                padding,
            },
            items: Mutex::new(Vec::new()),
        }
    }

    pub fn push(&self, elem: Elem, opts: ItemOpts) {
        self.items.lock().unwrap().push(LayoutItem { elem, opts });
    }
}

/// Collect every widget referenced by the layout tree, in depth-first order.
///
/// Rejects duplicate widget entries (one native widget cannot live in two
/// places) and cycles in the container graph.
pub fn collect_widgets(root: &Arc<LayoutSpec>) -> Result<Vec<Arc<WidgetNode>>, String> {
    let mut widgets = Vec::new();
    let mut keys = std::collections::HashSet::new();
    let mut panels = std::collections::HashSet::new();
    collect_inner(root, &mut widgets, &mut keys, &mut panels)?;
    Ok(widgets)
}

fn collect_inner(
    node: &Arc<LayoutSpec>,
    widgets: &mut Vec<Arc<WidgetNode>>,
    keys: &mut std::collections::HashSet<NodeKey>,
    panels: &mut std::collections::HashSet<LayoutKey>,
) -> Result<(), String> {
    if !panels.insert(node.uid) {
        return Err(format!(
            "layout cycle detected: container {} is reachable twice",
            node.uid
        ));
    }
    for item in node.items.lock().unwrap().iter() {
        match &item.elem {
            Elem::Widget(w) => {
                if !keys.insert(w.key) {
                    return Err(format!(
                        "widget {:?} ({}) is used more than once in the layout",
                        w.id, w.key
                    ));
                }
                widgets.push(w.clone());
            }
            Elem::Panel(p) => collect_inner(p, widgets, keys, panels)?,
        }
    }
    Ok(())
}

/// Runtime handles published by the winio component, shared with Python.
#[derive(Default)]
pub struct AppRuntime {
    /// One sender per widget node, so Python can post commands directly.
    pub senders: std::collections::HashMap<NodeKey, ComponentSender<AnyWidget>>,
    /// The root component's sender, used by `App.quit()`.
    pub root: Option<ComponentSender<PyRoot>>,
}

/// State shared between Python objects and the winio component tree.
pub struct Shared {
    pub runtime: Mutex<Option<AppRuntime>>,
    /// Native top-level window handle, used to wake the event loop when a
    /// command is posted from another thread.
    hwnd: AtomicIsize,
    next_node: AtomicU64,
    next_layout: AtomicU64,
}

impl Shared {
    pub fn new() -> Self {
        Self {
            runtime: Mutex::new(None),
            hwnd: AtomicIsize::new(0),
            next_node: AtomicU64::new(1),
            next_layout: AtomicU64::new(1),
        }
    }

    pub fn next_node_key(&self) -> NodeKey {
        self.next_node.fetch_add(1, Ordering::Relaxed)
    }

    pub fn next_layout_key(&self) -> LayoutKey {
        self.next_layout.fetch_add(1, Ordering::Relaxed)
    }

    pub fn set_window(&self, hwnd: isize) {
        self.hwnd.store(hwnd, Ordering::Relaxed);
    }

    pub fn clear_window(&self) {
        self.hwnd.store(0, Ordering::Relaxed);
    }

    /// Wake a message loop blocked in `MsgWaitForMultipleObjectsEx`.
    ///
    /// The channel waker alone is not reliable on win32 when the post comes
    /// from another thread; posting a no-op message to the top-level window is
    /// the canonical way to wake the loop.
    fn wake(&self) {
        let hwnd = self.hwnd.load(Ordering::Relaxed);
        if hwnd == 0 {
            return;
        }
        #[cfg(windows)]
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(
                hwnd as windows_sys::Win32::Foundation::HWND,
                windows_sys::Win32::UI::WindowsAndMessaging::WM_NULL,
                0,
                0,
            );
        }
    }

    /// Post a command to a widget. No-op when the window is not running yet.
    pub fn post(&self, key: NodeKey, cmd: Cmd) {
        if let Some(rt) = self.runtime.lock().unwrap().as_ref()
            && let Some(sender) = rt.senders.get(&key)
        {
            sender.post(cmd);
            self.wake();
        }
    }

    /// Ask the root component to close the window.
    pub fn quit(&self) {
        if let Some(rt) = self.runtime.lock().unwrap().as_ref()
            && let Some(sender) = &rt.root
        {
            sender.post(Msg::Quit);
            self.wake();
        }
    }
}

impl Default for Shared {
    fn default() -> Self {
        Self::new()
    }
}

/// Default window size used when Python does not pass one.
pub const DEFAULT_SIZE: (f64, f64) = (720.0, 480.0);
