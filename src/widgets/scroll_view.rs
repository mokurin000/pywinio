use super::support::*;
use crate::model::*;
use crate::widget::Cmd;
use pyo3::prelude::*;
use std::sync::Arc;

/// A scrollable container.
#[pyclass(name = "ScrollView")]
pub struct ScrollView {
    pub(crate) shared: Arc<Shared>,
    pub(crate) node: Arc<WidgetNode>,
}

#[pymethods]
impl ScrollView {
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
