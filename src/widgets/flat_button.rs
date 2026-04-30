//! A flat, theme-aware clickable container.
//!
//! Unlike `masonry::widgets::Button`, this widget has no built-in gradient
//! or border chrome — the background color per state (idle / hover / active)
//! is chosen by the caller. Used for toolbar buttons, tab buttons, selector
//! rows, and sidebar rows.

use std::any::TypeId;

use accesskit::{Node, Role};
use masonry::core::{
    AccessCtx, AccessEvent, BoxConstraints, ChildrenIds, EventCtx, LayoutCtx, NewWidget, PaintCtx,
    PointerButton, PointerButtonEvent, PointerEvent, PropertiesMut, PropertiesRef, RegisterCtx,
    TextEvent, Update, UpdateCtx, Widget, WidgetId, WidgetMut, WidgetPod,
};
use masonry::kurbo::{Affine, Point, RoundedRect, Size};
use masonry::peniko::{Color, Fill};
use masonry::vello::Scene;
use tracing::{trace_span, Span};

// --- MARK: STYLE ---

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlatStyle {
    pub idle_bg: Option<Color>,
    pub hover_bg: Color,
    pub active_bg: Option<Color>,
    pub radius: f64,
    pub padding_v: f64,
    pub padding_h: f64,
}

impl Default for FlatStyle {
    fn default() -> Self {
        Self {
            idle_bg: None,
            hover_bg: Color::from_rgba8(0, 0, 0, 20),
            active_bg: None,
            radius: 4.0,
            padding_v: 4.0,
            padding_h: 8.0,
        }
    }
}

/// Action submitted when the user clicks/keys the button.
///
/// `button` carries the originating mouse button (or `None` for keyboard /
/// touch activation). Currently unused by the View layer's callback — the
/// callback only fires for the primary button equivalent — but kept on the
/// action so future call sites that care can read it.
#[derive(Clone, Debug)]
pub struct FlatButtonPress {
    #[expect(dead_code, reason = "kept for future callers; see doc comment")]
    pub button: Option<PointerButton>,
}

// --- MARK: WIDGET ---

pub struct FlatButton {
    pub(crate) child: WidgetPod<dyn Widget>,
    pub(crate) style: FlatStyle,
    pub(crate) active: bool,
}

impl FlatButton {
    pub fn from_child(
        child: NewWidget<impl Widget + ?Sized>,
        style: FlatStyle,
        active: bool,
    ) -> Self {
        Self {
            child: child.erased().to_pod(),
            style,
            active,
        }
    }

    pub fn child_mut<'t>(this: &'t mut WidgetMut<'_, Self>) -> WidgetMut<'t, dyn Widget> {
        this.ctx.get_mut(&mut this.widget.child)
    }

    pub fn set_style(this: &mut WidgetMut<'_, Self>, style: FlatStyle) {
        this.widget.style = style;
        this.ctx.request_paint_only();
    }

    pub fn set_active(this: &mut WidgetMut<'_, Self>, active: bool) {
        if this.widget.active != active {
            this.widget.active = active;
            this.ctx.request_paint_only();
        }
    }
}

impl Widget for FlatButton {
    type Action = FlatButtonPress;

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        match event {
            PointerEvent::Down(..) if !ctx.is_disabled() => {
                ctx.capture_pointer();
                // Stop bubbling so an ancestor ClickableBox (e.g. the
                // ctx-menu backdrop) doesn't steal capture from this
                // button.
                ctx.set_handled();
                ctx.request_paint_only();
            }
            PointerEvent::Up(PointerButtonEvent { button, .. }) => {
                if ctx.is_pointer_capture_target() && ctx.is_hovered() && !ctx.is_disabled() {
                    ctx.submit_action::<Self::Action>(FlatButtonPress { button: *button });
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
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &AccessEvent,
    ) {
        if event.action == accesskit::Action::Click {
            ctx.submit_action::<Self::Action>(FlatButtonPress { button: None });
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        if matches!(
            event,
            Update::HoveredChanged(_) | Update::FocusChanged(_) | Update::DisabledChanged(_)
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
        let pad_h = self.style.padding_h;
        let pad_v = self.style.padding_v;
        let inner_bc = bc.shrink((pad_h * 2.0, pad_v * 2.0));
        let child_size = ctx.run_layout(&mut self.child, &inner_bc);

        let our_size = Size::new(
            child_size.width + pad_h * 2.0,
            child_size.height + pad_v * 2.0,
        );
        let our_size = bc.constrain(our_size);

        let baseline = ctx.child_baseline_offset(&self.child);
        ctx.place_child(&mut self.child, Point::new(pad_h, pad_v));
        ctx.set_baseline_offset(baseline + pad_v);
        our_size
    }

    fn paint(&mut self, ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, scene: &mut Scene) {
        let is_hovered = ctx.is_hovered();
        let size = ctx.size();

        let bg = if self.active {
            self.style.active_bg.or(self.style.idle_bg)
        } else if is_hovered {
            Some(self.style.hover_bg)
        } else {
            self.style.idle_bg
        };

        if let Some(color) = bg {
            let rect = RoundedRect::new(0.0, 0.0, size.width, size.height, self.style.radius);
            scene.fill(Fill::NonZero, Affine::IDENTITY, color, None, &rect);
        }
    }

    fn accessibility_role(&self) -> Role {
        Role::Button
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        node.add_action(accesskit::Action::Click);
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::from_slice(&[self.child.id()])
    }

    fn make_trace_span(&self, id: WidgetId) -> Span {
        trace_span!("FlatButton", id = id.trace())
    }
}

// --- MARK: XILEM VIEW ---

use xilem::core::{MessageContext, MessageResult, Mut, View, ViewId, ViewMarker, ViewPathTracker};
use xilem::{Pod, ViewCtx, WidgetView};

/// Build a flat button view. `child` is any widget view (typically a styled
/// label) that becomes the button's content.
pub fn flat_button<State, Action, V, F>(
    child: V,
    style: FlatStyle,
    active: bool,
    callback: F,
) -> FlatButtonView<V, F, State, Action>
where
    V: WidgetView<State, Action>,
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    FlatButtonView {
        child,
        style,
        active,
        callback,
        phantom: std::marker::PhantomData,
    }
}

#[must_use = "View values do nothing unless provided to Xilem."]
pub struct FlatButtonView<V, F, State, Action> {
    child: V,
    style: FlatStyle,
    active: bool,
    callback: F,
    phantom: std::marker::PhantomData<fn() -> (State, Action)>,
}

const CHILD_VIEW_ID: ViewId = ViewId::new(0);

impl<V, F, State, Action> ViewMarker for FlatButtonView<V, F, State, Action> {}

impl<V, F, State, Action> View<State, Action, ViewCtx> for FlatButtonView<V, F, State, Action>
where
    V: WidgetView<State, Action>,
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
    State: 'static,
    Action: 'static,
{
    type Element = Pod<FlatButton>;
    type ViewState = V::ViewState;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let (child, child_state) =
            ctx.with_id(CHILD_VIEW_ID, |ctx| self.child.build(ctx, app_state));
        let pod = ctx.with_action_widget(|ctx| {
            ctx.create_pod(FlatButton::from_child(
                child.new_widget,
                self.style,
                self.active,
            ))
        });
        (pod, child_state)
    }

    fn rebuild(
        &self,
        prev: &Self,
        state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        ctx.with_id(CHILD_VIEW_ID, |ctx| {
            let mut child = FlatButton::child_mut(&mut element);
            self.child
                .rebuild(&prev.child, state, ctx, child.downcast(), app_state);
        });
        if self.style != prev.style {
            FlatButton::set_style(&mut element, self.style);
        }
        if self.active != prev.active {
            FlatButton::set_active(&mut element, self.active);
        }
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
    ) {
        ctx.with_id(CHILD_VIEW_ID, |ctx| {
            let mut child = FlatButton::child_mut(&mut element);
            self.child.teardown(view_state, ctx, child.downcast());
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
                let mut child = FlatButton::child_mut(&mut element);
                self.child
                    .message(view_state, message, child.downcast(), app_state)
            }
            None => match message.take_message::<FlatButtonPress>() {
                Some(_press) => MessageResult::Action((self.callback)(app_state)),
                None => {
                    tracing::error!("Wrong message type in FlatButton::message: {message:?}");
                    MessageResult::Stale
                }
            },
            _ => {
                tracing::warn!("Got unexpected id path in FlatButton::message");
                MessageResult::Stale
            }
        }
    }
}
