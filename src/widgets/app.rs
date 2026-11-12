use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::model::Track;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

use super::*;
use crate::model;
use crate::model::{DEFAULT_SIZE, LayoutSpec, Shared, WidgetNode};
use crate::{layout, root};

/// The application object.
///
/// Create it on the Python main thread, describe the UI, then call
/// [`run`](App::run). `run` releases the GIL and blocks until the window is
/// closed (or [`quit`](App::quit) is called).
#[pyclass(name = "App")]
pub struct App {
    shared: Arc<Shared>,
    title: String,
    size: (f64, f64),
    layout: Mutex<Option<Arc<LayoutSpec>>>,
    root_obj: Mutex<Option<Py<PyAny>>>,
    timers: Mutex<Vec<Arc<WidgetNode>>>,
    running: AtomicBool,
}

#[pymethods]
impl App {
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
    fn stack(&self, orient: &str, spacing: f64, padding: f64) -> PyResult<StackPanel> {
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
    ) -> PyResult<Grid> {
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
    ) -> PyResult<Button> {
        check_callback(py, &on_click, "on_click")?;
        make_button(&self.shared, id, text, on_click)
    }

    /// Create a text label.
    #[pyo3(signature = (id, text = ""))]
    fn label(&self, id: &str, text: &str) -> Label {
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
    ) -> PyResult<Edit> {
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
    ) -> PyResult<CheckBox> {
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
    ) -> PyResult<LinkLabel> {
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
    ) -> PyResult<Progress> {
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
    fn text_box(&self, id: &str, text: &str) -> PyResult<TextBox> {
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
    ) -> PyResult<Slider> {
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
    ) -> PyResult<UpDownEdit> {
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
    ) -> PyResult<ComboBox> {
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
    ) -> PyResult<RadioButton> {
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
    ) -> PyResult<ListBox> {
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
    fn scroll_view(&self, id: &str, hscroll: bool, vscroll: bool) -> PyResult<ScrollView> {
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
    ) -> PyResult<Timer> {
        if interval_ms == 0 {
            return Err(PyValueError::new_err("interval_ms must be positive"));
        }
        check_callback(py, &on_tick, "on_tick")?;
        let node = make_timer(&self.shared, id, interval_ms, on_tick, enabled);
        self.timers.lock().unwrap().push(node.clone());
        Ok(Timer {
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
