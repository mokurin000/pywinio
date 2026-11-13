use super::support::*;
use crate::model::*;
use crate::widget::Cmd;
use pyo3::prelude::*;
use std::sync::Arc;

/// A progress bar.
#[pyclass(name = "Progress")]
pub struct Progress {
    pub(crate) shared: Arc<Shared>,
    pub(crate) node: Arc<WidgetNode>,
}

#[pymethods]
impl Progress {
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
