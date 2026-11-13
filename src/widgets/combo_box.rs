use super::support::*;
use crate::model::*;
use crate::widget::Cmd;
use pyo3::prelude::*;
use std::sync::Arc;

/// A combo box.
#[pyclass(name = "ComboBox")]
pub struct ComboBox {
    pub(crate) shared: Arc<Shared>,
    pub(crate) node: Arc<WidgetNode>,
}

#[pymethods]
impl ComboBox {
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
    fn items(&self) -> Vec<String> {
        self.node.props().items
    }

    #[setter]
    fn set_items(&self, items: Vec<String>) {
        let selected = {
            let mut props = self.node.props.lock().unwrap();
            props.items = items.clone();
            if props.selected.is_some_and(|index| index >= items.len()) {
                props.selected = None;
            }
            props.selected
        };
        self.shared.post(self.node.key, Cmd::SetItems(items));
        self.shared.post(self.node.key, Cmd::SetSelection(selected));
    }

    #[getter]
    fn selected(&self) -> Option<usize> {
        self.node.props().selected
    }

    #[setter]
    fn set_selected(&self, selected: Option<usize>) {
        self.node.props.lock().unwrap().selected = selected;
        self.shared.post(self.node.key, Cmd::SetSelection(selected));
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
    fn editable(&self) -> bool {
        self.node.props().editable
    }

    #[setter]
    fn set_editable(&self, editable: bool) {
        self.node.props.lock().unwrap().editable = editable;
        self.shared.post(self.node.key, Cmd::SetEditable(editable));
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
            "<ComboBox {:?} items={} selected={:?}>",
            self.node.id,
            self.node.props().items.len(),
            self.node.props().selected
        )
    }
}
