use super::{
    animation::{self, Fade},
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
    status_open: bool,
    status_pinned: bool,
    status_version: u64,
    status_task: Option<Task<()>>,
    refreshing: bool,
    refresh_notes: Vec<String>,
    refresh_expanded: bool,
    /// Every surface that can appear or disappear fades through one shared
    /// animation vocabulary. `sync_fades` keeps this in step with the fields
    /// above at the start of each render.
    fades: Fades,
    endpoint_last: Endpoint,
    map_pick_last: Endpoint,
    log_preview_open: bool,
    log_danger_last: Option<(usize, domain::Point)>,
    routes_last: Vec<RouteEstimate>,
    last_layers: bool,
    focus: FocusHandle,
    transition: u64,
    page_transition: u64,
    rendered_page: Page,
    planning: bool,
    planning_version: u64,
    replan_task: Option<Task<()>>,
    /// Height of the open planner body as measured in the last frame it was
    /// open. Collapsing animates from this real height instead of the panel's
    /// maximum, so the whole shrink happens at a steady pace.
    planner_height: std::rc::Rc<std::cell::Cell<f32>>,
    /// Height frozen when the planner starts opening or closing, plus the fade
    /// generation that froze it, so the animation has one stable endpoint.
    planner_from: std::cell::Cell<f32>,
    planner_gen: std::cell::Cell<u64>,
}

struct Fades {
    details: Fade,
    endpoint: Fade,
    map_pick: Fade,
    planner: Fade,
    notes: Fade,
    preview: Fade,
    danger: Fade,
    toast: Fade,
    routes: Fade,
    tab: Fade,
}

impl Fades {
    fn new() -> Self {
        Self {
            details: Fade::new(false),
            endpoint: Fade::new(false),
            map_pick: Fade::new(false),
            planner: Fade::new(false),
            notes: Fade::new(false),
            preview: Fade::new(false),
            danger: Fade::new(false),
            toast: Fade::new(false),
            routes: Fade::new(false),
            tab: Fade::new(true),
        }
    }
}

struct Panorama {
    image: Arc<RenderImage>,
}
impl Render for Panorama {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let _timing = performance::scope("panorama_render_callback");
        let height = f32::from(window.viewport_size().height).max(1.);
        // The asset already contains two identical, seamlessly crossfaded loops
        // (3320 × 342), so a single image draws the whole background. Sliding it
        // by exactly one loop and restarting is invisible.
        let loop_len = (height * (1660. / 342.)).max(f32::from(window.viewport_size().width));
        let layer = div()
            .absolute()
            .top_0()
            .h_full()
            .w(px(loop_len * 2.))
            .child(img(self.image.clone()).size_full());
        let layer = layer.with_animation(
            "panorama",
            Animation::new(Duration::from_secs(160)).repeat(),
            move |e, t| e.left(px(-loop_len * t)),
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
            status_open: false,
            status_pinned: false,
            status_version: 0,
            status_task: None,
            refreshing: false,
            refresh_notes: vec![],
            refresh_expanded: false,
            fades: Fades::new(),
            endpoint_last: Endpoint::Start,
            map_pick_last: Endpoint::Start,
            log_preview_open: false,
            log_danger_last: None,
            routes_last: vec![],
            last_layers: false,
            focus: cx.focus_handle(),
            transition: 0,
            page_transition: 0,
            rendered_page: Page::Home,
            planning: false,
            planning_version: 0,
            replan_task: None,
            planner_height: std::rc::Rc::new(std::cell::Cell::new(0.)),
            planner_from: std::cell::Cell::new(0.),
            planner_gen: std::cell::Cell::new(0),
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
                            this.plan(true, cx);
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
                            this.dismiss_status(cx);
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
        self.status_open = true;
        self.status_pinned = false;
        self.status_version += 1;
        let version = self.status_version;
        // Notifications leave after five seconds without interaction. Any
        // pointer or keyboard attention pins the toast until it is closed.
        self.status_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(5)).await;
            let _ = this.update(cx, |this, cx| {
                if this.status_version == version && !this.status_pinned {
                    this.dismiss_status(cx);
                }
            });
        }));
        cx.notify();
    }
    fn dismiss_status(&mut self, cx: &mut Context<Self>) {
        if !self.status_open {
            return;
        }
        self.status_open = false;
        self.refresh_expanded = false;
        // Dropping the handle cancels the inactivity timer.
        self.status_task = None;
        cx.notify();
    }
    fn pin_status(&mut self, cx: &mut Context<Self>) {
        if self.status_open && !self.status_pinned {
            self.status_pinned = true;
            self.status_task = None;
            cx.notify();
        }
    }
    fn sync_fades(&self, cx: &mut Context<Self>) {
        self.fades.details.sync(self.details, cx);
        self.fades.endpoint.sync(self.endpoint_menu.is_some(), cx);
        self.fades.map_pick.sync(self.map_pick.is_some(), cx);
        self.fades.planner.sync(self.planner, cx);
        self.fades
            .notes
            .sync(self.refresh_expanded && self.status_open, cx);
        self.fades.preview.sync(self.log_preview_open, cx);
        self.fades
            .danger
            .sync(self.log_preview_open && self.log_danger.is_some(), cx);
        self.fades.toast.sync(self.status_open, cx);
        self.fades.routes.sync(!self.routes.is_empty(), cx);
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
        self.replan_task = None;
        self.dismiss_status(cx);
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
    fn plan(&mut self, record: bool, cx: &mut Context<Self>) {
        let (Some(start), Some(end)) = (&self.start, &self.end) else {
            self.set_status("Choose a starting point and a destination.", cx);
            cx.notify();
            return;
        };
        if self.planning {
            // A newer mode/speed/endpoint choice supersedes the in-flight
            // calculation; its result is ignored when it arrives.
            self.planning_version += 1;
            self.planning = false;
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
                        // The planner always returns the terrain-screened direct
                        // route first and the recorded rover corridor second.
                        // Each travel mode selects its own geometry: Rover
                        // follows the published traverse, on foot takes the
                        // direct screened path.
                        let preferred = if mode == "Rover" {
                            routes.iter().position(|r| r.uses_observed_traverse)
                        } else {
                            routes.iter().position(|r| !r.uses_observed_traverse)
                        };
                        this.route_index = preferred.unwrap_or(0);
                        this.routes = routes;
                        // Only an explicit "Plan route" adds a mission-log
                        // entry. Mode, speed and endpoint recalculations update
                        // the live route without spamming duplicate history.
                        if record {
                            this.record_route(cx);
                        }
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

    /// Recalculates an existing route after the transport mode or assumed
    /// speed changes, so the planner never needs a second "Plan route" click.
    /// A short delay coalesces repeated speed taps into one calculation.
    fn replan_if_ready(&mut self, cx: &mut Context<Self>) {
        if self.start.is_none() || self.end.is_none() {
            self.routes.clear();
            self.sync_map(cx);
            return;
        }
        self.replan_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(220))
                .await;
            let _ = this.update(cx, |this, cx| this.plan(false, cx));
        }));
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
                Ok(true) => self.set_status("Route planned and added to Mission log.", cx),
                Ok(false) => {
                    self.set_status("Route already in Mission log; it was not added again.", cx)
                }
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
                    self.log_preview_open = false;
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
        // The header floats HEADER_INSET from every window edge, so its own
        // width is the window minus both side margins.
        let header_width = (f32::from(window.viewport_size().width) - 2. * HEADER_INSET).max(320.);
        let narrow = header_width < 1250.;
        let show_brand = header_width >= 1150.;
        div()
            .relative()
            .occlude()
            .w_full()
            .flex()
            .items_center()
            .justify_between()
            .gap(SPACE)
            .px_3()
            .h(px(HEADER_HEIGHT))
            // Internal padding matches HEADER_INSET on every side.
            .pb_3()
            .flex_shrink_0()
            .rounded(px(14.))
            .backdrop_blur(BLUR)
            // Warm, translucent glass so the header blurs the scene like the
            // panels below it while picking up a faint orange cast.
            .bg(rgba(0x2a17109a))
            .border_1()
            .border_color(rgba(0xff945e38))
            .shadow_lg()
            .when(show_brand, |header| {
                header.child(
                    div()
                        .absolute()
                        .left_3()
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
                        HEADER_INSET
                    }))
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(!show_brand, |d| d.justify_start())
                    .gap(SPACE)
                    .child(
                        nav("home", "Home", self.page != Page::Log).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.page = this.browse_page;
                                if let Some(map) = &this.map {
                                    map.update(cx, |_, cx| cx.notify());
                                }
                                this.dismiss_status(cx);
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
                                this.dismiss_status(cx);
                                cx.notify();
                            }),
                        ),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .right_3()
                    .top_0()
                    .h_full()
                    .flex()
                    .items_center()
                    .gap(SPACE)
                    .child(
                        div()
                            .flex()
                            .p(px(2.))
                            .rounded(px(10.))
                            .bg(rgba(0xffffff08))
                            .border_1()
                            .border_color(rgba(0xffffff1f))
                            .occlude()
                            .child(
                                segment(
                                    "units-metric",
                                    "metric",
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
                                    "scientific",
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
                        // Microphone: voice control is not available yet, so the
                        // button explains itself instead of starting a session.
                        button("voice-toggle", "")
                            .w(px(44.))
                            .h(px(44.))
                            .p_0()
                            .flex_shrink_0()
                            .child(icon("mic").text_color(rgb(INK)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_status("Voice functionality coming soon", cx)
                            })),
                    )
                    .child(
                        button("refresh", if self.refreshing { "…" } else { "" })
                            .px_2()
                            .child(icon("refresh"))
                            .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                    )
                    .child(self.clock.clone()),
            )
            .into_any_element()
    }
    fn home(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let width = f32::from(window.viewport_size().width);
        // Three cards share one row. The title scales with the card so region
        // names stay on a single line at the default window size.
        let card_w = ((width - 128.) / 3.).clamp(210., 380.);
        let card_h = (f32::from(window.viewport_size().height) - 180.).clamp(240., 370.);
        let title_size = (card_w * 0.088).clamp(18., 34.);
        let arrow = || {
            div()
                .size(px(38.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(10.))
                .border_1()
                .border_color(rgba(0xffffff66))
                .text_size(px(18.))
                .child("↗")
        };
        let mut cards = div()
            .relative()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .gap(SPACE_LOOSE);
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
                .rounded(px(10.))
                .hover(|s| s.border_color(rgb(ORANGE)))
                .on_click(cx.listener(move |this, _, _, cx| this.open_site(i, cx)))
                .child(
                    img(self.images.cards[i].clone())
                        .absolute()
                        .size_full()
                        .object_fit(ObjectFit::Fill)
                        .rounded(px(10.)),
                )
                .child(
                    div()
                        .relative()
                        .size_full()
                        .flex()
                        .flex_col()
                        .justify_between()
                        .p_6()
                        .child(heading(*name, title_size))
                        .child(div().flex().justify_end().child(arrow())),
                );
            cards = cards.child(card);
        }
        // A third, non-interactive card previews the next region. The image is
        // NASA's public-domain Viking 1 global mosaic of Valles Marineris.
        cards = cards.child(
            div()
                .occlude()
                .border_1()
                .border_color(rgba(0xffffff2e))
                .shadow_lg()
                .w(px(card_w))
                .h(px(card_h))
                .relative()
                .overflow_hidden()
                .rounded(px(10.))
                .child(
                    img(self.images.coming_soon.clone())
                        .absolute()
                        .size_full()
                        .object_fit(ObjectFit::Fill)
                        .rounded(px(10.)),
                )
                .child(
                    div()
                        .relative()
                        .size_full()
                        .flex()
                        .flex_col()
                        .justify_between()
                        .p_6()
                        .child(heading("Coming Soon…", title_size * 0.9))
                        .child(
                            div()
                                .flex()
                                .items_end()
                                .justify_between()
                                .gap_2()
                                .child(label("Valles Marineris").text_size(px(13.))),
                        ),
                ),
        );
        div()
            .relative()
            .flex_1()
            .w_full()
            .overflow_hidden()
            .child(div().absolute().size_full().child(self.panorama.clone()))
            .child(animation::fade_in("home-enter", cards, self.transition))
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
                div()
                    .absolute()
                    .top(CONTENT_TOP)
                    .left_3()
                    .flex()
                    .flex_col()
                    .items_start()
                    .gap(SPACE)
                    .child(
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
                                                this.dismiss_status(cx);
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
                    )
                    // One SPACE between the region card and the icon toolbar,
                    // matching every other stacked group in the app.
                    .child(
                        div()
                            .flex()
                            .when(compact_controls, |d| d.flex_row())
                            .when(!compact_controls, |d| d.flex_col())
                            .items_start()
                            .gap(SPACE_TIGHT)
                            .child(map_button("zoom-in", "+").on_click(cx.listener(
                                |this, _, _, cx| {
                                    if let Some(m) = &this.map {
                                        m.update(cx, |m, cx| {
                                            m.zoom(1.35);
                                            cx.notify();
                                        });
                                    }
                                },
                            )))
                            .child(map_button("zoom-out", "−").on_click(cx.listener(
                                |this, _, _, cx| {
                                    if let Some(m) = &this.map {
                                        m.update(cx, |m, cx| {
                                            m.zoom(1. / 1.35);
                                            cx.notify();
                                        });
                                    }
                                },
                            )))
                            .child(
                                // One control for both jobs: frame the planned
                                // route when there is one, otherwise reset the
                                // map to its full extent.
                                map_button(
                                    "fit-map",
                                    icon(if self.routes.is_empty() {
                                        "fit"
                                    } else {
                                        "route"
                                    }),
                                )
                                .on_click(cx.listener(
                                    |this, _, _, cx| {
                                        let route = this
                                            .routes
                                            .get(this.route_index)
                                            .map(|r| r.points.clone());
                                        if let Some(m) = &this.map {
                                            m.update(cx, |m, cx| {
                                                match &route {
                                                    Some(points) => m.fit_route(points),
                                                    None => m.fit(),
                                                }
                                                cx.notify();
                                            });
                                        }
                                    },
                                )),
                            ),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .top(CONTENT_TOP)
                    .right_3()
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
            .child(self.planner_view(width, f32::from(window.viewport_size().height), cx));
        if self.fades.details.present() {
            view = view.child(
                self.fades
                    .details
                    .element("details-panel", self.details_view(window, cx)),
            );
        }
        if self.fades.endpoint.present() {
            let endpoint = self.endpoint_menu.unwrap_or(self.endpoint_last);
            view = view.child(self.fades.endpoint.element(
                "endpoint-picker",
                self.endpoint_picker(
                    endpoint,
                    width,
                    f32::from(window.viewport_size().height),
                    cx,
                ),
            ));
        }
        if self.fades.map_pick.present() {
            let endpoint = self.map_pick.unwrap_or(self.map_pick_last);
            let hint = div().absolute().top(CONTENT_TOP).left_3().child(
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
                    .child(
                        button("cancel-map-pick", "Cancel")
                            .mt_2()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.map_pick = None;
                                cx.notify();
                            })),
                    ),
            );
            view = view.child(self.fades.map_pick.element("map-pick-hint", hint));
        }
        view.into_any_element()
    }

    /// The one-line route summary. Empty until a route exists, so the planner
    /// reserves that line without showing placeholder instructions.
    fn route_summary(&self) -> String {
        let Some(route) = self.routes.get(self.route_index) else {
            return String::new();
        };
        format!(
            "{}  ·  {}  ·  {}  ·  {}",
            self.mode,
            self.units().distance_km(route.distance_km),
            UnitSystem::duration(route.duration_seconds),
            route.name
        )
    }
    fn planner_view(&self, width: f32, height: f32, cx: &mut Context<Self>) -> AnyElement {
        // A fixed width keeps the planner from jumping when the Info panel
        // opens or closes, so that fade stays visually continuous.
        let w = (width - 410.).clamp(300., 820.);
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
                        // Reserved summary line: blank until a route exists, so
                        // the card keeps one stable height and the planner never
                        // jumps between its empty and planned states.
                        .child(
                            div().h(px(21.)).flex().items_center().child(
                                label(self.route_summary())
                                    .text_color(rgb(INK))
                                    .text_size(px(14.)),
                            ),
                        ),
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
            // Bottom-align the row so the swap control lines up with the two
            // endpoint buttons rather than with the midpoint of their labels.
            let mut endpoints = div().flex().items_end().gap_2();
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
                    endpoints = endpoints.child(button("swap", "⇄").px_2().on_click(cx.listener(
                        |this, _, _, cx| {
                            std::mem::swap(&mut this.start, &mut this.end);
                            this.routes.clear();
                            this.sync_map(cx);
                            cx.notify();
                        },
                    )));
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
                                        this.replan_if_ready(cx);
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(button("speed-less", "−").px_3().on_click(cx.listener(
                                |this, _, _, cx| {
                                    this.store.state.settings.speed_kmh =
                                        (this.store.state.settings.speed_kmh - 0.5).max(0.5);
                                    this.save(cx);
                                    this.replan_if_ready(cx);
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
                                    this.save(cx);
                                    this.replan_if_ready(cx);
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
                                    this.replan_task = None;
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
                                .on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.replan_task = None;
                                        // Pressing "Plan route" again while the
                                        // same calculation is in flight would
                                        // append a second identical mission.
                                        if !this.planning {
                                            this.plan(true, cx);
                                        }
                                    },
                                )),
                            ),
                    ),
            );
            if self.fades.routes.present() {}
        }
        div()
            .absolute()
            .left(px(left.max(HEADER_INSET)))
            .bottom_3()
            .child(animation::fade_in(
                "planner-enter",
                glass()
                    .occlude()
                    .p(INSET)
                    .w(px(w))
                    .max_h(px((height - 284.).max(180.)))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(title)
                    .when(self.fades.planner.present(), |card| {
                        let open = self.fades.planner.visible();
                        let generation = self.fades.planner.generation();
                        let full = (height - 410.).max(130.);
                        // Freeze the starting height once per transition. The
                        // measured height is refreshed only while fully open, so
                        // reading it live inside the animation would make the
                        // endpoint drift and clip the panel.
                        if self.planner_gen.get() != generation {
                            self.planner_gen.set(generation);
                            self.planner_from.set(self.planner_height.get());
                        }
                        let stored = self.planner_from.get();
                        let from = if stored > 1. { stored } else { full };
                        // The card is as tall as its content, not as tall as its
                        // maximum. Collapse from the height actually measured
                        // while open so every frame changes the size, instead of
                        // idling under `max_h` and then snapping at the end.
                        let measured = self.planner_height.clone();
                        let clip = div()
                            .id("planner-clip")
                            .min_h_0()
                            .overflow_hidden()
                            .when(open, |clip| {
                                let measured = measured.clone();
                                clip.on_prepaint(move |payload, _, _| {
                                    let h = f32::from(payload.bounds.size.height);
                                    if h > 0. {
                                        measured.set(h);
                                    }
                                })
                            })
                            .with_animation(
                                ElementId::named_usize("planner-collapse", generation as usize),
                                Animation::new(animation::COLLAPSE)
                                    .with_easing(animation::ease_in_out),
                                move |clip, t| {
                                    if open {
                                        // Opening only fades: the content takes
                                        // its real height at once, so layout and
                                        // input never lag behind the fade.
                                        clip.max_h(px(full)).opacity(t)
                                    } else {
                                        // Closing animates the measured height,
                                        // so the shrink is continuous.
                                        clip.max_h(px(from * (1. - t))).opacity(1. - t)
                                    }
                                },
                            )
                            .child(
                                div()
                                    .id("planner-scroll")
                                    .occlude()
                                    .min_h_0()
                                    .overflow_y_scroll()
                                    .child(pane),
                            );
                        card.child(clip)
                    }),
                0,
            ))
            .into_any_element()
    }

    fn endpoint_picker(
        &self,
        endpoint: Endpoint,
        width: f32,
        height: f32,
        cx: &mut Context<Self>,
    ) -> Div {
        // The chooser floats above everything else, so it can use most of the
        // window: a wider, taller list shows many more candidate locations.
        let w = (width * 0.62).clamp(460., 820.);
        let mut menu = solid_panel()
            .w(px(w))
            .p(INSET)
            .flex()
            .flex_col()
            .gap(SPACE)
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
            // The chooser is the front-most surface, so the list gets most of
            // its height: more candidate locations are visible at once.
            .max_h(px((height * 0.45).clamp(180., 460.)))
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
        let bottom = 12.;
        div().absolute().left_3().bottom(px(bottom)).child(
            div()
                .id("endpoint-popup-scroll")
                .occlude()
                .max_h(px((height - bottom - 12.).max(240.)))
                .overflow_y_scroll()
                .rounded(px(10.))
                .child(menu.flex_shrink_0()),
        )
    }

    fn details_view(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        let panel = glass()
            .occlude()
            .w(px(350.))
            .h_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(
                div()
                    .p(INSET)
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
            .child(self.fades.tab.animate_in(
                "details-content",
                if self.layers {
                    self.layers_content(cx)
                } else {
                    self.conditions_content(cx)
                },
            ));
        let top = f32::from(CONTENT_TOP) + 56.;
        let bottom = 98.;
        let height = f32::from(window.viewport_size().height) - top - bottom;
        div()
            .absolute()
            .right_3()
            .top(px(top))
            .h(px(height.max(150.)))
            .child(panel)
    }

    fn conditions_content(&self, cx: &mut Context<Self>) -> Stateful<Div> {
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
            .px(INSET)
            .pb(INSET)
            .flex()
            .flex_col()
            .gap_3();
        let grid = &self.images.hazards[self.site_index];
        if let Some((zone, slope)) = grid.danger(point) {
            content=content.child(div().id("danger-explanation").p_3().rounded(px(10.)).border_1().border_color(rgba(0xff694580)).bg(rgba(0x481c1c55)).flex().flex_col().gap_2()
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
            // One rule between each measured field so values read as separate
            // categories instead of one run-on block.
            .gap_3()
            .child(heading("Regional weather", 20.))
            .child(label(
                "MCD 6.2 · seasonal climate model, not live measurements",
            ));
        let mut first_field = true;
        for key in [
            "air_temperature",
            "surface_temperature",
            "wind_speed",
            "wind_direction",
            "dust_tau",
            "solar_irradiance",
        ] {
            if let Some(p) = predicted.get(key) {
                if !first_field {
                    forecast = forecast.child(rule());
                }
                first_field = false;
                forecast = forecast.child(
                    div()
                        .flex()
                        .justify_between()
                        .gap(SPACE)
                        .py_2()
                        .child(label(p.label).w(px(112.)).flex_shrink_0())
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .text_right()
                                .text_color(rgb(INK))
                                .text_size(px(14.))
                                .child(units.observation(Some(p.value), p.unit)),
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
                .rounded(px(10.))
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
                        .rounded(px(10.))
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
        content
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
    fn layers_content(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let Some(map) = &self.map else {
            return div().id("layers-scroll");
        };
        let options = &map.read(cx).options;
        let mut content = div()
            .id("layers-scroll")
            .occlude()
            .overflow_y_scroll()
            .flex_1()
            .px(INSET)
            .pb(INSET)
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
                    .child(switch_visual(id as usize, enabled))
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
            .child(source_button("imagery-credit","NASA map source ↗",self.sites[self.site_index].map_source_url.clone()))
    }

    fn log(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut content = div()
            .id("mission-log-scroll")
            .overflow_y_scroll()
            .size_full()
            .p_8()
            .pt(CONTENT_TOP)
            .flex()
            .flex_col()
            .gap(SPACE_LOOSE)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(heading("Mission log", 32.))
                    .when(!self.store.state.planned_routes.is_empty(), |row| {
                        row.child(button("clear-log", "Clear").flex_shrink_0().on_click(
                            cx.listener(|this, _, _, cx| {
                                match this.store.clear_routes() {
                                    Ok(()) => {
                                        this.log_preview_open = false;
                                        this.set_status("Mission log cleared.", cx);
                                    }
                                    Err(error) => this.set_status(error.to_string(), cx),
                                }
                                cx.notify();
                            }),
                        ))
                    }),
            );
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
                    .id(("open-log-route", i))
                    // Pointer-test diagnostics address this card by id.
                    .child(super::interaction::probe(format!(
                        "NamedInteger(\"open-log-route\", {i})"
                    )))
                    .flex_shrink_0()
                    .p_5()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let r = &record_for_open;
                        let map = cx.new(|_| {
                            let mut map =
                                MapView::new(this.sites[index].clone(), index, this.images.clone());
                            map.units = this.units();
                            map.route = Arc::new(r.route.points.clone());
                            map.start = r.route.points.first().copied();
                            map.end = r.route.points.last().copied();
                            map.fit_route(&r.route.points);
                            map
                        });
                        cx.subscribe(&map, move |this, _, event: &MapSelection, cx| {
                            this.log_danger = this.images.hazards[index]
                                .danger(event.point)
                                .map(|_| (index, event.point));
                            cx.notify();
                        })
                        .detach();
                        this.log_danger = None;
                        this.log_preview = Some(map);
                        this.log_preview_index = Some(i);
                        this.log_preview_open = true;
                        cx.notify();
                    }))
                    .child(
                        div().flex().justify_between().items_center().gap_3().child(
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
        // Keep the transient-surface state and the retained snapshots that let
        // a surface finish its fade-out after its logical data is cleared.
        if let Some(endpoint) = self.endpoint_menu {
            self.endpoint_last = endpoint;
        }
        if let Some(endpoint) = self.map_pick {
            self.map_pick_last = endpoint;
        }
        if let Some(danger) = self.log_danger {
            self.log_danger_last = Some(danger);
        }
        if !self.routes.is_empty() {
            self.routes_last = self.routes.clone();
        }
        if self.last_layers != self.layers {
            self.last_layers = self.layers;
            self.fades.tab.restart();
        }
        self.sync_fades(cx);
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
            .font_family("Departure Mono")
            .text_size(px(18.))
            .on_key_down(cx.listener(|this, event, window, cx| this.keyboard(event, window, cx)))
            .relative()
            .child(animation::fade_in(
                "page-enter",
                div()
                    .relative()
                    .size_full()
                    .min_h_0()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .child(body),
                self.page_transition,
            ))
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .w_full()
                    .p_3()
                    .child(self.header(window, cx)),
            );
        if self.page == Page::Log
            && self.fades.preview.present()
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
            let fit_points = record.map(|r| r.route.points.clone());
            let modal = div()
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
                                    .p(INSET)
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap(SPACE)
                                    .child(
                                        div()
                                            .min_w_0()
                                            .flex_1()
                                            .flex()
                                            .flex_col()
                                            .gap(SPACE_TIGHT)
                                            .child(heading("Saved route", 22.))
                                            .child(label(title))
                                            .child(label(summary).text_size(px(14.))),
                                    )
                                    .child(
                                        button("close-log-preview", "Close").child("×").on_click(
                                            cx.listener(|this, _, _, cx| {
                                                // Keep the map and record alive so the
                                                // modal can fade out before it is reused.
                                                this.log_preview_open = false;
                                                cx.notify();
                                            }),
                                        ),
                                    ),
                            )
                            .child(
                                div().flex_1().w_full().px(INSET).pb(INSET).child(
                                div()
                                    .relative()
                                    .size_full()
                                    .rounded(px(10.))
                                    .overflow_hidden()
                                    .child(preview.clone())
                                    // Same camera toolbar as the Explorer map:
                                    // zoom in/out and refit the saved route.
                                    .child(
                                        div()
                                            .absolute()
                                            .left_3()
                                            .bottom_3()
                                            .flex()
                                            .gap(SPACE_TIGHT)
                                            .child(map_button("log-zoom-in", "+").on_click({
                                                let preview = preview.clone();
                                                cx.listener(move |_, _, _, cx| {
                                                    preview.update(cx, |m, cx| {
                                                        m.zoom(1.35);
                                                        cx.notify();
                                                    });
                                                })
                                            }))
                                            .child(map_button("log-zoom-out", "−").on_click({
                                                let preview = preview.clone();
                                                cx.listener(move |_, _, _, cx| {
                                                    preview.update(cx, |m, cx| {
                                                        m.zoom(1. / 1.35);
                                                        cx.notify();
                                                    });
                                                })
                                            }))
                                            .child(
                                                map_button("log-fit-map", icon("route")).on_click({
                                                    let preview = preview.clone();
                                                    let points = fit_points.clone();
                                                    cx.listener(move |_, _, _, cx| {
                                                        preview.update(cx, |m, cx| {
                                                            match &points {
                                                                Some(points) => {
                                                                    m.fit_route(points)
                                                                }
                                                                None => m.fit(),
                                                            }
                                                            cx.notify();
                                                        });
                                                    })
                                                }),
                                            ),
                                    )
                                    .when(self.fades.danger.present(), |view| {
                                        let Some((index, p)) =
                                            self.log_danger.or(self.log_danger_last)
                                        else {
                                            return view;
                                        };
                                        let grid = &self.images.hazards[index];
                                        let Some((_, slope)) = grid.danger(p) else {
                                            return view;
                                        };
                                        let popover = div().absolute().top_3().left_3().child(glass().id("log-danger-scroll").occlude().w(px((width-48.).min(330.))).max_h(px((height-180.).max(140.))).overflow_y_scroll().p_4().flex().flex_col().gap_2()
                                            .child(div().flex().items_center().justify_between().gap_2().child(heading(format!("Steep terrain · {:.1}°",slope),18.)).child(button("close-log-danger","×").px_2().on_click(cx.listener(|this,_,_,cx|{this.log_danger=None;cx.notify();}))))
                                            .child(label("Slipping / rollover risk. Terrain-aware plans exclude slopes ≥15° at MOLA’s ~463 m scale. Small obstacles and sand traps remain unassessed."))
                                            .child(source_button("log-danger-source","NASA MOLA source ↗",grid.metadata.source_url.clone())));
                                        view.child(self.fades.danger.element("log-danger", popover))
                                    }),
                                ),
                            ),
                    );
            app = app.child(self.fades.preview.element("log-preview", modal));
        }
        if self.fades.toast.present() {
            let mut toast = notice_panel()
                .id("status-toast")
                .occlude()
                .w(px((f32::from(window.viewport_size().width) - 32.).min(460.)))
                .p_4()
                .flex()
                .flex_col()
                .gap_3()
                // Any attention pins the notification; only the close control
                // removes it once pinned.
                .on_hover(cx.listener(|this, hovered, _, cx| {
                    if *hovered {
                        this.pin_status(cx);
                    }
                }))
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
                        .child(
                            button("dismiss-status", "×")
                                .px_3()
                                .on_click(cx.listener(|this, _, _, cx| this.dismiss_status(cx))),
                        ),
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
                        this.pin_status(cx);
                        this.refresh_expanded = !this.refresh_expanded;
                        cx.notify();
                    })),
                );
                if self.fades.notes.present() {
                    toast = toast.child(
                        self.fades.notes.element(
                            "refresh-notes",
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
                        ),
                    );
                }
            }
            let viewport = f32::from(window.viewport_size().width);
            let toast_width = if viewport < 1100. { 360. } else { 460. };
            app = app.child(
                div()
                    .absolute()
                    .top(CONTENT_TOP)
                    .left(px((viewport - toast_width) / 2.))
                    .child(
                        self.fades
                            .toast
                            .element("status-toast-fade", toast.w(px(toast_width))),
                    ),
            );
        }
        if super::interaction::enabled() {
            super::interaction::capture(serde_json::json!({
                "page":match self.page{Page::Home=>"home",Page::Explorer=>"explorer",Page::Log=>"log"},
                "log_preview":self.log_preview_open,"browse_page":if self.browse_page==Page::Explorer{"explorer"}else{"home"},"planner":self.planner,"details":self.details,"layers":self.layers,"endpoint_menu":self.endpoint_menu.map(|e|format!("{e:?}")),
                "selected":self.selection.as_ref().map(|l|l.point),"start":self.start.as_ref().map(|l|l.point),"end":self.end.as_ref().map(|l|l.point),
                "rover":self.sites[self.site_index].rover().map(|l|serde_json::json!({"lat":l.point.latitude,"lon":l.point.longitude,"sol":l.sol})),"danger_selected":self.selection.as_ref().and_then(|l|self.images.hazards[self.site_index].danger(l.point)).is_some(),"projected_time":self.route_summary(),"coordinate_text":self.coordinate_text,"coordinate_editing":self.coordinate_editing,"log_count":self.store.state.planned_routes.len(),"route_count":self.routes.len(),"units":self.units().label(),"earth":self.clock.read(cx).earth,
                "camera_center":self.map.as_ref().map(|m| {let v=m.read(cx);let c=v.camera.borrow();[c.x,c.y]}),"bearing":self.map.as_ref().map(|m|m.read(cx).camera.borrow().bearing),"zoom":self.map.as_ref().map(|m|m.read(cx).camera.borrow().zoom),"map_options":self.map.as_ref().map(|m|{
                    let o=&m.read(cx).options;serde_json::json!({"terrain":o.terrain,"rover":o.rover,"landmarks":o.landmarks,"traverse":o.traverse,"night":o.terminator,"dangers":o.dangers})
                }),"status":if self.status_open{self.status.clone()}else{String::new()},"refreshing":self.refreshing,"refresh_expanded":self.refresh_expanded,
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
        .unwrap_or_else(|| size(px(1200.), px(800.)));
    gpui_platform::application()
        .with_assets(super::assets::EmbeddedAssets)
        .run(move |cx: &mut App| {
            // Orbitron carries every heading; Departure Mono carries all body
            // copy, labels and controls. Sizes stay unchanged so the existing
            // layout rhythm is preserved.
            cx.text_system()
                .add_fonts(vec![
                    std::borrow::Cow::Borrowed(include_bytes!(
                        "../../assets/fonts/DepartureMono-1.500/DepartureMono-Regular.otf"
                    )),
                    std::borrow::Cow::Borrowed(include_bytes!(
                        "../../assets/fonts/Orbitron/static/Orbitron-Regular.ttf"
                    )),
                    std::borrow::Cow::Borrowed(include_bytes!(
                        "../../assets/fonts/Orbitron/static/Orbitron-Medium.ttf"
                    )),
                    std::borrow::Cow::Borrowed(include_bytes!(
                        "../../assets/fonts/Orbitron/static/Orbitron-SemiBold.ttf"
                    )),
                    std::borrow::Cow::Borrowed(include_bytes!(
                        "../../assets/fonts/Orbitron/static/Orbitron-Bold.ttf"
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
