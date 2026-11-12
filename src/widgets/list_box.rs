use super::support::*;
use crate::model::*;
use crate::widget::Cmd;
use pyo3::prelude::*;
use std::sync::Arc;

/// A list box.
#[pyclass(name = "ListBox")]
pub struct ListBox {
    pub(crate) shared: Arc<Shared>,
    pub(crate) node: Arc<WidgetNode>,
}

#[pymethods]
impl ListBox {
    #[getter]
    fn id(&self) -> &str {
        &self.node.id
    }

    #[getter]
    fn items(&self) -> Vec<String> {
        self.node.props().items
    }

    #[setter]
    fn set_items(&self, items: Vec<String>) {
        let selected = {
            let mut props = self.node.props.lock().unwrap();
            props.items = items.clone();
            props.selected_indices.retain(|index| *index < items.len());
            props.selected_indices.clone()
        };
        self.shared.post(self.node.key, Cmd::SetItems(items));
        self.shared
            .post(self.node.key, Cmd::SetSelectedIndices(selected));
    }

    #[getter]
    fn selected(&self) -> Vec<usize> {
        self.node.props().selected_indices
    }

    #[setter]
    fn set_selected(&self, selected: Vec<usize>) {
        let selected = {
            let props = self.node.props();
            selected
                .into_iter()
                .filter(|index| *index < props.items.len())
                .collect::<Vec<_>>()
        };
        self.node.props.lock().unwrap().selected_indices = selected.clone();
        self.shared
            .post(self.node.key, Cmd::SetSelectedIndices(selected));
    }

    #[getter]
    fn multiple(&self) -> bool {
        self.node.props().multiple
    }

    #[setter]
    fn set_multiple(&self, multiple: bool) {
        self.node.props.lock().unwrap().multiple = multiple;
        self.shared.post(self.node.key, Cmd::SetMultiple(multiple));
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
    fn on_select(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        get_callback(&self.node.on_select, py)
    }

    #[setter]
    fn set_on_select(&self, py: Python<'_>, callback: Option<Py<PyAny>>) -> PyResult<()> {
        set_callback(&self.node.on_select, py, callback, "on_select")
    }

    fn __repr__(&self) -> String {
        format!(
            "<ListBox {:?} items={} selected={:?}>",
            self.node.id,
            self.node.props().items.len(),
            self.node.props().selected_indices
        )
    }
}
