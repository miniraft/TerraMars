//! Optional pointer-test diagnostics; inactive in ordinary builds/runs.
use gpui::{prelude::*, *};
use serde_json::{Value, json};
use std::{cell::RefCell, path::PathBuf};
#[derive(Default)]
struct Trace {
    path: Option<PathBuf>,
    state: Value,
    boxes: serde_json::Map<String, Value>,
}
thread_local! {static TRACE:RefCell<Trace>=RefCell::new(Trace::default());}
pub fn configure(path: PathBuf) {
    TRACE.with(|t| t.borrow_mut().path = Some(path));
}
pub fn enabled() -> bool {
    TRACE.with(|t| t.borrow().path.is_some())
}
pub fn capture(state: Value) {
    TRACE.with(|t| {
        let mut t = t.borrow_mut();
        if t.path.is_some() {
            t.state = state;
            t.boxes.retain(|k, _| {
                k.starts_with("map-label/")
                    || k.starts_with("landmark/")
                    || k.starts_with("danger/")
                    || k.contains("compass")
            });
        }
    });
}
pub fn clear_map() {
    TRACE.with(|t| {
        t.borrow_mut().boxes.retain(|k, _| {
            !k.starts_with("map-label/") && !k.starts_with("landmark/") && !k.starts_with("danger/")
        })
    });
}
pub fn painted(name: String, bounds: Bounds<Pixels>, extra: Value) {
    TRACE.with(|t| {
        let mut t = t.borrow_mut();
        if t.path.is_some() {
            t.boxes.insert(name, json!({"x":f32::from(bounds.origin.x),"y":f32::from(bounds.origin.y),"w":f32::from(bounds.size.width),"h":f32::from(bounds.size.height),"detail":extra}));
        }
    });
}
pub fn probe(name: String) -> AnyElement {
    if !enabled() {
        return Empty.into_any_element();
    }
    canvas(|_,_,_|{}, move |b,_,window,_| {
        TRACE.with(|t| {let mut t=t.borrow_mut();if t.path.is_some(){
            let clip=window.content_mask().bounds;
            t.boxes.insert(name,json!({"x":f32::from(b.origin.x),"y":f32::from(b.origin.y),"w":f32::from(b.size.width),"h":f32::from(b.size.height),"clip":{"x":f32::from(clip.origin.x),"y":f32::from(clip.origin.y),"w":f32::from(clip.size.width),"h":f32::from(clip.size.height)}}));
        }});
    }).absolute().top_0().left_0().size_full().into_any_element()
}
pub fn flush() -> AnyElement {
    canvas(
        |_, _, _| {},
        |_, _, _, _| {
            TRACE.with(|t| {
                let t = t.borrow();
                if let Some(p) = &t.path {
                    let payload = json!({"state":t.state,"boxes":t.boxes});
                    let temp = p.with_extension("tmp");
                    if std::fs::write(&temp, payload.to_string()).is_ok() {
                        let _ = std::fs::rename(temp, p);
                    }
                }
            });
        },
    )
    .absolute()
    .size(px(1.))
    .into_any_element()
}
