//! Custom row-gutter widget for the commit graph. Paints lane lines,
//! diagonal branch / merge connectors, and the commit's node directly to
//! the Vello scene — replacing the previous per-cell flex layout that
//! could only produce right-angle elbows.

use std::any::TypeId;

use accesskit::{Node, Role};
use masonry::core::{
    AccessCtx, AccessEvent, BoxConstraints, ChildrenIds, EventCtx, LayoutCtx, NoAction, PaintCtx,
    PointerEvent, PropertiesMut, PropertiesRef, RegisterCtx, TextEvent, Update, UpdateCtx, Widget,
    WidgetId,
};
use masonry::kurbo::{Affine, BezPath, Circle, Line, Point, Size, Stroke};
use masonry::peniko::{Color, Fill};
use masonry::vello::Scene;
use tracing::{trace_span, Span};

use crate::graph_layout::RowLayout;

const LANE_PITCH: f64 = 14.0;
const ROW_H: f64 = 24.0;
const LANE_W: f64 = 2.0;
const NODE_D: f64 = 10.0;

/// All the data the gutter needs to paint one row. Cloned per row by the
/// view layer; cheap because RowLayout holds small Vecs of u8.
#[derive(Clone, Debug, PartialEq)]
pub struct GutterStyle {
    pub row: RowLayout,
    pub lane_count: u8,
    /// Per-column lane palette (indexed `col % lanes.len()`).
    pub lanes: Vec<Color>,
    /// Background colour painted around the node so it cleanly overlaps
    /// any vertical lines passing through.
    pub bg: Color,
}

// --- MARK: WIDGET ---

pub struct GraphGutter {
    style: GutterStyle,
}

impl GraphGutter {
    pub fn new(style: GutterStyle) -> Self {
        Self { style }
    }

    fn lane_color(&self, col: u8) -> Color {
        let lanes = &self.style.lanes;
        if lanes.is_empty() {
            Color::from_rgba8(128, 128, 128, 255)
        } else {
            lanes[(col as usize) % lanes.len()]
        }
    }

    fn lane_x(col: u8) -> f64 {
        (col as f64) * LANE_PITCH + LANE_PITCH * 0.5
    }
}

impl Widget for GraphGutter {
    type Action = NoAction;

    fn on_pointer_event(
        &mut self,
        _ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        _event: &PointerEvent,
    ) {
    }
    fn on_text_event(
        &mut self,
        _ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        _event: &TextEvent,
    ) {
    }
    fn on_access_event(
        &mut self,
        _ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        _event: &AccessEvent,
    ) {
    }
    fn update(
        &mut self,
        _ctx: &mut UpdateCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        _event: &Update,
    ) {
    }
    fn register_children(&mut self, _ctx: &mut RegisterCtx<'_>) {}
    fn property_changed(&mut self, _ctx: &mut UpdateCtx<'_>, _property_type: TypeId) {}

    /// Hover/click targeting: the gutter lives inside the row's
    /// ClickableBox, but the bare flex / sized_box wrappers in the row
    /// already accept pointer interaction. Keeping this true so the row
    /// remains hit-testable across the gutter area.
    fn accepts_pointer_interaction(&self) -> bool {
        true
    }

    fn layout(
        &mut self,
        _ctx: &mut LayoutCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        // Width is intrinsic to the lane count.
        //
        // Height: on flex's first pass we get a loose bc whose max is the
        // row's available cross-axis (often the whole panel) — claiming
        // that would make flex think the row should be enormous. So return
        // our *natural* ROW_H, letting other children (like a wrapped
        // subject) drive the row height.
        //
        // Flex's CrossAxisAlignment::Fill re-runs layout for us with a
        // tight bc (min == max == row height); we honour that so the
        // gutter stretches to fill the row and lane lines connect across
        // rows of different heights.
        let w = self.style.lane_count.max(1) as f64 * LANE_PITCH;
        let h = if bc.min().height >= bc.max().height && bc.min().height > 0.0 {
            // Tight cross-axis constraint — flex's Fill-pass.
            bc.min().height
        } else {
            ROW_H
        };
        Size::new(w, h)
    }

    fn paint(&mut self, ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, scene: &mut Scene) {
        let row = &self.style.row;
        // Row height is whatever flex assigned us (from CrossAxisAlignment::
        // Fill on the parent). The node always sits at vertical centre so
        // diagonals to/from neighbouring rows land at their centres too.
        let row_h = ctx.size().height;
        let center_y = row_h * 0.5;
        let stroke = Stroke {
            width: LANE_W,
            ..Default::default()
        };
        let own_x = Self::lane_x(row.column);
        let own_color = self.lane_color(row.column);

        // Through lanes: full vertical line top→bottom.
        for &col in &row.through {
            let x = Self::lane_x(col);
            let line = Line::new(Point::new(x, 0.0), Point::new(x, row_h));
            scene.stroke(&stroke, Affine::IDENTITY, self.lane_color(col), None, &line);
        }

        // Own lane: paint the upper half only when the lane continues from
        // a row above (i.e. *not* a branch tip), and the lower half only
        // when it continues to a row below (i.e. *not* a root commit).
        // The node is drawn on top so a full line would visually be the
        // same as upper-only + lower-only — but skipping the half lets a
        // branch tip's lane end cleanly at the dot instead of overshooting.
        if !row.lane_starts_here {
            let upper = Line::new(Point::new(own_x, 0.0), Point::new(own_x, center_y));
            scene.stroke(&stroke, Affine::IDENTITY, own_color, None, &upper);
        }
        if !row.lane_ends_here {
            let lower = Line::new(Point::new(own_x, center_y), Point::new(own_x, row_h));
            scene.stroke(&stroke, Affine::IDENTITY, own_color, None, &lower);
        }

        // Terminating lanes (a side branch converging here): a line from
        // (col_x, 0) curving / angling down to (own_x, center_y). Use a
        // short cubic so the diagonal eases into the vertical lanes
        // above and below — gitk-style.
        for &col in &row.terminating {
            if col == row.column {
                continue;
            }
            let color = self.lane_color(col);
            let from = Point::new(Self::lane_x(col), 0.0);
            let to = Point::new(own_x, center_y);
            scene.stroke(
                &stroke,
                Affine::IDENTITY,
                color,
                None,
                &diag_curve(from, to),
            );
        }

        // Extra-parent (merge spawn) lanes: line from (own_x, center_y) to
        // (col_x, row_h), continuing down on the new lane.
        for &col in &row.extra_parent_columns {
            if col == row.column {
                continue;
            }
            let color = self.lane_color(col);
            let from = Point::new(own_x, center_y);
            let to = Point::new(Self::lane_x(col), row_h);
            scene.stroke(
                &stroke,
                Affine::IDENTITY,
                color,
                None,
                &diag_curve(from, to),
            );
        }

        // Node: filled circle in the lane colour, with a bg-coloured halo
        // so it visibly overlaps any through line passing exactly through
        // (which only happens degenerately, but the halo improves overall
        // contrast against any background).
        let centre = Point::new(own_x, center_y);
        let halo = Circle::new(centre, NODE_D * 0.5 + 1.5);
        scene.fill(Fill::NonZero, Affine::IDENTITY, self.style.bg, None, &halo);
        let node = Circle::new(centre, NODE_D * 0.5);
        scene.fill(Fill::NonZero, Affine::IDENTITY, own_color, None, &node);

        // Merge dot: smaller bg-coloured inner circle.
        if row.is_merge {
            let dot = Circle::new(centre, NODE_D / 6.0);
            scene.fill(Fill::NonZero, Affine::IDENTITY, self.style.bg, None, &dot);
        }
    }

    fn accessibility_role(&self) -> Role {
        Role::Image
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        _node: &mut Node,
    ) {
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::new()
    }

    fn make_trace_span(&self, id: WidgetId) -> Span {
        trace_span!("GraphGutter", id = id.trace())
    }
}

/// Build a smooth s-curve between two points so the diagonal blends into
/// the vertical lanes at both ends rather than meeting them with a
/// visible corner.
fn diag_curve(from: Point, to: Point) -> BezPath {
    let mut p = BezPath::new();
    p.move_to(from);
    let dy = to.y - from.y;
    // Control points sit on the straight verticals at each end, half the
    // vertical span away from the endpoint — that pulls the curve tangent
    // to the lane direction, which is what gives gitk its smooth feel.
    let c1 = Point::new(from.x, from.y + dy * 0.5);
    let c2 = Point::new(to.x, to.y - dy * 0.5);
    p.curve_to(c1, c2, to);
    p
}

// --- MARK: XILEM VIEW ---

use xilem::core::{MessageContext, MessageResult, Mut, View, ViewMarker};
use xilem::{Pod, ViewCtx};

pub fn graph_gutter<State, Action>(style: GutterStyle) -> GraphGutterView<State, Action> {
    GraphGutterView {
        style,
        phantom: std::marker::PhantomData,
    }
}

#[must_use = "View values do nothing unless provided to Xilem."]
pub struct GraphGutterView<State, Action> {
    style: GutterStyle,
    phantom: std::marker::PhantomData<fn() -> (State, Action)>,
}

impl<State, Action> ViewMarker for GraphGutterView<State, Action> {}

impl<State, Action> View<State, Action, ViewCtx> for GraphGutterView<State, Action>
where
    State: 'static,
    Action: 'static,
{
    type Element = Pod<GraphGutter>;
    type ViewState = ();

    fn build(&self, ctx: &mut ViewCtx, _: &mut State) -> (Self::Element, Self::ViewState) {
        let pod = ctx.create_pod(GraphGutter::new(self.style.clone()));
        (pod, ())
    }

    fn rebuild(
        &self,
        prev: &Self,
        _: &mut Self::ViewState,
        _ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        _: &mut State,
    ) {
        if self.style != prev.style {
            element.widget.style = self.style.clone();
            element.ctx.request_layout();
            element.ctx.request_paint_only();
        }
    }

    fn teardown(
        &self,
        _: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        ctx.teardown_leaf(element);
    }

    fn message(
        &self,
        _: &mut Self::ViewState,
        _message: &mut MessageContext,
        _: Mut<'_, Self::Element>,
        _app_state: &mut State,
    ) -> MessageResult<Action> {
        MessageResult::Stale
    }
}
