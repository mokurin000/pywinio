use std::sync::{Arc, Mutex};

use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use winio::prelude::{HAlign, Orient, VAlign};

use super::{
    Button, CheckBox, ComboBox, Edit, Grid, Label, LinkLabel, ListBox, Progress, RadioButton,
    ScrollView, Slider, StackPanel, TextBox, UpDownEdit,
};
use crate::model::*;
use crate::widget::Cmd;

pub(crate) fn set_text_prop(node: &Arc<WidgetNode>, shared: &Arc<Shared>, text: String) {
    node.props.lock().unwrap().text = text.clone();
    shared.post(node.key, Cmd::SetText(text));
}

pub(crate) fn set_visible_prop(node: &Arc<WidgetNode>, shared: &Arc<Shared>, visible: bool) {
    node.props.lock().unwrap().visible = visible;
    shared.post(node.key, Cmd::SetVisible(visible));
}

pub(crate) fn set_enabled_prop(node: &Arc<WidgetNode>, shared: &Arc<Shared>, enabled: bool) {
    node.props.lock().unwrap().enabled = enabled;
    shared.post(node.key, Cmd::SetEnabled(enabled));
}

pub(crate) fn set_tooltip_prop(node: &Arc<WidgetNode>, shared: &Arc<Shared>, tooltip: String) {
    node.props.lock().unwrap().tooltip = tooltip.clone();
    shared.post(node.key, Cmd::SetTooltip(tooltip));
}

pub(crate) fn set_pos_prop(
    node: &Arc<WidgetNode>,
    shared: &Arc<Shared>,
    value: usize,
    command: impl FnOnce(usize) -> Cmd,
) {
    let props = node.props();
    let maximum = props.maximum.max(props.minimum);
    let value = value.clamp(props.minimum, maximum);
    let mut guard = node.props.lock().unwrap();
    match node.kind {
        WidgetKind::UpDownEdit => guard.value = value,
        _ => guard.pos = value,
    }
    drop(guard);
    shared.post(node.key, command(value));
}

pub(crate) fn get_callback(slot: &Mutex<Option<Py<PyAny>>>, py: Python<'_>) -> Option<Py<PyAny>> {
    slot.lock()
        .unwrap()
        .as_ref()
        .map(|callback| callback.clone_ref(py))
}

pub(crate) fn set_callback(
    slot: &Mutex<Option<Py<PyAny>>>,
    py: Python<'_>,
    callback: Option<Py<PyAny>>,
    name: &str,
) -> PyResult<()> {
    check_callback(py, &callback, name)?;
    *slot.lock().unwrap() = callback;
    Ok(())
}

pub(crate) fn check_callback(
    py: Python<'_>,
    callback: &Option<Py<PyAny>>,
    name: &str,
) -> PyResult<()> {
    if let Some(callback) = callback
        && !callback.bind(py).is_callable()
    {
        return Err(PyTypeError::new_err(format!("{name} must be callable")));
    }
    Ok(())
}

pub(crate) fn new_stack(
    shared: &Arc<Shared>,
    orient: Orient,
    spacing: f64,
    padding: f64,
) -> StackPanel {
    let key = shared.next_layout_key();
    StackPanel {
        layout: Arc::new(LayoutSpec::stack(
            key,
            orient,
            spacing.max(0.0),
            Edges::all(padding.max(0.0)),
        )),
    }
}

pub(crate) fn new_grid(
    shared: &Arc<Shared>,
    columns: Vec<Track>,
    rows: Vec<Track>,
    col_gap: f64,
    row_gap: f64,
    padding: f64,
) -> Grid {
    let key = shared.next_layout_key();
    Grid {
        layout: Arc::new(LayoutSpec::grid(
            key,
            columns,
            rows,
            col_gap.max(0.0),
            row_gap.max(0.0),
            Edges::all(padding.max(0.0)),
        )),
    }
}

pub(crate) fn make_label(shared: &Arc<Shared>, id: &str, text: &str) -> Label {
    let key = shared.next_node_key();
    Label {
        shared: shared.clone(),
        node: Arc::new(WidgetNode::new(
            key,
            id.to_string(),
            WidgetKind::Label,
            text.to_string(),
        )),
    }
}

pub(crate) fn make_button(
    shared: &Arc<Shared>,
    id: &str,
    text: &str,
    on_click: Option<Py<PyAny>>,
) -> PyResult<Button> {
    let key = shared.next_node_key();
    let node = Arc::new(WidgetNode::new(
        key,
        id.to_string(),
        WidgetKind::Button,
        text.to_string(),
    ));
    *node.on_click.lock().unwrap() = on_click;
    Ok(Button {
        shared: shared.clone(),
        node,
    })
}

pub(crate) fn make_node(
    shared: &Arc<Shared>,
    id: &str,
    kind: WidgetKind,
    text: &str,
) -> Arc<WidgetNode> {
    Arc::new(WidgetNode::new(
        shared.next_node_key(),
        id.to_string(),
        kind,
        text.to_string(),
    ))
}

pub(crate) fn make_edit(
    shared: &Arc<Shared>,
    id: &str,
    text: &str,
    on_change: Option<Py<PyAny>>,
) -> Edit {
    let node = make_node(shared, id, WidgetKind::Edit, text);
    *node.on_change.lock().unwrap() = on_change;
    Edit {
        shared: shared.clone(),
        node,
    }
}

pub(crate) fn make_check_box(
    shared: &Arc<Shared>,
    id: &str,
    text: &str,
    checked: bool,
    on_click: Option<Py<PyAny>>,
) -> CheckBox {
    let node = make_node(shared, id, WidgetKind::CheckBox, text);
    node.props.lock().unwrap().checked = checked;
    *node.on_click.lock().unwrap() = on_click;
    CheckBox {
        shared: shared.clone(),
        node,
    }
}

pub(crate) fn make_link_label(
    shared: &Arc<Shared>,
    id: &str,
    text: &str,
    uri: &str,
    on_click: Option<Py<PyAny>>,
) -> LinkLabel {
    let node = make_node(shared, id, WidgetKind::LinkLabel, text);
    node.props.lock().unwrap().uri = uri.to_string();
    *node.on_click.lock().unwrap() = on_click;
    LinkLabel {
        shared: shared.clone(),
        node,
    }
}

pub(crate) fn make_progress(
    shared: &Arc<Shared>,
    id: &str,
    pos: usize,
    minimum: usize,
    maximum: usize,
    indeterminate: bool,
) -> Progress {
    let node = make_node(shared, id, WidgetKind::Progress, "");
    let maximum = maximum.max(minimum);
    {
        let mut props = node.props.lock().unwrap();
        props.minimum = minimum;
        props.maximum = maximum;
        props.pos = pos.clamp(minimum, maximum);
        props.indeterminate = indeterminate;
    }
    Progress {
        shared: shared.clone(),
        node,
    }
}

pub(crate) fn make_text_box(shared: &Arc<Shared>, id: &str, text: &str) -> TextBox {
    TextBox {
        shared: shared.clone(),
        node: make_node(shared, id, WidgetKind::TextBox, text),
    }
}

pub(crate) fn make_slider(
    shared: &Arc<Shared>,
    id: &str,
    pos: usize,
    minimum: usize,
    maximum: usize,
    on_change: Option<Py<PyAny>>,
) -> Slider {
    let node = make_node(shared, id, WidgetKind::Slider, "");
    let maximum = maximum.max(minimum);
    {
        let mut props = node.props.lock().unwrap();
        props.minimum = minimum;
        props.maximum = maximum;
        props.pos = pos.clamp(minimum, maximum);
    }
    *node.on_change.lock().unwrap() = on_change;
    Slider {
        shared: shared.clone(),
        node,
    }
}

pub(crate) fn make_up_down_edit(
    shared: &Arc<Shared>,
    id: &str,
    value: usize,
    minimum: usize,
    maximum: usize,
    step: usize,
    on_change: Option<Py<PyAny>>,
) -> UpDownEdit {
    let node = make_node(shared, id, WidgetKind::UpDownEdit, "");
    let maximum = maximum.max(minimum);
    {
        let mut props = node.props.lock().unwrap();
        props.minimum = minimum;
        props.maximum = maximum;
        props.step = step;
        props.value = value.clamp(minimum, maximum);
    }
    *node.on_change.lock().unwrap() = on_change;
    UpDownEdit {
        shared: shared.clone(),
        node,
    }
}

pub(crate) fn make_combo_box(
    shared: &Arc<Shared>,
    id: &str,
    items: Vec<String>,
    selected: Option<usize>,
    text: &str,
    editable: bool,
    on_select: Option<Py<PyAny>>,
    on_change: Option<Py<PyAny>>,
) -> ComboBox {
    let node = make_node(shared, id, WidgetKind::ComboBox, text);
    {
        let mut props = node.props.lock().unwrap();
        props.selected = selected.filter(|index| *index < items.len());
        props.items = items;
        props.editable = editable;
    }
    *node.on_select.lock().unwrap() = on_select;
    *node.on_change.lock().unwrap() = on_change;
    ComboBox {
        shared: shared.clone(),
        node,
    }
}

pub(crate) fn make_radio_button(
    shared: &Arc<Shared>,
    id: &str,
    text: &str,
    checked: bool,
    on_click: Option<Py<PyAny>>,
) -> RadioButton {
    let node = make_node(shared, id, WidgetKind::RadioButton, text);
    node.props.lock().unwrap().checked = checked;
    *node.on_click.lock().unwrap() = on_click;
    RadioButton {
        shared: shared.clone(),
        node,
    }
}

pub(crate) fn make_list_box(
    shared: &Arc<Shared>,
    id: &str,
    items: Vec<String>,
    selected: Vec<usize>,
    multiple: bool,
    on_select: Option<Py<PyAny>>,
) -> ListBox {
    let node = make_node(shared, id, WidgetKind::ListBox, "");
    {
        let mut props = node.props.lock().unwrap();
        props.selected_indices = selected
            .into_iter()
            .filter(|index| *index < items.len())
            .collect();
        props.items = items;
        props.multiple = multiple;
    }
    *node.on_select.lock().unwrap() = on_select;
    ListBox {
        shared: shared.clone(),
        node,
    }
}

pub(crate) fn make_scroll_view(
    shared: &Arc<Shared>,
    id: &str,
    hscroll: bool,
    vscroll: bool,
) -> ScrollView {
    let node = make_node(shared, id, WidgetKind::ScrollView, "");
    {
        let mut props = node.props.lock().unwrap();
        props.hscroll = hscroll;
        props.vscroll = vscroll;
    }
    ScrollView {
        shared: shared.clone(),
        node,
    }
}

pub(crate) fn make_timer(
    shared: &Arc<Shared>,
    id: &str,
    interval_ms: u64,
    on_tick: Option<Py<PyAny>>,
    enabled: bool,
) -> Arc<WidgetNode> {
    let node = make_node(shared, id, WidgetKind::Timer, "");
    {
        let mut props = node.props.lock().unwrap();
        props.interval_ms = interval_ms;
        props.enabled = enabled;
    }
    *node.on_tick.lock().unwrap() = on_tick;
    node
}

pub(crate) fn item_opts(
    grow: bool,
    margin: Option<(f64, f64, f64, f64)>,
    halign: &str,
    valign: &str,
    width: Option<f64>,
    height: Option<f64>,
) -> PyResult<ItemOpts> {
    Ok(ItemOpts {
        grow,
        margin: margin
            .map(|(left, top, right, bottom)| Edges::new(left, top, right, bottom))
            .unwrap_or_default(),
        halign: parse_halign(halign)?,
        valign: parse_valign(valign)?,
        width,
        height,
        ..ItemOpts::default()
    })
}

/// Add a child (widget or nested container) to a container.
pub(crate) fn push_child(
    parent: &Arc<LayoutSpec>,
    child: &Bound<'_, PyAny>,
    opts: ItemOpts,
) -> PyResult<()> {
    let elem = extract_elem(child)?;

    if let Elem::Panel(panel) = &elem
        && Arc::ptr_eq(parent, panel)
    {
        return Err(PyValueError::new_err("a container cannot contain itself"));
    }

    check_grid_bounds(parent, &opts)?;
    parent.push(elem, opts);
    Ok(())
}

pub(crate) fn extract_elem(child: &Bound<'_, PyAny>) -> PyResult<Elem> {
    if let Ok(label) = child.extract::<PyRef<'_, Label>>() {
        return Ok(Elem::Widget(label.node.clone()));
    }
    if let Ok(button) = child.extract::<PyRef<'_, Button>>() {
        return Ok(Elem::Widget(button.node.clone()));
    }
    if let Ok(edit) = child.extract::<PyRef<'_, Edit>>() {
        return Ok(Elem::Widget(edit.node.clone()));
    }
    if let Ok(check_box) = child.extract::<PyRef<'_, CheckBox>>() {
        return Ok(Elem::Widget(check_box.node.clone()));
    }
    if let Ok(link_label) = child.extract::<PyRef<'_, LinkLabel>>() {
        return Ok(Elem::Widget(link_label.node.clone()));
    }
    if let Ok(progress) = child.extract::<PyRef<'_, Progress>>() {
        return Ok(Elem::Widget(progress.node.clone()));
    }
    if let Ok(text_box) = child.extract::<PyRef<'_, TextBox>>() {
        return Ok(Elem::Widget(text_box.node.clone()));
    }
    if let Ok(slider) = child.extract::<PyRef<'_, Slider>>() {
        return Ok(Elem::Widget(slider.node.clone()));
    }
    if let Ok(up_down) = child.extract::<PyRef<'_, UpDownEdit>>() {
        return Ok(Elem::Widget(up_down.node.clone()));
    }
    if let Ok(combo) = child.extract::<PyRef<'_, ComboBox>>() {
        return Ok(Elem::Widget(combo.node.clone()));
    }
    if let Ok(radio) = child.extract::<PyRef<'_, RadioButton>>() {
        return Ok(Elem::Widget(radio.node.clone()));
    }
    if let Ok(list) = child.extract::<PyRef<'_, ListBox>>() {
        return Ok(Elem::Widget(list.node.clone()));
    }
    if let Ok(scroll) = child.extract::<PyRef<'_, ScrollView>>() {
        return Ok(Elem::Widget(scroll.node.clone()));
    }
    if let Ok(panel) = child.extract::<PyRef<'_, StackPanel>>() {
        return Ok(Elem::Panel(panel.layout.clone()));
    }
    if let Ok(grid) = child.extract::<PyRef<'_, Grid>>() {
        return Ok(Elem::Panel(grid.layout.clone()));
    }
    Err(PyTypeError::new_err(
        "expected a widget (Label/Button/Edit/CheckBox/LinkLabel/Progress/TextBox/\
         Slider/UpDownEdit/ComboBox/RadioButton/ListBox/ScrollView), StackPanel or Grid",
    ))
}

pub(crate) fn extract_layout(node: &Bound<'_, PyAny>) -> PyResult<Arc<LayoutSpec>> {
    if let Ok(panel) = node.extract::<PyRef<'_, StackPanel>>() {
        return Ok(panel.layout.clone());
    }
    if let Ok(grid) = node.extract::<PyRef<'_, Grid>>() {
        return Ok(grid.layout.clone());
    }
    Err(PyTypeError::new_err(
        "app.root must be a StackPanel or a Grid",
    ))
}

pub(crate) fn check_grid_bounds(parent: &Arc<LayoutSpec>, opts: &ItemOpts) -> PyResult<()> {
    if let LayoutKind::Grid { columns, rows, .. } = &parent.kind {
        let row_span = opts.row_span.max(1);
        let col_span = opts.col_span.max(1);
        if opts.row + row_span > rows.len() || opts.col + col_span > columns.len() {
            return Err(PyValueError::new_err(format!(
                "grid item (row={}, col={}, row_span={}, col_span={}) exceeds a {}x{} grid",
                opts.row,
                opts.col,
                row_span,
                col_span,
                rows.len(),
                columns.len()
            )));
        }
    }
    Ok(())
}

pub(crate) fn parse_orient(value: &str) -> PyResult<Orient> {
    match value.to_ascii_lowercase().as_str() {
        "vertical" | "v" => Ok(Orient::Vertical),
        "horizontal" | "h" => Ok(Orient::Horizontal),
        _ => Err(PyValueError::new_err(format!(
            "invalid orientation {value:?}: expected 'vertical' or 'horizontal'"
        ))),
    }
}

pub(crate) fn parse_halign(value: &str) -> PyResult<HAlign> {
    match value.to_ascii_lowercase().as_str() {
        "left" | "start" => Ok(HAlign::Left),
        "center" | "middle" => Ok(HAlign::Center),
        "right" | "end" => Ok(HAlign::Right),
        "stretch" | "fill" => Ok(HAlign::Stretch),
        _ => Err(PyValueError::new_err(format!(
            "invalid halign {value:?}: expected left, center, right or stretch"
        ))),
    }
}

pub(crate) fn parse_valign(value: &str) -> PyResult<VAlign> {
    match value.to_ascii_lowercase().as_str() {
        "top" | "start" => Ok(VAlign::Top),
        "center" | "middle" => Ok(VAlign::Center),
        "bottom" | "end" => Ok(VAlign::Bottom),
        "stretch" | "fill" => Ok(VAlign::Stretch),
        _ => Err(PyValueError::new_err(format!(
            "invalid valign {value:?}: expected top, center, bottom or stretch"
        ))),
    }
}
