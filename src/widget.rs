//! The single component type that wraps every Python-declared widget.
//!
//! Python only chooses the *kind* and the initial properties; the ELM
//! plumbing (`start`/`update`/`update_children`/`render_*`) is implemented
//! once here and reused for every node, so Python never registers callbacks
//! with the component runtime directly.

use std::sync::Arc;

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
    /// Internal: turn a native widget event into an output event.
    Raise(Evt),
}

/// Events emitted by a widget component towards the root.
#[derive(Debug, Clone)]
pub enum Evt {
    Clicked(u64),
    Changed(u64),
    Closed,
    Resized,
}

/// Initialization data for one [`AnyWidget`].
pub struct AnyInit<'a> {
    pub node: Arc<WidgetNode>,
    pub parent: &'a Child<Window>,
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
            _ => {}
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
        dispatch_child!(self, child => child.preferred_size())
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
                Ok(Self::Button { node, child })
            }
            WidgetKind::Label => {
                let mut child = Child::<Label>::init(parent).await?;
                child.set_text(&props.text)?;
                child.set_visible(props.visible)?;
                Ok(Self::Label { child })
            }
            WidgetKind::Edit => {
                let mut child = Child::<Edit>::init(parent).await?;
                child.set_text(&props.text)?;
                child.set_enabled(props.enabled)?;
                child.set_visible(props.visible)?;
                Ok(Self::Edit { node, child })
            }
            WidgetKind::CheckBox => {
                let mut child = Child::<CheckBox>::init(parent).await?;
                child.set_text(&props.text)?;
                child.set_checked(props.checked)?;
                child.set_enabled(props.enabled)?;
                child.set_visible(props.visible)?;
                Ok(Self::CheckBox { node, child })
            }
            WidgetKind::LinkLabel => {
                let mut child = Child::<LinkLabel>::init(parent).await?;
                child.set_text(&props.text)?;
                child.set_uri(&props.uri)?;
                child.set_enabled(props.enabled)?;
                child.set_visible(props.visible)?;
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
                Ok(Self::Progress { node, child })
            }
            WidgetKind::TextBox => {
                let mut child = Child::<TextBox>::init(parent).await?;
                child.set_text(&props.text)?;
                child.set_enabled(props.enabled)?;
                child.set_visible(props.visible)?;
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
                Ok(Self::UpDownEdit { node, child })
            }
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
                    Self::Label { .. } => return Ok(false),
                }
                Ok(false)
            }
            Cmd::SetVisible(visible) => {
                dispatch_child!(self, child => child.set_visible(visible)?);
                Ok(true)
            }
            Cmd::SetChecked(checked) => {
                if let Self::CheckBox { child, .. } = self {
                    child.set_checked(checked)?;
                    Ok(true)
                } else {
                    Ok(false)
                }
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
