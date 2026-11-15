use gpui::{prelude::*, *};
use std::time::Instant;

pub const INK: u32 = 0xf2f3f6;
pub const MUTED: u32 = 0xb2b6c1;
pub const ORANGE: u32 = 0xff945e;
pub const ACCENT: u32 = ORANGE;
pub const TEAL: u32 = 0x8cddc9;
/// One backdrop-blur radius, used by every surface so the blur stays
/// consistent (and subtle) instead of patchy between panels, buttons
/// and the header.
pub const BLUR: Pixels = px(8.);
/// GPU backdrop filtering samples the scene behind each surface rather than
/// painting a decorative texture. Keep tints light enough to see orbital detail.
/// Input belongs to the outer panel/scroll viewport, not its decorative material.
pub fn glass() -> Div {
    div()
        .relative()
        .rounded(px(10.))
        .overflow_hidden()
        .backdrop_blur(BLUR)
        .bg(rgba(0x10151b65))
        .border_1()
        .border_color(rgba(0xffffff35))
        .shadow_md()
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
        .backdrop_blur(BLUR)
        .bg(rgba(0x10151b45))
        .font_family("Inter")
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
    text: &'static str,
    selected: bool,
    narrow: bool,
) -> Stateful<Div> {
    button(id, text)
        .px(px(if narrow { 9. } else { 14. }))
        .text_size(px(if narrow { 14. } else { 16. }))
        .min_h(px(36.))
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
        .text_color(rgb(if selected { ORANGE } else { MUTED }))
}
pub fn switch_visual(enabled: bool) -> Div {
    div()
        .w(px(40.))
        .h(px(24.))
        .rounded(px(10.))
        .flex_shrink_0()
        .px(px(3.))
        .flex()
        .items_center()
        .when(enabled, |d| d.justify_end())
        .rounded(px(10.))
        .bg(if enabled { rgb(ORANGE) } else { rgb(0x555d68) })
        .child(div().size(px(18.)).rounded(px(10.)).bg(rgb(INK)))
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
        .font_family("Inter")
        .text_size(px(15.))
        .text_color(rgb(MUTED))
        .child(text.into())
}
pub fn heading(text: impl Into<SharedString>, size: f32) -> Div {
    div()
        .min_w_0()
        .font_family("Manrope")
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(size))
        .child(text.into())
}
pub fn rule() -> Div {
    div().h(px(1.)).w_full().bg(rgba(0xffffff18)).my_2()
}
pub fn reveal(element: Div, id: impl Into<ElementId>, _: bool) -> AnyElement {
    element
        .with_animation(
            id,
            Animation::new(std::time::Duration::from_millis(180))
                .with_easing(|t| 1. - (1. - t).powi(3)),
            |e, t| e.opacity(t).top(px((1. - t) * 8.)),
        )
        .into_any_element()
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
        let offset = if overflow > 1. {
            static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
            let travel = overflow / 28.;
            let phase =
                START.get_or_init(Instant::now).elapsed().as_secs_f32() % (2. * travel + 3.);
            window.request_animation_frame();
            if phase < 1.5 {
                0.
            } else if phase < travel + 1.5 {
                (phase - 1.5) * 28.
            } else if phase < travel + 3. {
                overflow
            } else {
                overflow - (phase - travel - 3.) * 28.
            }
        } else {
            0.
        };
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
