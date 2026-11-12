use super::support::*;
use crate::model::*;
use crate::widget::Cmd;
use pyo3::prelude::*;
use std::sync::Arc;

/// A headless periodic timer.
#[pyclass(name = "Timer")]
pub struct Timer {
    pub(crate) shared: Arc<Shared>,
    pub(crate) node: Arc<WidgetNode>,
}

#[pymethods]
impl Timer {
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
