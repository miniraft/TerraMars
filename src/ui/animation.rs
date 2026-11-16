//! Shared fade transitions for every surface that enters or leaves the UI.
//!
//! GPUI rebuilds the element tree each frame, so an element that is no longer
//! rendered cannot animate away by itself. [`Fade`] owns the requested
//! visibility plus a short "still present" window: when a surface is hidden it
//! keeps rendering for one fade-out and then drops out of the tree. Panels,
//! popovers, modals, toasts and expandable sections all use the same timing so
//! the interface feels like one system instead of a patchwork of durations.

use gpui::{prelude::*, *};
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

/// How long a surface takes to become fully visible.
pub const FADE_IN: Duration = Duration::from_millis(240);
/// How long a surface takes to become fully transparent.
pub const FADE_OUT: Duration = Duration::from_millis(220);
/// Height collapse/expand for expandable cards such as the route planner.
pub const COLLAPSE: Duration = Duration::from_millis(260);
/// Presence lasts well past every animation so a surface never disappears
/// mid-transition, even if a frame is late.
const PRESENCE: Duration = Duration::from_millis(322);

/// The single easing used by every transition in the app: quick movement
/// first, then a gentle settle. Applied to both entering and leaving surfaces.
pub fn ease_out(t: f32) -> f32 {
    1. - (1. - t).powi(3)
}

/// Symmetric easing for size changes. Unlike [`ease_out`] it does not jump a
/// large distance on the first frame, which reads as a stutter on a tall card.
pub fn ease_in_out(t: f32) -> f32 {
    if t < 0.5 {
        4. * t * t * t
    } else {
        1. - (-2. * t + 2.).powi(3) / 2.
    }
}

struct FadeState {
    /// The requested visibility, independent of the fade currently playing.
    visible: bool,
    /// Bumped on every change so `with_animation` restarts from zero.
    generation: u64,
    /// While set (and in the future) a hidden surface still renders.
    hide_at: Option<Instant>,
}

/// Tracks whether a surface should be in the tree and whether it is entering
/// or leaving. Cheap to clone through shared state, and safe to keep on the
/// view because all mutation goes through a `RefCell`.
#[derive(Clone)]
pub struct Fade {
    state: Rc<RefCell<FadeState>>,
}

impl Fade {
    pub fn new(visible: bool) -> Self {
        Self {
            state: Rc::new(RefCell::new(FadeState {
                visible,
                generation: 0,
                hide_at: None,
            })),
        }
    }

    /// The requested visibility (ignores an in-flight fade-out).
    pub fn visible(&self) -> bool {
        self.state.borrow().visible
    }

    /// True while the surface should be rendered: visible, or still playing
    /// its fade-out.
    pub fn present(&self) -> bool {
        let state = self.state.borrow();
        state.visible
            || state
                .hide_at
                .is_some_and(|deadline| Instant::now() < deadline)
    }

    pub fn generation(&self) -> u64 {
        self.state.borrow().generation
    }

    /// Restarts the enter animation without changing visibility. Used when the
    /// same container swaps its contents (for example details tabs).
    pub fn restart(&self) {
        self.state.borrow_mut().generation += 1;
    }

    /// Requests a new visibility. Call this on every render with the current
    /// logical state: unchanged values are free, changes start a fade and, for
    /// a hide, schedule the repaint that removes the surface once the fade is
    /// finished.
    pub fn sync<T: 'static>(&self, visible: bool, cx: &Context<T>) {
        let hiding = {
            let mut state = self.state.borrow_mut();
            // Only transitions matter. Once hidden, later syncs must leave the
            // fade-out and its deadline untouched or it would restart forever.
            if state.visible == visible {
                return;
            }
            state.visible = visible;
            state.generation += 1;
            if visible {
                state.hide_at = None;
                false
            } else {
                state.hide_at = Some(Instant::now() + PRESENCE);
                true
            }
        };
        if hiding {
            // The animation requests frames while it runs. This timer only
            // guarantees one more repaint after the presence window, which is
            // what actually drops the element from the tree.
            let entity = cx.entity();
            cx.spawn(async move |_, cx| {
                cx.background_executor()
                    .timer(PRESENCE + Duration::from_millis(16))
                    .await;
                entity.update(cx, |_, cx| cx.notify());
            })
            .detach();
        }
    }

    /// Wraps an element in the fade matching its current direction.
    ///
    /// A surface that is leaving the tree becomes click-through for the length of
    /// its fade, so the controls it still covers accept input immediately instead
    /// of waiting for the panel to finish disappearing.
    pub fn element<E>(&self, name: &'static str, element: E) -> AnyElement
    where
        E: FadeSurface + IntoElement + Styled + 'static,
    {
        let visible = self.visible();
        let id = ElementId::named_usize(name, self.generation() as usize);
        let animation = if visible {
            Animation::new(FADE_IN).with_easing(ease_out)
        } else {
            Animation::new(FADE_OUT).with_easing(ease_out)
        };
        let element = if visible {
            element
        } else {
            element.click_through()
        };
        element
            .with_animation(id, animation, move |element, t| {
                let opacity = if visible { t } else { 1. - t };
                element.opacity(opacity)
            })
            .into_any_element()
    }

    /// Fades an element in whenever `generation` changes. Used for containers
    /// that stay mounted but replace their contents.
    pub fn animate_in<E>(&self, name: &'static str, element: E) -> AnyElement
    where
        E: IntoElement + Styled + 'static,
    {
        fade_in(name, element, self.generation())
    }
}

/// Surfaces that can stop accepting pointer input while they fade away.
///
/// The panels that fade out are divs, so they can drop their hitboxes without
/// losing their painted appearance. That keeps input responsive during the
/// ~200 ms a closing surface is still on screen.
pub trait FadeSurface: Sized {
    /// Makes this surface and its descendants click-through but still painted.
    fn click_through(self) -> Self;
}

impl FadeSurface for Div {
    fn click_through(self) -> Self {
        self.pointer_events_none()
    }
}

impl FadeSurface for Stateful<Div> {
    fn click_through(self) -> Self {
        self.pointer_events_none()
    }
}

/// One-shot fade-in for content that is rebuilt with a new identity, such as a
/// page body after navigation.
pub fn fade_in<E>(name: &'static str, element: E, generation: u64) -> AnyElement
where
    E: IntoElement + Styled + 'static,
{
    element
        .with_animation(
            ElementId::named_usize(name, generation as usize),
            Animation::new(FADE_IN).with_easing(ease_out),
            |element, t| element.opacity(t),
        )
        .into_any_element()
}
