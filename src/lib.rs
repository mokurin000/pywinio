//! pywinio: minimal PyO3 bindings for [winio](https://docs.rs/winio).
//!
//! Python builds a tree of components and layout containers; the Rust side
//! turns that tree into winio components and wires `start`/`update`
//! automatically. WebView and the compio compatibility layer are deliberately
//! out of scope.
//!
//! ```python
//! import pywinio as w
//!
//! app = w.App("Counter", (640, 480))
//! root = app.stack(orient="vertical", spacing=8, padding=16)
//! counter = app.label("counter", "0 clicks")
//! root.add(counter)
//! root.add(app.button("inc", "+1", on_click=lambda: ...))
//! app.root = root
//! app.run()
//! ```

use pyo3::prelude::*;

mod layout;
mod model;
mod root;
mod widget;
mod widgets;

pub use widgets::*;

/// Error type used by the component tree.
#[derive(Debug)]
pub enum AppError {
    Winio(winio::Error),
    Layout(String),
}

impl AppError {
    pub fn layout(message: impl Into<String>) -> Self {
        Self::Layout(message.into())
    }
}

impl From<winio::Error> for AppError {
    fn from(error: winio::Error) -> Self {
        Self::Winio(error)
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Winio(error) => write!(f, "winio error: {error:?}"),
            Self::Layout(error) => write!(f, "layout error: {error}"),
        }
    }
}

impl std::error::Error for AppError {}

#[pymodule]
pub mod _pywinio {
    #[pymodule_export]
    use super::{
        App, Button, CheckBox, ComboBox, Edit, Grid, Label, LinkLabel, ListBox, Progress,
        RadioButton, ScrollView, Slider, StackPanel, TabView, TabViewItem, TextBox, Timer,
        UpDownEdit,
    };

    #[pymodule_export]
    #[allow(non_upper_case_globals)]
    pub const __version__: &str = env!("CARGO_PKG_VERSION");
}
