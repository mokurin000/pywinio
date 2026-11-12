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
    /// Internal: turn a native widget event into an output event.
    Raise(Evt),
}

/// Events emitted by a widget component towards the root.
#[derive(Debug, Clone)]
pub enum Evt {
    Clicked(u64),
    Closed,
    Resized,
}

/// Initialization data for one [`AnyWidget`].
pub struct AnyInit<'a> {
    pub node: Arc<WidgetNode>,
    pub parent: &'a Child<Window>,
}

/// A heterogeneous, ELM-compatible wrapper around the supported widgets.
///
/// This is what the root component stores in its `HashMap<NodeKey, Child<_>>`.
pub enum AnyWidget {
    Button { key: u64, child: Child<Button> },
    Label { child: Child<Label> },
}

impl Failable for AnyWidget {
    type Error = Error;
}

impl Layoutable for AnyWidget {
    fn loc(&self) -> Result<Point> {
        match self {
            Self::Button { child, .. } => child.loc(),
            Self::Label { child, .. } => child.loc(),
        }
    }

    fn set_loc(&mut self, p: Point) -> Result<()> {
        match self {
            Self::Button { child, .. } => child.set_loc(p),
            Self::Label { child, .. } => child.set_loc(p),
        }
    }

    fn size(&self) -> Result<Size> {
        match self {
            Self::Button { child, .. } => child.size(),
            Self::Label { child, .. } => child.size(),
        }
    }

    fn set_size(&mut self, s: Size) -> Result<()> {
        match self {
            Self::Button { child, .. } => child.set_size(s),
            Self::Label { child, .. } => child.set_size(s),
        }
    }

    fn preferred_size(&self) -> Result<Size> {
        match self {
            Self::Button { child, .. } => child.preferred_size(),
            Self::Label { child, .. } => child.preferred_size(),
        }
    }
}

impl Component for AnyWidget {
    type Error = Error;
    type Event = Evt;
    type Init<'a> = AnyInit<'a>;
    type Message = Cmd;

    async fn init(init: Self::Init<'_>, _sender: &ComponentSender<Self>) -> Result<Self> {
        let props = init.node.props();
        let parent: BorrowedContainer<'_> = init.parent.into();

        match init.node.kind {
            WidgetKind::Button => {
                let mut child = Child::<Button>::init(parent).await?;
                child.set_text(&props.text)?;
                child.set_enabled(props.enabled)?;
                child.set_visible(props.visible)?;
                Ok(Self::Button {
                    key: init.node.key,
                    child,
                })
            }
            WidgetKind::Label => {
                let mut child = Child::<Label>::init(parent).await?;
                child.set_text(&props.text)?;
                child.set_visible(props.visible)?;
                Ok(Self::Label { child })
            }
        }
    }

    async fn start(&mut self, sender: &ComponentSender<Self>) -> ! {
        match self {
            Self::Button { key, child } => {
                let key = *key;
                child
                    .start(sender, move |event| match event {
                        ButtonEvent::Click => Some(Cmd::Raise(Evt::Clicked(key))),
                        _ => None,
                    })
                    .await
            }
            Self::Label { child, .. } => child.start(sender, |_event: LabelEvent| None).await,
        }
    }

    async fn update_children(&mut self) -> Result<bool> {
        match self {
            Self::Button { child, .. } => child.update().await,
            Self::Label { child, .. } => child.update().await,
        }
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
                }
                Ok(true)
            }
            Cmd::SetEnabled(enabled) => {
                if let Self::Button { child, .. } = self {
                    child.set_enabled(enabled)?;
                }
                Ok(false)
            }
            Cmd::SetVisible(visible) => {
                match self {
                    Self::Button { child, .. } => child.set_visible(visible)?,
                    Self::Label { child, .. } => child.set_visible(visible)?,
                }
                Ok(true)
            }
            Cmd::Raise(event) => {
                sender.output(event);
                Ok(false)
            }
        }
    }

    fn render_children(&mut self) -> Result<()> {
        match self {
            Self::Button { child, .. } => child.render(),
            Self::Label { child, .. } => child.render(),
        }
    }
}
