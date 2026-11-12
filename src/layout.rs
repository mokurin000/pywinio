//! Runtime layout for the Python-defined tree.
//!
//! The engine is a deliberately small subset of `winio-layout` (`StackPanel`
//! and `Grid`). It is implemented here instead of reusing `winio_layout`
//! because `winio_layout::StackPanel` borrows its children mutably for its
//! whole lifetime, which makes a *runtime-built*, arbitrarily nested tree
//! impossible to express without an arena or unsafe code.
//!
//! Two passes are used:
//!
//! 1. [`measure`] asks every native widget for its preferred size and computes
//!    preferred sizes bottom-up.
//! 2. [`arrange`] resolves the container rectangles top-down and returns the
//!    final rect of every widget.
//!
//! Both passes are pure with respect to widgets (measure only calls a
//! user-supplied closure, arrange returns rects), so they are unit-testable
//! without a window.

use std::collections::{HashMap, HashSet};

use winio::prelude::{HAlign, Orient, Point, Rect, Size, VAlign};

use crate::AppError;
use crate::model::{Edges, Elem, ItemOpts, LayoutItem, LayoutKind, LayoutSpec, NodeKey, Track};

/// Cached preferred sizes of one layout pass.
#[derive(Debug, Default)]
pub struct Measured {
    pub widgets: HashMap<NodeKey, Size>,
    pub panels: HashMap<u64, Size>,
}

/// Compute preferred sizes for every widget and container below `root`.
pub fn measure(
    root: &LayoutSpec,
    mut widget_size: impl FnMut(NodeKey) -> Result<Size, AppError>,
) -> Result<Measured, AppError> {
    let mut measured = Measured::default();
    let mut visiting = HashSet::new();
    measure_node(root, &mut widget_size, &mut measured, &mut visiting)?;
    Ok(measured)
}

fn measure_node(
    node: &LayoutSpec,
    widget_size: &mut impl FnMut(NodeKey) -> Result<Size, AppError>,
    measured: &mut Measured,
    visiting: &mut HashSet<u64>,
) -> Result<Size, AppError> {
    if !visiting.insert(node.uid) {
        return Err(AppError::layout(format!(
            "layout cycle at container {}",
            node.uid
        )));
    }

    let items = node.items.lock().unwrap();
    let mut children = Vec::with_capacity(items.len());
    for item in items.iter() {
        let size = match &item.elem {
            Elem::Widget(widget) => {
                if let Some(size) = measured.widgets.get(&widget.key) {
                    *size
                } else {
                    let size = widget_size(widget.key)?;
                    measured.widgets.insert(widget.key, size);
                    size
                }
            }
            Elem::Panel(panel) => measure_node(panel, widget_size, measured, visiting)?,
        };
        children.push((size, &item.opts));
    }

    let size = match &node.kind {
        LayoutKind::Stack {
            orient,
            spacing,
            padding,
        } => measure_stack(&children, *orient, *spacing, *padding),
        LayoutKind::Grid {
            columns,
            rows,
            col_gap,
            row_gap,
            padding,
        } => measure_grid(&children, columns, rows, *col_gap, *row_gap, *padding)?,
    };

    visiting.remove(&node.uid);
    measured.panels.insert(node.uid, size);
    Ok(size)
}

fn measure_stack(
    children: &[(Size, &ItemOpts)],
    orient: Orient,
    spacing: f64,
    padding: Edges,
) -> Size {
    let mut main = 0.0;
    let mut cross: f64 = 0.0;
    for (index, (size, opts)) in children.iter().enumerate() {
        let content = content_size(*size, opts);
        let (child_main, child_cross) = match orient {
            Orient::Vertical => (
                content.height + opts.margin.vertical(),
                content.width + opts.margin.horizontal(),
            ),
            Orient::Horizontal => (
                content.width + opts.margin.horizontal(),
                content.height + opts.margin.vertical(),
            ),
        };
        main += child_main;
        if index + 1 < children.len() {
            main += spacing;
        }
        cross = cross.max(child_cross);
    }

    match orient {
        Orient::Vertical => Size::new(cross + padding.horizontal(), main + padding.vertical()),
        Orient::Horizontal => Size::new(main + padding.horizontal(), cross + padding.vertical()),
    }
}

fn measure_grid(
    children: &[(Size, &ItemOpts)],
    columns: &[Track],
    rows: &[Track],
    col_gap: f64,
    row_gap: f64,
    padding: Edges,
) -> Result<Size, AppError> {
    if columns.is_empty() || rows.is_empty() {
        return Err(AppError::layout(
            "grid needs at least one column and one row",
        ));
    }

    let mut col_auto = vec![0.0f64; columns.len()];
    let mut row_auto = vec![0.0f64; rows.len()];

    // First pass: single-span items define the auto tracks.
    for (size, opts) in children {
        let span = span_of(opts, rows.len(), columns.len())?;
        let content = content_size(*size, opts);
        if span.col_span == 1 {
            let need = content.width + opts.margin.horizontal();
            col_auto[span.col] = col_auto[span.col].max(need);
        }
        if span.row_span == 1 {
            let need = content.height + opts.margin.vertical();
            row_auto[span.row] = row_auto[span.row].max(need);
        }
    }

    let mut col_sizes = measure_track_sizes(columns, &col_auto);
    let mut row_sizes = measure_track_sizes(rows, &row_auto);

    // Second pass: spanning items may need extra space in auto tracks.
    for (size, opts) in children {
        let span = span_of(opts, rows.len(), columns.len())?;
        let content = content_size(*size, opts);
        if span.col_span > 1 {
            let need = content.width + opts.margin.horizontal();
            let have = span_sum(&col_sizes, span.col, span.col_span)
                + col_gap * (span.col_span - 1) as f64;
            grow_auto(
                &mut col_sizes,
                columns,
                span.col,
                span.col_span,
                need - have,
            );
        }
        if span.row_span > 1 {
            let need = content.height + opts.margin.vertical();
            let have = span_sum(&row_sizes, span.row, span.row_span)
                + row_gap * (span.row_span - 1) as f64;
            grow_auto(&mut row_sizes, rows, span.row, span.row_span, need - have);
        }
    }

    Ok(Size::new(
        col_sizes.iter().sum::<f64>() + col_gap * (columns.len() - 1) as f64 + padding.horizontal(),
        row_sizes.iter().sum::<f64>() + row_gap * (rows.len() - 1) as f64 + padding.vertical(),
    ))
}

/// Resolve the final rect of every widget below `root`.
pub fn arrange(
    root: &LayoutSpec,
    rect: Rect,
    measured: &Measured,
) -> Result<Vec<(NodeKey, Rect)>, AppError> {
    let mut out = Vec::new();
    let mut visiting = HashSet::new();
    arrange_node(root, rect, measured, &mut out, &mut visiting)?;
    Ok(out)
}

fn arrange_node(
    node: &LayoutSpec,
    rect: Rect,
    measured: &Measured,
    out: &mut Vec<(NodeKey, Rect)>,
    visiting: &mut HashSet<u64>,
) -> Result<(), AppError> {
    if !visiting.insert(node.uid) {
        return Err(AppError::layout(format!(
            "layout cycle at container {}",
            node.uid
        )));
    }
    let items = node.items.lock().unwrap();
    match &node.kind {
        LayoutKind::Stack {
            orient,
            spacing,
            padding,
        } => arrange_stack(
            &items, rect, *orient, *spacing, *padding, measured, out, visiting,
        )?,
        LayoutKind::Grid {
            columns,
            rows,
            col_gap,
            row_gap,
            padding,
        } => arrange_grid(
            &items, rect, columns, rows, *col_gap, *row_gap, *padding, measured, out, visiting,
        )?,
    }
    visiting.remove(&node.uid);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn arrange_stack(
    items: &[LayoutItem],
    rect: Rect,
    orient: Orient,
    spacing: f64,
    padding: Edges,
    measured: &Measured,
    out: &mut Vec<(NodeKey, Rect)>,
    visiting: &mut HashSet<u64>,
) -> Result<(), AppError> {
    let inner = inset(rect, padding);
    let available = match orient {
        Orient::Vertical => inner.height(),
        Orient::Horizontal => inner.width(),
    };

    let mut base = 0.0;
    let mut grow_count = 0usize;
    for item in items {
        let content = item_content(item, measured);
        let (child_main, margin) = match orient {
            Orient::Vertical => (content.height, item.opts.margin.vertical()),
            Orient::Horizontal => (content.width, item.opts.margin.horizontal()),
        };
        base += child_main + margin;
        if item.opts.grow {
            grow_count += 1;
        }
    }
    base += spacing * items.len().saturating_sub(1) as f64;
    let extra = if grow_count > 0 {
        ((available - base) / grow_count as f64).max(0.0)
    } else {
        0.0
    };

    let mut cursor = match orient {
        Orient::Vertical => inner.origin.y,
        Orient::Horizontal => inner.origin.x,
    };

    for (index, item) in items.iter().enumerate() {
        let content = item_content(item, measured);
        let (child_main, margin) = match orient {
            Orient::Vertical => (content.height, item.opts.margin.vertical()),
            Orient::Horizontal => (content.width, item.opts.margin.horizontal()),
        };
        let slot = child_main + margin + if item.opts.grow { extra } else { 0.0 };

        let mut opts = item.opts.clone();
        let area = match orient {
            Orient::Vertical => {
                opts.height = Some((slot - opts.margin.vertical()).max(0.0));
                opts.valign = VAlign::Stretch;
                Rect::new(
                    Point::new(inner.origin.x, cursor),
                    Size::new(inner.width(), slot),
                )
            }
            Orient::Horizontal => {
                opts.width = Some((slot - opts.margin.horizontal()).max(0.0));
                opts.halign = HAlign::Stretch;
                Rect::new(
                    Point::new(cursor, inner.origin.y),
                    Size::new(slot, inner.height()),
                )
            }
        };
        let child_rect = place(content, &opts, area);

        match &item.elem {
            Elem::Widget(widget) => out.push((widget.key, child_rect)),
            Elem::Panel(panel) => {
                arrange_node(panel, child_rect, measured, out, visiting)?;
            }
        }

        cursor += slot;
        if index + 1 < items.len() {
            cursor += spacing;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn arrange_grid(
    items: &[LayoutItem],
    rect: Rect,
    columns: &[Track],
    rows: &[Track],
    col_gap: f64,
    row_gap: f64,
    padding: Edges,
    measured: &Measured,
    out: &mut Vec<(NodeKey, Rect)>,
    visiting: &mut HashSet<u64>,
) -> Result<(), AppError> {
    let inner = inset(rect, padding);

    let mut col_auto = vec![0.0f64; columns.len()];
    let mut row_auto = vec![0.0f64; rows.len()];
    for item in items {
        let span = span_of(&item.opts, rows.len(), columns.len())?;
        let content = item_content(item, measured);
        if span.col_span == 1 {
            col_auto[span.col] =
                col_auto[span.col].max(content.width + item.opts.margin.horizontal());
        }
        if span.row_span == 1 {
            row_auto[span.row] =
                row_auto[span.row].max(content.height + item.opts.margin.vertical());
        }
    }

    let col_sizes = resolve_tracks(columns, &col_auto, inner.width(), col_gap);
    let row_sizes = resolve_tracks(rows, &row_auto, inner.height(), row_gap);
    let col_offsets = offsets(&col_sizes, col_gap);
    let row_offsets = offsets(&row_sizes, row_gap);

    for item in items {
        let span = span_of(&item.opts, rows.len(), columns.len())?;
        let x = inner.origin.x + col_offsets[span.col];
        let y = inner.origin.y + row_offsets[span.row];
        let w =
            span_sum(&col_sizes, span.col, span.col_span) + col_gap * (span.col_span - 1) as f64;
        let h =
            span_sum(&row_sizes, span.row, span.row_span) + row_gap * (span.row_span - 1) as f64;
        let area = Rect::new(Point::new(x, y), Size::new(w.max(0.0), h.max(0.0)));
        let content = item_content(item, measured);
        let child_rect = place(content, &item.opts, area);

        match &item.elem {
            Elem::Widget(widget) => out.push((widget.key, child_rect)),
            Elem::Panel(panel) => {
                arrange_node(panel, child_rect, measured, out, visiting)?;
            }
        }
    }
    Ok(())
}

/// Place `content` inside `area`, honoring explicit size and alignment.
fn place(content: Size, opts: &ItemOpts, area: Rect) -> Rect {
    let avail_w = (area.width() - opts.margin.horizontal()).max(0.0);
    let avail_h = (area.height() - opts.margin.vertical()).max(0.0);

    let width = opts.width.unwrap_or(match opts.halign {
        HAlign::Stretch => avail_w,
        _ => content.width.min(avail_w),
    });
    let height = opts.height.unwrap_or(match opts.valign {
        VAlign::Stretch => avail_h,
        _ => content.height.min(avail_h),
    });

    let x = area.origin.x
        + opts.margin.left
        + match opts.halign {
            HAlign::Left | HAlign::Stretch => 0.0,
            HAlign::Center => (avail_w - width) / 2.0,
            HAlign::Right => avail_w - width,
        };
    let y = area.origin.y
        + opts.margin.top
        + match opts.valign {
            VAlign::Top | VAlign::Stretch => 0.0,
            VAlign::Center => (avail_h - height) / 2.0,
            VAlign::Bottom => avail_h - height,
        };

    Rect::new(Point::new(x, y), Size::new(width.max(0.0), height.max(0.0)))
}

fn content_size(size: Size, opts: &ItemOpts) -> Size {
    Size::new(
        opts.width.unwrap_or(size.width).max(0.0),
        opts.height.unwrap_or(size.height).max(0.0),
    )
}

fn item_content(item: &LayoutItem, measured: &Measured) -> Size {
    let pref = match &item.elem {
        Elem::Widget(widget) => measured
            .widgets
            .get(&widget.key)
            .copied()
            .unwrap_or_else(Size::zero),
        Elem::Panel(panel) => measured
            .panels
            .get(&panel.uid)
            .copied()
            .unwrap_or_else(Size::zero),
    };
    content_size(pref, &item.opts)
}

fn track_sizes(tracks: &[Track], auto: &[f64]) -> Vec<f64> {
    tracks
        .iter()
        .enumerate()
        .map(|(index, track)| match track {
            Track::Fixed(v) => *v,
            Track::Auto => auto.get(index).copied().unwrap_or(0.0),
            Track::Weight(_) => 0.0,
        })
        .collect()
}

/// Track sizes used while measuring a container's *preferred* size.
///
/// Weighted tracks take their content size here; during arrangement they take
/// a share of the available space instead.
fn measure_track_sizes(tracks: &[Track], auto: &[f64]) -> Vec<f64> {
    tracks
        .iter()
        .enumerate()
        .map(|(index, track)| match track {
            Track::Fixed(v) => *v,
            Track::Auto | Track::Weight(_) => auto.get(index).copied().unwrap_or(0.0),
        })
        .collect()
}

fn resolve_tracks(tracks: &[Track], auto: &[f64], available: f64, gap: f64) -> Vec<f64> {
    let mut sizes = track_sizes(tracks, auto);
    if sizes.is_empty() {
        return sizes;
    }
    let used = sizes.iter().sum::<f64>() + gap * (sizes.len() - 1) as f64;
    let leftover = (available - used).max(0.0);
    let total_weight: f64 = tracks
        .iter()
        .filter_map(|track| match track {
            Track::Weight(w) => Some(*w),
            _ => None,
        })
        .sum();
    if total_weight > 0.0 {
        for (index, track) in tracks.iter().enumerate() {
            if let Track::Weight(w) = track {
                sizes[index] = leftover * w / total_weight;
            }
        }
    }
    sizes
}

fn grow_auto(sizes: &mut [f64], tracks: &[Track], start: usize, span: usize, missing: f64) {
    if missing <= 0.0 {
        return;
    }
    let autos: Vec<usize> = (start..start + span)
        .filter(|&index| !matches!(tracks[index], Track::Fixed(_)))
        .collect();
    if autos.is_empty() {
        return;
    }
    let add = missing / autos.len() as f64;
    for index in autos {
        sizes[index] += add;
    }
}

fn span_sum(sizes: &[f64], start: usize, span: usize) -> f64 {
    sizes[start..(start + span).min(sizes.len())].iter().sum()
}

fn offsets(sizes: &[f64], gap: f64) -> Vec<f64> {
    let mut offsets = Vec::with_capacity(sizes.len() + 1);
    let mut acc = 0.0;
    for size in sizes {
        offsets.push(acc);
        acc += size + gap;
    }
    offsets.push(acc);
    offsets
}

#[derive(Debug, Clone, Copy)]
struct Span {
    row: usize,
    col: usize,
    row_span: usize,
    col_span: usize,
}

fn span_of(opts: &ItemOpts, rows: usize, cols: usize) -> Result<Span, AppError> {
    let row_span = opts.row_span.max(1);
    let col_span = opts.col_span.max(1);
    if opts.row >= rows || opts.col >= cols {
        return Err(AppError::layout(format!(
            "grid item at ({}, {}) is outside the {}x{} grid",
            opts.row, opts.col, rows, cols
        )));
    }
    if opts.row + row_span > rows || opts.col + col_span > cols {
        return Err(AppError::layout(format!(
            "grid item at ({}, {}) spanning {}x{} exceeds the {}x{} grid",
            opts.row, opts.col, row_span, col_span, rows, cols
        )));
    }
    Ok(Span {
        row: opts.row,
        col: opts.col,
        row_span,
        col_span,
    })
}

fn inset(rect: Rect, padding: Edges) -> Rect {
    Rect::new(
        Point::new(rect.origin.x + padding.left, rect.origin.y + padding.top),
        Size::new(
            (rect.size.width - padding.horizontal()).max(0.0),
            (rect.size.height - padding.vertical()).max(0.0),
        ),
    )
}

/// Check a tree for structural errors before the window is created.
pub fn validate(root: &LayoutSpec) -> Result<(), AppError> {
    let mut visiting = HashSet::new();
    validate_node(root, &mut visiting)
}

fn validate_node(node: &LayoutSpec, visiting: &mut HashSet<u64>) -> Result<(), AppError> {
    if !visiting.insert(node.uid) {
        return Err(AppError::layout(format!(
            "layout cycle at container {}",
            node.uid
        )));
    }
    let items = node.items.lock().unwrap();
    for item in items.iter() {
        if let LayoutKind::Grid { columns, rows, .. } = &node.kind {
            span_of(&item.opts, rows.len(), columns.len())?;
        }
        if let Elem::Panel(panel) = &item.elem {
            validate_node(panel, visiting)?;
        }
    }
    visiting.remove(&node.uid);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use crate::model::{LayoutSpec, WidgetKind, WidgetNode};

    fn widget(id: u64) -> Arc<WidgetNode> {
        Arc::new(WidgetNode::new(
            id,
            format!("w{id}"),
            WidgetKind::Label,
            String::new(),
        ))
    }

    fn measured_from(sizes: &[(NodeKey, Size)]) -> Measured {
        Measured {
            widgets: sizes.iter().copied().collect(),
            panels: HashMap::new(),
        }
    }

    #[test]
    fn stack_stretches_cross_axis() {
        let root = LayoutSpec::stack(1, Orient::Vertical, 10.0, Edges::all(5.0));
        root.push(Elem::Widget(widget(1)), ItemOpts::default());
        root.push(Elem::Widget(widget(2)), ItemOpts::default());

        let sizes = measured_from(&[(1, Size::new(100.0, 20.0)), (2, Size::new(100.0, 20.0))]);
        let rects = arrange(
            &root,
            Rect::new(Point::zero(), Size::new(300.0, 200.0)),
            &sizes,
        )
        .unwrap();

        assert_eq!(
            rects[0].1,
            Rect::new(Point::new(5.0, 5.0), Size::new(290.0, 20.0))
        );
        assert_eq!(
            rects[1].1,
            Rect::new(Point::new(5.0, 35.0), Size::new(290.0, 20.0))
        );
    }

    #[test]
    fn stack_grow_shares_leftover() {
        let root = LayoutSpec::stack(1, Orient::Vertical, 0.0, Edges::default());
        let mut grow = ItemOpts::default();
        grow.grow = true;
        root.push(Elem::Widget(widget(1)), ItemOpts::default());
        root.push(Elem::Widget(widget(2)), grow);

        let sizes = measured_from(&[(1, Size::new(10.0, 20.0)), (2, Size::new(10.0, 20.0))]);
        let rects = arrange(
            &root,
            Rect::new(Point::zero(), Size::new(100.0, 100.0)),
            &sizes,
        )
        .unwrap();

        assert_eq!(rects[0].1.size.height, 20.0);
        assert_eq!(rects[1].1.size.height, 80.0);
        assert_eq!(rects[1].1.origin.y, 20.0);
    }

    #[test]
    fn grid_weights_split_width() {
        let root = LayoutSpec::grid(
            1,
            vec![Track::Weight(1.0), Track::Weight(2.0)],
            vec![Track::Weight(1.0)],
            0.0,
            0.0,
            Edges::default(),
        );
        let mut right = ItemOpts::default();
        right.col = 1;
        root.push(Elem::Widget(widget(1)), ItemOpts::default());
        root.push(Elem::Widget(widget(2)), right);

        let sizes = measured_from(&[(1, Size::new(10.0, 10.0)), (2, Size::new(10.0, 10.0))]);
        let rects = arrange(
            &root,
            Rect::new(Point::zero(), Size::new(300.0, 50.0)),
            &sizes,
        )
        .unwrap();

        assert_eq!(rects[0].1.origin.x, 0.0);
        assert_eq!(rects[0].1.size.width, 100.0);
        assert_eq!(rects[1].1.origin.x, 100.0);
        assert_eq!(rects[1].1.size.width, 200.0);
    }

    #[test]
    fn grid_auto_row_uses_preferred_height() {
        let root = LayoutSpec::grid(
            1,
            vec![Track::Weight(1.0)],
            vec![Track::Auto, Track::Weight(1.0)],
            0.0,
            0.0,
            Edges::default(),
        );
        let mut second = ItemOpts::default();
        second.row = 1;
        root.push(Elem::Widget(widget(1)), ItemOpts::default());
        root.push(Elem::Widget(widget(2)), second);

        let sizes = measured_from(&[(1, Size::new(10.0, 30.0)), (2, Size::new(10.0, 10.0))]);
        assert_eq!(
            measure(&root, |key| Ok(sizes.widgets[&key]))
                .unwrap()
                .panels[&1],
            Size::new(10.0, 40.0)
        );

        let rects = arrange(
            &root,
            Rect::new(Point::zero(), Size::new(100.0, 100.0)),
            &sizes,
        )
        .unwrap();
        assert_eq!(rects[0].1.size.height, 30.0);
        assert_eq!(rects[1].1.origin.y, 30.0);
        assert_eq!(rects[1].1.size.height, 70.0);
    }

    #[test]
    fn nested_stack_inside_grid() {
        let inner = Arc::new(LayoutSpec::stack(
            10,
            Orient::Horizontal,
            0.0,
            Edges::default(),
        ));
        inner.push(Elem::Widget(widget(1)), ItemOpts::default());
        inner.push(Elem::Widget(widget(2)), ItemOpts::default());

        let root = LayoutSpec::grid(
            1,
            vec![Track::Weight(1.0)],
            vec![Track::Weight(1.0)],
            0.0,
            0.0,
            Edges::default(),
        );
        root.push(Elem::Panel(inner), ItemOpts::default());

        let sizes = measured_from(&[(1, Size::new(20.0, 20.0)), (2, Size::new(20.0, 20.0))]);
        let rects = arrange(
            &root,
            Rect::new(Point::zero(), Size::new(100.0, 50.0)),
            &sizes,
        )
        .unwrap();
        assert_eq!(
            rects[0].1,
            Rect::new(Point::new(0.0, 0.0), Size::new(20.0, 50.0))
        );
        assert_eq!(
            rects[1].1,
            Rect::new(Point::new(20.0, 0.0), Size::new(20.0, 50.0))
        );
    }

    #[test]
    fn grid_span_includes_gaps() {
        let root = LayoutSpec::grid(
            1,
            vec![
                Track::Fixed(100.0),
                Track::Fixed(100.0),
                Track::Fixed(100.0),
            ],
            vec![Track::Fixed(40.0)],
            10.0,
            0.0,
            Edges::default(),
        );
        let mut span = ItemOpts::default();
        span.col_span = 2;
        root.push(Elem::Widget(widget(1)), span);

        let sizes = measured_from(&[(1, Size::new(10.0, 10.0))]);
        let rects = arrange(
            &root,
            Rect::new(Point::zero(), Size::new(320.0, 40.0)),
            &sizes,
        )
        .unwrap();
        assert_eq!(rects[0].1.size.width, 210.0);
        assert_eq!(rects[0].1.origin.x, 0.0);
    }

    #[test]
    fn stack_margin_and_center_alignment() {
        let root = LayoutSpec::stack(1, Orient::Vertical, 0.0, Edges::default());
        let mut opts = ItemOpts::default();
        opts.width = Some(50.0);
        opts.halign = HAlign::Center;
        opts.margin = Edges::new(10.0, 5.0, 10.0, 5.0);
        root.push(Elem::Widget(widget(1)), opts);

        let sizes = measured_from(&[(1, Size::new(50.0, 20.0))]);
        let rects = arrange(
            &root,
            Rect::new(Point::zero(), Size::new(100.0, 50.0)),
            &sizes,
        )
        .unwrap();
        assert_eq!(rects[0].1.origin.x, 25.0);
        assert_eq!(rects[0].1.origin.y, 5.0);
        assert_eq!(rects[0].1.size, Size::new(50.0, 20.0));
    }
}
