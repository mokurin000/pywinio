use crate::model::*;
use crate::widget::Cmd;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use std::sync::Arc;

/// A clickable button.
#[pyclass(name = "Button")]
pub struct Button {
    pub(crate) shared: Arc<Shared>,
    pub(crate) node: Arc<WidgetNode>,
}

#[pymethods]
impl Button {
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
