use super::support::*;
use crate::model::*;
use pyo3::prelude::*;
use std::sync::Arc;

/// A single-line text input.
#[pyclass(name = "Edit")]
pub struct Edit {
    pub(crate) shared: Arc<Shared>,
    pub(crate) node: Arc<WidgetNode>,
}

#[pymethods]
impl Edit {
    #[getter]
    fn id(&self) -> &str {
        &self.node.id
    }

    #[getter]
    fn tooltip(&self) -> String {
        self.node.props().tooltip
    }

    #[setter]
    fn set_tooltip(&self, tooltip: String) {
        set_tooltip_prop(&self.node, &self.shared, tooltip);
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
