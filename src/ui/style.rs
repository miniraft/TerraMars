use gpui::{prelude::*, *};

pub const INK: u32 = 0xf2f3f6;
pub const MUTED: u32 = 0xb2b6c1;
pub const ORANGE: u32 = 0xff945e;
pub const ACCENT: u32 = ORANGE;
pub const TEAL: u32 = 0x8cddc9;

// ---- Spacing scale -------------------------------------------------------
// The only gaps and insets the interface uses. See docs/DESIGN_RULES.md.
/// Standard gap between sibling controls, rows and cards.
pub const SPACE: Pixels = px(12.);
/// Gap inside one control cluster (speed stepper, icon toolbar).
pub const SPACE_TIGHT: Pixels = px(8.);
/// Gap between top-level groups (home cards, log cards).
pub const SPACE_LOOSE: Pixels = px(20.);
/// Padding inside panels and cards.
pub const INSET: Pixels = px(16.);
/// Margin between the floating header and the window edges.
pub const HEADER_INSET: f32 = 12.;
/// Height of the floating header: tall enough for the two-line clock plus
/// `HEADER_INSET` above and below, so all four gaps around its content match.
pub const HEADER_HEIGHT: f32 = 68.;
/// Top inset for content that begins below the floating header.
pub const CONTENT_TOP: Pixels = px(HEADER_INSET + HEADER_HEIGHT + HEADER_INSET);
/// One backdrop-blur radius, used by every large surface. Keep it small so
/// panels stay legible and the GPU blur pass stays cheap while scrolling.
pub const BLUR: Pixels = px(2.);
/// GPU backdrop filtering samples the scene behind each surface rather than
/// painting a decorative texture. Keep tints light enough to see orbital detail.
/// Input belongs to the outer panel/scroll viewport, not its decorative material.
pub fn glass() -> Div {
    div()
        .relative()
        .rounded(px(10.))
        .overflow_hidden()
        .backdrop_blur(BLUR)
        .bg(rgba(0x10151b9a))
        .border_1()
        .border_color(rgba(0xffffff35))
        .shadow_md()
}
/// Opaque surface for notifications and popovers. A slight burnt-orange cast
/// keeps them distinct from the map while a black keyline separates them from
/// whatever they cover. No backdrop blur: these surfaces are meant to read as
/// solid objects, and skipping the blur keeps scrolling at 60 fps.
pub fn solid_panel() -> Div {
    div()
        .relative()
        .rounded(px(10.))
        .overflow_hidden()
        .bg(rgb(0x3a1f12))
        .border_2()
        .border_color(rgb(0x000000))
}
/// Notification surface. Same shape and keyline as [`solid_panel`], but a
/// brighter orange fill so a toast reads as an alert rather than blending into
/// the dark chocolate of the popovers.
pub const NOTICE: u32 = 0xb54a10;
pub fn notice_panel() -> Div {
    solid_panel().bg(rgb(NOTICE))
}
pub fn button(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Stateful<Div> {
    let id = id.into();
    let trace = format!("{id:?}");
    let text_trace = trace.clone();
    let text = text.into();
    div()
        .id(id)
        .relative()
        .block_mouse_except_scroll()
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .gap_2()
        .min_w_0()
        .overflow_hidden()
        .px_4()
        .py_2()
        .min_h(px(44.))
        .rounded(px(10.))
        .border_1()
        .border_color(rgba(0xffffff22))
        // Buttons sit inside panels and lists; a translucent fill keeps them
        // crisp without adding another GPU backdrop-blur pass per control.
        .bg(rgba(0x10151b8a))
        .font_family("Departure Mono")
        .text_size(px(17.))
        .line_height(px(24.))
        .text_color(rgb(INK))
        .cursor_pointer()
        .hover(|s| s.bg(rgba(0xffffff18)).border_color(rgba(0xffffff50)))
        .active(|s| s.bg(rgba(0xffffff28)))
        .child(super::interaction::probe(trace))
        .when(!text.is_empty(), |b| {
            b.child(ButtonText {
                text,
                trace: text_trace,
            })
        })
}
pub fn primary(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Stateful<Div> {
    button(id, text)
        .backdrop_filter(Vec::new())
        .bg(rgb(ORANGE))
        .text_color(rgb(0x21160f))
        .font_weight(FontWeight::SEMIBOLD)
        .hover(|s| s.bg(rgb(0xffb18a)))
}
pub fn nav(id: &'static str, text: &'static str, active: bool) -> Stateful<Div> {
    div()
        .id(id)
        .relative()
        .occlude()
        .py_3()
        .px_1()
        .cursor_pointer()
        .text_size(px(17.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(if active { ORANGE } else { INK }))
        .hover(|s| s.text_color(rgb(ORANGE)))
        .child(super::interaction::probe(id.into()))
        .child(text)
}
pub fn segment(
    id: &'static str,
    glyph: &'static str,
    text: &'static str,
    selected: bool,
    narrow: bool,
) -> Stateful<Div> {
    let color = if selected { ORANGE } else { MUTED };
    let element_id: ElementId = id.into();
    let trace = format!("{element_id:?}");
    button(element_id, "")
        .px(px(if narrow { 6. } else { 8. }))
        .gap_1()
        .min_h(px(36.))
        // 11 px is the crisp design-pixel size for Departure Mono.
        .text_size(px(11.))
        .border_color(if selected {
            rgba(0xff945e70)
        } else {
            rgba(0xffffff00)
        })
        .bg(if selected {
            rgba(0xff945e24)
        } else {
            rgba(0xffffff00)
        })
        .text_color(rgb(color))
        .child(icon(glyph).size(px(15.)).text_color(rgb(color)))
        .child(ButtonText {
            text: text.into(),
            trace,
        })
}
pub fn switch_visual(id: usize, enabled: bool) -> AnyElement {
    let ease = |t: f32| 1. - (1. - t).powi(3);
    let animation_id = ElementId::named_usize("switch", id * 2 + usize::from(enabled));
    let off = rgb(0x555d68);
    let on = rgb(ORANGE);
    div()
        .relative()
        .w(px(40.))
        .h(px(24.))
        .rounded(px(10.))
        .flex_shrink_0()
        .with_animation(
            ElementId::named_usize("switch-track", id * 2 + usize::from(enabled)),
            Animation::new(std::time::Duration::from_millis(160)).with_easing(ease),
            move |track, t| {
                let position = if enabled { t } else { 1. - t };
                track.bg(off.lerp(&on, position))
            },
        )
        .child(
            div()
                .absolute()
                .top(px(3.))
                .size(px(18.))
                .rounded(px(9.))
                .bg(rgb(INK))
                .with_animation(
                    animation_id,
                    Animation::new(std::time::Duration::from_millis(160)).with_easing(ease),
                    move |knob, t| {
                        let position = if enabled { t } else { 1. - t };
                        knob.left(px(3. + position * 16.))
                    },
                ),
        )
        .into_any_element()
}
/// Square map-camera control. Zoom in, zoom out and fit share one size so the
/// toolbar reads as a single column (or row, when compact).
pub fn map_button(id: impl Into<ElementId>, content: impl IntoElement) -> Stateful<Div> {
    button(id, "").w(px(44.)).h(px(44.)).p_0().child(content)
}
pub fn icon(name: &'static str) -> Svg {
    svg()
        .path(format!("icons/{name}.svg"))
        .size(px(22.))
        .flex_shrink_0()
        .text_color(rgb(INK))
}
pub fn label(text: impl Into<SharedString>) -> Div {
    div()
        .min_w_0()
        .font_family("Departure Mono")
        .text_size(px(15.))
        .text_color(rgb(MUTED))
        .child(text.into())
}
pub fn heading(text: impl Into<SharedString>, size: f32) -> Div {
    div()
        .min_w_0()
        .font_family("Orbitron")
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(size))
        .child(text.into())
}
pub fn rule() -> Div {
    div().h(px(1.)).w_full().bg(rgba(0xffffff18)).my_2()
}

/// A single measured line for every button. Its natural size centers short
/// labels, while flex-shrink lets long labels scroll inside the available space.
/// Empty labels never create a phantom flex item beside an icon.
struct ButtonText {
    text: SharedString,
    trace: String,
}
impl IntoElement for ButtonText {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for ButtonText {
    type RequestLayoutState = ShapedLine;
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        _: &mut App,
    ) -> (LayoutId, ShapedLine) {
        let text_style = window.text_style();
        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let line = window.text_system().shape_line(
            self.text.clone(),
            font_size,
            &[TextRun {
                len: self.text.len(),
                font: text_style.font(),
                color: text_style.color,
                background_color: None,
                underline: None,
                strikethrough: None,
                letter_spacing: None,
            }],
        );
        let natural = line.width;
        let height = window.line_height();
        let style = Style {
            min_size: Size {
                width: px(0.).into(),
                ..Default::default()
            },
            flex_shrink: 1.,
            ..Default::default()
        };
        let layout = window.request_measured_layout(style, move |known, available, _, _| {
            let limit = match available.width {
                AvailableSpace::Definite(w) => w,
                _ => natural,
            };
            size(
                known.width.unwrap_or(natural.min(limit)),
                known.height.unwrap_or(height),
            )
        });
        (layout, line)
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut ShapedLine,
        _: &mut Window,
        _: &mut App,
    ) {
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        line: &mut ShapedLine,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let overflow = f32::from(line.width - bounds.size.width).max(0.);
        // Keep overflowing labels clipped at their start. A perpetual marquee
        // here would request an animation frame for every long label on every
        // frame, keeping the entire window redrawing even while idle.
        let offset = 0.;
        if super::interaction::enabled() {
            super::interaction::painted(
                format!("button-label/{}", self.trace),
                bounds,
                serde_json::json!({"text":self.text,"overflow":overflow,"offset":offset}),
            );
        }
        window.with_content_mask(
            Some(ContentMask {
                bounds,
                ..Default::default()
            }),
            |window| {
                let _ = line.paint(
                    point(bounds.origin.x - px(offset), bounds.origin.y),
                    bounds.size.height,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            },
        );
    }
}
