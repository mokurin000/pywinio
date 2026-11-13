//! The single component type that wraps every Python-declared widget.
//!
//! Python only chooses the *kind* and the initial properties; the ELM
//! plumbing (`start`/`update`/`update_children`/`render_*`) is implemented
//! once here and reused for every node, so Python never registers callbacks
//! with the component runtime directly.

use std::sync::Arc;
use std::time::Duration;

use winio::prelude::*;

use crate::model::{WidgetKind, WidgetNode};

/// Commands posted from Python (or from the root component) to one widget.
///
/// A `Cmd` is always delivered to a specific node through its own
/// `ComponentSender`, so it does not need to carry a node id.
#[derive(Debug)]
pub enum Cmd {
    SetText(String),
    SetEnabled(bool),
    SetVisible(bool),
    SetChecked(bool),
    SetUri(String),
    SetPos(usize),
    SetMinimum(usize),
    SetMaximum(usize),
    SetIndeterminate(bool),
    SetValue(usize),
    SetStep(usize),
    SetItems(Vec<String>),
    SetSelection(Option<usize>),
    SetSelectedIndices(Vec<usize>),
    SetMultiple(bool),
    SetEditable(bool),
    SetHScroll(bool),
    SetVScroll(bool),
    SetTooltip(String),
    /// Internal: turn a native widget event into an output event.
    Raise(Evt),
}

/// Events emitted by a widget component towards the root.
#[derive(Debug, Clone)]
pub enum Evt {
    Clicked(u64),
    Changed(u64),
    Selected(u64),
    Ticked(u64),
    Closed,
    Resized,
}

/// Initialization data for one [`AnyWidget`].
pub struct AnyInit<'a> {
    pub node: Arc<WidgetNode>,
    pub parent: BorrowedContainer<'a>,
}

/// A heterogeneous, ELM-compatible wrapper around the supported widgets.
pub enum AnyWidget {
    Button {
        node: Arc<WidgetNode>,
        child: Child<Button>,
    },
    Label {
        child: Child<Label>,
    },
    Edit {
        node: Arc<WidgetNode>,
        child: Child<Edit>,
    },
    CheckBox {
        node: Arc<WidgetNode>,
        child: Child<CheckBox>,
    },
    LinkLabel {
        node: Arc<WidgetNode>,
        child: Child<LinkLabel>,
    },
    Progress {
        node: Arc<WidgetNode>,
        child: Child<Progress>,
    },
    TextBox {
        node: Arc<WidgetNode>,
        child: Child<TextBox>,
    },
    Slider {
        node: Arc<WidgetNode>,
        child: Child<Slider>,
    },
    UpDownEdit {
        node: Arc<WidgetNode>,
        child: Child<UpDownEdit>,
    },
    ComboBox {
        node: Arc<WidgetNode>,
        child: Child<ComboBox>,
    },
    RadioButton {
        node: Arc<WidgetNode>,
        child: Child<RadioButton>,
    },
    ListBox {
        node: Arc<WidgetNode>,
        child: Child<ListBox>,
    },
    ScrollView {
        child: Child<ScrollView>,
    },
}

/// Expand to `$body` once per variant, binding the variant's `child` field.
macro_rules! dispatch_child {
    ($this:expr, $child:ident => $body:expr) => {
        match $this {
            Self::Button { $child, .. } => $body,
            Self::Label { $child, .. } => $body,
            Self::Edit { $child, .. } => $body,
            Self::CheckBox { $child, .. } => $body,
            Self::LinkLabel { $child, .. } => $body,
            Self::Progress { $child, .. } => $body,
            Self::TextBox { $child, .. } => $body,
            Self::Slider { $child, .. } => $body,
            Self::UpDownEdit { $child, .. } => $body,
            Self::ComboBox { $child, .. } => $body,
            Self::RadioButton { $child, .. } => $body,
            Self::ListBox { $child, .. } => $body,
            Self::ScrollView { $child, .. } => $body,
        }
    };
}

/// Same as `dispatch_child!`, but skipping `ScrollView` (it has no preferred
/// size in winio 0.13).
macro_rules! dispatch_preferred {
    ($this:expr, $child:ident => $body:expr) => {
        match $this {
            Self::Button { $child, .. } => $body,
            Self::Label { $child, .. } => $body,
            Self::Edit { $child, .. } => $body,
            Self::CheckBox { $child, .. } => $body,
            Self::LinkLabel { $child, .. } => $body,
            Self::Progress { $child, .. } => $body,
            Self::TextBox { $child, .. } => $body,
            Self::Slider { $child, .. } => $body,
            Self::UpDownEdit { $child, .. } => $body,
            Self::ComboBox { $child, .. } => $body,
            Self::RadioButton { $child, .. } => $body,
            Self::ListBox { $child, .. } => $body,
            Self::ScrollView { .. } => Ok(Size::zero()),
        }
    };
}

impl AnyWidget {
    /// Mirror native state that the user can change back into the shadow
    /// properties before a callback runs.
    fn sync_props(&self) {
        match self {
            Self::Edit { node, child } => {
                if let Ok(text) = child.text() {
                    node.props.lock().unwrap().text = text;
                }
            }
            Self::TextBox { node, child } => {
                if let Ok(text) = child.text() {
                    node.props.lock().unwrap().text = text;
                }
            }
            Self::CheckBox { node, child } => {
                if let Ok(checked) = child.is_checked() {
                    node.props.lock().unwrap().checked = checked;
                }
            }
            Self::Slider { node, child } => {
                if let Ok(pos) = child.pos() {
                    node.props.lock().unwrap().pos = pos;
                }
            }
            Self::UpDownEdit { node, child } => {
                if let Ok(value) = child.value() {
                    node.props.lock().unwrap().value = value;
                }
            }
            Self::ComboBox { node, child } => {
                let mut props = node.props.lock().unwrap();
                if let Ok(selection) = child.selection() {
                    props.selected = selection;
                }
                if let Ok(text) = child.text() {
                    props.text = text;
                }
            }
            Self::RadioButton { node, child } => {
                if let Ok(checked) = child.is_checked() {
                    node.props.lock().unwrap().checked = checked;
                }
            }
            Self::ListBox { node, child } => {
                if let Ok(len) = child.len() {
                    let mut selected = Vec::new();
                    for i in 0..len {
                        if child.is_selected(i).unwrap_or(false) {
                            selected.push(i);
                        }
                    }
                    node.props.lock().unwrap().selected_indices = selected;
                }
            }
            _ => {}
        }
    }

    /// The native container used to parent widgets placed inside a
    /// `ScrollView` content panel.
    pub fn as_scroll_container(&self) -> Option<BorrowedContainer<'_>> {
        match self {
            Self::ScrollView { child, .. } => Some(child.as_container()),
            _ => None,
        }
    }
}

impl Failable for AnyWidget {
    type Error = Error;
}

impl Layoutable for AnyWidget {
    fn loc(&self) -> Result<Point> {
        dispatch_child!(self, child => child.loc())
    }

    fn set_loc(&mut self, p: Point) -> Result<()> {
        dispatch_child!(self, child => child.set_loc(p))
    }

    fn size(&self) -> Result<Size> {
        dispatch_child!(self, child => child.size())
    }

    fn set_size(&mut self, s: Size) -> Result<()> {
        dispatch_child!(self, child => child.set_size(s))
    }

    fn preferred_size(&self) -> Result<Size> {
        dispatch_preferred!(self, child => child.preferred_size())
    }
}

impl Component for AnyWidget {
    type Error = Error;
    type Event = Evt;
    type Init<'a> = AnyInit<'a>;
    type Message = Cmd;

    async fn init(init: Self::Init<'_>, _sender: &ComponentSender<Self>) -> Result<Self> {
        let props = init.node.props();
        let kind = init.node.kind;
        let node = init.node.clone();
        let parent: BorrowedContainer<'_> = init.parent.into();

        match kind {
            WidgetKind::Button => {
                let mut child = Child::<Button>::init(parent).await?;
                child.set_text(&props.text)?;
                child.set_enabled(props.enabled)?;
                child.set_visible(props.visible)?;
                child.set_tooltip(&props.tooltip)?;
                Ok(Self::Button { node, child })
            }
            WidgetKind::Label => {
                let mut child = Child::<Label>::init(parent).await?;
                child.set_text(&props.text)?;
                child.set_visible(props.visible)?;
                child.set_tooltip(&props.tooltip)?;
                Ok(Self::Label { child })
            }
            WidgetKind::Edit => {
                let mut child = Child::<Edit>::init(parent).await?;
                child.set_text(&props.text)?;
                child.set_enabled(props.enabled)?;
                child.set_visible(props.visible)?;
                child.set_tooltip(&props.tooltip)?;
                Ok(Self::Edit { node, child })
            }
            WidgetKind::CheckBox => {
                let mut child = Child::<CheckBox>::init(parent).await?;
                child.set_text(&props.text)?;
                child.set_checked(props.checked)?;
                child.set_enabled(props.enabled)?;
                child.set_visible(props.visible)?;
                child.set_tooltip(&props.tooltip)?;
                Ok(Self::CheckBox { node, child })
            }
            WidgetKind::LinkLabel => {
                let mut child = Child::<LinkLabel>::init(parent).await?;
                child.set_text(&props.text)?;
                child.set_uri(&props.uri)?;
                child.set_enabled(props.enabled)?;
                child.set_visible(props.visible)?;
                child.set_tooltip(&props.tooltip)?;
                Ok(Self::LinkLabel { node, child })
            }
            WidgetKind::Progress => {
                let mut child = Child::<Progress>::init(parent).await?;
                let maximum = props.maximum.max(props.minimum);
                // Toggle the marquee state before setting the position: on
                // Win32 `PBM_SETMARQUEE` with 0 resets the position.
                child.set_indeterminate(props.indeterminate)?;
                child.set_minimum(props.minimum)?;
                child.set_maximum(maximum)?;
                child.set_pos(props.pos.clamp(props.minimum, maximum))?;
                child.set_enabled(props.enabled)?;
                child.set_visible(props.visible)?;
                child.set_tooltip(&props.tooltip)?;
                Ok(Self::Progress { node, child })
            }
            WidgetKind::TextBox => {
                let mut child = Child::<TextBox>::init(parent).await?;
                child.set_text(&props.text)?;
                child.set_enabled(props.enabled)?;
                child.set_visible(props.visible)?;
                child.set_tooltip(&props.tooltip)?;
                Ok(Self::TextBox { node, child })
            }
            WidgetKind::Slider => {
                let mut child = Child::<Slider>::init(parent).await?;
                let maximum = props.maximum.max(props.minimum);
                child.set_minimum(props.minimum)?;
                child.set_maximum(maximum)?;
                child.set_pos(props.pos.clamp(props.minimum, maximum))?;
                child.set_enabled(props.enabled)?;
                child.set_visible(props.visible)?;
                child.set_tooltip(&props.tooltip)?;
                Ok(Self::Slider { node, child })
            }
            WidgetKind::UpDownEdit => {
                let mut child = Child::<UpDownEdit>::init(parent).await?;
                let maximum = props.maximum.max(props.minimum);
                child.set_minimum(props.minimum)?;
                child.set_maximum(maximum)?;
                child.set_step(props.step)?;
                child.set_value(props.value.clamp(props.minimum, maximum))?;
                child.set_enabled(props.enabled)?;
                child.set_visible(props.visible)?;
                child.set_tooltip(&props.tooltip)?;
                Ok(Self::UpDownEdit { node, child })
            }
            WidgetKind::ComboBox => {
                let mut child = Child::<ComboBox>::init(parent).await?;
                child.set_items(props.items.clone())?;
                child.set_editable(props.editable)?;
                if !props.text.is_empty() {
                    child.set_text(&props.text)?;
                }
                if let Some(selected) = props.selected
                    && selected < props.items.len()
                {
                    child.set_selection(selected)?;
                }
                child.set_enabled(props.enabled)?;
                child.set_visible(props.visible)?;
                child.set_tooltip(&props.tooltip)?;
                Ok(Self::ComboBox { node, child })
            }
            WidgetKind::RadioButton => {
                let mut child = Child::<RadioButton>::init(parent).await?;
                child.set_text(&props.text)?;
                child.set_checked(props.checked)?;
                child.set_enabled(props.enabled)?;
                child.set_visible(props.visible)?;
                child.set_tooltip(&props.tooltip)?;
                Ok(Self::RadioButton { node, child })
            }
            WidgetKind::ListBox => {
                let mut child = Child::<ListBox>::init(parent).await?;
                child.set_multiple(props.multiple)?;
                child.set_items(props.items.clone())?;
                for index in &props.selected_indices {
                    if *index < props.items.len() {
                        child.set_selected(*index, true)?;
                    }
                }
                child.set_enabled(props.enabled)?;
                child.set_visible(props.visible)?;
                child.set_tooltip(&props.tooltip)?;
                Ok(Self::ListBox { node, child })
            }
            WidgetKind::ScrollView => {
                let mut child = Child::<ScrollView>::init(parent).await?;
                child.set_hscroll(props.hscroll)?;
                child.set_vscroll(props.vscroll)?;
                child.set_enabled(props.enabled)?;
                child.set_visible(props.visible)?;
                Ok(Self::ScrollView { child })
            }
            WidgetKind::Timer => unreachable!("timers are not AnyWidgets"),
        }
    }

    async fn start(&mut self, sender: &ComponentSender<Self>) -> ! {
        match self {
            Self::Button { node, child } => {
                let key = node.key;
                child
                    .start(sender, move |event| match event {
                        ButtonEvent::Click => Some(Cmd::Raise(Evt::Clicked(key))),
                        _ => None,
                    })
                    .await
            }
            Self::Edit { node, child } => {
                let key = node.key;
                child
                    .start(sender, move |event| match event {
                        EditEvent::Change => Some(Cmd::Raise(Evt::Changed(key))),
                        _ => None,
                    })
                    .await
            }
            Self::CheckBox { node, child } => {
                let key = node.key;
                child
                    .start(sender, move |event| match event {
                        CheckBoxEvent::Click => Some(Cmd::Raise(Evt::Clicked(key))),
                        _ => None,
                    })
                    .await
            }
            Self::LinkLabel { node, child } => {
                let key = node.key;
                child
                    .start(sender, move |event| match event {
                        LinkLabelEvent::Click => Some(Cmd::Raise(Evt::Clicked(key))),
                        _ => None,
                    })
                    .await
            }
            Self::Slider { node, child } => {
                let key = node.key;
                child
                    .start(sender, move |event| match event {
                        SliderEvent::Change => Some(Cmd::Raise(Evt::Changed(key))),
                        _ => None,
                    })
                    .await
            }
            Self::UpDownEdit { node, child } => {
                let key = node.key;
                child
                    .start(sender, move |event| match event {
                        UpDownEditEvent::Change => Some(Cmd::Raise(Evt::Changed(key))),
                        _ => None,
                    })
                    .await
            }
            Self::ComboBox { node, child } => {
                let key = node.key;
                child
                    .start(sender, move |event| match event {
                        ComboBoxEvent::Select => Some(Cmd::Raise(Evt::Selected(key))),
                        ComboBoxEvent::Change => Some(Cmd::Raise(Evt::Changed(key))),
                        _ => None,
                    })
                    .await
            }
            Self::RadioButton { node, child } => {
                let key = node.key;
                child
                    .start(sender, move |event| match event {
                        RadioButtonEvent::Click => Some(Cmd::Raise(Evt::Clicked(key))),
                        _ => None,
                    })
                    .await
            }
            Self::ListBox { node, child } => {
                let key = node.key;
                child
                    .start(sender, move |event| match event {
                        ListBoxEvent::Select => Some(Cmd::Raise(Evt::Selected(key))),
                        _ => None,
                    })
                    .await
            }
            Self::ScrollView { child, .. } => {
                child.start(sender, |_event: ScrollViewEvent| None).await
            }
            Self::Label { child, .. } => child.start(sender, |_event: LabelEvent| None).await,
            Self::Progress { child, .. } => child.start(sender, |_event: ProgressEvent| None).await,
            Self::TextBox { child, .. } => child.start(sender, |_event: TextBoxEvent| None).await,
        }
    }

    async fn update_children(&mut self) -> Result<bool> {
        dispatch_child!(self, child => child.update().await)
    }

    async fn update(
        &mut self,
        message: Self::Message,
        sender: &ComponentSender<Self>,
    ) -> Result<bool> {
        match message {
            Cmd::SetText(text) => {
                match self {
                    Self::Button { child, .. } => child.set_text(&text)?,
                    Self::Label { child, .. } => child.set_text(&text)?,
                    Self::Edit { child, .. } => child.set_text(&text)?,
                    Self::CheckBox { child, .. } => child.set_text(&text)?,
                    Self::LinkLabel { child, .. } => child.set_text(&text)?,
                    Self::TextBox { child, .. } => child.set_text(&text)?,
                    Self::ComboBox { child, .. } => child.set_text(&text)?,
                    Self::RadioButton { child, .. } => child.set_text(&text)?,
                    _ => return Ok(false),
                }
                Ok(true)
            }
            Cmd::SetEnabled(enabled) => {
                match self {
                    Self::Button { child, .. } => child.set_enabled(enabled)?,
                    Self::Edit { child, .. } => child.set_enabled(enabled)?,
                    Self::CheckBox { child, .. } => child.set_enabled(enabled)?,
                    Self::LinkLabel { child, .. } => child.set_enabled(enabled)?,
                    Self::Progress { child, .. } => child.set_enabled(enabled)?,
                    Self::TextBox { child, .. } => child.set_enabled(enabled)?,
                    Self::Slider { child, .. } => child.set_enabled(enabled)?,
                    Self::UpDownEdit { child, .. } => child.set_enabled(enabled)?,
                    Self::ComboBox { child, .. } => child.set_enabled(enabled)?,
                    Self::RadioButton { child, .. } => child.set_enabled(enabled)?,
                    Self::ListBox { child, .. } => child.set_enabled(enabled)?,
                    Self::ScrollView { child, .. } => child.set_enabled(enabled)?,
                    Self::Label { .. } => return Ok(false),
                }
                Ok(false)
            }
            Cmd::SetVisible(visible) => {
                dispatch_child!(self, child => child.set_visible(visible)?);
                Ok(true)
            }
            Cmd::SetChecked(checked) => {
                match self {
                    Self::CheckBox { child, .. } => child.set_checked(checked)?,
                    Self::RadioButton { child, .. } => child.set_checked(checked)?,
                    _ => return Ok(false),
                }
                Ok(true)
            }
            Cmd::SetUri(uri) => {
                if let Self::LinkLabel { child, .. } = self {
                    child.set_uri(&uri)?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            Cmd::SetPos(pos) => {
                match self {
                    Self::Progress { child, .. } => child.set_pos(pos)?,
                    Self::Slider { child, .. } => child.set_pos(pos)?,
                    _ => return Ok(false),
                }
                Ok(true)
            }
            Cmd::SetMinimum(minimum) => {
                match self {
                    Self::Progress { child, .. } => child.set_minimum(minimum)?,
                    Self::Slider { child, .. } => child.set_minimum(minimum)?,
                    Self::UpDownEdit { child, .. } => child.set_minimum(minimum)?,
                    _ => return Ok(false),
                }
                Ok(true)
            }
            Cmd::SetMaximum(maximum) => {
                match self {
                    Self::Progress { child, .. } => child.set_maximum(maximum)?,
                    Self::Slider { child, .. } => child.set_maximum(maximum)?,
                    Self::UpDownEdit { child, .. } => child.set_maximum(maximum)?,
                    _ => return Ok(false),
                }
                Ok(true)
            }
            Cmd::SetIndeterminate(indeterminate) => {
                if let Self::Progress { node, child } = self {
                    child.set_indeterminate(indeterminate)?;
                    // Restore the position; Win32 resets it when the marquee
                    // state is turned off.
                    child.set_pos(node.props().pos)?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            Cmd::SetValue(value) => {
                if let Self::UpDownEdit { child, .. } = self {
                    child.set_value(value)?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            Cmd::SetStep(step) => {
                if let Self::UpDownEdit { child, .. } = self {
                    child.set_step(step)?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            Cmd::SetItems(items) => {
                match self {
                    Self::ComboBox { child, .. } => child.set_items(items)?,
                    Self::ListBox { child, .. } => child.set_items(items)?,
                    _ => return Ok(false),
                }
                Ok(true)
            }
            Cmd::SetSelection(selection) => {
                if let Self::ComboBox { child, .. } = self {
                    if let Some(index) = selection {
                        child.set_selection(index)?;
                    }
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            Cmd::SetSelectedIndices(selected) => {
                if let Self::ListBox { child, .. } = self {
                    let len = child.len()?;
                    for index in 0..len {
                        child.set_selected(index, selected.contains(&index))?;
                    }
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            Cmd::SetMultiple(multiple) => {
                if let Self::ListBox { child, .. } = self {
                    child.set_multiple(multiple)?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            Cmd::SetEditable(editable) => {
                if let Self::ComboBox { child, .. } = self {
                    child.set_editable(editable)?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            Cmd::SetHScroll(hscroll) => {
                if let Self::ScrollView { child, .. } = self {
                    child.set_hscroll(hscroll)?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            Cmd::SetVScroll(vscroll) => {
                if let Self::ScrollView { child, .. } = self {
                    child.set_vscroll(vscroll)?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            Cmd::SetTooltip(tooltip) => {
                match self {
                    Self::Button { child, .. } => child.set_tooltip(&tooltip)?,
                    Self::Label { child, .. } => child.set_tooltip(&tooltip)?,
                    Self::Edit { child, .. } => child.set_tooltip(&tooltip)?,
                    Self::CheckBox { child, .. } => child.set_tooltip(&tooltip)?,
                    Self::LinkLabel { child, .. } => child.set_tooltip(&tooltip)?,
                    Self::Progress { child, .. } => child.set_tooltip(&tooltip)?,
                    Self::TextBox { child, .. } => child.set_tooltip(&tooltip)?,
                    Self::Slider { child, .. } => child.set_tooltip(&tooltip)?,
                    Self::UpDownEdit { child, .. } => child.set_tooltip(&tooltip)?,
                    Self::ComboBox { child, .. } => child.set_tooltip(&tooltip)?,
                    Self::RadioButton { child, .. } => child.set_tooltip(&tooltip)?,
                    Self::ListBox { child, .. } => child.set_tooltip(&tooltip)?,
                    _ => return Ok(false),
                }
                Ok(false)
            }
            Cmd::Raise(event) => {
                self.sync_props();
                sender.output(event);
                Ok(false)
            }
        }
    }

    fn render_children(&mut self) -> Result<()> {
        dispatch_child!(self, child => child.render())
    }
}

/// Initialization data for a headless [`TimerWidget`].
pub struct TimerInit {
    pub node: Arc<WidgetNode>,
}

/// A headless periodic timer. It has no parent window and never participates
/// in layout.
pub struct TimerWidget {
    node: Arc<WidgetNode>,
    child: Child<Timer>,
}

impl Component for TimerWidget {
    type Error = Error;
    type Event = Evt;
    type Init<'a> = TimerInit;
    type Message = Cmd;

    async fn init(init: Self::Init<'_>, _sender: &ComponentSender<Self>) -> Result<Self> {
        let props = init.node.props();
        let interval = Duration::from_millis(props.interval_ms.max(1));
        let mut child = Child::<Timer>::init(interval).await?;
        child.set_enabled(props.enabled)?;
        Ok(Self {
            node: init.node,
            child,
        })
    }

    async fn start(&mut self, sender: &ComponentSender<Self>) -> ! {
        let key = self.node.key;
        self.child
            .start(sender, move |event| match event {
                TimerEvent::Tick => Some(Cmd::Raise(Evt::Ticked(key))),
                _ => None,
            })
            .await
    }

    async fn update_children(&mut self) -> Result<bool> {
        self.child.update().await
    }

    async fn update(
        &mut self,
        message: Self::Message,
        sender: &ComponentSender<Self>,
    ) -> Result<bool> {
        match message {
            Cmd::SetEnabled(enabled) => {
                self.child.set_enabled(enabled)?;
                Ok(false)
            }
            Cmd::Raise(event) => {
                sender.output(event);
                Ok(false)
            }
            _ => Ok(false),
        }
    }

    fn render_children(&mut self) -> Result<()> {
        self.child.render()
    }
}
