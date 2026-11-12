use super::support::*;
use crate::model::*;
use pyo3::prelude::*;
use std::sync::Arc;

/// A grid layout container.
#[pyclass(name = "Grid")]
pub struct Grid {
    pub(crate) layout: Arc<LayoutSpec>,
}

#[pymethods]
impl Grid {
    #[getter]
    fn rows(&self) -> usize {
        match &self.layout.kind {
            LayoutKind::Grid { rows, .. } => rows.len(),
            _ => 0,
        }
    }

    #[getter]
    fn columns(&self) -> usize {
        match &self.layout.kind {
            LayoutKind::Grid { columns, .. } => columns.len(),
            _ => 0,
        }
    }

    /// Add a widget or a nested container at a grid position.
    #[pyo3(signature = (child, *, row = 0, col = 0, row_span = 1, col_span = 1, margin = None, halign: "Literal['left', 'center', 'right', 'stretch']" = "stretch", valign: "Literal['top', 'center', 'bottom', 'stretch']" = "stretch", width = None, height = None))]
    #[allow(clippy::too_many_arguments)]
    fn add(
        &self,
        child: &Bound<'_, PyAny>,
        row: usize,
        col: usize,
        row_span: usize,
        col_span: usize,
        margin: Option<(f64, f64, f64, f64)>,
        halign: &str,
        valign: &str,
        width: Option<f64>,
        height: Option<f64>,
    ) -> PyResult<()> {
        let mut opts = item_opts(false, margin, halign, valign, width, height)?;
        opts.row = row;
        opts.col = col;
        opts.row_span = row_span.max(1);
        opts.col_span = col_span.max(1);
        check_grid_bounds(&self.layout, &opts)?;
        push_child(&self.layout, child, opts)
    }
}
