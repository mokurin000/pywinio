use super::support::*;
use crate::model::*;
use pyo3::prelude::*;
use std::sync::Arc;
use winio::prelude::Orient;

/// A stacked layout container.
#[pyclass(name = "StackPanel")]
pub struct StackPanel {
    pub(crate) layout: Arc<LayoutSpec>,
}

#[pymethods]
impl StackPanel {
    #[getter]
    fn orient(&self) -> &'static str {
        match &self.layout.kind {
            LayoutKind::Stack {
                orient: Orient::Vertical,
                ..
            } => "vertical",
            _ => "horizontal",
        }
    }

    /// Add a widget or a nested container.
    #[pyo3(signature = (child, *, grow = false, margin = None, halign: "Literal['left', 'center', 'right', 'stretch']" = "stretch", valign: "Literal['top', 'center', 'bottom', 'stretch']" = "stretch", width = None, height = None))]
    #[allow(clippy::too_many_arguments)]
    fn add(
        &self,
        child: &Bound<'_, PyAny>,
        grow: bool,
        margin: Option<(f64, f64, f64, f64)>,
        halign: &str,
        valign: &str,
        width: Option<f64>,
        height: Option<f64>,
    ) -> PyResult<()> {
        let opts = item_opts(grow, margin, halign, valign, width, height)?;
        push_child(&self.layout, child, opts)
    }
}
