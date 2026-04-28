//! A flow / wrap layout — arranges children left-to-right and breaks
//! to a new line when the next child wouldn't fit. Used by the commit
//! row to keep ref chips inside their fixed-width column instead of
//! pushing the rest of the row when a branch name is long.
//!
//! Each child is laid out with the container's max width as its own
//! max width, so a single chip wider than the column is truncated by
//! the child itself (e.g. a `LineBreaking::Clip` label) rather than
//! overflowing the row.

use std::any::TypeId;

use accesskit::{Node, Role};
use masonry::core::{
    AccessCtx, AccessEvent, BoxConstraints, EventCtx, FromDynWidget, LayoutCtx, PaintCtx,
    PointerEvent, PropertiesMut, PropertiesRef, QueryCtx, RegisterCtx, TextEvent, Update,
    UpdateCtx, Widget, WidgetId, WidgetPod,
};
use masonry::kurbo::{Point, Size};
use masonry::vello::Scene;
use smallvec::SmallVec;
use tracing::{Span, trace_span};

pub struct Flow {
    children: Vec<WidgetPod<dyn Widget>>,
    h_gap: f64,
    v_gap: f64,
}

impl Flow {
    pub fn new(children: Vec<WidgetPod<dyn Widget>>, h_gap: f64, v_gap: f64) -> Self {
        Self { children, h_gap, v_gap }
    }
}

impl Widget for Flow {
    fn on_pointer_event(
        &mut self,
        _ctx: &mut EventCtx,
        _props: &mut PropertiesMut<'_>,
        _event: &PointerEvent,
    ) {
    }
    fn on_text_event(
        &mut self,
        _ctx: &mut EventCtx,
        _props: &mut PropertiesMut<'_>,
        _event: &TextEvent,
    ) {
    }
    fn on_access_event(
        &mut self,
        _ctx: &mut EventCtx,
        _props: &mut PropertiesMut<'_>,
        _event: &AccessEvent,
    ) {
    }
    fn update(
        &mut self,
        _ctx: &mut UpdateCtx,
        _props: &mut PropertiesMut<'_>,
        _event: &Update,
    ) {
    }

    fn register_children(&mut self, ctx: &mut RegisterCtx) {
        for child in &mut self.children {
            ctx.register_child(child);
        }
    }

    fn property_changed(&mut self, _ctx: &mut UpdateCtx, _property_type: TypeId) {}

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        let max_w = bc.max().width;
        // Children get the column width as their max — a child wider than
        // that is responsible for truncating itself.
        let child_bc = BoxConstraints::new(
            Size::ZERO,
            Size::new(max_w, f64::INFINITY),
        );

        // First pass: measure every child.
        let mut sizes: Vec<Size> = Vec::with_capacity(self.children.len());
        for child in &mut self.children {
            let sz = ctx.run_layout(child, &child_bc);
            sizes.push(sz);
        }

        // Second pass: pack into lines.
        let mut x: f64 = 0.0;
        let mut y: f64 = 0.0;
        let mut line_h: f64 = 0.0;
        let mut max_line_w: f64 = 0.0;
        for (i, child) in self.children.iter_mut().enumerate() {
            let sz = sizes[i];
            // Wrap if this child wouldn't fit on the current line — but
            // never wrap when we're at the start of a line (single child
            // wider than the column just overflows / clips).
            if x > 0.0 && x + sz.width > max_w {
                y += line_h + self.v_gap;
                x = 0.0;
                line_h = 0.0;
            }
            ctx.place_child(child, Point::new(x, y));
            x += sz.width + self.h_gap;
            line_h = line_h.max(sz.height);
            max_line_w = max_line_w.max(x - self.h_gap);
        }

        let total_h = if self.children.is_empty() { 0.0 } else { y + line_h };
        // Always claim the full bc.max().width when finite. Two cases
        // we care about:
        //   1. Tight bc (sized_box.width(N) wrapper): max == min == N.
        //      We must return N so neighbouring columns line up.
        //   2. Loose bc with a finite max (a flex(1.0) allocation in a
        //      flex row): max == the slot the parent gave us. If we
        //      returned our content's natural width here, flex would
        //      advance by the smaller value and the next column would
        //      slide left as the window widens — exactly what was
        //      pulling the author column toward the subject text.
        // Only fall back to natural width when bc.max is unbounded.
        let w = if max_w.is_finite() {
            max_w.max(bc.min().width)
        } else {
            max_line_w.max(bc.min().width)
        };
        Size::new(w, total_h)
    }

    fn paint(&mut self, _ctx: &mut PaintCtx, _props: &PropertiesRef<'_>, _scene: &mut Scene) {}

    fn accessibility_role(&self) -> Role {
        Role::GenericContainer
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx,
        _props: &PropertiesRef<'_>,
        _node: &mut Node,
    ) {
    }

    fn children_ids(&self) -> SmallVec<[WidgetId; 16]> {
        self.children.iter().map(|c| c.id()).collect()
    }

    fn make_trace_span(&self, ctx: &QueryCtx<'_>) -> Span {
        trace_span!("Flow", id = ctx.widget_id().trace())
    }
}

const _: fn() = || {
    fn assert_from_dyn<T: FromDynWidget>() {}
    assert_from_dyn::<Flow>();
};

// --- MARK: XILEM VIEW ---

use xilem::core::{DynMessage, Mut, MessageResult, View, ViewId, ViewMarker, ViewPathTracker};
use xilem::{AnyWidgetView, Pod, ViewCtx};

/// A flow/wrap layout view. Pass child views as boxed `AnyWidgetView`s
/// (typically chips) plus horizontal and vertical gaps.
pub fn flow<State, Action>(
    children: Vec<Box<AnyWidgetView<State, Action>>>,
    h_gap: f64,
    v_gap: f64,
) -> FlowView<State, Action>
where
    State: 'static,
    Action: 'static,
{
    FlowView { children, h_gap, v_gap }
}

#[must_use = "View values do nothing unless provided to Xilem."]
pub struct FlowView<State, Action> {
    children: Vec<Box<AnyWidgetView<State, Action>>>,
    h_gap: f64,
    v_gap: f64,
}

impl<State, Action> ViewMarker for FlowView<State, Action> {}

impl<State, Action> View<State, Action, ViewCtx> for FlowView<State, Action>
where
    State: 'static,
    Action: 'static,
{
    type Element = Pod<Flow>;
    type ViewState = Vec<<Box<AnyWidgetView<State, Action>> as View<State, Action, ViewCtx>>::ViewState>;

    fn build(&self, ctx: &mut ViewCtx) -> (Self::Element, Self::ViewState) {
        let mut child_pods: Vec<WidgetPod<dyn Widget>> = Vec::with_capacity(self.children.len());
        let mut child_states: Self::ViewState = Vec::with_capacity(self.children.len());
        for (i, child) in self.children.iter().enumerate() {
            let id = ViewId::new(i as u64);
            let (pod, state) = ctx.with_id(id, |ctx| child.build(ctx));
            child_pods.push(pod.erased_widget_pod());
            child_states.push(state);
        }
        let pod = ctx.new_pod(Flow::new(child_pods, self.h_gap, self.v_gap));
        (pod, child_states)
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<Self::Element>,
    ) {
        let new_len = self.children.len();
        let prev_len = prev.children.len();
        let common = new_len.min(prev_len);

        // Rebuild children that exist in both prev and current.
        for i in 0..common {
            let id = ViewId::new(i as u64);
            ctx.with_id(id, |ctx| {
                let mut child = element.ctx.get_mut(&mut element.widget.children[i]);
                self.children[i].rebuild(
                    &prev.children[i],
                    &mut view_state[i],
                    ctx,
                    child.downcast(),
                );
            });
        }

        // Tear down trailing children that were removed. Walk backward so
        // index shifts don't matter.
        if prev_len > new_len {
            for i in (new_len..prev_len).rev() {
                let id = ViewId::new(i as u64);
                ctx.with_id(id, |ctx| {
                    let mut child_mut = element.ctx.get_mut(&mut element.widget.children[i]);
                    prev.children[i].teardown(&mut view_state[i], ctx, child_mut.downcast());
                });
                let pod = element.widget.children.remove(i);
                element.ctx.remove_child(pod);
                view_state.remove(i);
            }
            element.ctx.request_layout();
        }

        // Build any new trailing children.
        if new_len > prev_len {
            for i in prev_len..new_len {
                let id = ViewId::new(i as u64);
                let (pod, state) = ctx.with_id(id, |ctx| self.children[i].build(ctx));
                element.widget.children.push(pod.erased_widget_pod());
                view_state.push(state);
            }
            element.ctx.children_changed();
            element.ctx.request_layout();
        }

        if self.h_gap != prev.h_gap || self.v_gap != prev.v_gap {
            element.widget.h_gap = self.h_gap;
            element.widget.v_gap = self.v_gap;
            element.ctx.request_layout();
        }
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<Self::Element>,
    ) {
        for (i, child) in self.children.iter().enumerate() {
            let id = ViewId::new(i as u64);
            ctx.with_id(id, |ctx| {
                let mut child_mut = element.ctx.get_mut(&mut element.widget.children[i]);
                child.teardown(&mut view_state[i], ctx, child_mut.downcast());
            });
        }
        ctx.teardown_leaf(element);
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        id_path: &[ViewId],
        message: DynMessage,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        let (head, rest) = match id_path.split_first() {
            Some(p) => p,
            None => return MessageResult::Stale(message),
        };
        let idx = head.routing_id() as usize;
        if idx >= self.children.len() {
            return MessageResult::Stale(message);
        }
        self.children[idx].message(&mut view_state[idx], rest, message, app_state)
    }
}
