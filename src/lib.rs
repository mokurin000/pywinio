//! pywinio: minimal PyO3 bindings for [winio](https://docs.rs/winio).
//!
//! This first PoC keeps the Python surface declarative:
//!
//! ```python
//! import pywinio as w
//!
//! app = w.App("Counter", (640, 480))
//! root = app.stack(orient="vertical", spacing=8, padding=16)
//! counter = root.add_label("counter", "0 clicks")
//! inc = root.add_button("inc", "+1", on_click=lambda: ...)
//! app.root = root
//! app.run()
//! ```
//!
//! Python builds a tree of components and layout containers; the Rust side
//! turns that tree into winio components and wires `start`/`update`
//! automatically. WebView and the compio compatibility layer are deliberately
//! out of scope.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use pyo3::exceptions::{PyRuntimeError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use winio::prelude::{HAlign, Orient, VAlign};

mod layout;
mod model;
mod root;
mod widget;

use model::{
    DEFAULT_SIZE, Edges, Elem, ItemOpts, LayoutKind, LayoutSpec, Shared, Track, WidgetKind,
    WidgetNode,
};
use widget::Cmd;

/// Error type used by the component tree.
#[derive(Debug)]
pub enum AppError {
    Winio(winio::Error),
    Layout(String),
}

impl AppError {
    pub fn layout(message: impl Into<String>) -> Self {
        Self::Layout(message.into())
    }
}

impl From<winio::Error> for AppError {
    fn from(error: winio::Error) -> Self {
        Self::Winio(error)
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Winio(error) => write!(f, "winio error: {error:?}"),
            Self::Layout(error) => write!(f, "layout error: {error}"),
        }
    }
}

impl std::error::Error for AppError {}

/// The application object.
///
/// Create it on the Python main thread, describe the UI, then call
/// [`run`](PyApp::run). `run` releases the GIL and blocks until the window is
/// closed (or [`quit`](PyApp::quit) is called).
#[pyclass(name = "App")]
pub struct PyApp {
    shared: Arc<Shared>,
    title: String,
    size: (f64, f64),
    layout: Mutex<Option<Arc<LayoutSpec>>>,
    running: AtomicBool,
}

#[pymethods]
impl PyApp {
    #[new]
    #[pyo3(signature = (title, size = DEFAULT_SIZE))]
    fn new(title: String, size: (f64, f64)) -> PyResult<Self> {
        if !(size.0.is_finite() && size.1.is_finite() && size.0 > 0.0 && size.1 > 0.0) {
            return Err(PyValueError::new_err(
                "size must be a (width, height) pair of positive numbers",
            ));
        }
        Ok(Self {
            shared: Arc::new(Shared::new()),
            title,
            size,
            layout: Mutex::new(None),
            running: AtomicBool::new(false),
        })
    }

    /// Create a vertical or horizontal stack container.
    #[pyo3(signature = (orient = "vertical", spacing = 0.0, padding = 0.0))]
    fn stack(&self, orient: &str, spacing: f64, padding: f64) -> PyResult<PyStackPanel> {
        Ok(new_stack(
            &self.shared,
            parse_orient(orient)?,
            spacing,
            padding,
        ))
    }

    /// Create a grid container.
    ///
    /// `columns` and `rows` use winio's track notation: `"1*,2*"` for weighted
    /// tracks, `"120"` for a fixed track and `"auto"` for content sizing.
    #[pyo3(signature = (columns = "", rows = "", col_gap = 0.0, row_gap = 0.0, padding = 0.0))]
    fn grid(
        &self,
        columns: &str,
        rows: &str,
        col_gap: f64,
        row_gap: f64,
        padding: f64,
    ) -> PyResult<PyGrid> {
        let columns = Track::parse_list(columns).map_err(PyValueError::new_err)?;
        let rows = Track::parse_list(rows).map_err(PyValueError::new_err)?;
        Ok(new_grid(
            &self.shared,
            columns,
            rows,
            col_gap,
            row_gap,
            padding,
        ))
    }

    /// The root layout container. Must be set before `run()`.
    #[setter]
    fn set_root(&mut self, node: &Bound<'_, PyAny>) -> PyResult<()> {
        let layout = extract_layout(node)?;
        *self.layout.lock().unwrap() = Some(layout);
        Ok(())
    }

    /// Run the window loop until the window is closed.
    ///
    /// This blocks the calling thread. It must be called from the same thread
    /// that created the `App`.
    fn run(&self, py: Python<'_>) -> PyResult<()> {
        if self.running.swap(true, Ordering::SeqCst) {
            return Err(PyRuntimeError::new_err("App is already running"));
        }

        let result = (|| -> PyResult<()> {
            let layout = self.layout.lock().unwrap().clone().ok_or_else(|| {
                PyRuntimeError::new_err("no root layout: set `app.root = ...` before run()")
            })?;

            layout::validate(&layout).map_err(|error| PyValueError::new_err(error.to_string()))?;
            let widgets = model::collect_widgets(&layout).map_err(PyValueError::new_err)?;

            let shared = self.shared.clone();
            let title = self.title.clone();
            let size = self.size;
            py.detach(move || root::run_app(shared, layout, widgets, title, size))
                .map_err(PyRuntimeError::new_err)
        })();

        self.running.store(false, Ordering::SeqCst);
        result
    }

    /// Ask the root component to close the window.
    ///
    /// Safe to call from another Python thread while `run()` is blocking.
    fn quit(&self) {
        self.shared.quit();
    }

    /// Whether the window loop is currently running.
    #[getter]
    fn running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    #[getter]
    fn title(&self) -> &str {
        &self.title
    }
}

/// A stacked layout container.
#[pyclass(name = "StackPanel")]
pub struct PyStackPanel {
    shared: Arc<Shared>,
    layout: Arc<LayoutSpec>,
}

#[pymethods]
impl PyStackPanel {
    #[getter]
    fn orient(&self) -> &'static str {
        match &self.layout.kind {
            LayoutKind::Stack {
                orient: Orient::Vertical,
                ..
            } => "vertical",
            _ => "horizontal",
        }
    }

    /// Add a widget or a nested container.
    #[pyo3(signature = (child, *, grow = false, margin = None, halign = "stretch", valign = "stretch", width = None, height = None))]
    #[allow(clippy::too_many_arguments)]
    fn add(
        &self,
        child: &Bound<'_, PyAny>,
        grow: bool,
        margin: Option<(f64, f64, f64, f64)>,
        halign: &str,
        valign: &str,
        width: Option<f64>,
        height: Option<f64>,
    ) -> PyResult<()> {
        let opts = item_opts(grow, margin, halign, valign, width, height)?;
        push_child(&self.layout, child, opts)
    }

    /// Create a label and append it.
    #[pyo3(signature = (id, text))]
    fn add_label(&self, id: &str, text: &str) -> PyResult<PyLabel> {
        let label = make_label(&self.shared, id, text);
        self.layout
            .push(Elem::Widget(label.node.clone()), ItemOpts::default());
        Ok(label)
    }

    /// Create a button and append it.
    #[pyo3(signature = (id, text, *, on_click = None))]
    fn add_button(&self, id: &str, text: &str, on_click: Option<Py<PyAny>>) -> PyResult<PyButton> {
        let button = make_button(&self.shared, id, text, on_click)?;
        self.layout
            .push(Elem::Widget(button.node.clone()), ItemOpts::default());
        Ok(button)
    }

    /// Create a nested stack and append it.
    #[pyo3(signature = (orient = "vertical", spacing = 0.0, padding = 0.0))]
    fn add_stack(&self, orient: &str, spacing: f64, padding: f64) -> PyResult<PyStackPanel> {
        let panel = new_stack(&self.shared, parse_orient(orient)?, spacing, padding);
        self.layout
            .push(Elem::Panel(panel.layout.clone()), ItemOpts::default());
        Ok(panel)
    }

    /// Create a nested grid and append it.
    #[pyo3(signature = (columns = "", rows = "", col_gap = 0.0, row_gap = 0.0, padding = 0.0))]
    fn add_grid(
        &self,
        columns: &str,
        rows: &str,
        col_gap: f64,
        row_gap: f64,
        padding: f64,
    ) -> PyResult<PyGrid> {
        let columns = Track::parse_list(columns).map_err(PyValueError::new_err)?;
        let rows = Track::parse_list(rows).map_err(PyValueError::new_err)?;
        let grid = new_grid(&self.shared, columns, rows, col_gap, row_gap, padding);
        self.layout
            .push(Elem::Panel(grid.layout.clone()), ItemOpts::default());
        Ok(grid)
    }
}

/// A grid layout container.
#[pyclass(name = "Grid")]
pub struct PyGrid {
    shared: Arc<Shared>,
    layout: Arc<LayoutSpec>,
}

#[pymethods]
impl PyGrid {
    #[getter]
    fn rows(&self) -> usize {
        match &self.layout.kind {
            LayoutKind::Grid { rows, .. } => rows.len(),
            _ => 0,
        }
    }

    #[getter]
    fn columns(&self) -> usize {
        match &self.layout.kind {
            LayoutKind::Grid { columns, .. } => columns.len(),
            _ => 0,
        }
    }

    /// Add a widget or a nested container at a grid position.
    #[pyo3(signature = (child, *, row = 0, col = 0, row_span = 1, col_span = 1, margin = None, halign = "stretch", valign = "stretch", width = None, height = None))]
    #[allow(clippy::too_many_arguments)]
    fn add(
        &self,
        child: &Bound<'_, PyAny>,
        row: usize,
        col: usize,
        row_span: usize,
        col_span: usize,
        margin: Option<(f64, f64, f64, f64)>,
        halign: &str,
        valign: &str,
        width: Option<f64>,
        height: Option<f64>,
    ) -> PyResult<()> {
        let mut opts = item_opts(false, margin, halign, valign, width, height)?;
        opts.row = row;
        opts.col = col;
        opts.row_span = row_span.max(1);
        opts.col_span = col_span.max(1);
        check_grid_bounds(&self.layout, &opts)?;
        push_child(&self.layout, child, opts)
    }

    /// Create a label and append it at a grid position.
    #[pyo3(signature = (id, text, *, row = 0, col = 0, row_span = 1, col_span = 1))]
    fn add_label(
        &self,
        id: &str,
        text: &str,
        row: usize,
        col: usize,
        row_span: usize,
        col_span: usize,
    ) -> PyResult<PyLabel> {
        let mut opts = ItemOpts::default();
        opts.row = row;
        opts.col = col;
        opts.row_span = row_span.max(1);
        opts.col_span = col_span.max(1);
        check_grid_bounds(&self.layout, &opts)?;
        let label = make_label(&self.shared, id, text);
        self.layout.push(Elem::Widget(label.node.clone()), opts);
        Ok(label)
    }

    /// Create a button and append it at a grid position.
    #[pyo3(signature = (id, text, *, on_click = None, row = 0, col = 0, row_span = 1, col_span = 1))]
    #[allow(clippy::too_many_arguments)]
    fn add_button(
        &self,
        id: &str,
        text: &str,
        on_click: Option<Py<PyAny>>,
        row: usize,
        col: usize,
        row_span: usize,
        col_span: usize,
    ) -> PyResult<PyButton> {
        let mut opts = ItemOpts::default();
        opts.row = row;
        opts.col = col;
        opts.row_span = row_span.max(1);
        opts.col_span = col_span.max(1);
        check_grid_bounds(&self.layout, &opts)?;
        let button = make_button(&self.shared, id, text, on_click)?;
        self.layout.push(Elem::Widget(button.node.clone()), opts);
        Ok(button)
    }

    /// Create a nested stack and append it at a grid position.
    #[pyo3(signature = (orient = "vertical", spacing = 0.0, padding = 0.0, *, row = 0, col = 0, row_span = 1, col_span = 1))]
    #[allow(clippy::too_many_arguments)]
    fn add_stack(
        &self,
        orient: &str,
        spacing: f64,
        padding: f64,
        row: usize,
        col: usize,
        row_span: usize,
        col_span: usize,
    ) -> PyResult<PyStackPanel> {
        let mut opts = ItemOpts::default();
        opts.row = row;
        opts.col = col;
        opts.row_span = row_span.max(1);
        opts.col_span = col_span.max(1);
        check_grid_bounds(&self.layout, &opts)?;
        let panel = new_stack(&self.shared, parse_orient(orient)?, spacing, padding);
        self.layout.push(Elem::Panel(panel.layout.clone()), opts);
        Ok(panel)
    }
}

/// A read-only text widget.
#[pyclass(name = "Label")]
pub struct PyLabel {
    shared: Arc<Shared>,
    node: Arc<WidgetNode>,
}

#[pymethods]
impl PyLabel {
    #[getter]
    fn id(&self) -> &str {
        &self.node.id
    }

    #[getter]
    fn text(&self) -> String {
        self.node.props().text
    }

    #[setter]
    fn set_text(&self, text: String) {
        self.node.props.lock().unwrap().text = text.clone();
        self.shared.post(self.node.key, Cmd::SetText(text));
    }

    #[getter]
    fn visible(&self) -> bool {
        self.node.props().visible
    }

    #[setter]
    fn set_visible(&self, visible: bool) {
        self.node.props.lock().unwrap().visible = visible;
        self.shared.post(self.node.key, Cmd::SetVisible(visible));
    }

    fn __repr__(&self) -> String {
        format!(
            "<Label {:?} text={:?}>",
            self.node.id,
            self.node.props().text
        )
    }
}

/// A clickable button.
#[pyclass(name = "Button")]
pub struct PyButton {
    shared: Arc<Shared>,
    node: Arc<WidgetNode>,
}

#[pymethods]
impl PyButton {
    #[getter]
    fn id(&self) -> &str {
        &self.node.id
    }

    #[getter]
    fn text(&self) -> String {
        self.node.props().text
    }

    #[setter]
    fn set_text(&self, text: String) {
        self.node.props.lock().unwrap().text = text.clone();
        self.shared.post(self.node.key, Cmd::SetText(text));
    }

    #[getter]
    fn enabled(&self) -> bool {
        self.node.props().enabled
    }

    #[setter]
    fn set_enabled(&self, enabled: bool) {
        self.node.props.lock().unwrap().enabled = enabled;
        self.shared.post(self.node.key, Cmd::SetEnabled(enabled));
    }

    #[getter]
    fn visible(&self) -> bool {
        self.node.props().visible
    }

    #[setter]
    fn set_visible(&self, visible: bool) {
        self.node.props.lock().unwrap().visible = visible;
        self.shared.post(self.node.key, Cmd::SetVisible(visible));
    }

    /// The click callback. Can be replaced while the window is running.
    #[getter]
    fn on_click(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.node
            .on_click
            .lock()
            .unwrap()
            .as_ref()
            .map(|callback| callback.clone_ref(py))
    }

    #[setter]
    fn set_on_click(&self, py: Python<'_>, callback: Option<Py<PyAny>>) -> PyResult<()> {
        if let Some(callback) = &callback
            && !callback.bind(py).is_callable()
        {
            return Err(PyTypeError::new_err("on_click must be callable"));
        }
        *self.node.on_click.lock().unwrap() = callback;
        Ok(())
    }

    fn __repr__(&self) -> String {
        format!(
            "<Button {:?} text={:?}>",
            self.node.id,
            self.node.props().text
        )
    }
}

fn new_stack(shared: &Arc<Shared>, orient: Orient, spacing: f64, padding: f64) -> PyStackPanel {
    let key = shared.next_layout_key();
    PyStackPanel {
        shared: shared.clone(),
        layout: Arc::new(LayoutSpec::stack(
            key,
            orient,
            spacing.max(0.0),
            Edges::all(padding.max(0.0)),
        )),
    }
}

fn new_grid(
    shared: &Arc<Shared>,
    columns: Vec<Track>,
    rows: Vec<Track>,
    col_gap: f64,
    row_gap: f64,
    padding: f64,
) -> PyGrid {
    let key = shared.next_layout_key();
    PyGrid {
        shared: shared.clone(),
        layout: Arc::new(LayoutSpec::grid(
            key,
            columns,
            rows,
            col_gap.max(0.0),
            row_gap.max(0.0),
            Edges::all(padding.max(0.0)),
        )),
    }
}

fn make_label(shared: &Arc<Shared>, id: &str, text: &str) -> PyLabel {
    let key = shared.next_node_key();
    PyLabel {
        shared: shared.clone(),
        node: Arc::new(WidgetNode::new(
            key,
            id.to_string(),
            WidgetKind::Label,
            text.to_string(),
        )),
    }
}

fn make_button(
    shared: &Arc<Shared>,
    id: &str,
    text: &str,
    on_click: Option<Py<PyAny>>,
) -> PyResult<PyButton> {
    let key = shared.next_node_key();
    let node = Arc::new(WidgetNode::new(
        key,
        id.to_string(),
        WidgetKind::Button,
        text.to_string(),
    ));
    *node.on_click.lock().unwrap() = on_click;
    Ok(PyButton {
        shared: shared.clone(),
        node,
    })
}

fn item_opts(
    grow: bool,
    margin: Option<(f64, f64, f64, f64)>,
    halign: &str,
    valign: &str,
    width: Option<f64>,
    height: Option<f64>,
) -> PyResult<ItemOpts> {
    Ok(ItemOpts {
        grow,
        margin: margin
            .map(|(left, top, right, bottom)| Edges::new(left, top, right, bottom))
            .unwrap_or_default(),
        halign: parse_halign(halign)?,
        valign: parse_valign(valign)?,
        width,
        height,
        ..ItemOpts::default()
    })
}

/// Add a child (widget or nested container) to a container.
fn push_child(parent: &Arc<LayoutSpec>, child: &Bound<'_, PyAny>, opts: ItemOpts) -> PyResult<()> {
    let elem = extract_elem(child)?;

    if let Elem::Panel(panel) = &elem
        && Arc::ptr_eq(parent, panel)
    {
        return Err(PyValueError::new_err("a container cannot contain itself"));
    }

    check_grid_bounds(parent, &opts)?;
    parent.push(elem, opts);
    Ok(())
}

fn extract_elem(child: &Bound<'_, PyAny>) -> PyResult<Elem> {
    if let Ok(label) = child.extract::<PyRef<'_, PyLabel>>() {
        return Ok(Elem::Widget(label.node.clone()));
    }
    if let Ok(button) = child.extract::<PyRef<'_, PyButton>>() {
        return Ok(Elem::Widget(button.node.clone()));
    }
    if let Ok(panel) = child.extract::<PyRef<'_, PyStackPanel>>() {
        return Ok(Elem::Panel(panel.layout.clone()));
    }
    if let Ok(grid) = child.extract::<PyRef<'_, PyGrid>>() {
        return Ok(Elem::Panel(grid.layout.clone()));
    }
    Err(PyTypeError::new_err(
        "expected a Label, Button, StackPanel or Grid",
    ))
}

fn extract_layout(node: &Bound<'_, PyAny>) -> PyResult<Arc<LayoutSpec>> {
    if let Ok(panel) = node.extract::<PyRef<'_, PyStackPanel>>() {
        return Ok(panel.layout.clone());
    }
    if let Ok(grid) = node.extract::<PyRef<'_, PyGrid>>() {
        return Ok(grid.layout.clone());
    }
    Err(PyTypeError::new_err(
        "app.root must be a StackPanel or a Grid",
    ))
}

fn check_grid_bounds(parent: &Arc<LayoutSpec>, opts: &ItemOpts) -> PyResult<()> {
    if let LayoutKind::Grid { columns, rows, .. } = &parent.kind {
        let row_span = opts.row_span.max(1);
        let col_span = opts.col_span.max(1);
        if opts.row + row_span > rows.len() || opts.col + col_span > columns.len() {
            return Err(PyValueError::new_err(format!(
                "grid item (row={}, col={}, row_span={}, col_span={}) exceeds a {}x{} grid",
                opts.row,
                opts.col,
                row_span,
                col_span,
                rows.len(),
                columns.len()
            )));
        }
    }
    Ok(())
}

fn parse_orient(value: &str) -> PyResult<Orient> {
    match value.to_ascii_lowercase().as_str() {
        "vertical" | "v" => Ok(Orient::Vertical),
        "horizontal" | "h" => Ok(Orient::Horizontal),
        _ => Err(PyValueError::new_err(format!(
            "invalid orientation {value:?}: expected 'vertical' or 'horizontal'"
        ))),
    }
}

fn parse_halign(value: &str) -> PyResult<HAlign> {
    match value.to_ascii_lowercase().as_str() {
        "left" | "start" => Ok(HAlign::Left),
        "center" | "middle" => Ok(HAlign::Center),
        "right" | "end" => Ok(HAlign::Right),
        "stretch" | "fill" => Ok(HAlign::Stretch),
        _ => Err(PyValueError::new_err(format!(
            "invalid halign {value:?}: expected left, center, right or stretch"
        ))),
    }
}

fn parse_valign(value: &str) -> PyResult<VAlign> {
    match value.to_ascii_lowercase().as_str() {
        "top" | "start" => Ok(VAlign::Top),
        "center" | "middle" => Ok(VAlign::Center),
        "bottom" | "end" => Ok(VAlign::Bottom),
        "stretch" | "fill" => Ok(VAlign::Stretch),
        _ => Err(PyValueError::new_err(format!(
            "invalid valign {value:?}: expected top, center, bottom or stretch"
        ))),
    }
}

#[pymodule]
fn _pywinio(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyApp>()?;
    module.add_class::<PyStackPanel>()?;
    module.add_class::<PyGrid>()?;
    module.add_class::<PyLabel>()?;
    module.add_class::<PyButton>()?;
    module.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
