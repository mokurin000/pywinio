//! Declarative model shared by the Python side and the winio component side.
//!
//! Python builds a tree of [`WidgetNode`]s and [`LayoutSpec`]s before the
//! window exists. The tree is plain data (plus Python callables), so it can be
//! created and inspected from Python and then handed to the winio root
//! component when `App.run()` is called.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use pyo3::prelude::*;
use winio::prelude::{ComponentSender, HAlign, Orient, VAlign};

use crate::root::{Msg, PyRoot};
use crate::widget::{AnyWidget, Cmd, TimerWidget};

/// Identifier of a widget node, unique inside one [`Shared`].
pub type NodeKey = u64;

/// Identifier of a layout container, unique inside one [`Shared`].
pub type LayoutKey = u64;

/// Which native widget a [`WidgetNode`] stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetKind {
    Button,
    Label,
    Edit,
    CheckBox,
    LinkLabel,
    Progress,
    TextBox,
    Slider,
    UpDownEdit,
    ComboBox,
    RadioButton,
    ListBox,
    ScrollView,
    Timer,
    TabView,
    TabViewItem,
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
    pub checked: bool,
    pub uri: String,
    pub pos: usize,
    pub minimum: usize,
    pub maximum: usize,
    pub indeterminate: bool,
    pub value: usize,
    pub step: usize,
    pub items: Vec<String>,
    pub selected: Option<usize>,
    pub selected_indices: Vec<usize>,
    pub multiple: bool,
    pub editable: bool,
    pub hscroll: bool,
    pub vscroll: bool,
    pub interval_ms: u64,
    pub tooltip: String,
}

impl Default for Props {
    fn default() -> Self {
        Self {
            text: String::new(),
            enabled: true,
            visible: true,
            checked: false,
            uri: String::new(),
            pos: 0,
            minimum: 0,
            maximum: 100,
            indeterminate: false,
            value: 0,
            step: 1,
            items: Vec::new(),
            selected: None,
            selected_indices: Vec::new(),
            multiple: false,
            editable: false,
            hscroll: true,
            vscroll: true,
            interval_ms: 500,
            tooltip: String::new(),
        }
    }
}

/// A pre-registered widget.
///
/// Python holds one `Arc<WidgetNode>` per widget, and so does the layout tree.
/// The node itself is immutable except for its property shadow and its
/// callbacks, both behind mutexes.
pub struct WidgetNode {
    pub key: NodeKey,
    pub id: String,
    pub kind: WidgetKind,
    pub props: Mutex<Props>,
    pub on_click: Mutex<Option<Py<PyAny>>>,
    pub on_change: Mutex<Option<Py<PyAny>>>,
    pub on_select: Mutex<Option<Py<PyAny>>>,
    pub on_tick: Mutex<Option<Py<PyAny>>>,
    pub scroll_content: Mutex<Option<Arc<LayoutSpec>>>,
    pub tab_items: Mutex<Vec<Arc<WidgetNode>>>,
    pub tab_content: Mutex<Option<Arc<LayoutSpec>>>,
}

impl WidgetNode {
    pub fn new(key: NodeKey, id: String, kind: WidgetKind, text: String) -> Self {
        Self {
            key,
            id,
            kind,
            props: Mutex::new(Props {
                text,
                ..Props::default()
            }),
            on_click: Mutex::new(None),
            on_change: Mutex::new(None),
            on_select: Mutex::new(None),
            on_tick: Mutex::new(None),
            scroll_content: Mutex::new(None),
            tab_items: Mutex::new(Vec::new()),
            tab_content: Mutex::new(None),
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

/// One widget to materialize, plus the native container it is parented to.
#[derive(Clone)]
pub struct WidgetPlan {
    pub node: Arc<WidgetNode>,
    pub scroll_parent: Option<NodeKey>,
    pub tab_view_parent: Option<NodeKey>,
    pub tab_item_parent: Option<NodeKey>,
}

/// Collect every widget referenced by the layout tree, in depth-first order.
///
/// Rejects duplicate widget entries (one native widget cannot live in two
/// places) and cycles in the container graph. Widgets inside a `ScrollView`
/// content panel carry its key as `scroll_parent`; widgets inside a `TabView`
/// page carry the `TabViewItem` key as `tab_item_parent`, and the item itself
/// carries `tab_view_parent`.
pub fn collect_widgets(root: &Arc<LayoutSpec>) -> Result<Vec<WidgetPlan>, String> {
    let mut widgets = Vec::new();
    let mut keys = std::collections::HashSet::new();
    let mut panels = std::collections::HashSet::new();
    collect_inner(root, None, None, &mut widgets, &mut keys, &mut panels)?;
    Ok(widgets)
}

fn collect_inner(
    node: &Arc<LayoutSpec>,
    scroll_parent: Option<NodeKey>,
    tab_item_parent: Option<NodeKey>,
    widgets: &mut Vec<WidgetPlan>,
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
                widgets.push(WidgetPlan {
                    node: w.clone(),
                    scroll_parent,
                    tab_view_parent: None,
                    tab_item_parent,
                });
                if w.kind == WidgetKind::ScrollView
                    && let Some(content) = w.scroll_content.lock().unwrap().clone()
                {
                    collect_inner(&content, Some(w.key), None, widgets, keys, panels)?;
                }
                if w.kind == WidgetKind::TabView {
                    for tab in w.tab_items.lock().unwrap().iter() {
                        if !keys.insert(tab.key) {
                            return Err(format!(
                                "tab item {:?} ({}) is used more than once",
                                tab.id, tab.key
                            ));
                        }
                        widgets.push(WidgetPlan {
                            node: tab.clone(),
                            scroll_parent: None,
                            tab_view_parent: Some(w.key),
                            tab_item_parent: None,
                        });
                        if let Some(content) = tab.tab_content.lock().unwrap().clone() {
                            collect_inner(&content, None, Some(tab.key), widgets, keys, panels)?;
                        }
                    }
                }
            }
            Elem::Panel(p) => {
                collect_inner(p, scroll_parent, tab_item_parent, widgets, keys, panels)?
            }
        }
    }
    Ok(())
}

/// Runtime handles published by the winio component, shared with Python.
#[derive(Default)]
pub struct AppRuntime {
    /// One sender per widget node, so Python can post commands directly.
    pub senders: std::collections::HashMap<NodeKey, ComponentSender<AnyWidget>>,
    /// One sender per headless timer node.
    pub timer_senders: std::collections::HashMap<NodeKey, ComponentSender<TimerWidget>>,
    /// The root component's sender, used by `App.quit()`.
    pub root: Option<ComponentSender<PyRoot>>,
}

/// State shared between Python objects and the winio component tree.
pub struct Shared {
    pub runtime: Mutex<Option<AppRuntime>>,
    next_node: AtomicU64,
    next_layout: AtomicU64,
}

impl Shared {
    pub fn new() -> Self {
        Self {
            runtime: Mutex::new(None),
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

    /// Post a command to a widget. No-op when the window is not running yet.
    pub fn post(&self, key: NodeKey, cmd: Cmd) {
        if let Some(rt) = self.runtime.lock().unwrap().as_ref()
            && let Some(sender) = rt.senders.get(&key)
        {
            sender.post(cmd);
        }
    }

    /// Post a command to a headless timer.
    pub fn post_timer(&self, key: NodeKey, cmd: Cmd) {
        if let Some(rt) = self.runtime.lock().unwrap().as_ref()
            && let Some(sender) = rt.timer_senders.get(&key)
        {
            sender.post(cmd);
        }
    }

    /// Ask the root component to close the window.
    pub fn quit(&self) {
        if let Some(rt) = self.runtime.lock().unwrap().as_ref()
            && let Some(sender) = &rt.root
        {
            sender.post(Msg::Quit);
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
