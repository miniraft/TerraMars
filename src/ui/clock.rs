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
            format!("{}  UTC", now.format("%H:%M:%S"))
        } else {
            format!(
                "{}  {}",
                mars.formatted_local_time(),
                if self.location.is_some() {
                    "LMST"
                } else {
                    "AMT"
                }
            )
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
                    .text_size(px(18.))
                    .font_weight(FontWeight::MEDIUM)
                    .child(format!("{title}   ⇄")),
            )
            .child(label(date).text_size(px(14.)))
    }
}
