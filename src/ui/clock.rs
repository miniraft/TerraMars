use super::style::*;
use chrono::Utc;
use gpui::{prelude::*, *};
use std::time::Duration;
use terramars::domain::MarsClock;

pub struct TimeToggled(pub bool);
pub struct ClockView {
    pub earth: bool,
    pub location: Option<(f64, f64)>,
}
impl EventEmitter<TimeToggled> for ClockView {}
impl ClockView {
    pub fn new(earth: bool, cx: &mut Context<Self>) -> Self {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        })
        .detach();
        Self {
            earth,
            location: None,
        }
    }
}
impl Render for ClockView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Utc::now();
        let (lon, lat) = self.location.unwrap_or((0., 0.));
        let mars = MarsClock::at(now, lon, lat);
        let title = if self.earth {
            now.format("%H:%M:%S").to_string()
        } else {
            mars.formatted_local_time()
        };
        let zone = if self.earth {
            "UTC"
        } else if self.location.is_some() {
            "LMST"
        } else {
            "AMT"
        };
        let date = if self.earth {
            now.format("%d %b %Y").to_string()
        } else {
            mars.date_label()
        };
        div()
            .id("clock")
            .relative()
            .occlude()
            .child(super::interaction::probe("clock".into()))
            .min_w(px(200.))
            .px_3()
            .py_1()
            .rounded(px(10.))
            .cursor_pointer()
            .hover(|s| s.bg(rgba(0xffffff0c)))
            .on_click(cx.listener(|this, _, _, cx| {
                this.earth = !this.earth;
                cx.emit(TimeToggled(this.earth));
                cx.notify();
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            // Departure Mono is a 7×14 pixel design with a
                            // 550-unit em. Multiples of 11 map its design pixels
                            // onto whole screen pixels, so digits keep equal
                            // weight instead of alternating thin and thick
                            // strokes. 22 px is two design pixels per unit.
                            .text_size(px(22.))
                            .line_height(px(22.))
                            .font_weight(FontWeight::NORMAL)
                            .child(title),
                    )
                    // The swap glyph comes from a fallback face, so it stays on
                    // its own 11 px line instead of perturbing the time shaping.
                    .child(
                        label("⇄")
                            .text_size(px(11.))
                            .text_color(rgb(MUTED))
                            .mt(px(2.)),
                    ),
            )
            // 11 px keeps the secondary line crisp and compact next to the
            // 22 px time.
            .child(label(format!("{zone} · {date}")).text_size(px(11.)))
    }
}
