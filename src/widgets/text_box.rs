use super::support::*;
use crate::model::*;
use pyo3::prelude::*;
use std::sync::Arc;

/// A multi-line text input.
#[pyclass(name = "TextBox")]
pub struct TextBox {
    pub(crate) shared: Arc<Shared>,
    pub(crate) node: Arc<WidgetNode>,
}

#[pymethods]
impl TextBox {
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
