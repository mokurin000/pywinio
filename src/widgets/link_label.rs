use super::support::*;
use crate::model::*;
use crate::widget::Cmd;
use pyo3::prelude::*;
use std::sync::Arc;

/// A hyperlink-style label.
#[pyclass(name = "LinkLabel")]
pub struct LinkLabel {
    pub(crate) shared: Arc<Shared>,
    pub(crate) node: Arc<WidgetNode>,
}

#[pymethods]
impl LinkLabel {
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
