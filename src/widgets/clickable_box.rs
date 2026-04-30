//! A clickable wrapper around any child widget.
//!
//! Unlike `FlatButton` (which only wraps a `Label`), `ClickableBox` accepts an
//! arbitrary child via type-erased `WidgetPod<dyn Widget>` and forwards the
//! pressed pointer button + window-relative position to its callback.
//!
//! Used by the commit-row click target — left click selects, right click
//! opens the context menu at the pointer position.

use std::any::TypeId;

use accesskit::{Node, Role};
use masonry::core::{
    AccessCtx, AccessEvent, BoxConstraints, ChildrenIds, EventCtx, FromDynWidget, LayoutCtx,
    NewWidget, PaintCtx, PointerButton, PointerButtonEvent, PointerEvent, PropertiesMut,
    PropertiesRef, RegisterCtx, TextEvent, Update, UpdateCtx, Widget, WidgetId, WidgetMut,
    WidgetPod,
};
use masonry::kurbo::{Affine, Point, RoundedRect, Size};
use masonry::peniko::{Color, Fill};
use masonry::vello::Scene;
use tracing::{trace_span, Span};

// --- MARK: STYLE ---

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct ClickStyle {
    pub idle_bg: Option<Color>,
    pub hover_bg: Option<Color>,
    pub selected_bg: Option<Color>,
    pub radius: f64,
}

/// Carries the button + window-relative position from a click. Submitted as
/// the widget's `Action` so the View layer can forward it to the user
/// callback.
#[derive(Clone, Debug)]
pub struct ClickInfo {
    pub button: Option<PointerButton>,
    pub x: f64,
    pub y: f64,
}

// --- MARK: WIDGET ---

pub struct ClickableBox {
    child: WidgetPod<dyn Widget>,
    style: ClickStyle,
    selected: bool,
}

impl ClickableBox {
    pub fn new_pod(
        child: NewWidget<impl Widget + ?Sized>,
        style: ClickStyle,
        selected: bool,
    ) -> Self {
        Self {
            child: child.erased().to_pod(),
            style,
            selected,
        }
    }

    pub fn child_mut<'t>(this: &'t mut WidgetMut<'_, Self>) -> WidgetMut<'t, dyn Widget> {
        this.ctx.get_mut(&mut this.widget.child)
    }

    pub fn set_style(this: &mut WidgetMut<'_, Self>, style: ClickStyle) {
        this.widget.style = style;
        this.ctx.request_paint_only();
    }

    pub fn set_selected(this: &mut WidgetMut<'_, Self>, selected: bool) {
        if this.widget.selected != selected {
            this.widget.selected = selected;
            this.ctx.request_paint_only();
        }
    }
}

impl Widget for ClickableBox {
    type Action = ClickInfo;

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        match event {
            PointerEvent::Down(..) => {
                if !ctx.is_disabled() {
                    ctx.capture_pointer();
                    // Stop the Down from bubbling to ancestor click targets;
                    // otherwise an outer ClickableBox would steal the capture
                    // and our Up callback would never fire. Critical for the
                    // context-menu pattern (menu items inside a backdrop).
                    ctx.set_handled();
                    ctx.request_paint_only();
                }
            }
            PointerEvent::Up(PointerButtonEvent { button, state, .. }) => {
                // Don't use ctx.has_hovered() / is_hovered(): when the
                // pointer is captured, masonry's hover semantics say "either
                // the capture target itself is hovered, or nothing is".
                // Because our child widgets (sized_box, flex) accept pointer
                // interaction, find_widget_under_pointer never returns the
                // ClickableBox itself — so neither flag is ever true during
                // capture, and every click would be silently dropped.
                //
                // Instead, check the release position against our local
                // bounds: if the user released inside the widget, fire.
                if ctx.is_pointer_capture_target() && !ctx.is_disabled() {
                    let local_pt = ctx.local_position(state.position);
                    let in_bounds = ctx.size().to_rect().contains(local_pt);
                    if in_bounds {
                        let window_pt = ctx.to_window(local_pt);
                        let info = ClickInfo {
                            button: *button,
                            x: window_pt.x,
                            y: window_pt.y,
                        };
                        ctx.submit_action::<Self::Action>(info);
                    }
                }
                ctx.request_paint_only();
            }
            _ => {}
        }
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

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        // ChildHoveredChanged is essential — paint() reads has_hovered() so we
        // need to repaint when a descendant's hover state flips.
        if matches!(
            event,
            Update::HoveredChanged(_)
                | Update::ChildHoveredChanged(_)
                | Update::FocusChanged(_)
                | Update::DisabledChanged(_)
        ) {
            ctx.request_paint_only();
        }
    }

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        ctx.register_child(&mut self.child);
    }

    fn property_changed(&mut self, _ctx: &mut UpdateCtx<'_>, _property_type: TypeId) {}

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        let size = ctx.run_layout(&mut self.child, bc);
        ctx.place_child(&mut self.child, Point::ZERO);
        size
    }

    fn paint(&mut self, ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, scene: &mut Scene) {
        // has_hovered: descendants that don't act on the pointer are still
        // "in the row" for hover-feedback purposes.
        let is_hovered = ctx.has_hovered();
        let size = ctx.size();
        let rect = RoundedRect::new(0.0, 0.0, size.width, size.height, self.style.radius);

        // Layer 1 — base background (idle, optionally swapped for hover).
        let base = if is_hovered {
            self.style.hover_bg.or(self.style.idle_bg)
        } else {
            self.style.idle_bg
        };
        if let Some(color) = base {
            scene.fill(Fill::NonZero, Affine::IDENTITY, color, None, &rect);
        }

        // Layer 2 — selected overlay painted *on top* so callers can use a
        // translucent tint that preserves any base color (e.g. the working
        // tree's "dirty" green).
        if self.selected {
            if let Some(color) = self.style.selected_bg {
                scene.fill(Fill::NonZero, Affine::IDENTITY, color, None, &rect);
            }
        }
    }

    fn accessibility_role(&self) -> Role {
        Role::Button
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        _node: &mut Node,
    ) {
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::from_slice(&[self.child.id()])
    }

    fn make_trace_span(&self, id: WidgetId) -> Span {
        trace_span!("ClickableBox", id = id.trace())
    }
}

// Sanity: FromDynWidget is auto-impl'd for any T: Widget — keeps Pod<ClickableBox> happy.
const _: fn() = || {
    fn assert_from_dyn<T: FromDynWidget>() {}
    assert_from_dyn::<ClickableBox>();
};

// --- MARK: XILEM VIEW ---

use std::marker::PhantomData;
use xilem::core::{MessageContext, MessageResult, Mut, View, ViewId, ViewMarker, ViewPathTracker};
use xilem::{Pod, ViewCtx, WidgetView};

const CHILD_VIEW_ID: ViewId = ViewId::new(0);

/// Wrap any view in a clickable container that fires `callback(state, info)` on
/// pointer release. `info.button` distinguishes Primary/Secondary; `info.x/y`
/// are window coords useful for placing context menus.
pub fn clickable_box<State, Action, V, F>(
    inner: V,
    style: ClickStyle,
    selected: bool,
    callback: F,
) -> ClickableBoxView<V, F, State, Action>
where
    V: WidgetView<State, Action>,
    F: Fn(&mut State, ClickInfo) -> Action + Send + Sync + 'static,
{
    ClickableBoxView {
        inner,
        style,
        selected,
        callback,
        phantom: PhantomData,
    }
}

#[must_use = "View values do nothing unless provided to Xilem."]
pub struct ClickableBoxView<V, F, State, Action> {
    inner: V,
    style: ClickStyle,
    selected: bool,
    callback: F,
    phantom: PhantomData<fn() -> (State, Action)>,
}

impl<V, F, State, Action> ViewMarker for ClickableBoxView<V, F, State, Action> {}

impl<V, F, State, Action> View<State, Action, ViewCtx> for ClickableBoxView<V, F, State, Action>
where
    V: WidgetView<State, Action>,
    F: Fn(&mut State, ClickInfo) -> Action + Send + Sync + 'static,
    State: 'static,
    Action: 'static,
{
    type Element = Pod<ClickableBox>;
    type ViewState = V::ViewState;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let (child, child_state) =
            ctx.with_id(CHILD_VIEW_ID, |ctx| self.inner.build(ctx, app_state));
        let pod = ctx.with_action_widget(|ctx| {
            ctx.create_pod(ClickableBox::new_pod(
                child.new_widget,
                self.style,
                self.selected,
            ))
        });
        (pod, child_state)
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        if self.style != prev.style {
            ClickableBox::set_style(&mut element, self.style);
        }
        if self.selected != prev.selected {
            ClickableBox::set_selected(&mut element, self.selected);
        }
        ctx.with_id(CHILD_VIEW_ID, |ctx| {
            let mut child = ClickableBox::child_mut(&mut element);
            self.inner
                .rebuild(&prev.inner, view_state, ctx, child.downcast(), app_state);
        });
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
    ) {
        ctx.with_id(CHILD_VIEW_ID, |ctx| {
            let mut child = ClickableBox::child_mut(&mut element);
            self.inner.teardown(view_state, ctx, child.downcast());
        });
        ctx.teardown_leaf(element);
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        message: &mut MessageContext,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        match message.take_first() {
            Some(CHILD_VIEW_ID) => {
                let mut child = ClickableBox::child_mut(&mut element);
                self.inner
                    .message(view_state, message, child.downcast(), app_state)
            }
            None => match message.take_message::<ClickInfo>() {
                Some(info) => MessageResult::Action((self.callback)(app_state, *info)),
                None => {
                    tracing::error!("Wrong message type in ClickableBox::message: {message:?}");
                    MessageResult::Stale
                }
            },
            _ => {
                tracing::warn!("Got unexpected id path in ClickableBox::message");
                MessageResult::Stale
            }
        }
    }
}
