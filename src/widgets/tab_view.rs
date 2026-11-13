use std::sync::Arc;

use pyo3::prelude::*;

use crate::model::*;
use crate::widget::Cmd;

use super::support::*;

/// A tabbed container.
#[pyclass(name = "TabView")]
pub struct TabView {
    pub(crate) shared: Arc<Shared>,
    pub(crate) node: Arc<WidgetNode>,
}

#[pymethods]
impl TabView {
    #[getter]
    fn id(&self) -> &str {
        &self.node.id
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
            "<TabView {:?} selected={:?}>",
            self.node.id,
            self.node.props().selected
        )
    }
}

/// One page of a `TabView`.
#[pyclass(name = "TabViewItem")]
pub struct TabViewItem {
    pub(crate) shared: Arc<Shared>,
    pub(crate) node: Arc<WidgetNode>,
}

#[pymethods]
impl TabViewItem {
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

    /// Set the layout panel displayed on this page.
    fn set_content(&self, content: &Bound<'_, PyAny>) -> PyResult<()> {
        let layout = extract_layout(content)?;
        *self.node.tab_content.lock().unwrap() = Some(layout);
        Ok(())
    }

    fn __repr__(&self) -> String {
        format!(
            "<TabViewItem {:?} text={:?}>",
            self.node.id,
            self.node.props().text
        )
    }
}

pub(crate) fn make_tab_view(
    shared: &Arc<Shared>,
    id: &str,
    selected: Option<usize>,
    on_select: Option<Py<PyAny>>,
) -> TabView {
    let node = make_node(shared, id, WidgetKind::TabView, "");
    node.props.lock().unwrap().selected = selected;
    *node.on_select.lock().unwrap() = on_select;
    TabView {
        shared: shared.clone(),
        node,
    }
}

pub(crate) fn make_tab_view_item(shared: &Arc<Shared>, id: &str, title: &str) -> TabViewItem {
    let node = make_node(shared, id, WidgetKind::TabViewItem, title);
    TabViewItem {
        shared: shared.clone(),
        node,
    }
}
