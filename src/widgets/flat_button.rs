//! A flat, theme-aware clickable container.
//!
//! Unlike `masonry::widgets::Button`, this widget has no built-in gradient
//! or border chrome — the background color per state (idle / hover / active)
//! is chosen by the caller. Used for toolbar buttons, tab buttons, selector
//! rows, and sidebar rows.

use std::any::TypeId;

use accesskit::{Node, Role};
use masonry::core::{
    AccessCtx, AccessEvent, Action, BoxConstraints, EventCtx, LayoutCtx, PaintCtx, PointerEvent,
    PropertiesMut, PropertiesRef, QueryCtx, RegisterCtx, TextEvent, Update, UpdateCtx, Widget,
    WidgetId, WidgetMut, WidgetPod,
};
use masonry::kurbo::{Affine, Point, RoundedRect, Size};
use masonry::peniko::{Color, Fill};
use masonry::vello::Scene;
use masonry::widgets::Label;
use smallvec::{smallvec, SmallVec};
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

// --- MARK: WIDGET ---

pub struct FlatButton {
    pub(crate) label: WidgetPod<Label>,
    pub(crate) style: FlatStyle,
    pub(crate) active: bool,
}

impl FlatButton {
    pub fn from_label_pod(label: WidgetPod<Label>, style: FlatStyle, active: bool) -> Self {
        Self {
            label,
            style,
            active,
        }
    }

    pub fn label_mut<'t>(this: &'t mut WidgetMut<'_, Self>) -> WidgetMut<'t, Label> {
        this.ctx.get_mut(&mut this.widget.label)
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
    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        match event {
            PointerEvent::Down { .. } => {
                if !ctx.is_disabled() {
                    ctx.capture_pointer();
                    // Stop bubbling so an ancestor ClickableBox (e.g. the
                    // ctx-menu backdrop) doesn't steal capture from this
                    // button.
                    ctx.set_handled();
                    ctx.request_paint_only();
                }
            }
            PointerEvent::Up { button, .. } => {
                if ctx.is_pointer_capture_target() && ctx.is_hovered() && !ctx.is_disabled() {
                    ctx.submit_action(Action::ButtonPressed(*button));
                }
                ctx.request_paint_only();
            }
            _ => {}
        }
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
        ctx: &mut EventCtx,
        _props: &mut PropertiesMut<'_>,
        event: &AccessEvent,
    ) {
        if ctx.target() == ctx.widget_id() && event.action == accesskit::Action::Click {
            ctx.submit_action(Action::ButtonPressed(None));
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx, _props: &mut PropertiesMut<'_>, event: &Update) {
        if matches!(
            event,
            Update::HoveredChanged(_) | Update::FocusChanged(_) | Update::DisabledChanged(_)
        ) {
            ctx.request_paint_only();
        }
    }

    fn register_children(&mut self, ctx: &mut RegisterCtx) {
        ctx.register_child(&mut self.label);
    }

    fn property_changed(&mut self, _ctx: &mut UpdateCtx, _property_type: TypeId) {}

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        let pad_h = self.style.padding_h;
        let pad_v = self.style.padding_v;
        let inner_bc = bc.shrink((pad_h * 2.0, pad_v * 2.0));
        let child_size = ctx.run_layout(&mut self.label, &inner_bc);

        let our_size = Size::new(
            child_size.width + pad_h * 2.0,
            child_size.height + pad_v * 2.0,
        );
        let our_size = bc.constrain(our_size);

        let baseline = ctx.child_baseline_offset(&self.label);
        ctx.place_child(&mut self.label, Point::new(pad_h, pad_v));
        ctx.set_baseline_offset(baseline + pad_v);
        our_size
    }

    fn paint(&mut self, ctx: &mut PaintCtx, _props: &PropertiesRef<'_>, scene: &mut Scene) {
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

    fn accessibility(&mut self, _ctx: &mut AccessCtx, _props: &PropertiesRef<'_>, node: &mut Node) {
        node.add_action(accesskit::Action::Click);
    }

    fn children_ids(&self) -> SmallVec<[WidgetId; 16]> {
        smallvec![self.label.id()]
    }

    fn make_trace_span(&self, ctx: &QueryCtx<'_>) -> Span {
        trace_span!("FlatButton", id = ctx.widget_id().trace())
    }
}

// --- MARK: XILEM VIEW ---

use xilem::core::{DynMessage, MessageResult, Mut, View, ViewId, ViewMarker, ViewPathTracker};
use xilem::{Pod, ViewCtx};

/// Build a flat button view. `label` may be a plain string or a pre-styled
/// `xilem::view::Label`.
pub fn flat_button<State, Action, F>(
    label: impl Into<xilem::view::Label>,
    style: FlatStyle,
    active: bool,
    callback: F,
) -> FlatButtonView<F>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    FlatButtonView {
        label: label.into(),
        style,
        active,
        callback,
        _phantom: std::marker::PhantomData,
    }
}

#[must_use = "View values do nothing unless provided to Xilem."]
pub struct FlatButtonView<F> {
    label: xilem::view::Label,
    style: FlatStyle,
    active: bool,
    callback: F,
    _phantom: std::marker::PhantomData<F>,
}

const LABEL_VIEW_ID: ViewId = ViewId::new(0);

impl<F> ViewMarker for FlatButtonView<F> {}

impl<F, State, Action> View<State, Action, ViewCtx> for FlatButtonView<F>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
    State: 'static,
    Action: 'static,
{
    type Element = Pod<FlatButton>;
    type ViewState = ();

    fn build(&self, ctx: &mut ViewCtx) -> (Self::Element, Self::ViewState) {
        let (child, ()) = ctx.with_id(LABEL_VIEW_ID, |ctx| {
            View::<State, Action, _>::build(&self.label, ctx)
        });
        ctx.with_leaf_action_widget(|ctx| {
            ctx.new_pod(FlatButton::from_label_pod(
                child.into_widget_pod(),
                self.style,
                self.active,
            ))
        })
    }

    fn rebuild(
        &self,
        prev: &Self,
        state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<Self::Element>,
    ) {
        ctx.with_id(LABEL_VIEW_ID, |ctx| {
            View::<State, Action, _>::rebuild(
                &self.label,
                &prev.label,
                state,
                ctx,
                FlatButton::label_mut(&mut element),
            );
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
        _: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<Self::Element>,
    ) {
        ctx.with_id(LABEL_VIEW_ID, |ctx| {
            View::<State, Action, _>::teardown(
                &self.label,
                &mut (),
                ctx,
                FlatButton::label_mut(&mut element),
            );
        });
        ctx.teardown_leaf(element);
    }

    fn message(
        &self,
        _: &mut Self::ViewState,
        id_path: &[ViewId],
        message: DynMessage,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        match id_path.split_first() {
            Some((&LABEL_VIEW_ID, rest)) => self.label.message(&mut (), rest, message, app_state),
            None => match message.downcast::<masonry::core::Action>() {
                Ok(action) => {
                    if matches!(*action, masonry::core::Action::ButtonPressed(_)) {
                        MessageResult::Action((self.callback)(app_state))
                    } else {
                        MessageResult::Stale(DynMessage(action))
                    }
                }
                Err(m) => MessageResult::Stale(m),
            },
            _ => MessageResult::Stale(message),
        }
    }
}
