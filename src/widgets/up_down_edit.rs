use super::support::*;
use crate::model::*;
use crate::widget::Cmd;
use pyo3::prelude::*;
use std::sync::Arc;

/// An integer input with up/down buttons.
#[pyclass(name = "UpDownEdit")]
pub struct UpDownEdit {
    pub(crate) shared: Arc<Shared>,
    pub(crate) node: Arc<WidgetNode>,
}

#[pymethods]
impl UpDownEdit {
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
