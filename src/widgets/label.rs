use crate::model::*;
use crate::widget::Cmd;
use pyo3::prelude::*;
use std::sync::Arc;

/// A read-only text widget.
#[pyclass(name = "Label")]
pub struct Label {
    pub(crate) shared: Arc<Shared>,
    pub(crate) node: Arc<WidgetNode>,
}

#[pymethods]
impl Label {
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
