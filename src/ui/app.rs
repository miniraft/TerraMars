use super::{
    assets::Images,
    clock::{ClockView, TimeToggled},
    map::{MapSelection, MapView},
    performance,
    style::*,
};
use chrono::Utc;
use gpui::{prelude::*, *};
use std::{collections::BTreeSet, sync::Arc, time::Duration};
use terramars::{
    domain::{self, *},
    services,
};

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Home,
    Explorer,
    Log,
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum Endpoint {
    Start,
    End,
}
#[derive(Clone)]
struct Location {
    point: domain::Point,
    label: String,
    landmark: Option<usize>,
}

pub struct TerraMars {
    sites: Vec<MissionSite>,
    climate: domain::climate::ClimateModel,
    images: Arc<Images>,
    store: MissionStore,
    clock: Entity<ClockView>,
    panorama: Entity<Panorama>,
    page: Page,
    browse_page: Page,
    log_preview: Option<Entity<MapView>>,
    log_preview_index: Option<usize>,
    log_danger: Option<(usize, domain::Point)>,
    site_index: usize,
    map: Option<Entity<MapView>>,
    selection: Option<Location>,
    start: Option<Location>,
    end: Option<Location>,
    details: bool,
    layers: bool,
    planner: bool,
    endpoint_menu: Option<Endpoint>,
    map_pick: Option<Endpoint>,
    coordinate_text: String,
    coordinate_editing: bool,
    expanded: BTreeSet<String>,
    expanded_observation: Option<String>,
    routes: Vec<RouteEstimate>,
    route_index: usize,
    mode: String,
    status: String,
    status_version: u64,
    status_task: Option<Task<()>>,
    refreshing: bool,
    refresh_notes: Vec<String>,
    refresh_expanded: bool,
    focus: FocusHandle,
    transition: u64,
    page_transition: u64,
    rendered_page: Page,
    planning: bool,
    planning_version: u64,
}

struct Panorama {
    image: Arc<RenderImage>,
}
impl Render for Panorama {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let _timing = performance::scope("panorama_render_callback");
        let height = f32::from(window.viewport_size().height).max(1.);
        let period = (height * (1660. / 342.)).max(f32::from(window.viewport_size().width));
        let layer = div()
            .absolute()
            .top_0()
            .h_full()
            .w(px(period * 2.))
            .flex()
            .child(img(self.image.clone()).w(px(period)).h_full())
            .child(img(self.image.clone()).w(px(period)).h_full());
        let layer = layer.with_animation(
            "panorama",
            Animation::new(Duration::from_secs(160)).repeat(),
            move |e, t| e.left(px(-period * t)),
        );
        div()
            .size_full()
            .overflow_hidden()
            .relative()
            .child(div().absolute().inset_0().opacity(0.48).child(layer))
            .child(div().absolute().size_full().bg(rgba(0x10101844)))
    }
}

impl TerraMars {
    fn new(
        sites: Vec<MissionSite>,
        images: Arc<Images>,
        store: MissionStore,
        cx: &mut Context<Self>,
    ) -> Self {
        let clock = cx.new(|cx| ClockView::new(store.state.settings.earth_time, cx));
        cx.subscribe(&clock, |this, _, event: &TimeToggled, cx| {
            this.store.state.settings.earth_time = event.0;
            this.save(cx);
            cx.notify();
        })
        .detach();
        let panorama = cx.new(|_| Panorama {
            image: images.panorama.clone(),
        });
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_secs(60))
                    .await;
                if this
                    .update(cx, |this, cx| {
                        if this.details {
                            cx.notify();
                        }
                        if let Some(map) = &this.map {
                            map.update(cx, |_, cx| cx.notify());
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        Self {
            climate: domain::climate::ClimateModel::bundled().expect("Validated climate grids"),
            sites,
            images,
            store,
            clock,
            panorama,
            page: Page::Home,
            browse_page: Page::Home,
            log_preview: None,
            log_preview_index: None,
            log_danger: None,
            site_index: 0,
            map: None,
            selection: None,
            start: None,
            end: None,
            details: false,
            layers: false,
            planner: false,
            endpoint_menu: None,
            map_pick: None,
            coordinate_text: String::new(),
            coordinate_editing: false,
            expanded: BTreeSet::new(),
            expanded_observation: None,
            routes: vec![],
            route_index: 0,
            mode: "On foot".into(),
            status: String::new(),
            status_version: 0,
            status_task: None,
            refreshing: false,
            refresh_notes: vec![],
            refresh_expanded: false,
            focus: cx.focus_handle(),
            transition: 0,
            page_transition: 0,
            rendered_page: Page::Home,
            planning: false,
            planning_version: 0,
        }
    }
    fn units(&self) -> UnitSystem {
        self.store.state.settings.unit_system
    }
    fn smoke_driver(&self, window: &mut Window, cx: &mut Context<Self>) {
        let test_window = window.window_handle();
        performance::set_stage("home");
        performance::check(
            "starts at home",
            self.page == Page::Home,
            "Native GPUI root opened",
        );
        cx.spawn(async move |this, cx| {
            for frame in 0..1200 {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                if frame == 1080 {
                    let _ = test_window.update(cx, |_, window, cx| {
                        let p = point(
                            window.viewport_size().width * 0.55,
                            window.viewport_size().height * 0.42,
                        );
                        window.dispatch_event(
                            PlatformInput::MouseMove(MouseMoveEvent {
                                position: p,
                                ..Default::default()
                            }),
                            cx,
                        );
                        for phase in [TouchPhase::Started, TouchPhase::Moved, TouchPhase::Ended] {
                            window.dispatch_event(
                                PlatformInput::Pinch(PinchEvent {
                                    position: p,
                                    phase,
                                    rotation: if phase == TouchPhase::Moved { 0.25 } else { 0. },
                                    delta: if phase == TouchPhase::Moved { 0.1 } else { 0. },
                                    ..Default::default()
                                }),
                                cx,
                            );
                        }
                    });
                }
                let alive = this.update(cx, |this, cx| {
                    match frame {
                        180 => {
                            this.open_site(0, cx);
                            performance::set_stage("jezero_map");
                        }
                        230 => {
                            performance::check(
                                "explorer starts clean",
                                !this.details
                                    && !this.planner
                                    && this.start.is_none()
                                    && this.end.is_none(),
                                "Panels hidden and route endpoints unset",
                            );
                            if let Some(map) = &this.map {
                                map.update(cx, |m, cx| {
                                    m.focus_rover();
                                    cx.notify();
                                });
                            }
                        }
                        430 => {
                            performance::set_stage("details");
                            let rover = this.sites[0].rover().unwrap().point;
                            let pin =
                                domain::Point::new(rover.latitude + 0.01, rover.longitude + 0.01);
                            if let Some(map) = &this.map {
                                map.update(cx, |_, cx| {
                                    cx.emit(MapSelection {
                                        point: pin,
                                        landmark: None,
                                    })
                                });
                            }
                            this.expanded.insert("Solar geometry".into());
                        }
                        480 => {
                            performance::check(
                                "selection opens panels",
                                this.selection.is_some() && this.details && this.planner,
                                "Map selection emitted through GPUI event subscription",
                            );
                            let original = this.selection.as_ref().unwrap().point;
                            performance::check(
                                "selection fills destination",
                                this.end.as_ref().is_some_and(|e| e.point == original),
                                "B defaults to the inspected map point",
                            );
                            let waypoint = &this.sites[0].landmarks[1];
                            let location = Location {
                                point: waypoint.point,
                                label: waypoint.name.clone(),
                                landmark: Some(1),
                            };
                            this.set_endpoint(Endpoint::Start, location, cx);
                            let rover = this.sites[0].rover().unwrap();
                            this.set_endpoint(
                                Endpoint::End,
                                Location {
                                    point: rover.point,
                                    label: rover.name.clone(),
                                    landmark: Some(0),
                                },
                                cx,
                            );
                            performance::check(
                                "endpoints preserve inspected pin",
                                this.selection.as_ref().unwrap().point == original
                                    && this.map.as_ref().unwrap().read(cx).selected
                                        == Some(original),
                                "Choosing sourced landmarks did not move selection",
                            );
                        }
                        660 => {
                            this.plan(cx);
                        }
                        720 => {
                            performance::set_stage("route");
                            performance::check(
                                "route automatically logged",
                                !this.routes.is_empty()
                                    && this.store.state.planned_routes.len() == 1,
                                "Real Mars geometry and persisted planned-route record",
                            );
                            if let Some(route) = this.routes.first() {
                                let path = this.store.file_path().with_extension("route.json");
                                let result = services::export::write_route(
                                    &path,
                                    &this.sites[0],
                                    route,
                                    &this.mode,
                                    "",
                                );
                                performance::check(
                                    "Mars route export",
                                    result.is_ok(),
                                    path.display().to_string(),
                                );
                            }
                            if this.routes.len() > 1 {
                                this.route_index = 1;
                                this.record_route(cx);
                                this.sync_map(cx);
                                performance::check(
                                    "alternative route logged",
                                    this.store.state.planned_routes.len() == 2,
                                    "Explicit alternative selection recorded",
                                );
                            }
                        }
                        790 => {
                            this.page = Page::Log;
                            this.status.clear();
                            performance::set_stage("mission_log");
                            let loaded =
                                MissionStore::load(this.store.file_path().parent().unwrap());
                            performance::check(
                                "mission log survives reload",
                                loaded.is_ok_and(|s| {
                                    s.state.planned_routes.len()
                                        == this.store.state.planned_routes.len()
                                }),
                                "Serialized plans reloaded from isolated smoke-test storage",
                            );
                        }
                        870 => {
                            this.open_site(1, cx);
                            performance::set_stage("gale_map");
                            performance::check(
                                "Gale opens independently",
                                this.sites[this.site_index].id == "gale"
                                    && this.start.is_none()
                                    && this.end.is_none(),
                                "No stale Jezero endpoints",
                            );
                        }
                        950 => {
                            this.store.state.settings.unit_system = UnitSystem::Scientific;
                            performance::check(
                                "scientific units",
                                this.units().temperature_c(Some(-60.)) == "213.1 K",
                                "Celsius converted to Kelvin",
                            );
                            this.clock.update(cx, |clock, cx| {
                                clock.earth = true;
                                cx.emit(TimeToggled(true));
                                cx.notify();
                            });
                            this.details = true;
                            this.layers = true;
                            if let Some(map) = &this.map {
                                map.update(cx, |m, cx| {
                                    m.options.terrain = true;
                                    m.units = UnitSystem::Scientific;
                                    cx.notify();
                                });
                            }
                            performance::set_stage("gale_terrain");
                        }
                        1000 => {
                            this.planner = true;
                            this.endpoint_menu = Some(Endpoint::Start);
                            performance::set_stage("endpoint_picker");
                        }
                        1040 => {
                            this.details = false;
                            this.planner = false;
                            this.endpoint_menu = None;
                            performance::set_stage("native_gesture");
                            cx.notify();
                        }
                        1100 => {
                            if let Some(map) = &this.map {
                                let map = map.read(cx);
                                let c = map.camera.borrow();
                                performance::check(
                                    "pinch rotation dispatch",
                                    (c.bearing + 0.25).abs() < 0.001,
                                    "Native gesture event rotates the actual map camera",
                                );
                                performance::check(
                                    "pinch scale dispatch",
                                    (c.zoom - 1.1).abs() < 0.001,
                                    "Native gesture event zooms around its focal point",
                                );
                            }
                        }
                        _ => {}
                    }
                    if (250..680).contains(&frame)
                        && let Some(map) = &this.map
                    {
                        map.update(cx, |m, cx| {
                            let p = m.site.rover().unwrap().point;
                            let b = m.site.map_bounds;
                            let t = frame as f32 / 60.;
                            let mut c = m.camera.borrow_mut();
                            c.zoom = 5.;
                            c.x = ((p.longitude - b.west) / (b.east - b.west)) as f32
                                + 0.025 * t.sin();
                            c.y = ((b.north - p.latitude) / (b.north - b.south)) as f32
                                + 0.02 * t.cos();
                            drop(c);
                            cx.notify();
                        });
                    }
                    if matches!(
                        frame,
                        180 | 230 | 430 | 480 | 660 | 720 | 790 | 870 | 950 | 1000
                    ) {
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    return;
                }
            }
            if let Err(error) = performance::finish() {
                eprintln!("Smoke-test report could not be saved: {error}");
            }
            cx.update(|cx| cx.quit());
        })
        .detach();
    }
    fn set_status(&mut self, message: impl Into<String>, cx: &mut Context<Self>) {
        self.status = message.into();
        self.status_version += 1;
        let version = self.status_version;
        self.status_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(5)).await;
            let _ = this.update(cx, |this, cx| {
                if this.status_version == version {
                    this.status.clear();
                    this.refresh_expanded = false;
                    cx.notify();
                }
            });
        }));
        cx.notify();
    }
    fn save(&mut self, cx: &mut Context<Self>) {
        if let Err(e) = self.store.save() {
            self.set_status(format!("Could not save mission settings: {e}"), cx);
        }
    }
    fn open_site(&mut self, index: usize, cx: &mut Context<Self>) {
        self.site_index = index;
        self.planning = false;
        self.planning_version += 1;
        self.page = Page::Explorer;
        self.browse_page = Page::Explorer;
        self.selection = None;
        self.start = None;
        self.end = None;
        self.routes.clear();
        self.details = false;
        self.layers = false;
        self.planner = false;
        self.endpoint_menu = None;
        self.map_pick = None;
        self.status.clear();
        self.transition += 1;
        let map = cx.new(|_| {
            let mut map = MapView::new(self.sites[index].clone(), index, self.images.clone());
            map.reduced_motion = false;
            map.units = self.units();
            map
        });
        cx.subscribe(&map, |this, _, event: &MapSelection, cx| {
            let site = &this.sites[this.site_index];
            let location = Location {
                point: event.point,
                label: event
                    .landmark
                    .map(|i| site.landmarks[i].name.clone())
                    .unwrap_or_else(|| coordinates(event.point)),
                landmark: event.landmark,
            };
            if let Some(endpoint) = this.map_pick.take() {
                this.set_endpoint(endpoint, location, cx);
                this.set_status("Route point set. Your inspected location is unchanged.", cx);
            } else {
                this.selection = Some(location.clone());
                this.end = Some(location);
                this.routes.clear();
                this.details = true;
                this.layers = false;
                this.planner = true;
                this.endpoint_menu = None;
                this.transition += 1;
                this.sync_map(cx);
            }
            cx.notify();
        })
        .detach();
        self.map = Some(map);
        let site = &self.sites[index];
        self.clock.update(cx, |clock, cx| {
            clock.location = Some((site.longitude, site.latitude));
            cx.notify();
        });
        cx.notify();
    }
    fn sync_map(&self, cx: &mut Context<Self>) {
        if let Some(map) = &self.map {
            let selected = self.selection.as_ref().map(|l| l.point);
            let start = self.start.as_ref().map(|l| l.point);
            let end = self.end.as_ref().map(|l| l.point);
            let route = Arc::new(
                self.routes
                    .get(self.route_index)
                    .map(|r| r.points.clone())
                    .unwrap_or_default(),
            );
            map.update(cx, |map, cx| {
                map.selected = selected;
                map.start = start;
                map.end = end;
                map.route = route;
                cx.notify();
            });
        }
    }
    fn set_endpoint(&mut self, endpoint: Endpoint, location: Location, cx: &mut Context<Self>) {
        match endpoint {
            Endpoint::Start => self.start = Some(location),
            Endpoint::End => self.end = Some(location),
        }
        self.endpoint_menu = None;
        self.coordinate_editing = false;
        self.map_pick = None;
        self.routes.clear();
        self.sync_map(cx);
        cx.notify();
    }
    fn plan(&mut self, cx: &mut Context<Self>) {
        let (Some(start), Some(end)) = (&self.start, &self.end) else {
            self.set_status("Choose a starting point and a destination.", cx);
            cx.notify();
            return;
        };
        if self.planning {
            return;
        }
        let (start_point, end_point) = (start.point, end.point);
        let speed = self.store.state.settings.speed_kmh;
        let site_index = self.site_index;
        let mode = self.mode.clone();
        let grid = self.images.hazards[site_index].clone();
        let traverse = self.sites[site_index].traverse.clone();
        self.planning = true;
        self.planning_version += 1;
        let version = self.planning_version;
        let task = cx
            .background_executor()
            .spawn(async move { grid.plan(start_point, end_point, speed, &traverse) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if this.planning_version != version {
                    return;
                }
                this.planning = false;
                if this.site_index != site_index
                    || this.start.as_ref().map(|l| l.point) != Some(start_point)
                    || this.end.as_ref().map(|l| l.point) != Some(end_point)
                    || this.mode != mode
                    || this.store.state.settings.speed_kmh != speed
                {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(routes) => {
                        this.routes = routes;
                        this.route_index = 0;
                        this.record_route(cx);
                        this.sync_map(cx);
                    }
                    Err(error) => {
                        this.routes.clear();
                        this.set_status(error.to_string(), cx);
                        this.sync_map(cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn record_route(&mut self, cx: &mut Context<Self>) {
        if let (Some(start), Some(end), Some(route)) =
            (&self.start, &self.end, self.routes.get(self.route_index))
        {
            match self.store.record_route(
                &self.sites[self.site_index].id,
                &start.label,
                &end.label,
                &self.mode,
                route,
            ) {
                Ok(()) => self.set_status("Route planned and added to Mission log.", cx),
                Err(e) => {
                    self.set_status(format!("Route calculated; saving the log failed: {e}"), cx)
                }
            }
        }
    }
    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.refreshing {
            return;
        }
        self.refreshing = true;
        self.refresh_notes.clear();
        self.refresh_expanded = false;
        self.set_status("Checking published NASA records…", cx);
        let sites = self.sites.clone();
        let task = cx.background_executor().spawn(async move {
            sites
                .iter()
                .map(services::nasa::refresh_site)
                .collect::<Vec<_>>()
        });
        cx.spawn(async move |this, cx| {
            let results = task.await;
            let _ = this.update(cx, |this, cx| {
                let mut count = 0;
                let mut warnings = Vec::new();
                for (index, result) in results.into_iter().enumerate() {
                    count += result.updated_sources.len();
                    warnings.extend(result.warnings);
                    this.sites[index] = result.site;
                }
                if let Some(map) = &this.map {
                    map.update(cx, |map, cx| {
                        map.update_site(this.sites[this.site_index].clone());
                        cx.notify();
                    });
                }
                // Preserve arbitrary map pins; relocate only source-named waypoints.
                for location in [&mut this.selection, &mut this.start, &mut this.end]
                    .into_iter()
                    .flatten()
                {
                    if let Some(i) = location.landmark
                        && let Some(l) = this.sites[this.site_index].landmarks.get(i)
                    {
                        location.point = l.point;
                    }
                }
                this.routes.clear();
                this.sync_map(cx);
                this.refreshing = false;
                this.refresh_notes = warnings;
                this.set_status(
                    format!(
                        "NASA · {count} sources refreshed{}",
                        if this.refresh_notes.is_empty() {
                            String::new()
                        } else {
                            format!(" · {} notes", this.refresh_notes.len())
                        }
                    ),
                    cx,
                );
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn keyboard(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if e.keystroke.key == "escape" {
            if self.page == Page::Log {
                if self.log_danger.take().is_none() {
                    self.log_preview = None;
                    self.log_preview_index = None;
                }
                cx.notify();
                return;
            }
            self.endpoint_menu = None;
            self.coordinate_editing = false;
            self.map_pick = None;
            cx.notify();
            return;
        }
        if self.page != Page::Explorer || !self.coordinate_editing {
            return;
        }
        if e.keystroke.key == "backspace" {
            self.coordinate_text.pop();
        } else if e.keystroke.key == "enter" {
            self.apply_coordinates(cx);
            return;
        } else if (e.keystroke.modifiers.control || e.keystroke.modifiers.platform)
            && e.keystroke.key == "v"
        {
            if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) {
                self.coordinate_text.push_str(
                    &text
                        .chars()
                        .filter(|c| c.is_ascii_digit() || ".,-+ ".contains(*c))
                        .collect::<String>(),
                );
            }
        } else if let Some(text) = &e.keystroke.key_char {
            self.coordinate_text.push_str(
                &text
                    .chars()
                    .filter(|c| c.is_ascii_digit() || ".,-+ ".contains(*c))
                    .collect::<String>(),
            );
        }
        window.focus(&self.focus, cx);
        cx.stop_propagation();
        cx.notify();
    }
    fn apply_coordinates(&mut self, cx: &mut Context<Self>) {
        let numbers = self
            .coordinate_text
            .split([',', ' '])
            .filter(|s| !s.trim().is_empty())
            .map(str::parse::<f64>)
            .collect::<Result<Vec<_>, _>>();
        if let (Ok(numbers), Some(endpoint)) = (numbers, self.endpoint_menu)
            && numbers.len() == 2
        {
            let p = domain::Point::new(numbers[0], numbers[1]);
            if p.validate().is_ok() && self.sites[self.site_index].map_bounds.contains(p) {
                self.set_endpoint(
                    endpoint,
                    Location {
                        point: p,
                        label: coordinates(p),
                        landmark: None,
                    },
                    cx,
                );
                return;
            }
        }
        self.set_status(
            "Enter latitude, east longitude within this crater's map coverage.",
            cx,
        );
        cx.notify();
    }

    fn set_units(&mut self, units: UnitSystem, cx: &mut Context<Self>) {
        self.store.state.settings.unit_system = units;
        if let Some(map) = &self.map {
            map.update(cx, |map, cx| {
                map.units = units;
                cx.notify();
            });
        }
        self.save(cx);
        cx.notify();
    }
    fn header(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let header_width = f32::from(window.viewport_size().width);
        let narrow = header_width < 1250.;
        let show_brand = header_width >= 1150.;
        div()
            .relative()
            .occlude()
            .w_full()
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .px_4()
            .h(px(82.))
            .flex_shrink_0()
            .backdrop_blur(px(16.))
            .bg(rgba(0x10151b60))
            .border_b_1()
            .border_color(rgba(0xffffff20))
            .when(show_brand, |header| {
                header.child(
                    div()
                        .absolute()
                        .left_4()
                        .top_0()
                        .h_full()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .flex()
                                .child(heading("Terra", if narrow { 22. } else { 27. }))
                                .child(
                                    heading("Mars", if narrow { 22. } else { 27. })
                                        .text_color(rgb(ORANGE)),
                                ),
                        ),
                )
            })
            .child(
                div()
                    .absolute()
                    .top_0()
                    .h_full()
                    .w(px(200.))
                    .left(px(if show_brand {
                        (header_width - 200.) / 2.
                    } else {
                        16.
                    }))
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(!show_brand, |d| d.justify_start())
                    .gap_4()
                    .child(
                        nav("home", "Home", self.page != Page::Log).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.page = this.browse_page;
                                if let Some(map) = &this.map {
                                    map.update(cx, |_, cx| cx.notify());
                                }
                                this.status.clear();
                                let location = if this.browse_page == Page::Explorer {
                                    let site = &this.sites[this.site_index];
                                    Some((site.longitude, site.latitude))
                                } else {
                                    None
                                };
                                this.clock.update(cx, |c, cx| {
                                    c.location = location;
                                    cx.notify();
                                });
                                this.transition += 1;
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        nav("mission-log", "Mission log", self.page == Page::Log).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.page = Page::Log;
                                this.status.clear();
                                cx.notify();
                            }),
                        ),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .right_4()
                    .top_0()
                    .h_full()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .p_1()
                            .rounded(px(14.))
                            .bg(rgba(0xffffff08))
                            .border_1()
                            .border_color(rgba(0xffffff1f))
                            .occlude()
                            .child(
                                segment(
                                    "units-metric",
                                    "Metric",
                                    self.units() == UnitSystem::Metric,
                                    narrow,
                                )
                                .on_click(cx.listener(
                                    |this, _, _, cx| this.set_units(UnitSystem::Metric, cx),
                                )),
                            )
                            .child(
                                segment(
                                    "units-scientific",
                                    "Scientific",
                                    self.units() == UnitSystem::Scientific,
                                    narrow,
                                )
                                .on_click(cx.listener(
                                    |this, _, _, cx| this.set_units(UnitSystem::Scientific, cx),
                                )),
                            ),
                    )
                    .child(
                        button("refresh", if self.refreshing { "…" } else { "" })
                            .px_3()
                            .child(icon("refresh"))
                            .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                    )
                    .child(self.clock.clone()),
            )
            .into_any_element()
    }
    fn home(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let width = f32::from(window.viewport_size().width);
        let card_w = ((width - 100.) / 2.).clamp(280., 440.);
        let card_h = (f32::from(window.viewport_size().height) - 180.).clamp(260., 370.);
        let mut cards = div()
            .relative()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .gap_6();
        for (i, name) in ["Jezero Crater", "Gale Crater"].iter().enumerate() {
            let card = div()
                .occlude()
                .border_1()
                .border_color(rgba(0xffffff40))
                .shadow_lg()
                .id(("region", i))
                .w(px(card_w))
                .h(px(card_h))
                .relative()
                .overflow_hidden()
                .cursor_pointer()
                .rounded(px(28.))
                .hover(|s| s.border_color(rgb(ORANGE)))
                .on_click(cx.listener(move |this, _, _, cx| this.open_site(i, cx)))
                .child(
                    img(self.images.imagery[i].clone())
                        .absolute()
                        .size_full()
                        .object_fit(ObjectFit::Fill)
                        .rounded(px(28.)),
                )
                .child(
                    div()
                        .relative()
                        .size_full()
                        .flex()
                        .flex_col()
                        .justify_between()
                        .p_7()
                        .child(
                            label(format!("0{}   /   MARS", i + 1))
                                .text_color(rgb(INK))
                                .bg(rgba(0x11151cae))
                                .rounded_lg()
                                .px_3()
                                .py_2(),
                        )
                        .child(
                            div()
                                .flex()
                                .items_end()
                                .justify_between()
                                .gap_4()
                                .child(
                                    heading(*name, if width < 1000. { 34. } else { 42. })
                                        .bg(rgba(0x11151cae))
                                        .rounded_lg()
                                        .px_3()
                                        .py_2(),
                                )
                                .child(
                                    div()
                                        .size(px(44.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded_full()
                                        .border_1()
                                        .border_color(rgba(0xffffff66))
                                        .child("↗"),
                                ),
                        ),
                );
            cards = cards.child(card);
        }
        div()
            .relative()
            .flex_1()
            .w_full()
            .overflow_hidden()
            .child(div().absolute().size_full().child(self.panorama.clone()))
            .child(reveal(
                cards,
                ("home-enter", self.transition as usize),
                false,
            ))
            .into_any_element()
    }

    fn explorer(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let width = f32::from(window.viewport_size().width);
        let compact_controls = width < 1050. || f32::from(window.viewport_size().height) < 740.;
        let Some(map) = &self.map else {
            return div().into_any_element();
        };
        let options = map.read(cx).options.clone();
        let name = self.sites[self.site_index].name.clone();
        let mut view = div()
            .relative()
            .flex_1()
            .w_full()
            .overflow_hidden()
            .child(map.clone())
            .child(
                div().absolute().top(px(98.)).left_4().child(
                    glass()
                        .occlude()
                        .px_4()
                        .py_3()
                        .child(
                            div()
                                .flex()
                                .gap_3()
                                .items_center()
                                .child(
                                    button("back-to-regions", "")
                                        .px_2()
                                        .child(icon("back"))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.page = Page::Home;
                                            this.browse_page = Page::Home;
                                            this.status.clear();
                                            this.clock.update(cx, |clock, cx| {
                                                clock.location = None;
                                                cx.notify();
                                            });
                                            cx.notify();
                                        })),
                                )
                                .child(heading(name, 18.))
                                .child(label(if options.terrain {
                                    "MOLA relief"
                                } else {
                                    "NASA CTX imagery"
                                })),
                        )
                        .child(
                            label(if options.terrain {
                                "NASA/JPL/GSFC · Mars 2000"
                            } else {
                                "NASA/JPL/MSSS/Caltech/Esri · Mars 2000"
                            })
                            .text_size(px(15.)),
                        ),
                ),
            )
            .child(
                div()
                    .absolute()
                    .top(px(98.))
                    .right_4()
                    .flex()
                    .gap_2()
                    .child(
                        button("show-details", "Info")
                            .child(icon("info").text_color(rgb(if self.details {
                                ORANGE
                            } else {
                                INK
                            })))
                            .when(self.details, |b| {
                                b.text_color(rgb(ORANGE))
                                    .border_color(rgba(0xff945e99))
                                    .bg(rgba(0xff945e25))
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.details = !this.details;
                                this.transition += 1;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div().absolute().left_4().top(px(204.)).child(
                    div()
                        .flex()
                        .when(compact_controls, |d| d.flex_row())
                        .when(!compact_controls, |d| d.flex_col())
                        .gap_1()
                        .child(
                            button("zoom-in", "+").on_click(cx.listener(|this, _, _, cx| {
                                if let Some(m) = &this.map {
                                    m.update(cx, |m, cx| {
                                        m.zoom(1.35);
                                        cx.notify();
                                    });
                                }
                            })),
                        )
                        .child(
                            button("zoom-out", "−").on_click(cx.listener(|this, _, _, cx| {
                                if let Some(m) = &this.map {
                                    m.update(cx, |m, cx| {
                                        m.zoom(1. / 1.35);
                                        cx.notify();
                                    });
                                }
                            })),
                        )
                        .child(
                            button("fit-map", "")
                                .child(icon("fit"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(m) = &this.map {
                                        m.update(cx, |m, cx| {
                                            m.fit();
                                            cx.notify();
                                        });
                                    }
                                })),
                        ),
                ),
            )
            .child(self.planner_view(width, f32::from(window.viewport_size().height), cx));
        if self.details {
            view = view.child(self.details_view(window, cx));
        }
        if self.endpoint_menu.is_some() {
            view = view.child(self.endpoint_picker(
                width,
                f32::from(window.viewport_size().height) - 82.,
                cx,
            ));
        }
        if let Some(endpoint) = self.map_pick {
            view =
                view.child(
                    div().absolute().top(px(90.)).left(px(24.)).child(
                        glass()
                            .occlude()
                            .p_4()
                            .child(format!(
                                "Click the map to choose your {} point",
                                if endpoint == Endpoint::Start {
                                    "starting"
                                } else {
                                    "ending"
                                }
                            ))
                            .child(button("cancel-map-pick", "Cancel").mt_2().on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.map_pick = None;
                                    cx.notify();
                                }),
                            )),
                    ),
                );
        }
        view.into_any_element()
    }

    fn projected_time(&self, cx: &App) -> String {
        let Some(route) = self.routes.get(self.route_index) else {
            return "Projected time · plan a route".into();
        };
        let duration =
            chrono::Duration::seconds(route.duration_seconds.min(i64::MAX as u64) as i64);
        let Some(arrival) = Utc::now().checked_add_signed(duration) else {
            return format!(
                "Projected · {}",
                UnitSystem::duration(route.duration_seconds)
            );
        };
        let arrival = if self.clock.read(cx).earth {
            format!("{} UTC", arrival.format("%H:%M"))
        } else {
            let p = self
                .end
                .as_ref()
                .map(|l| l.point)
                .unwrap_or_else(|| self.sites[self.site_index].center());
            format!(
                "{} LMST",
                MarsClock::at(arrival, p.longitude, p.latitude).formatted_local_time()
            )
        };
        format!(
            "Projected · {} · {arrival}",
            UnitSystem::duration(route.duration_seconds)
        )
    }
    fn planner_view(&self, width: f32, height: f32, cx: &mut Context<Self>) -> AnyElement {
        let available = if self.details {
            width - 410.
        } else {
            width - 280.
        };
        let w = available.clamp(300., 820.);
        let left: f32 = 16.;
        let mut pane = div().min_w_0().w_full().flex().flex_col().gap_3();
        let title =
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .min_w_0()
                        .flex_1()
                        .flex_col()
                        .items_start()
                        .gap_1()
                        .child(heading("Plan a route", 17.))
                        .child(label(self.projected_time(cx)).text_size(px(14.))),
                )
                .child(
                    button("toggle-planner", "")
                        .w(px(40.))
                        .h(px(36.))
                        .min_h(px(36.))
                        .p_0()
                        .flex_shrink_0()
                        .child(icon("chevron").with_transformation(Transformation::rotate(
                            radians(if self.planner {
                                std::f32::consts::FRAC_PI_2
                            } else {
                                -std::f32::consts::FRAC_PI_2
                            }),
                        )))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.planner = !this.planner;
                            if this.planner
                                && this.end.is_none()
                                && let Some(selected) = this.selection.clone()
                            {
                                this.end = Some(selected);
                                this.routes.clear();
                                this.sync_map(cx);
                            }
                            this.endpoint_menu = None;
                            this.transition += 1;
                            cx.notify();
                        })),
                );
        if self.planner {
            let mut endpoints = div().flex().items_center().gap_2();
            for (endpoint, title, location) in [
                (Endpoint::Start, "A  ·  Start", &self.start),
                (Endpoint::End, "B  ·  Destination", &self.end),
            ] {
                let text = location
                    .as_ref()
                    .map(|l| l.label.clone())
                    .unwrap_or_else(|| "Choose location…".into());
                endpoints = endpoints.child(
                    div().flex_1().min_w_0().child(label(title)).child(
                        button(
                            if endpoint == Endpoint::Start {
                                "start-point"
                            } else {
                                "end-point"
                            },
                            text,
                        )
                        .w_full()
                        .justify_start()
                        .truncate()
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.endpoint_menu = if this.endpoint_menu == Some(endpoint) {
                                    None
                                } else {
                                    Some(endpoint)
                                };
                                this.coordinate_text.clear();
                                this.coordinate_editing = false;
                                window.focus(&this.focus, cx);
                                cx.notify();
                            },
                        )),
                    ),
                );
                if endpoint == Endpoint::Start {
                    endpoints = endpoints.child(button("swap", "⇄").mt_4().px_2().on_click(
                        cx.listener(|this, _, _, cx| {
                            std::mem::swap(&mut this.start, &mut this.end);
                            this.routes.clear();
                            this.sync_map(cx);
                            cx.notify();
                        }),
                    ));
                }
            }
            pane = pane.child(endpoints);
            pane = pane.child(
                div()
                    .flex()
                    .gap_2()
                    .items_center()
                    .justify_between()
                    .when(w < 650., |d| d.flex_col().items_start())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                button("travel-mode", self.mode.clone()).on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.mode = if this.mode == "On foot" {
                                            "Rover"
                                        } else {
                                            "On foot"
                                        }
                                        .into();
                                        this.routes.clear();
                                        this.sync_map(cx);
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(button("speed-less", "−").px_3().on_click(cx.listener(
                                |this, _, _, cx| {
                                    this.store.state.settings.speed_kmh =
                                        (this.store.state.settings.speed_kmh - 0.5).max(0.5);
                                    this.routes.clear();
                                    this.sync_map(cx);
                                    this.save(cx);
                                    cx.notify();
                                },
                            )))
                            .child(
                                label(self.units().speed_kmh(self.store.state.settings.speed_kmh))
                                    .text_color(rgb(INK)),
                            )
                            .child(button("speed-more", "+").px_3().on_click(cx.listener(
                                |this, _, _, cx| {
                                    this.store.state.settings.speed_kmh =
                                        (this.store.state.settings.speed_kmh + 0.5).min(15.);
                                    this.routes.clear();
                                    this.sync_map(cx);
                                    this.save(cx);
                                    cx.notify();
                                },
                            ))),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(button("clear-route", "Clear").on_click(cx.listener(
                                |this, _, _, cx| {
                                    this.start = None;
                                    this.end = None;
                                    this.routes.clear();
                                    this.endpoint_menu = None;
                                    this.sync_map(cx);
                                    cx.notify();
                                },
                            )))
                            .child(
                                primary(
                                    "calculate-route",
                                    if self.planning {
                                        "Planning…"
                                    } else {
                                        "Plan route  ↗"
                                    },
                                )
                                .on_click(cx.listener(|this, _, _, cx| this.plan(cx))),
                            ),
                    ),
            );
            if !self.routes.is_empty() {
                let mut routes = div().flex().gap_2();
                for (i, route) in self.routes.iter().enumerate() {
                    let description = format!(
                        "{}  ·  {}  ·  {}",
                        route.name,
                        self.units().distance_km(route.distance_km),
                        UnitSystem::duration(route.duration_seconds)
                    );
                    routes = routes.child(
                        button(("route-choice", i), description)
                            .flex_1()
                            .truncate()
                            .text_size(px(15.))
                            .when(i == self.route_index, |b| {
                                b.border_color(rgb(ACCENT)).bg(rgba(0xb3aaff18))
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if this.route_index != i {
                                    this.route_index = i;
                                    this.record_route(cx);
                                    this.sync_map(cx);
                                    cx.notify();
                                }
                            })),
                    );
                }
                pane = pane.child(routes).child(
                    div()
                        .flex()
                        .justify_between()
                        .items_center()
                        .gap_3()
                        .child(
                            label("MOLA slope screening · small hazards unassessed")
                                .flex_1()
                                .text_size(px(14.)),
                        )
                        .child(
                            button("fit-route", "Fit route")
                                .py_1()
                                .min_h(px(30.))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let (Some(map), Some(route)) =
                                        (&this.map, this.routes.get(this.route_index))
                                    {
                                        map.update(cx, |map, cx| {
                                            map.fit_route(&route.points);
                                            cx.notify();
                                        });
                                    }
                                })),
                        ),
                );
            }
        }
        div()
            .absolute()
            .left(px(left.max(16.)))
            .bottom_4()
            .child(reveal(
                glass()
                    .occlude()
                    .p_4()
                    .w(px(w))
                    .max_h(px((height - 284.).max(180.)))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(title)
                    .when(self.planner, |card| {
                        card.child(
                            div()
                                .id("planner-scroll")
                                .occlude()
                                .min_h_0()
                                .flex_1()
                                .max_h(px((height - 410.).max(130.)))
                                .overflow_y_scroll()
                                .child(pane),
                        )
                    }),
                ("planner-enter", self.transition as usize),
                false,
            ))
            .into_any_element()
    }

    fn endpoint_picker(&self, width: f32, height: f32, cx: &mut Context<Self>) -> AnyElement {
        let endpoint = self.endpoint_menu.unwrap();
        let w = (width - 420.).clamp(330., 500.);
        let mut menu = glass()
            .bg(rgba(0x24252bfa))
            .w(px(w))
            .p_4()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(heading(
                        if endpoint == Endpoint::Start {
                            "Choose start"
                        } else {
                            "Choose destination"
                        },
                        18.,
                    ))
                    .child(button("close-endpoint-menu", "×").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.endpoint_menu = None;
                            this.coordinate_editing = false;
                            cx.notify();
                        },
                    ))),
            );
        if let Some(selection) = &self.selection {
            let l = selection.clone();
            menu = menu.child(
                button(
                    "use-selection",
                    format!("Use selected · {}", selection.label),
                )
                .justify_start()
                .truncate()
                .on_click(
                    cx.listener(move |this, _, _, cx| this.set_endpoint(endpoint, l.clone(), cx)),
                ),
            );
        }
        menu = menu
            .child(
                button("pick-on-map", "Pick a point on the map  ↗")
                    .justify_start()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.map_pick = Some(endpoint);
                        this.endpoint_menu = None;
                        this.coordinate_editing = false;
                        cx.notify();
                    })),
            )
            .child(label("SOURCED LOCATIONS"));
        let mut places = div()
            .id("landmark-choices")
            .occlude()
            .overflow_y_scroll()
            .max_h(px(if height < 720. { 105. } else { 205. }))
            .flex()
            .flex_col()
            .gap_1();
        for (i, landmark) in self.sites[self.site_index].landmarks.iter().enumerate() {
            let l = Location {
                point: landmark.point,
                label: landmark.name.clone(),
                landmark: Some(i),
            };
            places = places.child(
                button(("landmark-option", i), landmark.name.clone())
                    .justify_start()
                    .text_size(px(14.))
                    .truncate()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_endpoint(endpoint, l.clone(), cx)
                    })),
            );
        }
        menu = menu
            .child(places)
            .child(rule())
            .child(label("Latitude, east longitude · Mars degrees"))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        button(
                            "coordinate-input",
                            if self.coordinate_text.is_empty() {
                                if self.site_index == 0 {
                                    "18.44, 77.45"
                                } else {
                                    "-4.65, 137.40"
                                }
                                .into()
                            } else {
                                format!(
                                    "{}{}",
                                    self.coordinate_text,
                                    if self.coordinate_editing { "│" } else { "" }
                                )
                            },
                        )
                        .flex_1()
                        .justify_start()
                        .when(self.coordinate_editing, |b| b.border_color(rgb(ACCENT)))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.coordinate_editing = true;
                            window.focus(&this.focus, cx);
                            cx.notify();
                        })),
                    )
                    .child(
                        primary("apply-coordinates", "Set")
                            .on_click(cx.listener(|this, _, _, cx| this.apply_coordinates(cx))),
                    ),
            );
        // Float over the planner so every picker control remains reachable,
        // including after route alternatives increase the planner's height.
        let bottom = 16.;
        div()
            .absolute()
            .left_5()
            .bottom(px(bottom))
            .child(
                div()
                    .id("endpoint-popup-scroll")
                    .occlude()
                    .max_h(px((height - bottom - 80.).max(160.)))
                    .overflow_y_scroll()
                    .rounded(px(20.))
                    .child(menu.flex_shrink_0()),
            )
            .into_any_element()
    }

    fn details_view(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let panel = glass()
            .occlude()
            .w(px(350.))
            .h_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(
                div()
                    .p_5()
                    .pb_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(label(
                        self.selection
                            .as_ref()
                            .map(|l| {
                                if l.landmark.is_some() {
                                    l.label.clone()
                                } else {
                                    "Selected point".into()
                                }
                            })
                            .unwrap_or_else(|| self.sites[self.site_index].name.clone()),
                    ))
                    .when(self.selection.is_some(), |s| {
                        s.child(label(coordinates(self.selection.as_ref().unwrap().point)))
                    })
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .mt_2()
                            .child(
                                button("conditions-tab", "Conditions")
                                    .flex_1()
                                    .when(!self.layers, |b| b.bg(rgba(0xffffff20)))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.layers = false;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                button("layers-tab", "Visuals")
                                    .flex_1()
                                    .when(self.layers, |b| b.bg(rgba(0xffffff20)))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.layers = true;
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(if self.layers {
                self.layers_content(cx)
            } else {
                self.conditions_content(cx)
            });
        let top = 154.;
        let bottom = 98.;
        let height = f32::from(window.viewport_size().height) - top - bottom;
        div()
            .absolute()
            .right_4()
            .top(px(top))
            .h(px(height.max(150.)))
            .child(reveal(
                panel,
                ("details-enter", self.transition as usize),
                false,
            ))
            .into_any_element()
    }

    fn conditions_content(&self, cx: &mut Context<Self>) -> AnyElement {
        let site = &self.sites[self.site_index];
        let point = self
            .selection
            .as_ref()
            .map(|l| l.point)
            .unwrap_or_else(|| site.center());
        let mars = MarsClock::at(Utc::now(), point.longitude, point.latitude);
        let units = self.units();
        let predicted = self
            .climate
            .predict(&site.id, mars.solar_longitude, mars.local_true_hours);
        let mut content = div()
            .id("conditions-scroll")
            .occlude()
            .overflow_y_scroll()
            .flex_1()
            .min_h_0()
            .px_5()
            .pb_5()
            .flex()
            .flex_col()
            .gap_3();
        let grid = &self.images.hazards[self.site_index];
        if let Some((zone, slope)) = grid.danger(point) {
            content=content.child(div().id("danger-explanation").p_3().rounded_lg().border_1().border_color(rgba(0xff694580)).bg(rgba(0x481c1c55)).flex().flex_col().gap_2()
                .child(heading(format!("⚠ Steep terrain · {:.1}°",slope),20.).text_color(rgb(0xffaa83)))
                .child(label(format!("Danger zone {} · maximum {:.1}°",zone.id+1,zone.max_slope)))
                .child(label("Steep gradients can cause slipping or vehicle rollover. Routes exclude these cells using the app’s 15° slope planning limit."))
                .child(label(format!("Derived from NASA MOLA numerical topography at about {:.0} m per cell. Small rocks, ledges and sand traps are not resolved; this is a regional screening estimate, not terrain clearance.",grid.metadata.nominal_resolution_m)))
                .child(source_button("danger-source","MOLA data & provenance ↗",grid.metadata.source_url.clone())));
        } else if !grid.traversable(point) {
            content=content.child(heading("Unassessed terrain",20.)).child(label("No valid numerical slope sample covers this point. Routing excludes unassessed cells."));
        }
        let mut forecast = div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .child(heading("Regional weather", 20.))
                    .child(label("Predicted").text_color(rgb(ORANGE))),
            )
            .child(label(
                "MCD 6.2 · seasonal climate model, not live measurements",
            ));
        for key in [
            "air_temperature",
            "surface_temperature",
            "wind_speed",
            "wind_direction",
            "dust_tau",
            "solar_irradiance",
        ] {
            if let Some(p) = predicted.get(key) {
                forecast = forecast.child(
                    div()
                        .flex()
                        .justify_between()
                        .gap_3()
                        .child(label(p.label).flex_1())
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .text_right()
                                .text_color(rgb(INK))
                                .text_size(px(17.))
                                .child(format!(
                                    "{} · Predicted",
                                    units.observation(Some(p.value), p.unit)
                                )),
                        ),
                );
            }
        }
        forecast=forecast.child(label("Regional climatology at the crater center, interpolated for this Mars season and local time. Actual weather and storms may differ."))
            .child(source_button("climate-source","Climate model & provenance ↗",domain::climate::SOURCE.into()));
        content = content.child(forecast).child(rule());
        if let Some(i) = self.selection.as_ref().and_then(|l| l.landmark)
            && let Some(l) = site.landmarks.get(i)
        {
            content = content
                .child(label(l.description.clone()))
                .child(label(format!(
                    "{}{}",
                    l.sol
                        .map(|s| format!("Mission sol {s}"))
                        .unwrap_or_default(),
                    l.elevation_meters
                        .map(|m| format!(" · {} elevation", units.elevation_meters(m)))
                        .unwrap_or_default()
                )))
                .child(source_button(
                    ("landmark-source", i),
                    "Waypoint source",
                    l.source_url.clone(),
                ));
        }
        content = content.child(
            div()
                .rounded_lg()
                .p_3()
                .bg(rgba(0xb3aaff0c))
                .border_1()
                .border_color(rgba(0xb3aaff25))
                .child(
                    label("HISTORICAL STATION RECORDS")
                        .text_color(rgb(ACCENT))
                        .text_size(px(14.)),
                )
                .child(
                    label(format!(
                        "Measured at {}. Values are not forecasts or conditions at this pin.",
                        site.rover_name
                    ))
                    .mt_1(),
                ),
        );
        let mut values = div().flex().flex_wrap().gap_2();
        for key in ["air_min", "air_max", "pressure", "radiation"] {
            if let Some(o) = site.observations.iter().find(|o| o.key == key) {
                let short = match key {
                    "air_min" => "Air low",
                    "air_max" => "Air high",
                    "pressure" => "Pressure",
                    _ => "Radiation",
                };
                values = values.child(
                    div()
                        .w(px(145.))
                        .p_3()
                        .rounded_lg()
                        .bg(rgba(0xffffff06))
                        .child(label(short).text_size(px(14.)))
                        .child(
                            div()
                                .mt_1()
                                .text_size(px(19.))
                                .font_weight(FontWeight::MEDIUM)
                                .child(observation_text(units, o, &predicted)),
                        )
                        .child(
                            label(
                                o.observed_at
                                    .clone()
                                    .unwrap_or_else(|| "No observation date".into()),
                            )
                            .text_size(px(15.)),
                        ),
                );
            }
        }
        content = content.child(values);
        content = content.child(
            self.disclosure(
                "Solar geometry",
                format!(
                    "{} · {}",
                    mars.formatted_local_time(),
                    if mars.is_daylight() {
                        "Daylight"
                    } else {
                        "Night"
                    }
                ),
                cx,
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(label(format!("{} · {}", mars.date_label(), mars.season())))
                    .child(label(format!(
                        "AMT {} · Ls {:.2}°",
                        mars.formatted_airy_time(),
                        mars.solar_longitude
                    )))
                    .child(label(format!(
                        "Solar altitude {:.2}° · Sun {:.3} AU",
                        mars.solar_altitude, mars.heliocentric_distance_au
                    )))
                    .child(label(format!(
                        "2 m object shadow: {}",
                        mars.shadow_length(2.)
                            .map(|m| units.elevation_meters(m))
                            .unwrap_or_else(|| "Sun below horizon".into())
                    )))
                    .child(label(
                        "Flat-ground geometry only. Terrain shadows and glare are unavailable.",
                    ))
                    .child(source_button(
                        "solar-source",
                        "NASA Mars24",
                        MarsClock::SOURCE_URL.into(),
                    )),
            ),
        );
        let categories = [
            "Thermal",
            "Atmosphere",
            "Weather",
            "Terrain",
            "Surface",
            "Radiation",
            "Operations",
            "Time",
        ];
        for category in categories {
            let observations = site
                .observations
                .iter()
                .filter(|o| o.category == category)
                .collect::<Vec<_>>();
            let available = observations.iter().filter(|o| o.available()).count();
            let modeled = observations
                .iter()
                .filter(|o| !o.available() && predicted.contains_key(o.key.as_str()))
                .count();
            let mut body = div().flex().flex_col().gap_2();
            for o in &observations {
                let key = o.key.clone();
                let open = self.expanded_observation.as_ref() == Some(&key);
                let model = (!o.available())
                    .then(|| predicted.get(o.key.as_str()))
                    .flatten();
                let mut row = div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .py_2()
                    .border_b_1()
                    .border_color(rgba(0xffffff0c))
                    .child(
                        div()
                            .id(SharedString::from(format!("obs-{key}")))
                            .cursor_pointer()
                            .child(label(o.label.clone()))
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_size(px(17.))
                                            .text_color(if o.available() {
                                                rgb(INK)
                                            } else {
                                                rgb(MUTED)
                                            })
                                            .child(observation_text(units, o, &predicted)),
                                    )
                                    .child(label(if open { "−" } else { "+" })),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.expanded_observation =
                                    if this.expanded_observation.as_ref() == Some(&key) {
                                        None
                                    } else {
                                        Some(key.clone())
                                    };
                                cx.notify();
                            })),
                    );
                if open {
                    row = row
                        .when(model.is_some(), |r| {
                            r.child(
                                label(
                                    "Predicted · MCD 6.2 regional climatology, not a measurement.",
                                )
                                .text_color(rgb(ORANGE)),
                            )
                            .child(source_button(
                                SharedString::from(format!("prediction-{}", o.key)),
                                "Prediction model ↗",
                                domain::climate::SOURCE.into(),
                            ))
                        })
                        .child(label(o.note.clone()))
                        .child(label(format!("Coverage: {}", o.coverage)))
                        .child(label(format!(
                            "{}{}",
                            o.observed_at
                                .as_deref()
                                .unwrap_or("UTC observation date unavailable"),
                            o.sol
                                .map(|s| format!(" · Mission sol {s}"))
                                .unwrap_or_default()
                        )))
                        .child(source_button(
                            SharedString::from(format!("source-{}", o.key)),
                            "View source ↗",
                            o.source_url.clone(),
                        ));
                }
                body = body.child(row);
            }
            content = content.child(self.disclosure(
                category,
                format!("{available} recorded · {modeled} predicted"),
                cx,
                body,
            ));
        }
        let mut sources = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(label(format!("Dataset retrieved {}", site.retrieved_at)))
            .child(label(
                "Planetocentric latitude · east longitude · Mars 2000 sphere",
            ));
        for (i, source) in site.sources.iter().enumerate() {
            sources = sources.child(source_button(
                ("data-source", i),
                source.title.clone(),
                source.url.clone(),
            ));
        }
        content = content.child(self.disclosure(
            "Data & provenance",
            "NASA / USGS sources".into(),
            cx,
            sources,
        ));
        content.into_any_element()
    }
    fn disclosure(
        &self,
        title: &'static str,
        summary: String,
        cx: &mut Context<Self>,
        body: Div,
    ) -> Div {
        let open = self.expanded.contains(title);
        let mut row = div()
            .flex()
            .flex_col()
            .gap_2()
            .border_t_1()
            .border_color(rgba(0xffffff14))
            .pt_3()
            .child(
                div()
                    .id(title)
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .child(div().font_weight(FontWeight::MEDIUM).child(title))
                            .child(label(summary).text_size(px(14.))),
                    )
                    .child(label(if open { "−" } else { "+" }))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.expanded.remove(title) {
                            this.expanded.insert(title.into());
                        }
                        cx.notify();
                    })),
            );
        if open {
            row = row.child(body);
        }
        row
    }
    fn layers_content(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(map) = &self.map else {
            return div().into_any_element();
        };
        let options = &map.read(cx).options;
        let mut content = div()
            .id("layers-scroll")
            .occlude()
            .overflow_y_scroll()
            .flex_1()
            .px_5()
            .pb_5()
            .flex()
            .flex_col()
            .gap_3();
        for (id, title, enabled) in [
            (0, "MOLA terrain relief", options.terrain),
            (1, "Sourced landmarks", options.landmarks),
            (2, "Rover position", options.rover),
            (3, "Recorded rover traverse", options.traverse),
            (4, "Day / night shading", options.terminator),
            (5, "Danger zones · steep terrain", options.dangers),
        ] {
            content = content.child(
                button(("map-layer", id as usize), title)
                    .justify_start()
                    .w_full()
                    .justify_between()
                    .child(switch_visual(enabled))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(map) = &this.map {
                            map.update(cx, |map, cx| {
                                let o = &mut map.options;
                                match id {
                                    0 => o.terrain = !o.terrain,
                                    1 => o.landmarks = !o.landmarks,
                                    2 => o.rover = !o.rover,
                                    3 => o.traverse = !o.traverse,
                                    4 => o.terminator = !o.terminator,
                                    _ => o.dangers = !o.dangers,
                                }
                                cx.notify();
                            });
                        }
                        cx.notify();
                    })),
            );
        }
        content.child(rule()).child(label("CTX is orbital imagery; relief is a color visualization. Danger calculations use a separate numerical MOLA elevation grid."))
            .child(label("Danger zones derive from numerical MOLA slopes ≥15°. Routes avoid them even when the visual is hidden. Resolution ~463 m; small obstacles and sand are not assessed."))
            .child(label("Night shading uses planetary solar geometry. It does not model terrain shadows."))
            .child(label("Rover waypoints and recorded paths are dated observations, not astronaut routes."))
            .child(source_button("imagery-credit","NASA map source ↗",self.sites[self.site_index].map_source_url.clone())).into_any_element()
    }

    fn log(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut content = div()
            .id("mission-log-scroll")
            .overflow_y_scroll()
            .size_full()
            .p_8()
            .pt(px(110.))
            .flex()
            .flex_col()
            .gap_5()
            .child(heading("Mission log", 32.));
        if self.store.state.planned_routes.is_empty() {
            content = content.child(
                glass()
                    .flex_shrink_0()
                    .p_7()
                    .child(heading("No routes planned yet", 22.))
                    .child(label("Choose a crater and set a start and destination.").mt_2()),
            );
        }
        for (i, record) in self.store.state.planned_routes.iter().enumerate() {
            let index = if record.crater_id == "jezero" { 0 } else { 1 };
            let record = record.clone();
            let record_for_open = record.clone();
            content = content.child(
                glass()
                    .flex_shrink_0()
                    .p_5()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .min_w_0()
                                    .flex_1()
                                    .child(label(format!(
                                        "{} · {} UTC",
                                        self.sites[index].name,
                                        record.planned_at.format("%d %b %Y  %H:%M")
                                    )))
                                    .child(heading(
                                        format!("{} → {}", record.start_label, record.end_label),
                                        21.,
                                    )),
                            )
                            .child(
                                button(("open-log-route", i), "View route ↗")
                                    .flex_shrink_0()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let r = &record_for_open;
                                        let map = cx.new(|_| {
                                            let mut map = MapView::new(
                                                this.sites[index].clone(),
                                                index,
                                                this.images.clone(),
                                            );
                                            map.units = this.units();
                                            map.route = Arc::new(r.route.points.clone());
                                            map.start = r.route.points.first().copied();
                                            map.end = r.route.points.last().copied();
                                            map.fit_route(&r.route.points);
                                            map
                                        });
                                        cx.subscribe(
                                            &map,
                                            move |this, _, event: &MapSelection, cx| {
                                                this.log_danger = this.images.hazards[index]
                                                    .danger(event.point)
                                                    .map(|_| (index, event.point));
                                                cx.notify();
                                            },
                                        )
                                        .detach();
                                        this.log_danger = None;
                                        this.log_preview = Some(map);
                                        this.log_preview_index = Some(i);
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_5()
                            .flex_wrap()
                            .child(label(record.route.name.clone()))
                            .child(label(self.units().distance_km(record.route.distance_km)))
                            .child(label(UnitSystem::duration(record.route.duration_seconds)))
                            .child(label(format!(
                                "{} · assumed {}",
                                record.mode,
                                self.units().speed_kmh(record.route.assumed_speed_kmh)
                            ))),
                    )
                    .child(label(record.route.risk_note()).text_size(px(14.))),
            );
        }
        div()
            .relative()
            .size_full()
            .overflow_hidden()
            .child(div().absolute().inset_0().child(self.panorama.clone()))
            .child(content.relative())
            .into_any_element()
    }
}

impl Render for TerraMars {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _timing = performance::render_scope();
        if self.rendered_page != self.page {
            self.page_transition += 1;
            self.rendered_page = self.page;
        }
        let body = match self.page {
            Page::Home => self.home(window, cx),
            Page::Explorer => self.explorer(window, cx),
            Page::Log => self.log(cx),
        };
        let mut app = div()
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x0e1016))
            .text_color(rgb(INK))
            .font_family("Inter")
            .text_size(px(18.))
            .on_key_down(cx.listener(|this, event, window, cx| this.keyboard(event, window, cx)))
            .relative()
            .child(
                div()
                    .relative()
                    .size_full()
                    .min_h_0()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .child(body)
                    .with_animation(
                        ("page-enter", self.page_transition as usize),
                        Animation::new(Duration::from_millis(220))
                            .with_easing(|t| 1. - (1. - t).powi(3)),
                        |view, t| view.opacity(t).top(px((1. - t) * 10.)),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .w_full()
                    .child(self.header(window, cx)),
            );
        if self.page == Page::Log
            && let Some(preview) = &self.log_preview
        {
            let width = (f32::from(window.viewport_size().width) - 64.).min(1100.);
            let height = (f32::from(window.viewport_size().height) - 140.).max(300.);
            let record = self
                .log_preview_index
                .and_then(|i| self.store.state.planned_routes.get(i));
            let title = record
                .map(|r| format!("{} → {}", r.start_label, r.end_label))
                .unwrap_or_else(|| "Saved route".into());
            let summary = record
                .map(|r| {
                    format!(
                        "{} · {} · {} · assumed {}",
                        self.units().distance_km(r.route.distance_km),
                        UnitSystem::duration(r.route.duration_seconds),
                        r.mode,
                        self.units().speed_kmh(r.route.assumed_speed_kmh)
                    )
                })
                .unwrap_or_default();
            app = app.child(
                div()
                    .absolute()
                    .inset_0()
                    .occlude()
                    .bg(rgba(0x080b1060))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        glass()
                            .w(px(width))
                            .h(px(height))
                            .overflow_hidden()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .p_4()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_4()
                                    .child(
                                        div()
                                            .min_w_0()
                                            .flex_1()
                                            .child(heading("Saved route", 22.))
                                            .child(label(title))
                                            .child(label(summary).text_size(px(14.))),
                                    )
                                    .child(
                                        button("close-log-preview", "Close").child("×").on_click(
                                            cx.listener(|this, _, _, cx| {
                                                this.log_preview = None;
                                                this.log_preview_index = None;
                                                cx.notify();
                                            }),
                                        ),
                                    ),
                            )
                            .child(
                                div()
                                    .relative()
                                    .flex_1()
                                    .w_full()
                                    .overflow_hidden()
                                    .child(preview.clone())
                                    .when_some(self.log_danger, |view, (index,p)| {
                                        let grid=&self.images.hazards[index];
                                        let Some((_,slope))=grid.danger(p) else {return view;};
                                        view.child(div().absolute().top_4().left_4().child(glass().id("log-danger-scroll").occlude().w(px((width-48.).min(330.))).max_h(px((height-180.).max(140.))).overflow_y_scroll().p_4().flex().flex_col().gap_2()
                                            .child(div().flex().items_center().justify_between().gap_2().child(heading(format!("Steep terrain · {:.1}°",slope),18.)).child(button("close-log-danger","×").px_2().on_click(cx.listener(|this,_,_,cx|{this.log_danger=None;cx.notify();}))))
                                            .child(label("Slipping / rollover risk. Terrain-aware plans exclude slopes ≥15° at MOLA’s ~463 m scale. Small obstacles and sand traps remain unassessed."))
                                            .child(source_button("log-danger-source","NASA MOLA source ↗",grid.metadata.source_url.clone()))))
                                    }),
                            ),
                    ),
            );
        }
        if !self.status.is_empty() {
            let mut toast = glass()
                .occlude()
                .w(px((f32::from(window.viewport_size().width) - 32.).min(460.)))
                .p_4()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .text_size(px(16.))
                                .child(self.status.clone()),
                        )
                        .child(button("dismiss-status", "×").px_3().on_click(cx.listener(
                            |this, _, _, cx| {
                                this.status.clear();
                                this.refresh_expanded = false;
                                cx.notify();
                            },
                        ))),
                );
            if self.status.starts_with("NASA") && !self.refresh_notes.is_empty() {
                toast = toast.child(
                    button(
                        "refresh-details",
                        if self.refresh_expanded {
                            "Hide notes ⌃"
                        } else {
                            "Source notes ⌄"
                        },
                    )
                    .justify_start()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.refresh_expanded = !this.refresh_expanded;
                        cx.notify();
                    })),
                );
                if self.refresh_expanded {
                    toast = toast.child(
                        div()
                            .id("refresh-notes-scroll")
                            .occlude()
                            .max_h(px(160.))
                            .overflow_y_scroll()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .children(
                                self.refresh_notes
                                    .iter()
                                    .map(|n| label(n.clone()).text_size(px(14.))),
                            ),
                    );
                }
            }
            let viewport = f32::from(window.viewport_size().width);
            let toast_width = if viewport < 1100. { 360. } else { 460. };
            app = app.child(
                div()
                    .absolute()
                    .top(px(90.))
                    .left(px((viewport - toast_width) / 2.))
                    .child(toast.w(px(toast_width))),
            );
        }
        if super::interaction::enabled() {
            super::interaction::capture(serde_json::json!({
                "page":match self.page{Page::Home=>"home",Page::Explorer=>"explorer",Page::Log=>"log"},
                "log_preview":self.log_preview.is_some(),"browse_page":if self.browse_page==Page::Explorer{"explorer"}else{"home"},"planner":self.planner,"details":self.details,"layers":self.layers,"endpoint_menu":self.endpoint_menu.map(|e|format!("{e:?}")),
                "selected":self.selection.as_ref().map(|l|l.point),"start":self.start.as_ref().map(|l|l.point),"end":self.end.as_ref().map(|l|l.point),
                "danger_selected":self.selection.as_ref().and_then(|l|self.images.hazards[self.site_index].danger(l.point)).is_some(),"projected_time":self.projected_time(cx),"coordinate_text":self.coordinate_text,"coordinate_editing":self.coordinate_editing,"log_count":self.store.state.planned_routes.len(),"route_count":self.routes.len(),"units":self.units().label(),"earth":self.clock.read(cx).earth,
                "camera_center":self.map.as_ref().map(|m| {let v=m.read(cx);let c=v.camera.borrow();[c.x,c.y]}),"bearing":self.map.as_ref().map(|m|m.read(cx).camera.borrow().bearing),"zoom":self.map.as_ref().map(|m|m.read(cx).camera.borrow().zoom),"map_options":self.map.as_ref().map(|m|{
                    let o=&m.read(cx).options;serde_json::json!({"terrain":o.terrain,"rover":o.rover,"landmarks":o.landmarks,"traverse":o.traverse,"night":o.terminator,"dangers":o.dangers})
                }),"status":self.status,"refreshing":self.refreshing,"refresh_expanded":self.refresh_expanded,
            }));
            app = app.child(super::interaction::flush());
        }
        app
    }
}

fn coordinates(p: domain::Point) -> String {
    format!(
        "{:.5}°{}  {:.5}°E",
        p.latitude.abs(),
        if p.latitude < 0. { "S" } else { "N" },
        p.longitude
    )
}
fn source_button(
    id: impl Into<ElementId>,
    title: impl Into<SharedString>,
    url: String,
) -> Stateful<Div> {
    button(id, title)
        .text_size(px(14.))
        .min_h(px(30.))
        .py_1()
        .justify_start()
        .text_color(rgb(ACCENT))
        .truncate()
        .on_click(move |_, _, cx| cx.open_url(&url))
}

pub fn run() -> anyhow::Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    let smoke = args.iter().position(|a| a == "--smoke-test");
    let interaction = args.iter().position(|a| a == "--interaction-test");
    if let Some(i) = interaction {
        super::interaction::configure(args.get(i + 1).expect("interaction trace path").into());
    }
    if let Some(index) = smoke {
        performance::configure(
            args.get(index + 1)
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::env::temp_dir().join("terramars-gpui-smoke.json")),
        );
    }
    if performance::enabled() {
        performance::set_stage("home");
    }
    let sites = domain::catalog()?;
    let images = Arc::new(Images::load()?);
    #[cfg(target_os = "macos")]
    let app_icon_bytes = include_bytes!("../../assets/icons/app_logo/logo.png").as_slice();
    #[cfg(not(target_os = "macos"))]
    let app_icon_bytes = include_bytes!("../../assets/icons/app_logo/logo_small.png").as_slice();
    let app_icon = Arc::new(
        image::load_from_memory(app_icon_bytes)?
            .resize_exact(256, 256, image::imageops::FilterType::Lanczos3)
            .into_rgba8(),
    );
    let store = if smoke.is_some() || interaction.is_some() {
        MissionStore::new(
            std::env::temp_dir().join(format!("terramars-gpui-smoke-{}", std::process::id())),
        )
    } else {
        MissionStore::load_default()?
    };
    let window_size = args
        .iter()
        .position(|a| a == "--window-size")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.split_once('x'))
        .and_then(|(w, h)| Some((w.parse::<f32>().ok()?, h.parse::<f32>().ok()?)))
        .filter(|(w, h)| w.is_finite() && h.is_finite())
        .map(|(w, h)| size(px(w.max(800.)), px(h.max(600.))))
        .unwrap_or_else(|| size(px(1440.), px(940.)));
    gpui_platform::application()
        .with_assets(super::assets::EmbeddedAssets)
        .run(move |cx: &mut App| {
            cx.text_system()
                .add_fonts(vec![
                    std::borrow::Cow::Borrowed(include_bytes!(
                        "../../assets/fonts/Inter-Regular.ttf"
                    )),
                    std::borrow::Cow::Borrowed(include_bytes!(
                        "../../assets/fonts/Inter-SemiBold.ttf"
                    )),
                    std::borrow::Cow::Borrowed(include_bytes!(
                        "../../assets/fonts/Manrope-Bold.ttf"
                    )),
                    std::borrow::Cow::Borrowed(include_bytes!(
                        "../../assets/fonts/Manrope-SemiBold.ttf"
                    )),
                ])
                .expect("Embedded fonts should be valid");
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let bounds = Bounds::centered(None, window_size, cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(800.), px(600.))),
                    titlebar: Some(TitlebarOptions {
                        title: Some(
                            if interaction.is_some() {
                                "TerraMars Pointer Test"
                            } else {
                                "TerraMars"
                            }
                            .into(),
                        ),
                        ..Default::default()
                    }),
                    app_id: Some("org.terramars.app".into()),
                    icon: Some(app_icon.clone()),
                    ..Default::default()
                },
                |window, cx| {
                    let view = cx.new(|cx| TerraMars::new(sites, images, store, cx));
                    let focus = view.read(cx).focus.clone();
                    window.focus(&focus, cx);
                    if smoke.is_some() {
                        view.update(cx, |view, cx| view.smoke_driver(window, cx));
                    }
                    cx.on_app_quit(|_| {
                        let _ = performance::finish();
                        async {}
                    })
                    .detach();
                    view
                },
            )
            .expect("Could not open a GPU-accelerated desktop window");
            cx.activate(true);
        });
    Ok(())
}

fn observation_text(
    units: UnitSystem,
    o: &Observation,
    predicted: &std::collections::BTreeMap<&str, domain::climate::Prediction>,
) -> String {
    if o.key == "forecast" {
        return "Regional climate · Predicted".into();
    }
    if o.available() {
        return units.observation(o.value, &o.unit);
    }
    if let Some(p) = predicted.get(o.key.as_str()) {
        return format!("{} · Predicted", units.observation(Some(p.value), p.unit));
    }
    "Not modeled".into()
}
