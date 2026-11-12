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
    root_obj: Mutex<Option<Py<PyAny>>>,
    timers: Mutex<Vec<Arc<WidgetNode>>>,
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
            root_obj: Mutex::new(None),
            timers: Mutex::new(Vec::new()),
            running: AtomicBool::new(false),
        })
    }

    /// Create a vertical or horizontal stack container.
    #[pyo3(signature = (orient: "Literal['vertical', 'horizontal']" = "vertical", spacing = 0.0, padding = 0.0))]
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

    /// Create a push button.
    #[pyo3(signature = (id, text, *, on_click = None))]
    fn button(
        &self,
        py: Python<'_>,
        id: &str,
        text: &str,
        on_click: Option<Py<PyAny>>,
    ) -> PyResult<PyButton> {
        check_callback(py, &on_click, "on_click")?;
        make_button(&self.shared, id, text, on_click)
    }

    /// Create a text label.
    #[pyo3(signature = (id, text = ""))]
    fn label(&self, id: &str, text: &str) -> PyLabel {
        make_label(&self.shared, id, text)
    }

    /// Create a single-line text input.
    ///
    /// Add it to a container with `container.add(edit, ...)`.
    #[pyo3(signature = (id, text = "", *, on_change = None))]
    fn edit(
        &self,
        py: Python<'_>,
        id: &str,
        text: &str,
        on_change: Option<Py<PyAny>>,
    ) -> PyResult<PyEdit> {
        check_callback(py, &on_change, "on_change")?;
        Ok(make_edit(&self.shared, id, text, on_change))
    }

    /// Create a check box.
    #[pyo3(signature = (id, text = "", checked = false, *, on_click = None))]
    fn check_box(
        &self,
        py: Python<'_>,
        id: &str,
        text: &str,
        checked: bool,
        on_click: Option<Py<PyAny>>,
    ) -> PyResult<PyCheckBox> {
        check_callback(py, &on_click, "on_click")?;
        Ok(make_check_box(&self.shared, id, text, checked, on_click))
    }

    /// Create a hyperlink-style label.
    ///
    /// If `uri` is non-empty, clicking opens the URI and no `on_click`
    /// callback is invoked (winio behavior).
    #[pyo3(signature = (id, text = "", uri = "", *, on_click = None))]
    fn link_label(
        &self,
        py: Python<'_>,
        id: &str,
        text: &str,
        uri: &str,
        on_click: Option<Py<PyAny>>,
    ) -> PyResult<PyLinkLabel> {
        check_callback(py, &on_click, "on_click")?;
        Ok(make_link_label(&self.shared, id, text, uri, on_click))
    }

    /// Create a progress bar.
    #[pyo3(signature = (id, pos = 0, minimum = 0, maximum = 100, indeterminate = false))]
    fn progress(
        &self,
        id: &str,
        pos: usize,
        minimum: usize,
        maximum: usize,
        indeterminate: bool,
    ) -> PyResult<PyProgress> {
        Ok(make_progress(
            &self.shared,
            id,
            pos,
            minimum,
            maximum,
            indeterminate,
        ))
    }

    /// Create a multi-line text input.
    #[pyo3(signature = (id, text = ""))]
    fn text_box(&self, id: &str, text: &str) -> PyResult<PyTextBox> {
        Ok(make_text_box(&self.shared, id, text))
    }

    /// Create a slider.
    #[pyo3(signature = (id, pos = 0, minimum = 0, maximum = 100, *, on_change = None))]
    fn slider(
        &self,
        py: Python<'_>,
        id: &str,
        pos: usize,
        minimum: usize,
        maximum: usize,
        on_change: Option<Py<PyAny>>,
    ) -> PyResult<PySlider> {
        check_callback(py, &on_change, "on_change")?;
        Ok(make_slider(
            &self.shared,
            id,
            pos,
            minimum,
            maximum,
            on_change,
        ))
    }

    /// Create an integer input with up/down buttons.
    #[pyo3(signature = (id, value = 0, minimum = 0, maximum = 100, step = 1, *, on_change = None))]
    fn up_down_edit(
        &self,
        py: Python<'_>,
        id: &str,
        value: usize,
        minimum: usize,
        maximum: usize,
        step: usize,
        on_change: Option<Py<PyAny>>,
    ) -> PyResult<PyUpDownEdit> {
        check_callback(py, &on_change, "on_change")?;
        Ok(make_up_down_edit(
            &self.shared,
            id,
            value,
            minimum,
            maximum,
            step,
            on_change,
        ))
    }

    /// Create a combo box.
    #[pyo3(signature = (id, items, *, selected = None, text = "", editable = false, on_select = None, on_change = None))]
    #[allow(clippy::too_many_arguments)]
    fn combo_box(
        &self,
        py: Python<'_>,
        id: &str,
        items: Vec<String>,
        selected: Option<usize>,
        text: &str,
        editable: bool,
        on_select: Option<Py<PyAny>>,
        on_change: Option<Py<PyAny>>,
    ) -> PyResult<PyComboBox> {
        check_callback(py, &on_select, "on_select")?;
        check_callback(py, &on_change, "on_change")?;
        Ok(make_combo_box(
            &self.shared,
            id,
            items,
            selected,
            text,
            editable,
            on_select,
            on_change,
        ))
    }

    /// Create a radio button.
    #[pyo3(signature = (id, text = "", checked = false, *, on_click = None))]
    fn radio_button(
        &self,
        py: Python<'_>,
        id: &str,
        text: &str,
        checked: bool,
        on_click: Option<Py<PyAny>>,
    ) -> PyResult<PyRadioButton> {
        check_callback(py, &on_click, "on_click")?;
        Ok(make_radio_button(&self.shared, id, text, checked, on_click))
    }

    /// Create a list box.
    #[pyo3(signature = (id, items, *, selected = None, multiple = false, on_select = None))]
    fn list_box(
        &self,
        py: Python<'_>,
        id: &str,
        items: Vec<String>,
        selected: Option<Vec<usize>>,
        multiple: bool,
        on_select: Option<Py<PyAny>>,
    ) -> PyResult<PyListBox> {
        check_callback(py, &on_select, "on_select")?;
        Ok(make_list_box(
            &self.shared,
            id,
            items,
            selected.unwrap_or_default(),
            multiple,
            on_select,
        ))
    }

    /// Create a scrollable container. Use `set_content(panel)` before `run()`.
    #[pyo3(signature = (id, *, hscroll = true, vscroll = true))]
    fn scroll_view(&self, id: &str, hscroll: bool, vscroll: bool) -> PyResult<PyScrollView> {
        Ok(make_scroll_view(&self.shared, id, hscroll, vscroll))
    }

    /// Create a headless periodic timer. It is not added to the layout tree.
    #[pyo3(signature = (id, interval_ms = 500, *, on_tick = None, enabled = true))]
    fn timer(
        &self,
        py: Python<'_>,
        id: &str,
        interval_ms: u64,
        on_tick: Option<Py<PyAny>>,
        enabled: bool,
    ) -> PyResult<PyTimer> {
        if interval_ms == 0 {
            return Err(PyValueError::new_err("interval_ms must be positive"));
        }
        check_callback(py, &on_tick, "on_tick")?;
        let node = make_timer(&self.shared, id, interval_ms, on_tick, enabled);
        self.timers.lock().unwrap().push(node.clone());
        Ok(PyTimer {
            shared: self.shared.clone(),
            node,
        })
    }

    /// The root layout container. Must be set before `run()`.
    #[getter]
    fn root(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.root_obj
            .lock()
            .unwrap()
            .as_ref()
            .map(|root| root.clone_ref(py))
    }

    #[setter]
    fn set_root(&mut self, node: &Bound<'_, PyAny>) -> PyResult<()> {
        let layout = extract_layout(node)?;
        *self.layout.lock().unwrap() = Some(layout);
        *self.root_obj.lock().unwrap() = Some(node.clone().unbind());
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
            let timers = self.timers.lock().unwrap().clone();

            let shared = self.shared.clone();
            let title = self.title.clone();
            let size = self.size;
            py.detach(move || root::run_app(shared, layout, widgets, timers, title, size))
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
    #[pyo3(signature = (child, *, grow = false, margin = None, halign: "Literal['left', 'center', 'right', 'stretch']" = "stretch", valign: "Literal['top', 'center', 'bottom', 'stretch']" = "stretch", width = None, height = None))]
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
}

/// A grid layout container.
#[pyclass(name = "Grid")]
pub struct PyGrid {
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
    #[pyo3(signature = (child, *, row = 0, col = 0, row_span = 1, col_span = 1, margin = None, halign: "Literal['left', 'center', 'right', 'stretch']" = "stretch", valign: "Literal['top', 'center', 'bottom', 'stretch']" = "stretch", width = None, height = None))]
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

/// A single-line text input.
#[pyclass(name = "Edit")]
pub struct PyEdit {
    shared: Arc<Shared>,
    node: Arc<WidgetNode>,
}

#[pymethods]
impl PyEdit {
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
        set_text_prop(&self.node, &self.shared, text);
    }

    #[getter]
    fn visible(&self) -> bool {
        self.node.props().visible
    }

    #[setter]
    fn set_visible(&self, visible: bool) {
        set_visible_prop(&self.node, &self.shared, visible);
    }

    #[getter]
    fn enabled(&self) -> bool {
        self.node.props().enabled
    }

    #[setter]
    fn set_enabled(&self, enabled: bool) {
        set_enabled_prop(&self.node, &self.shared, enabled);
    }

    #[getter]
    fn on_change(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        get_callback(&self.node.on_change, py)
    }

    #[setter]
    fn set_on_change(&self, py: Python<'_>, callback: Option<Py<PyAny>>) -> PyResult<()> {
        set_callback(&self.node.on_change, py, callback, "on_change")
    }

    fn __repr__(&self) -> String {
        format!(
            "<Edit {:?} text={:?}>",
            self.node.id,
            self.node.props().text
        )
    }
}

/// A check box.
#[pyclass(name = "CheckBox")]
pub struct PyCheckBox {
    shared: Arc<Shared>,
    node: Arc<WidgetNode>,
}

#[pymethods]
impl PyCheckBox {
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
        set_text_prop(&self.node, &self.shared, text);
    }

    #[getter]
    fn checked(&self) -> bool {
        self.node.props().checked
    }

    #[setter]
    fn set_checked(&self, checked: bool) {
        self.node.props.lock().unwrap().checked = checked;
        self.shared.post(self.node.key, Cmd::SetChecked(checked));
    }

    #[getter]
    fn visible(&self) -> bool {
        self.node.props().visible
    }

    #[setter]
    fn set_visible(&self, visible: bool) {
        set_visible_prop(&self.node, &self.shared, visible);
    }

    #[getter]
    fn enabled(&self) -> bool {
        self.node.props().enabled
    }

    #[setter]
    fn set_enabled(&self, enabled: bool) {
        set_enabled_prop(&self.node, &self.shared, enabled);
    }

    #[getter]
    fn on_click(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        get_callback(&self.node.on_click, py)
    }

    #[setter]
    fn set_on_click(&self, py: Python<'_>, callback: Option<Py<PyAny>>) -> PyResult<()> {
        set_callback(&self.node.on_click, py, callback, "on_click")
    }

    fn __repr__(&self) -> String {
        format!(
            "<CheckBox {:?} text={:?} checked={}>",
            self.node.id,
            self.node.props().text,
            self.node.props().checked
        )
    }
}

/// A hyperlink-style label.
#[pyclass(name = "LinkLabel")]
pub struct PyLinkLabel {
    shared: Arc<Shared>,
    node: Arc<WidgetNode>,
}

#[pymethods]
impl PyLinkLabel {
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
        set_text_prop(&self.node, &self.shared, text);
    }

    #[getter]
    fn uri(&self) -> String {
        self.node.props().uri
    }

    #[setter]
    fn set_uri(&self, uri: String) {
        self.node.props.lock().unwrap().uri = uri.clone();
        self.shared.post(self.node.key, Cmd::SetUri(uri));
    }

    #[getter]
    fn visible(&self) -> bool {
        self.node.props().visible
    }

    #[setter]
    fn set_visible(&self, visible: bool) {
        set_visible_prop(&self.node, &self.shared, visible);
    }

    #[getter]
    fn enabled(&self) -> bool {
        self.node.props().enabled
    }

    #[setter]
    fn set_enabled(&self, enabled: bool) {
        set_enabled_prop(&self.node, &self.shared, enabled);
    }

    #[getter]
    fn on_click(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        get_callback(&self.node.on_click, py)
    }

    #[setter]
    fn set_on_click(&self, py: Python<'_>, callback: Option<Py<PyAny>>) -> PyResult<()> {
        set_callback(&self.node.on_click, py, callback, "on_click")
    }

    fn __repr__(&self) -> String {
        format!(
            "<LinkLabel {:?} text={:?} uri={:?}>",
            self.node.id,
            self.node.props().text,
            self.node.props().uri
        )
    }
}

/// A progress bar.
#[pyclass(name = "Progress")]
pub struct PyProgress {
    shared: Arc<Shared>,
    node: Arc<WidgetNode>,
}

#[pymethods]
impl PyProgress {
    #[getter]
    fn id(&self) -> &str {
        &self.node.id
    }

    #[getter]
    fn pos(&self) -> usize {
        self.node.props().pos
    }

    #[setter]
    fn set_pos(&self, pos: usize) {
        set_pos_prop(&self.node, &self.shared, pos, |key| Cmd::SetPos(key));
    }

    #[getter]
    fn minimum(&self) -> usize {
        self.node.props().minimum
    }

    #[setter]
    fn set_minimum(&self, minimum: usize) {
        self.node.props.lock().unwrap().minimum = minimum;
        self.shared.post(self.node.key, Cmd::SetMinimum(minimum));
    }

    #[getter]
    fn maximum(&self) -> usize {
        self.node.props().maximum
    }

    #[setter]
    fn set_maximum(&self, maximum: usize) {
        self.node.props.lock().unwrap().maximum = maximum;
        self.shared.post(self.node.key, Cmd::SetMaximum(maximum));
    }

    #[getter]
    fn indeterminate(&self) -> bool {
        self.node.props().indeterminate
    }

    #[setter]
    fn set_indeterminate(&self, indeterminate: bool) {
        self.node.props.lock().unwrap().indeterminate = indeterminate;
        self.shared
            .post(self.node.key, Cmd::SetIndeterminate(indeterminate));
    }

    #[getter]
    fn visible(&self) -> bool {
        self.node.props().visible
    }

    #[setter]
    fn set_visible(&self, visible: bool) {
        set_visible_prop(&self.node, &self.shared, visible);
    }

    #[getter]
    fn enabled(&self) -> bool {
        self.node.props().enabled
    }

    #[setter]
    fn set_enabled(&self, enabled: bool) {
        set_enabled_prop(&self.node, &self.shared, enabled);
    }

    fn __repr__(&self) -> String {
        format!(
            "<Progress {:?} pos={} min={} max={}>",
            self.node.id,
            self.node.props().pos,
            self.node.props().minimum,
            self.node.props().maximum
        )
    }
}

/// A multi-line text input.
#[pyclass(name = "TextBox")]
pub struct PyTextBox {
    shared: Arc<Shared>,
    node: Arc<WidgetNode>,
}

#[pymethods]
impl PyTextBox {
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
        set_text_prop(&self.node, &self.shared, text);
    }

    #[getter]
    fn visible(&self) -> bool {
        self.node.props().visible
    }

    #[setter]
    fn set_visible(&self, visible: bool) {
        set_visible_prop(&self.node, &self.shared, visible);
    }

    #[getter]
    fn enabled(&self) -> bool {
        self.node.props().enabled
    }

    #[setter]
    fn set_enabled(&self, enabled: bool) {
        set_enabled_prop(&self.node, &self.shared, enabled);
    }

    fn __repr__(&self) -> String {
        format!(
            "<TextBox {:?} text={:?}>",
            self.node.id,
            self.node.props().text
        )
    }
}

/// A slider.
#[pyclass(name = "Slider")]
pub struct PySlider {
    shared: Arc<Shared>,
    node: Arc<WidgetNode>,
}

#[pymethods]
impl PySlider {
    #[getter]
    fn id(&self) -> &str {
        &self.node.id
    }

    #[getter]
    fn pos(&self) -> usize {
        self.node.props().pos
    }

    #[setter]
    fn set_pos(&self, pos: usize) {
        set_pos_prop(&self.node, &self.shared, pos, Cmd::SetPos);
    }

    #[getter]
    fn minimum(&self) -> usize {
        self.node.props().minimum
    }

    #[setter]
    fn set_minimum(&self, minimum: usize) {
        self.node.props.lock().unwrap().minimum = minimum;
        self.shared.post(self.node.key, Cmd::SetMinimum(minimum));
    }

    #[getter]
    fn maximum(&self) -> usize {
        self.node.props().maximum
    }

    #[setter]
    fn set_maximum(&self, maximum: usize) {
        self.node.props.lock().unwrap().maximum = maximum;
        self.shared.post(self.node.key, Cmd::SetMaximum(maximum));
    }

    #[getter]
    fn visible(&self) -> bool {
        self.node.props().visible
    }

    #[setter]
    fn set_visible(&self, visible: bool) {
        set_visible_prop(&self.node, &self.shared, visible);
    }

    #[getter]
    fn enabled(&self) -> bool {
        self.node.props().enabled
    }

    #[setter]
    fn set_enabled(&self, enabled: bool) {
        set_enabled_prop(&self.node, &self.shared, enabled);
    }

    #[getter]
    fn on_change(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        get_callback(&self.node.on_change, py)
    }

    #[setter]
    fn set_on_change(&self, py: Python<'_>, callback: Option<Py<PyAny>>) -> PyResult<()> {
        set_callback(&self.node.on_change, py, callback, "on_change")
    }

    fn __repr__(&self) -> String {
        format!(
            "<Slider {:?} pos={} min={} max={}>",
            self.node.id,
            self.node.props().pos,
            self.node.props().minimum,
            self.node.props().maximum
        )
    }
}

/// An integer input with up/down buttons.
#[pyclass(name = "UpDownEdit")]
pub struct PyUpDownEdit {
    shared: Arc<Shared>,
    node: Arc<WidgetNode>,
}

#[pymethods]
impl PyUpDownEdit {
    #[getter]
    fn id(&self) -> &str {
        &self.node.id
    }

    #[getter]
    fn value(&self) -> usize {
        self.node.props().value
    }

    #[setter]
    fn set_value(&self, value: usize) {
        set_pos_prop(&self.node, &self.shared, value, Cmd::SetValue);
    }

    #[getter]
    fn minimum(&self) -> usize {
        self.node.props().minimum
    }

    #[setter]
    fn set_minimum(&self, minimum: usize) {
        self.node.props.lock().unwrap().minimum = minimum;
        self.shared.post(self.node.key, Cmd::SetMinimum(minimum));
    }

    #[getter]
    fn maximum(&self) -> usize {
        self.node.props().maximum
    }

    #[setter]
    fn set_maximum(&self, maximum: usize) {
        self.node.props.lock().unwrap().maximum = maximum;
        self.shared.post(self.node.key, Cmd::SetMaximum(maximum));
    }

    #[getter]
    fn step(&self) -> usize {
        self.node.props().step
    }

    #[setter]
    fn set_step(&self, step: usize) {
        self.node.props.lock().unwrap().step = step;
        self.shared.post(self.node.key, Cmd::SetStep(step));
    }

    #[getter]
    fn visible(&self) -> bool {
        self.node.props().visible
    }

    #[setter]
    fn set_visible(&self, visible: bool) {
        set_visible_prop(&self.node, &self.shared, visible);
    }

    #[getter]
    fn enabled(&self) -> bool {
        self.node.props().enabled
    }

    #[setter]
    fn set_enabled(&self, enabled: bool) {
        set_enabled_prop(&self.node, &self.shared, enabled);
    }

    #[getter]
    fn on_change(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        get_callback(&self.node.on_change, py)
    }

    #[setter]
    fn set_on_change(&self, py: Python<'_>, callback: Option<Py<PyAny>>) -> PyResult<()> {
        set_callback(&self.node.on_change, py, callback, "on_change")
    }

    fn __repr__(&self) -> String {
        format!(
            "<UpDownEdit {:?} value={} min={} max={} step={}>",
            self.node.id,
            self.node.props().value,
            self.node.props().minimum,
            self.node.props().maximum,
            self.node.props().step
        )
    }
}

/// A combo box.
#[pyclass(name = "ComboBox")]
pub struct PyComboBox {
    shared: Arc<Shared>,
    node: Arc<WidgetNode>,
}

#[pymethods]
impl PyComboBox {
    #[getter]
    fn id(&self) -> &str {
        &self.node.id
    }

    #[getter]
    fn items(&self) -> Vec<String> {
        self.node.props().items
    }

    #[setter]
    fn set_items(&self, items: Vec<String>) {
        let selected = {
            let mut props = self.node.props.lock().unwrap();
            props.items = items.clone();
            if props.selected.is_some_and(|index| index >= items.len()) {
                props.selected = None;
            }
            props.selected
        };
        self.shared.post(self.node.key, Cmd::SetItems(items));
        self.shared.post(self.node.key, Cmd::SetSelection(selected));
    }

    #[getter]
    fn selected(&self) -> Option<usize> {
        self.node.props().selected
    }

    #[setter]
    fn set_selected(&self, selected: Option<usize>) {
        self.node.props.lock().unwrap().selected = selected;
        self.shared.post(self.node.key, Cmd::SetSelection(selected));
    }

    #[getter]
    fn text(&self) -> String {
        self.node.props().text
    }

    #[setter]
    fn set_text(&self, text: String) {
        set_text_prop(&self.node, &self.shared, text);
    }

    #[getter]
    fn editable(&self) -> bool {
        self.node.props().editable
    }

    #[setter]
    fn set_editable(&self, editable: bool) {
        self.node.props.lock().unwrap().editable = editable;
        self.shared.post(self.node.key, Cmd::SetEditable(editable));
    }

    #[getter]
    fn visible(&self) -> bool {
        self.node.props().visible
    }

    #[setter]
    fn set_visible(&self, visible: bool) {
        set_visible_prop(&self.node, &self.shared, visible);
    }

    #[getter]
    fn enabled(&self) -> bool {
        self.node.props().enabled
    }

    #[setter]
    fn set_enabled(&self, enabled: bool) {
        set_enabled_prop(&self.node, &self.shared, enabled);
    }

    #[getter]
    fn on_select(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        get_callback(&self.node.on_select, py)
    }

    #[setter]
    fn set_on_select(&self, py: Python<'_>, callback: Option<Py<PyAny>>) -> PyResult<()> {
        set_callback(&self.node.on_select, py, callback, "on_select")
    }

    #[getter]
    fn on_change(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        get_callback(&self.node.on_change, py)
    }

    #[setter]
    fn set_on_change(&self, py: Python<'_>, callback: Option<Py<PyAny>>) -> PyResult<()> {
        set_callback(&self.node.on_change, py, callback, "on_change")
    }

    fn __repr__(&self) -> String {
        format!(
            "<ComboBox {:?} items={} selected={:?}>",
            self.node.id,
            self.node.props().items.len(),
            self.node.props().selected
        )
    }
}

/// A radio button.
#[pyclass(name = "RadioButton")]
pub struct PyRadioButton {
    shared: Arc<Shared>,
    node: Arc<WidgetNode>,
}

#[pymethods]
impl PyRadioButton {
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
        set_text_prop(&self.node, &self.shared, text);
    }

    #[getter]
    fn checked(&self) -> bool {
        self.node.props().checked
    }

    #[setter]
    fn set_checked(&self, checked: bool) {
        self.node.props.lock().unwrap().checked = checked;
        self.shared.post(self.node.key, Cmd::SetChecked(checked));
    }

    #[getter]
    fn visible(&self) -> bool {
        self.node.props().visible
    }

    #[setter]
    fn set_visible(&self, visible: bool) {
        set_visible_prop(&self.node, &self.shared, visible);
    }

    #[getter]
    fn enabled(&self) -> bool {
        self.node.props().enabled
    }

    #[setter]
    fn set_enabled(&self, enabled: bool) {
        set_enabled_prop(&self.node, &self.shared, enabled);
    }

    #[getter]
    fn on_click(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        get_callback(&self.node.on_click, py)
    }

    #[setter]
    fn set_on_click(&self, py: Python<'_>, callback: Option<Py<PyAny>>) -> PyResult<()> {
        set_callback(&self.node.on_click, py, callback, "on_click")
    }

    fn __repr__(&self) -> String {
        format!(
            "<RadioButton {:?} text={:?} checked={}>",
            self.node.id,
            self.node.props().text,
            self.node.props().checked
        )
    }
}

/// A list box.
#[pyclass(name = "ListBox")]
pub struct PyListBox {
    shared: Arc<Shared>,
    node: Arc<WidgetNode>,
}

#[pymethods]
impl PyListBox {
    #[getter]
    fn id(&self) -> &str {
        &self.node.id
    }

    #[getter]
    fn items(&self) -> Vec<String> {
        self.node.props().items
    }

    #[setter]
    fn set_items(&self, items: Vec<String>) {
        let selected = {
            let mut props = self.node.props.lock().unwrap();
            props.items = items.clone();
            props.selected_indices.retain(|index| *index < items.len());
            props.selected_indices.clone()
        };
        self.shared.post(self.node.key, Cmd::SetItems(items));
        self.shared
            .post(self.node.key, Cmd::SetSelectedIndices(selected));
    }

    #[getter]
    fn selected(&self) -> Vec<usize> {
        self.node.props().selected_indices
    }

    #[setter]
    fn set_selected(&self, selected: Vec<usize>) {
        let selected = {
            let props = self.node.props();
            selected
                .into_iter()
                .filter(|index| *index < props.items.len())
                .collect::<Vec<_>>()
        };
        self.node.props.lock().unwrap().selected_indices = selected.clone();
        self.shared
            .post(self.node.key, Cmd::SetSelectedIndices(selected));
    }

    #[getter]
    fn multiple(&self) -> bool {
        self.node.props().multiple
    }

    #[setter]
    fn set_multiple(&self, multiple: bool) {
        self.node.props.lock().unwrap().multiple = multiple;
        self.shared.post(self.node.key, Cmd::SetMultiple(multiple));
    }

    #[getter]
    fn visible(&self) -> bool {
        self.node.props().visible
    }

    #[setter]
    fn set_visible(&self, visible: bool) {
        set_visible_prop(&self.node, &self.shared, visible);
    }

    #[getter]
    fn enabled(&self) -> bool {
        self.node.props().enabled
    }

    #[setter]
    fn set_enabled(&self, enabled: bool) {
        set_enabled_prop(&self.node, &self.shared, enabled);
    }

    #[getter]
    fn on_select(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        get_callback(&self.node.on_select, py)
    }

    #[setter]
    fn set_on_select(&self, py: Python<'_>, callback: Option<Py<PyAny>>) -> PyResult<()> {
        set_callback(&self.node.on_select, py, callback, "on_select")
    }

    fn __repr__(&self) -> String {
        format!(
            "<ListBox {:?} items={} selected={:?}>",
            self.node.id,
            self.node.props().items.len(),
            self.node.props().selected_indices
        )
    }
}

/// A scrollable container.
#[pyclass(name = "ScrollView")]
pub struct PyScrollView {
    shared: Arc<Shared>,
    node: Arc<WidgetNode>,
}

#[pymethods]
impl PyScrollView {
    #[getter]
    fn id(&self) -> &str {
        &self.node.id
    }

    /// Set the layout panel scrolled by this view. Must be called before run().
    fn set_content(&self, content: &Bound<'_, PyAny>) -> PyResult<()> {
        let layout = extract_layout(content)?;
        *self.node.scroll_content.lock().unwrap() = Some(layout);
        Ok(())
    }

    #[getter]
    fn hscroll(&self) -> bool {
        self.node.props().hscroll
    }

    #[setter]
    fn set_hscroll(&self, hscroll: bool) {
        self.node.props.lock().unwrap().hscroll = hscroll;
        self.shared.post(self.node.key, Cmd::SetHScroll(hscroll));
    }

    #[getter]
    fn vscroll(&self) -> bool {
        self.node.props().vscroll
    }

    #[setter]
    fn set_vscroll(&self, vscroll: bool) {
        self.node.props.lock().unwrap().vscroll = vscroll;
        self.shared.post(self.node.key, Cmd::SetVScroll(vscroll));
    }

    #[getter]
    fn visible(&self) -> bool {
        self.node.props().visible
    }

    #[setter]
    fn set_visible(&self, visible: bool) {
        set_visible_prop(&self.node, &self.shared, visible);
    }

    #[getter]
    fn enabled(&self) -> bool {
        self.node.props().enabled
    }

    #[setter]
    fn set_enabled(&self, enabled: bool) {
        set_enabled_prop(&self.node, &self.shared, enabled);
    }

    fn __repr__(&self) -> String {
        format!("<ScrollView {:?}>", self.node.id)
    }
}

/// A headless periodic timer.
#[pyclass(name = "Timer")]
pub struct PyTimer {
    shared: Arc<Shared>,
    node: Arc<WidgetNode>,
}

#[pymethods]
impl PyTimer {
    #[getter]
    fn id(&self) -> &str {
        &self.node.id
    }

    #[getter]
    fn interval_ms(&self) -> u64 {
        self.node.props().interval_ms
    }

    #[getter]
    fn enabled(&self) -> bool {
        self.node.props().enabled
    }

    #[setter]
    fn set_enabled(&self, enabled: bool) {
        self.node.props.lock().unwrap().enabled = enabled;
        self.shared
            .post_timer(self.node.key, Cmd::SetEnabled(enabled));
    }

    #[getter]
    fn on_tick(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        get_callback(&self.node.on_tick, py)
    }

    #[setter]
    fn set_on_tick(&self, py: Python<'_>, callback: Option<Py<PyAny>>) -> PyResult<()> {
        set_callback(&self.node.on_tick, py, callback, "on_tick")
    }

    fn __repr__(&self) -> String {
        format!(
            "<Timer {:?} interval_ms={} enabled={}>",
            self.node.id,
            self.node.props().interval_ms,
            self.node.props().enabled
        )
    }
}

fn set_text_prop(node: &Arc<WidgetNode>, shared: &Arc<Shared>, text: String) {
    node.props.lock().unwrap().text = text.clone();
    shared.post(node.key, Cmd::SetText(text));
}

fn set_visible_prop(node: &Arc<WidgetNode>, shared: &Arc<Shared>, visible: bool) {
    node.props.lock().unwrap().visible = visible;
    shared.post(node.key, Cmd::SetVisible(visible));
}

fn set_enabled_prop(node: &Arc<WidgetNode>, shared: &Arc<Shared>, enabled: bool) {
    node.props.lock().unwrap().enabled = enabled;
    shared.post(node.key, Cmd::SetEnabled(enabled));
}

fn set_pos_prop(
    node: &Arc<WidgetNode>,
    shared: &Arc<Shared>,
    value: usize,
    command: impl FnOnce(usize) -> Cmd,
) {
    let props = node.props();
    let maximum = props.maximum.max(props.minimum);
    let value = value.clamp(props.minimum, maximum);
    let mut guard = node.props.lock().unwrap();
    match node.kind {
        WidgetKind::UpDownEdit => guard.value = value,
        _ => guard.pos = value,
    }
    drop(guard);
    shared.post(node.key, command(value));
}

fn get_callback(slot: &Mutex<Option<Py<PyAny>>>, py: Python<'_>) -> Option<Py<PyAny>> {
    slot.lock()
        .unwrap()
        .as_ref()
        .map(|callback| callback.clone_ref(py))
}

fn set_callback(
    slot: &Mutex<Option<Py<PyAny>>>,
    py: Python<'_>,
    callback: Option<Py<PyAny>>,
    name: &str,
) -> PyResult<()> {
    check_callback(py, &callback, name)?;
    *slot.lock().unwrap() = callback;
    Ok(())
}

fn check_callback(py: Python<'_>, callback: &Option<Py<PyAny>>, name: &str) -> PyResult<()> {
    if let Some(callback) = callback
        && !callback.bind(py).is_callable()
    {
        return Err(PyTypeError::new_err(format!("{name} must be callable")));
    }
    Ok(())
}

fn new_stack(shared: &Arc<Shared>, orient: Orient, spacing: f64, padding: f64) -> PyStackPanel {
    let key = shared.next_layout_key();
    PyStackPanel {
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

fn make_node(shared: &Arc<Shared>, id: &str, kind: WidgetKind, text: &str) -> Arc<WidgetNode> {
    Arc::new(WidgetNode::new(
        shared.next_node_key(),
        id.to_string(),
        kind,
        text.to_string(),
    ))
}

fn make_edit(shared: &Arc<Shared>, id: &str, text: &str, on_change: Option<Py<PyAny>>) -> PyEdit {
    let node = make_node(shared, id, WidgetKind::Edit, text);
    *node.on_change.lock().unwrap() = on_change;
    PyEdit {
        shared: shared.clone(),
        node,
    }
}

fn make_check_box(
    shared: &Arc<Shared>,
    id: &str,
    text: &str,
    checked: bool,
    on_click: Option<Py<PyAny>>,
) -> PyCheckBox {
    let node = make_node(shared, id, WidgetKind::CheckBox, text);
    node.props.lock().unwrap().checked = checked;
    *node.on_click.lock().unwrap() = on_click;
    PyCheckBox {
        shared: shared.clone(),
        node,
    }
}

fn make_link_label(
    shared: &Arc<Shared>,
    id: &str,
    text: &str,
    uri: &str,
    on_click: Option<Py<PyAny>>,
) -> PyLinkLabel {
    let node = make_node(shared, id, WidgetKind::LinkLabel, text);
    node.props.lock().unwrap().uri = uri.to_string();
    *node.on_click.lock().unwrap() = on_click;
    PyLinkLabel {
        shared: shared.clone(),
        node,
    }
}

fn make_progress(
    shared: &Arc<Shared>,
    id: &str,
    pos: usize,
    minimum: usize,
    maximum: usize,
    indeterminate: bool,
) -> PyProgress {
    let node = make_node(shared, id, WidgetKind::Progress, "");
    let maximum = maximum.max(minimum);
    {
        let mut props = node.props.lock().unwrap();
        props.minimum = minimum;
        props.maximum = maximum;
        props.pos = pos.clamp(minimum, maximum);
        props.indeterminate = indeterminate;
    }
    PyProgress {
        shared: shared.clone(),
        node,
    }
}

fn make_text_box(shared: &Arc<Shared>, id: &str, text: &str) -> PyTextBox {
    PyTextBox {
        shared: shared.clone(),
        node: make_node(shared, id, WidgetKind::TextBox, text),
    }
}

fn make_slider(
    shared: &Arc<Shared>,
    id: &str,
    pos: usize,
    minimum: usize,
    maximum: usize,
    on_change: Option<Py<PyAny>>,
) -> PySlider {
    let node = make_node(shared, id, WidgetKind::Slider, "");
    let maximum = maximum.max(minimum);
    {
        let mut props = node.props.lock().unwrap();
        props.minimum = minimum;
        props.maximum = maximum;
        props.pos = pos.clamp(minimum, maximum);
    }
    *node.on_change.lock().unwrap() = on_change;
    PySlider {
        shared: shared.clone(),
        node,
    }
}

fn make_up_down_edit(
    shared: &Arc<Shared>,
    id: &str,
    value: usize,
    minimum: usize,
    maximum: usize,
    step: usize,
    on_change: Option<Py<PyAny>>,
) -> PyUpDownEdit {
    let node = make_node(shared, id, WidgetKind::UpDownEdit, "");
    let maximum = maximum.max(minimum);
    {
        let mut props = node.props.lock().unwrap();
        props.minimum = minimum;
        props.maximum = maximum;
        props.step = step;
        props.value = value.clamp(minimum, maximum);
    }
    *node.on_change.lock().unwrap() = on_change;
    PyUpDownEdit {
        shared: shared.clone(),
        node,
    }
}

fn make_combo_box(
    shared: &Arc<Shared>,
    id: &str,
    items: Vec<String>,
    selected: Option<usize>,
    text: &str,
    editable: bool,
    on_select: Option<Py<PyAny>>,
    on_change: Option<Py<PyAny>>,
) -> PyComboBox {
    let node = make_node(shared, id, WidgetKind::ComboBox, text);
    {
        let mut props = node.props.lock().unwrap();
        props.selected = selected.filter(|index| *index < items.len());
        props.items = items;
        props.editable = editable;
    }
    *node.on_select.lock().unwrap() = on_select;
    *node.on_change.lock().unwrap() = on_change;
    PyComboBox {
        shared: shared.clone(),
        node,
    }
}

fn make_radio_button(
    shared: &Arc<Shared>,
    id: &str,
    text: &str,
    checked: bool,
    on_click: Option<Py<PyAny>>,
) -> PyRadioButton {
    let node = make_node(shared, id, WidgetKind::RadioButton, text);
    node.props.lock().unwrap().checked = checked;
    *node.on_click.lock().unwrap() = on_click;
    PyRadioButton {
        shared: shared.clone(),
        node,
    }
}

fn make_list_box(
    shared: &Arc<Shared>,
    id: &str,
    items: Vec<String>,
    selected: Vec<usize>,
    multiple: bool,
    on_select: Option<Py<PyAny>>,
) -> PyListBox {
    let node = make_node(shared, id, WidgetKind::ListBox, "");
    {
        let mut props = node.props.lock().unwrap();
        props.selected_indices = selected
            .into_iter()
            .filter(|index| *index < items.len())
            .collect();
        props.items = items;
        props.multiple = multiple;
    }
    *node.on_select.lock().unwrap() = on_select;
    PyListBox {
        shared: shared.clone(),
        node,
    }
}

fn make_scroll_view(shared: &Arc<Shared>, id: &str, hscroll: bool, vscroll: bool) -> PyScrollView {
    let node = make_node(shared, id, WidgetKind::ScrollView, "");
    {
        let mut props = node.props.lock().unwrap();
        props.hscroll = hscroll;
        props.vscroll = vscroll;
    }
    PyScrollView {
        shared: shared.clone(),
        node,
    }
}

fn make_timer(
    shared: &Arc<Shared>,
    id: &str,
    interval_ms: u64,
    on_tick: Option<Py<PyAny>>,
    enabled: bool,
) -> Arc<WidgetNode> {
    let node = make_node(shared, id, WidgetKind::Timer, "");
    {
        let mut props = node.props.lock().unwrap();
        props.interval_ms = interval_ms;
        props.enabled = enabled;
    }
    *node.on_tick.lock().unwrap() = on_tick;
    node
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
    if let Ok(edit) = child.extract::<PyRef<'_, PyEdit>>() {
        return Ok(Elem::Widget(edit.node.clone()));
    }
    if let Ok(check_box) = child.extract::<PyRef<'_, PyCheckBox>>() {
        return Ok(Elem::Widget(check_box.node.clone()));
    }
    if let Ok(link_label) = child.extract::<PyRef<'_, PyLinkLabel>>() {
        return Ok(Elem::Widget(link_label.node.clone()));
    }
    if let Ok(progress) = child.extract::<PyRef<'_, PyProgress>>() {
        return Ok(Elem::Widget(progress.node.clone()));
    }
    if let Ok(text_box) = child.extract::<PyRef<'_, PyTextBox>>() {
        return Ok(Elem::Widget(text_box.node.clone()));
    }
    if let Ok(slider) = child.extract::<PyRef<'_, PySlider>>() {
        return Ok(Elem::Widget(slider.node.clone()));
    }
    if let Ok(up_down) = child.extract::<PyRef<'_, PyUpDownEdit>>() {
        return Ok(Elem::Widget(up_down.node.clone()));
    }
    if let Ok(combo) = child.extract::<PyRef<'_, PyComboBox>>() {
        return Ok(Elem::Widget(combo.node.clone()));
    }
    if let Ok(radio) = child.extract::<PyRef<'_, PyRadioButton>>() {
        return Ok(Elem::Widget(radio.node.clone()));
    }
    if let Ok(list) = child.extract::<PyRef<'_, PyListBox>>() {
        return Ok(Elem::Widget(list.node.clone()));
    }
    if let Ok(scroll) = child.extract::<PyRef<'_, PyScrollView>>() {
        return Ok(Elem::Widget(scroll.node.clone()));
    }
    if let Ok(panel) = child.extract::<PyRef<'_, PyStackPanel>>() {
        return Ok(Elem::Panel(panel.layout.clone()));
    }
    if let Ok(grid) = child.extract::<PyRef<'_, PyGrid>>() {
        return Ok(Elem::Panel(grid.layout.clone()));
    }
    Err(PyTypeError::new_err(
        "expected a widget (Label/Button/Edit/CheckBox/LinkLabel/Progress/TextBox/\
         Slider/UpDownEdit/ComboBox/RadioButton/ListBox/ScrollView), StackPanel or Grid",
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
pub mod _pywinio {
    #[pymodule_export]
    use super::{
        PyApp, PyButton, PyCheckBox, PyComboBox, PyEdit, PyGrid, PyLabel, PyLinkLabel, PyListBox,
        PyProgress, PyRadioButton, PyScrollView, PySlider, PyStackPanel, PyTextBox, PyTimer,
        PyUpDownEdit,
    };

    #[pymodule_export]
    #[allow(non_upper_case_globals)]
    pub const __version__: &str = env!("CARGO_PKG_VERSION");
}
