use super::{assets::Images, style::*};
use gpui::{prelude::*, *};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};
use terramars::domain::{
    MapBounds, MarsClock, MarsTime, MissionSite, Point as GeoPoint, RoutePlanner, UnitSystem,
};

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub width: f32,
    pub height: f32,
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
    pub bearing: f32,
    pub origin_x: f32,
    pub origin_y: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            width: 0.,
            height: 0.,
            x: 0.5,
            y: 0.5,
            zoom: 1.,
            bearing: 0.,
            origin_x: 0.,
            origin_y: 0.,
        }
    }
}

impl Camera {
    fn compass_hit(self, p: Point<Pixels>) -> bool {
        let x = f32::from(p.x) - self.origin_x;
        let y = f32::from(p.y) - self.origin_y;
        x >= self.width - 80.
            && x <= self.width - 16.
            && y >= self.height - 80.
            && y <= self.height - 16.
    }
    fn edge(self) -> f32 {
        self.width.min(self.height) * self.zoom
    }
    fn local(self, x: f32, y: f32) -> (f32, f32) {
        let dx = x - self.origin_x - self.width / 2.;
        let dy = y - self.origin_y - self.height / 2.;
        let (sn, cs) = self.bearing.sin_cos();
        (
            self.x + (cs * dx - sn * dy) / self.edge(),
            self.y + (sn * dx + cs * dy) / self.edge(),
        )
    }
    fn screen(self, x: f32, y: f32) -> (f32, f32) {
        let (dx, dy) = ((x - self.x) * self.edge(), (y - self.y) * self.edge());
        let (sn, cs) = self.bearing.sin_cos();
        (
            self.origin_x + self.width / 2. + cs * dx + sn * dy,
            self.origin_y + self.height / 2. - sn * dx + cs * dy,
        )
    }
    fn pan(&mut self, dx: f32, dy: f32) {
        if !dx.is_finite() || !dy.is_finite() {
            return;
        }
        if self.edge() > 0. {
            let (sn, cs) = self.bearing.sin_cos();
            self.x -= (cs * dx - sn * dy) / self.edge();
            self.y -= (sn * dx + cs * dy) / self.edge();
        }
        self.clamp();
    }
    fn clamp(&mut self) {
        // Symmetric viewport-space bounds allow reversing at either image edge,
        // including when the raster is narrower than the window.
        let edge = self.edge().max(1.);
        let limit = |v: f32, viewport: f32| {
            let pad = (viewport * 0.2).min(edge * 0.45);
            let (lo, hi) = if edge < viewport {
                let center = (viewport - edge) / 2.;
                (center - pad, center + pad)
            } else {
                (viewport - edge - pad, pad)
            };
            let translation = viewport / 2. - v * edge;
            (viewport / 2. - translation.clamp(lo, hi)) / edge
        };
        let (sn, cs) = self.bearing.sin_cos();
        self.x = limit(self.x, cs.abs() * self.width + sn.abs() * self.height);
        self.y = limit(self.y, sn.abs() * self.width + cs.abs() * self.height);
    }
    fn rotate_at(&mut self, delta: f32, x: f32, y: f32) {
        if !delta.is_finite() || self.edge() <= 0. {
            return;
        }
        let (u, v) = self.local(x, y);
        self.bearing += delta;
        let (dx, dy) = (
            x - self.origin_x - self.width / 2.,
            y - self.origin_y - self.height / 2.,
        );
        let (sn, cs) = self.bearing.sin_cos();
        self.x = u - (cs * dx - sn * dy) / self.edge();
        self.y = v - (sn * dx + cs * dy) / self.edge();
        self.clamp();
    }
    fn zoom_at(&mut self, factor: f32, x: f32, y: f32) {
        if self.edge() <= 0. || !factor.is_finite() || factor <= 0. {
            return;
        }
        let (u, v) = self.local(x, y);
        self.zoom = (self.zoom * factor).clamp(0.5, 32.);
        let (dx, dy) = (
            x - self.origin_x - self.width / 2.,
            y - self.origin_y - self.height / 2.,
        );
        let (sn, cs) = self.bearing.sin_cos();
        self.x = u - (cs * dx - sn * dy) / self.edge();
        self.y = v - (sn * dx + cs * dy) / self.edge();
        self.clamp();
    }
}

const CAMERA_ANIMATION_DURATION: Duration = Duration::from_millis(170);

#[derive(Clone, Copy, Debug, PartialEq)]
struct CameraPose {
    x: f32,
    y: f32,
    zoom: f32,
    bearing: f32,
}

impl CameraPose {
    fn of(camera: Camera) -> Self {
        Self {
            x: camera.x,
            y: camera.y,
            zoom: camera.zoom,
            bearing: camera.bearing,
        }
    }
    fn apply(self, camera: &mut Camera) {
        camera.x = self.x;
        camera.y = self.y;
        camera.zoom = self.zoom;
        camera.bearing = self.bearing;
        camera.clamp();
    }
}

#[derive(Clone, Copy, Debug)]
struct CameraAnimation {
    from: CameraPose,
    to: CameraPose,
    started: Instant,
}

impl CameraAnimation {
    fn sample(self, now: Instant) -> (CameraPose, bool) {
        let elapsed = now.saturating_duration_since(self.started);
        if elapsed >= CAMERA_ANIMATION_DURATION {
            return (self.to, false);
        }
        let progress = elapsed.as_secs_f32() / CAMERA_ANIMATION_DURATION.as_secs_f32();
        let eased = 1.0 - (1.0 - progress).powi(3);
        (
            CameraPose {
                x: self.from.x + (self.to.x - self.from.x) * eased,
                y: self.from.y + (self.to.y - self.from.y) * eased,
                zoom: self.from.zoom + (self.to.zoom - self.from.zoom) * eased,
                bearing: self.from.bearing + (self.to.bearing - self.from.bearing) * eased,
            },
            true,
        )
    }
}

#[derive(Clone, Copy, Debug)]
struct NormalizedBounds {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

impl NormalizedBounds {
    fn for_route(points: &[GeoPoint], bounds: MapBounds) -> Option<Self> {
        let mut result: Option<Self> = None;
        for point in points.iter().filter(|point| point.validate().is_ok()) {
            let (x, y) = normalized(*point, bounds);
            if !x.is_finite() || !y.is_finite() {
                continue;
            }
            if let Some(result) = &mut result {
                result.left = result.left.min(x);
                result.top = result.top.min(y);
                result.right = result.right.max(x);
                result.bottom = result.bottom.max(y);
            } else {
                result = Some(Self {
                    left: x,
                    top: y,
                    right: x,
                    bottom: y,
                });
            }
        }
        result
    }

    fn fitted(self, mut camera: Camera) -> Camera {
        let base = camera.width.min(camera.height).max(1.0);
        let width = (self.right - self.left).max(0.00001);
        let height = (self.bottom - self.top).max(0.00001);
        let (sn, cs) = camera.bearing.sin_cos();
        camera.zoom = (0.9 * camera.width / (base * (cs.abs() * width + sn.abs() * height)))
            .min(0.9 * camera.height / (base * (sn.abs() * width + cs.abs() * height)))
            .clamp(0.5, 32.0);
        camera.x = (self.left + self.right) / 2.0;
        camera.y = (self.top + self.bottom) / 2.0;
        camera.clamp();
        camera
    }
}

pub struct MapSelection {
    pub point: GeoPoint,
    pub landmark: Option<usize>,
}

#[derive(Clone)]
pub struct MapOptions {
    pub terrain: bool,
    pub landmarks: bool,
    pub rover: bool,
    pub traverse: bool,
    pub terminator: bool,
    pub dangers: bool,
}

impl Default for MapOptions {
    fn default() -> Self {
        Self {
            terrain: false,
            landmarks: true,
            rover: true,
            traverse: true,
            terminator: false,
            dangers: true,
        }
    }
}

pub struct MapView {
    pub site: MissionSite,
    pub terrain_grid: Arc<terramars::domain::terrain::TerrainGrid>,
    images: Arc<Images>,
    index: usize,
    pub camera: Rc<RefCell<Camera>>,
    pub options: MapOptions,
    pub selected: Option<GeoPoint>,
    pub start: Option<GeoPoint>,
    pub end: Option<GeoPoint>,
    pub route: Arc<Vec<GeoPoint>>,
    pub reduced_motion: bool,
    pub units: UnitSystem,
    // Normalized geometry changes only when NASA's recorded traverse changes.
    traverse: Arc<Vec<(f32, f32)>>,
    /// Danger-run quads pre-normalized once so panning does not re-derive
    /// thousands of geographic corners every frame.
    danger_rects: Arc<Vec<[(f32, f32); 4]>>,
    drag: Option<(f32, f32, Camera)>,
    moved: bool,
    animation: Option<CameraAnimation>,
    pending_fit: Rc<RefCell<Option<NormalizedBounds>>>,
    /// The previous base raster, faded out when MOLA relief is toggled.
    terrain_fade_from: Option<Arc<RenderImage>>,
    terrain_fade_until: Option<Instant>,
    terrain_fade_generation: u64,
    last_terrain: bool,
}

impl EventEmitter<MapSelection> for MapView {}

fn normalized(p: GeoPoint, b: MapBounds) -> (f32, f32) {
    (
        ((p.longitude - b.west) / (b.east - b.west)) as f32,
        ((b.north - p.latitude) / (b.north - b.south)) as f32,
    )
}

impl MapView {
    pub fn new(site: MissionSite, index: usize, images: Arc<Images>) -> Self {
        let traverse = Arc::new(
            site.traverse
                .iter()
                .map(|&p| normalized(p, site.map_bounds))
                .collect(),
        );
        let terrain_grid = images.hazards[index].clone();
        let bounds = site.map_bounds;
        let danger_rects = Arc::new(
            terrain_grid
                .runs
                .iter()
                .map(|run| {
                    let a = terrain_grid.corner(run.first as f64, run.row as f64);
                    let z = terrain_grid.corner(run.end as f64, run.row as f64 + 1.);
                    [
                        a,
                        GeoPoint::new(a.latitude, z.longitude),
                        z,
                        GeoPoint::new(z.latitude, a.longitude),
                    ]
                    .map(|p| normalized(p, bounds))
                })
                .collect::<Vec<_>>(),
        );
        Self {
            terrain_grid,
            site,
            index,
            images,
            traverse,
            danger_rects,
            camera: Rc::new(RefCell::new(Camera::default())),
            options: MapOptions::default(),
            selected: None,
            start: None,
            end: None,
            route: Arc::new(Vec::new()),
            reduced_motion: false,
            units: UnitSystem::Metric,
            drag: None,
            moved: false,
            animation: None,
            pending_fit: Rc::new(RefCell::new(None)),
            terrain_fade_from: None,
            terrain_fade_until: None,
            terrain_fade_generation: 0,
            last_terrain: false,
        }
    }
    pub fn rotate_by(&mut self, delta: f32) {
        if delta.is_finite() {
            self.cancel_animation();
            let mut c = self.camera.borrow_mut();
            c.bearing += delta;
            c.clamp();
        }
    }
    pub fn next_bearing(&mut self) {
        self.advance_animation(Instant::now());
        let mut c = *self.camera.borrow();
        if let Some(animation) = self.animation {
            animation.to.apply(&mut c);
        }
        let target =
            ((c.bearing / std::f32::consts::FRAC_PI_2).floor() + 1.) * std::f32::consts::FRAC_PI_2;
        self.animate_to(Camera {
            bearing: target,
            ..c
        });
    }
    pub fn update_site(&mut self, site: MissionSite) {
        self.traverse = Arc::new(
            site.traverse
                .iter()
                .map(|&p| normalized(p, site.map_bounds))
                .collect(),
        );
        self.site = site;
    }
    /// Zooms about the centre of the viewport. Applied directly to the camera:
    /// the control can be used before the map has been laid out (for example in
    /// a preview opened from a snapshot), and an animated target built from a
    /// zero-sized viewport would silently do nothing.
    pub fn zoom(&mut self, factor: f32) {
        self.advance_animation(Instant::now());
        self.cancel_animation();
        let mut camera = *self.camera.borrow();
        let (x, y) = (camera.width / 2., camera.height / 2.);
        camera.zoom_at(factor, x, y);
        *self.camera.borrow_mut() = camera;
    }
    pub fn fit(&mut self) {
        let mut target = *self.camera.borrow();
        target.x = 0.5;
        target.y = 0.5;
        target.zoom = 1.;
        self.animate_to(target);
    }
    pub fn focus_rover(&mut self) {
        if let Some(l) = self.site.landmarks.iter().find(|l| l.kind == "rover") {
            let p = normalized(l.point, self.site.map_bounds);
            let mut target = *self.camera.borrow();
            target.x = p.0;
            target.y = p.1;
            target.zoom = 6.;
            target.clamp();
            self.animate_to(target);
        }
    }

    /// Fits route geometry without changing the inspected point or endpoints.
    pub fn fit_route(&mut self, points: &[GeoPoint]) {
        let Some(bounds) = NormalizedBounds::for_route(points, self.site.map_bounds) else {
            return;
        };
        let camera = *self.camera.borrow();
        if camera.width <= 0.0 || camera.height <= 0.0 {
            self.animation = None;
            *self.pending_fit.borrow_mut() = Some(bounds);
            return;
        }
        self.animate_to(bounds.fitted(camera));
    }

    fn animate_to(&mut self, mut target: Camera) {
        let now = Instant::now();
        self.advance_animation(now);
        self.drag = None;
        self.pending_fit.borrow_mut().take();
        target.clamp();
        let from = CameraPose::of(*self.camera.borrow());
        let to = CameraPose::of(target);
        let near = (from.x - to.x).abs()
            + (from.y - to.y).abs()
            + (from.zoom - to.zoom).abs()
            + (from.bearing - to.bearing).abs()
            < 0.00001;
        if self.reduced_motion || target.width <= 0.0 || target.height <= 0.0 || near {
            to.apply(&mut self.camera.borrow_mut());
            self.animation = None;
        } else {
            self.animation = Some(CameraAnimation {
                from,
                to,
                started: now,
            });
        }
    }

    /// Returns true only while another finite animation frame is needed.
    fn advance_animation(&mut self, now: Instant) -> bool {
        let Some(animation) = self.animation else {
            return false;
        };
        let (pose, running) = if self.reduced_motion {
            (animation.to, false)
        } else {
            animation.sample(now)
        };
        pose.apply(&mut self.camera.borrow_mut());
        if !running {
            self.animation = None;
        }
        running
    }

    fn cancel_animation(&mut self) {
        self.advance_animation(Instant::now());
        self.animation = None;
        self.pending_fit.borrow_mut().take();
    }
    fn select(&mut self, x: f32, y: f32, cx: &mut Context<Self>) {
        let camera = *self.camera.borrow();
        let (u, v) = camera.local(x, y);
        if !(0.0..=1.).contains(&u) || !(0.0..=1.).contains(&v) {
            return;
        }
        let landmark = self
            .site
            .landmarks
            .iter()
            .enumerate()
            .filter(|(_, l)| {
                if l.kind == "rover" {
                    self.options.rover
                } else {
                    self.options.landmarks
                }
            })
            .filter_map(|(i, l)| {
                let p = normalized(l.point, self.site.map_bounds);
                let s = camera.screen(p.0, p.1);
                let d = (s.0 - x).hypot(s.1 - y);
                (d < 18.).then_some((i, d))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|a| a.0);
        let point = landmark
            .map(|i| self.site.landmarks[i].point)
            .unwrap_or_else(|| {
                let b = self.site.map_bounds;
                GeoPoint::new(
                    b.north - v as f64 * (b.north - b.south),
                    b.west + u as f64 * (b.east - b.west),
                )
            });
        cx.emit(MapSelection { point, landmark });
    }
}

fn line(points: impl Iterator<Item = (f32, f32)>, color: u32, width: f32, window: &mut Window) {
    let mut builder = PathBuilder::stroke(px(width));
    let mut last: Option<(f32, f32)> = None;
    let mut n = 0;
    for (x, y) in points {
        // Subpixel vertices carry no extra visual information at the current zoom.
        if let Some((lx, ly)) = last
            && (x - lx).hypot(y - ly) < 0.7
        {
            continue;
        }
        if n == 0 {
            builder.move_to(point(px(x), px(y)));
        } else {
            builder.line_to(point(px(x), px(y)));
        }
        last = Some((x, y));
        n += 1;
    }
    if n > 1
        && let Ok(path) = builder.build()
    {
        window.paint_path(path, rgb(color));
    }
}

fn text(text: &str, x: f32, y: f32, color: u32, window: &mut Window, cx: &mut App) {
    if super::interaction::enabled() {
        super::interaction::painted(
            format!("map-label/{text}"),
            Bounds::new(point(px(x), px(y)), size(px(1.), px(18.))),
            serde_json::json!({"color":color}),
        );
    }
    let style = window.text_style();
    let run = TextRun {
        len: text.len(),
        font: style.font(),
        color: rgb_to_hsla(rgb(color)),
        background_color: None,
        underline: None,
        strikethrough: None,
        letter_spacing: None,
    };
    let line = window
        .text_system()
        .shape_line(text.to_owned().into(), px(15.), &[run]);
    let _ = line.paint(
        point(px(x), px(y)),
        px(18.),
        TextAlign::Left,
        None,
        window,
        cx,
    );
}

fn polygon(points: &[(f32, f32)], color: Rgba, window: &mut Window) {
    let mut builder = PathBuilder::fill();
    if let Some(&(x, y)) = points.first() {
        builder.move_to(point(px(x), px(y)));
        for &(x, y) in &points[1..] {
            builder.line_to(point(px(x), px(y)));
        }
        builder.close();
        if let Ok(path) = builder.build() {
            window.paint_path(path, color);
        }
    }
}

fn dot(x: f32, y: f32, r: f32, color: u32, window: &mut Window) {
    window.paint_quad(
        PaintQuad::new(Bounds::new(
            point(px(x - r), px(y - r)),
            size(px(r * 2.), px(r * 2.)),
        ))
        .corner_radii(px(r))
        .background(rgb(color))
        .border_widths(px(2.))
        .border_color(rgb(0x11121a)),
    );
}

/// Horizontal scale at the current camera-center latitude on NASA's Mars
/// sphere. Camera edge and the raster's real longitude bounds determine the
/// angular distance represented by these logical screen pixels.
fn kilometers_for_pixels(camera: Camera, bounds: MapBounds, pixels: f32) -> f64 {
    if camera.edge() <= 0.0 || pixels <= 0.0 {
        return 0.0;
    }
    let ratio = f64::from(pixels) / f64::from(camera.edge());
    let angle = f64::from(camera.bearing);
    let dlon = (bounds.east - bounds.west) * ratio * angle.cos();
    let dlat = -(bounds.north - bounds.south) * ratio * angle.sin();
    let lat = bounds.north - f64::from(camera.y) * (bounds.north - bounds.south);
    let lon = bounds.west + f64::from(camera.x) * (bounds.east - bounds.west);
    RoutePlanner::distance_km(
        GeoPoint::new(lat - dlat / 2., lon - dlon / 2.),
        GeoPoint::new(lat + dlat / 2., lon + dlon / 2.),
    )
}

/// Regional illumination reuses one orbital/time solution for every cell.
/// The only location-dependent terms are latitude and local hour angle.
struct SolarGeometry {
    prime_true_hours: f64,
    declination_sin: f64,
    declination_cos: f64,
}

impl SolarGeometry {
    fn from_snapshot(snapshot: &MarsTime) -> Self {
        Self {
            prime_true_hours: snapshot.local_true_hours,
            declination_sin: snapshot.solar_declination.to_radians().sin(),
            declination_cos: snapshot.solar_declination.to_radians().cos(),
        }
    }

    fn night_grid(&self, bounds: MapBounds) -> [[bool; 16]; 16] {
        let mut nights = [[false; 16]; 16];
        let longitude_cos: [f64; 16] = std::array::from_fn(|col| {
            let longitude = bounds.west + (col as f64 + 0.5) / 16.0 * (bounds.east - bounds.west);
            let local_true_hours = self.prime_true_hours + longitude / 15.0;
            ((local_true_hours - 12.0) * 15.0).to_radians().cos()
        });
        for (row, cells) in nights.iter_mut().enumerate() {
            let latitude = bounds.north - (row as f64 + 0.5) / 16.0 * (bounds.north - bounds.south);
            let graphic_latitude =
                (latitude.to_radians().tan() * (3396.2_f64 / 3376.2).powi(2)).atan();
            let sin_term = graphic_latitude.sin() * self.declination_sin;
            let cos_term = graphic_latitude.cos() * self.declination_cos;
            for (col, cell) in cells.iter_mut().enumerate() {
                // asin is monotonic: the sign of sin(elevation) determines
                // night/day without 256 inverse-trigonometric calls.
                *cell = sin_term + cos_term * longitude_cos[col] <= 0.0;
            }
        }
        nights
    }
}

impl Render for MapView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.advance_animation(Instant::now()) {
            window.request_animation_frame();
        }
        let camera = self.camera.clone();
        let pending_fit = self.pending_fit.clone();
        // Crossfade the base raster when MOLA relief is switched on or off.
        let now = Instant::now();
        if self.options.terrain != self.last_terrain {
            self.last_terrain = self.options.terrain;
            if cx.reduce_motion() {
                self.terrain_fade_from = None;
                self.terrain_fade_until = None;
            } else {
                self.terrain_fade_from = Some(if self.options.terrain {
                    self.images.imagery[self.index].clone()
                } else {
                    self.images.terrain[self.index].clone()
                });
                self.terrain_fade_until = Some(now + Duration::from_millis(220));
                self.terrain_fade_generation += 1;
            }
        }
        if self
            .terrain_fade_until
            .is_some_and(|deadline| now >= deadline)
        {
            self.terrain_fade_from = None;
            self.terrain_fade_until = None;
        }
        let image = if self.options.terrain {
            self.images.terrain[self.index].clone()
        } else {
            self.images.imagery[self.index].clone()
        };
        let fade_from = self.terrain_fade_from.clone().map(|old| {
            let camera = self.camera.clone();
            let generation = self.terrain_fade_generation;
            let bounds = div().absolute().inset_0();
            bounds
                .with_animation(
                    ElementId::named_usize("terrain-fade", generation as usize),
                    Animation::new(Duration::from_millis(240))
                        .with_easing(|t| 1. - (1. - t).powi(3)),
                    |layer, t| layer.opacity(1. - t),
                )
                .child(
                    canvas(
                        |_, _, _| {},
                        move |_, _, window, _| {
                            let c = *camera.borrow();
                            if c.width <= 0.0 || c.height <= 0.0 {
                                return;
                            }
                            let center = c.screen(0.5, 0.5);
                            let image_bounds = Bounds::new(
                                point(px(center.0 - c.edge() / 2.), px(center.1 - c.edge() / 2.)),
                                size(px(c.edge()), px(c.edge())),
                            );
                            let _ = window.paint_image_rotated(
                                image_bounds,
                                image_bounds,
                                Corners::all(px(0.)),
                                0.,
                                old,
                                0,
                                false,
                                -c.bearing,
                            );
                        },
                    )
                    .size_full(),
                )
        });
        let options = self.options.clone();
        let terrain_grid = self.terrain_grid.clone();
        let danger_rects = self.danger_rects.clone();
        let bounds = self.site.map_bounds;
        let landmarks = self.site.landmarks.clone();
        let traverse = self.traverse.clone();
        let route = self.route.clone();
        let selected = self.selected;
        let start = self.start;
        let end = self.end;
        let units = self.units;
        div()
            .id("mars-map")
            .size_full()
            .overflow_hidden()
            .bg(rgb(0x0b0d12))
            .cursor_grab()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, e: &MouseDownEvent, _, _| {
                    if this.camera.borrow().compass_hit(e.position) {
                        return;
                    }
                    this.cancel_animation();
                    this.drag = Some((
                        f32::from(e.position.x),
                        f32::from(e.position.y),
                        *this.camera.borrow(),
                    ));
                    this.moved = false;
                }),
            )
            .on_pinch(cx.listener(|this, e: &PinchEvent, _, cx| {
                this.cancel_animation();
                this.camera.borrow_mut().rotate_at(
                    -e.rotation,
                    f32::from(e.position.x),
                    f32::from(e.position.y),
                );
                if e.delta != 0. {
                    this.camera.borrow_mut().zoom_at(
                        (1. + e.delta).max(0.01),
                        f32::from(e.position.x),
                        f32::from(e.position.y),
                    );
                }
                cx.notify();
                cx.stop_propagation();
            }))
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
                if !e.dragging() {
                    this.drag = None;
                    return;
                }
                if let Some((x, y, c)) = this.drag {
                    let dx = f32::from(e.position.x) - x;
                    let dy = f32::from(e.position.y) - y;
                    if dx.hypot(dy) > 3. {
                        this.moved = true;
                    }
                    if this.moved {
                        let mut next = c;
                        next.pan(dx, dy);
                        *this.camera.borrow_mut() = next;
                        cx.notify();
                    }
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, e: &MouseUpEvent, _, cx| {
                    if this.camera.borrow().compass_hit(e.position) {
                        this.drag = None;
                        return;
                    }
                    if this.drag.take().is_some() && !this.moved {
                        this.select(f32::from(e.position.x), f32::from(e.position.y), cx);
                    }
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    this.drag = None;
                }),
            )
            .on_scroll_wheel(cx.listener(|this, e: &ScrollWheelEvent, _, cx| {
                if this.camera.borrow().compass_hit(e.position) {
                    cx.stop_propagation();
                    return;
                }
                this.cancel_animation();
                let delta = e.delta.pixel_delta(px(30.));
                if e.modifiers.alt {
                    this.rotate_by(f32::from(delta.x + delta.y) * 0.006);
                    cx.notify();
                    cx.stop_propagation();
                    return;
                }
                this.camera.borrow_mut().zoom_at(
                    (f32::from(delta.y) * 0.003).exp(),
                    f32::from(e.position.x),
                    f32::from(e.position.y),
                );
                cx.notify();
                cx.stop_propagation();
            }))
            .child(
                canvas(
                    move |b, _, _| {
                        let mut c = camera.borrow_mut();
                        c.width = f32::from(b.size.width);
                        c.height = f32::from(b.size.height);
                        c.origin_x = f32::from(b.origin.x);
                        c.origin_y = f32::from(b.origin.y);
                        if c.width > 0.0
                            && c.height > 0.0
                            && let Some(bounds) = pending_fit.borrow_mut().take()
                        {
                            *c = bounds.fitted(*c);
                        }
                        c.clamp();
                        *c
                    },
                    move |b, c, window, cx| {
                        let _timing = super::performance::map_scope();
                        let center = c.screen(0.5, 0.5);
                        let image_bounds = Bounds::new(
                            point(px(center.0 - c.edge() / 2.), px(center.1 - c.edge() / 2.)),
                            size(px(c.edge()), px(c.edge())),
                        );
                        let _ = window.paint_image_rotated(
                            image_bounds,
                            image_bounds,
                            Corners::all(px(0.)),
                            0.,
                            image,
                            0,
                            false,
                            -c.bearing,
                        );
                        if options.dangers {
                            let (left, right) = (f32::from(b.left()), f32::from(b.right()));
                            let (top, bottom) = (f32::from(b.top()), f32::from(b.bottom()));
                            for rect in danger_rects.iter() {
                                let points = rect.map(|(x, y)| c.screen(x, y));
                                let (min_x, max_x) = points
                                    .iter()
                                    .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), p| {
                                        (lo.min(p.0), hi.max(p.0))
                                    });
                                let (min_y, max_y) = points
                                    .iter()
                                    .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), p| {
                                        (lo.min(p.1), hi.max(p.1))
                                    });
                                if max_x < left || min_x > right || max_y < top || min_y > bottom {
                                    continue;
                                }
                                polygon(&points, rgba(0xff69456b), window);
                            }
                            let mut labels = 0;
                            for zone in terrain_grid
                                .zones
                                .iter()
                                .filter(|z| z.cells >= 3 || c.zoom >= 2.5)
                            {
                                let p = normalized(zone.peak, bounds);
                                let (x, y) = c.screen(p.0, p.1);
                                if x > f32::from(b.left())
                                    && x < f32::from(b.right()) - 100.
                                    && y > f32::from(b.top()) + 86.
                                    && y < f32::from(b.bottom()) - 90.
                                {
                                    if labels >= 16 {
                                        break;
                                    }
                                    labels += 1;
                                    text(
                                        &format!("⚠ Steep · {:.0}°", zone.max_slope),
                                        x + 8.,
                                        y - 10.,
                                        0xffaa83,
                                        window,
                                        cx,
                                    );
                                }
                            }
                        }
                        if options.traverse {
                            line(
                                traverse.iter().map(|&(x, y)| c.screen(x, y)),
                                0x8cddc9,
                                1.8,
                                window,
                            );
                        }
                        if options.terminator {
                            // Regional illumination sampled on a coarse geographic grid.
                            // Solar geometry only: this is not a terrain shadow map.
                            let snapshot = MarsClock::at(chrono::Utc::now(), 0.0, 0.0);
                            let nights = SolarGeometry::from_snapshot(&snapshot).night_grid(bounds);
                            for (row, cells) in nights.iter().enumerate() {
                                for (col, night) in cells.iter().enumerate() {
                                    if *night {
                                        let x = col as f32 / 16.;
                                        let y = row as f32 / 16.;
                                        polygon(
                                            &[
                                                c.screen(x, y),
                                                c.screen(x + 1. / 16., y),
                                                c.screen(x + 1. / 16., y + 1. / 16.),
                                                c.screen(x, y + 1. / 16.),
                                            ],
                                            rgba(0x10132e66),
                                            window,
                                        );
                                    }
                                }
                            }
                        }
                        line(
                            route.iter().map(|&p| {
                                let p = normalized(p, bounds);
                                c.screen(p.0, p.1)
                            }),
                            ORANGE,
                            3.5,
                            window,
                        );
                        if super::interaction::enabled() {
                            super::interaction::clear_map();
                        }
                        if options.dangers && super::interaction::enabled() {
                            for zone in &terrain_grid.zones {
                                let p = normalized(zone.peak, bounds);
                                let (x, y) = c.screen(p.0, p.1);
                                if x > f32::from(b.left()) + 16.
                                    && x < f32::from(b.right()) - 16.
                                    && y > f32::from(b.top()) + 100.
                                    && y < f32::from(b.bottom()) - 100.
                                {
                                    super::interaction::painted(
                                        format!("danger/{}", zone.id),
                                        Bounds::new(
                                            point(px(x - 3.), px(y - 3.)),
                                            size(px(6.), px(6.)),
                                        ),
                                        serde_json::json!({"slope":zone.max_slope}),
                                    );
                                }
                            }
                        }
                        for l in &landmarks {
                            if (l.kind == "rover" && !options.rover)
                                || (l.kind != "rover" && !options.landmarks)
                            {
                                continue;
                            }
                            let p = normalized(l.point, bounds);
                            let (x, y) = c.screen(p.0, p.1);
                            if x < f32::from(b.left()) - 30.
                                || x > f32::from(b.right()) + 30.
                                || y < f32::from(b.top()) - 30.
                                || y > f32::from(b.bottom()) + 30.
                            {
                                continue;
                            }
                            if super::interaction::enabled() {
                                super::interaction::painted(
                                    format!("landmark/{}", l.name),
                                    Bounds::new(
                                        point(px(x - 6.), px(y - 6.)),
                                        size(px(12.), px(12.)),
                                    ),
                                    serde_json::Value::Null,
                                );
                            }
                            let inspected = selected == Some(l.point);
                            let color = if inspected {
                                ACCENT
                            } else if l.kind == "rover" {
                                TEAL
                            } else {
                                0xdfdce9
                            };
                            dot(x, y, if l.kind == "rover" { 7. } else { 5. }, color, window);
                            if c.zoom >= 2.5 || l.kind == "rover" || inspected {
                                text(&l.name, x + 11., y - 10., color, window, cx);
                            }
                        }
                        for (p, color, name) in [
                            (selected, ACCENT, "Selected"),
                            (start, TEAL, "A"),
                            (end, ORANGE, "B"),
                        ] {
                            if let Some(p) = p {
                                if name != "Selected" && selected == Some(p) {
                                    continue;
                                }
                                // A sourced landmark already has its own label. Color that
                                // label instead of drawing Selected/B on top of its name.
                                if landmarks.iter().any(|l| {
                                    l.point == p
                                        && ((l.kind == "rover" && options.rover)
                                            || (l.kind != "rover" && options.landmarks))
                                        && (c.zoom >= 2.5
                                            || l.kind == "rover"
                                            || selected == Some(l.point))
                                }) {
                                    continue;
                                }
                                let is_danger = terrain_grid.danger(p).is_some();
                                let p = normalized(p, bounds);
                                let (x, y) = c.screen(p.0, p.1);
                                dot(x, y, 8., color, window);
                                if name == "Selected" && is_danger {
                                    continue;
                                }
                                text(name, x + 12., y - 11., color, window, cx);
                            }
                        }
                        // The source rasters are north-up. These annotations
                        // describe the actual map projection and Mars scale.
                        let right = f32::from(b.right());
                        let bottom = f32::from(b.bottom());
                        if c.width >= 170.0 && c.height >= 130.0 {
                            let x = right - 218.0;
                            // The compass button sits 16 px above the bottom
                            // edge; end the scale card on the same line so the
                            // two bottom-right controls share a baseline.
                            let y = bottom - 28.0;
                            window.paint_quad(
                                PaintQuad::new(Bounds::new(
                                    point(px(x - 12.0), px(y - 36.0)),
                                    size(px(136.0), px(52.0)),
                                ))
                                .corner_radii(px(8.))
                                .background(rgba(0x11131d70))
                                .border_widths(px(1.))
                                .border_color(rgba(0xffffff25)),
                            );
                            text(
                                &units.distance_km(kilometers_for_pixels(c, bounds, 100.0)),
                                x,
                                y - 30.0,
                                INK,
                                window,
                                cx,
                            );
                            line(
                                [(x, y - 5.0), (x, y), (x + 100.0, y), (x + 100.0, y - 5.0)]
                                    .into_iter(),
                                INK,
                                1.5,
                                window,
                            );
                        }
                    },
                )
                .size_full(),
            )
            .when_some(fade_from, |map, layer| map.child(layer))
            .child(
                div().absolute().right_3().bottom_3().child(
                    button("compass", "")
                        .w(px(64.))
                        .h(px(64.))
                        .p_0()
                        .flex_col()
                        .gap_0()
                        .child(
                            icon("back").with_transformation(Transformation::rotate(radians(
                                std::f32::consts::FRAC_PI_2 - self.camera.borrow().bearing,
                            ))),
                        )
                        .child(
                            label(
                                ["N", "E", "S", "W"][((self
                                    .camera
                                    .borrow()
                                    .bearing
                                    .to_degrees()
                                    .rem_euclid(360.)
                                    + 45.)
                                    / 90.)
                                    as usize
                                    % 4],
                            )
                            .text_color(rgb(INK))
                            .text_size(px(13.)),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.next_bearing();
                            cx.notify();
                        })),
                ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CAMERA_ANIMATION_DURATION, Camera, CameraAnimation, CameraPose, Images, MapView,
        NormalizedBounds, SolarGeometry, kilometers_for_pixels,
    };
    use gpui::RenderImage;
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };
    use terramars::domain::{MapBounds, MarsClock};
    #[test]
    fn wide_viewport_pan_reverses_after_hitting_either_edge() {
        let mut c = Camera {
            width: 1800.,
            height: 500.,
            ..Default::default()
        };
        c.pan(-10000., 0.);
        let left = c.x;
        c.pan(60., 0.);
        assert!(c.x < left);
        c.pan(10000., 0.);
        let right = c.x;
        c.pan(-60., 0.);
        assert!(c.x > right);
    }
    #[test]
    fn zoom_keeps_cursor_on_same_geographic_point() {
        let mut c = Camera {
            width: 1200.,
            height: 800.,
            zoom: 4.,
            ..Default::default()
        };
        let before = c.local(640., 430.);
        c.zoom_at(1.5, 640., 430.);
        let after = c.local(640., 430.);
        assert!((before.0 - after.0).abs() < 1e-6 && (before.1 - after.1).abs() < 1e-6);
    }

    #[test]
    fn map_coordinates_round_trip_with_window_offset_and_arbitrary_pan() {
        let camera = Camera {
            width: 980.,
            height: 620.,
            origin_x: 270.,
            origin_y: 150.,
            x: 0.3,
            y: 0.7,
            zoom: 3.,
            bearing: 0.,
        };
        for (u, v) in [(0., 0.), (0.2, 0.8), (1., 1.)] {
            let screen = camera.screen(u, v);
            let local = camera.local(screen.0, screen.1);
            assert!((u - local.0).abs() < 1e-6 && (v - local.1).abs() < 1e-6);
        }
    }

    #[test]
    fn image_can_pan_back_from_vertical_and_horizontal_limits_at_high_zoom() {
        let mut camera = Camera {
            width: 800.,
            height: 600.,
            zoom: 12.,
            ..Default::default()
        };
        camera.pan(-100000., -100000.);
        let at_edge = (camera.x, camera.y);
        camera.pan(20., 20.);
        assert!(camera.x < at_edge.0 && camera.y < at_edge.1);
        camera.pan(100000., 100000.);
        let opposite = (camera.x, camera.y);
        camera.pan(-20., -20.);
        assert!(camera.x > opposite.0 && camera.y > opposite.1);
    }

    #[test]
    fn one_snapshot_night_grid_matches_individual_mars24_locations() {
        use chrono::TimeZone;
        let instant = chrono::Utc
            .with_ymd_and_hms(2026, 11, 14, 12, 0, 0)
            .unwrap();
        let snapshot = MarsClock::at(instant, 0.0, 0.0);
        for site in terramars::domain::catalog().unwrap() {
            let bounds = site.map_bounds;
            let grid = SolarGeometry::from_snapshot(&snapshot).night_grid(bounds);
            for (row, cells) in grid.iter().enumerate() {
                for (col, night) in cells.iter().enumerate() {
                    let lat =
                        bounds.north - (row as f64 + 0.5) / 16. * (bounds.north - bounds.south);
                    let lon = bounds.west + (col as f64 + 0.5) / 16. * (bounds.east - bounds.west);
                    assert_eq!(*night, !MarsClock::at(instant, lon, lat).is_daylight());
                }
            }
        }
    }

    #[test]
    fn camera_animation_is_eased_finite_and_ends_at_the_exact_target() {
        let started = Instant::now();
        let from = CameraPose {
            x: 0.5,
            y: 0.5,
            zoom: 1.0,
            bearing: 0.,
        };
        let to = CameraPose {
            x: 0.2,
            y: 0.8,
            zoom: 6.0,
            bearing: 0.,
        };
        let animation = CameraAnimation { from, to, started };
        let (half, running) = animation.sample(started + Duration::from_millis(85));
        assert!(running);
        assert!(half.zoom > 3.5 && half.zoom < 6.0); // cubic ease-out progresses ahead of linear
        assert!(half.x.is_finite() && half.y.is_finite() && half.zoom.is_finite());
        let (finished, running) = animation.sample(started + CAMERA_ANIMATION_DURATION);
        assert!(!running);
        assert_eq!(finished, to);
        assert_eq!(
            animation.sample(started + Duration::from_secs(1)),
            (to, false)
        );
    }

    #[test]
    fn route_fit_preserves_aspect_and_leaves_five_percent_vertical_margin() {
        let camera = Camera {
            width: 1200.,
            height: 600.,
            origin_x: 270.,
            origin_y: 110.,
            ..Default::default()
        };
        let bounds = NormalizedBounds {
            left: 0.1,
            top: 0.2,
            right: 0.9,
            bottom: 0.8,
        };
        let fit = bounds.fitted(camera);
        let top_left = fit.screen(bounds.left, bounds.top);
        let bottom_right = fit.screen(bounds.right, bounds.bottom);
        assert!((bottom_right.1 - top_left.1 - camera.height * 0.9).abs() < 0.001);
        assert!(top_left.0 >= camera.origin_x + camera.width * 0.05);
        assert!(bottom_right.0 <= camera.origin_x + camera.width * 0.95);
        let whole = NormalizedBounds {
            left: 0.,
            top: 0.,
            right: 1.,
            bottom: 1.,
        }
        .fitted(camera);
        assert!((whole.zoom - 0.9).abs() < 0.00001);
    }

    fn view_fixture() -> MapView {
        let image = Arc::new(RenderImage::new(vec![image::Frame::new(
            image::RgbaImage::new(1, 1),
        )]));
        let images = Arc::new(Images {
            imagery: [image.clone(), image.clone()],
            cards: [image.clone(), image.clone()],
            terrain: [image.clone(), image.clone()],
            panorama: image.clone(),
            coming_soon: image.clone(),
            hazards: [
                terramars::domain::terrain::TerrainGrid::bundled("jezero").unwrap(),
                terramars::domain::terrain::TerrainGrid::bundled("gale").unwrap(),
            ],
        });
        let site = terramars::domain::catalog().unwrap().remove(0);
        let view = MapView::new(site, 0, images);
        *view.camera.borrow_mut() = Camera {
            width: 1000.,
            height: 650.,
            ..Default::default()
        };
        view
    }

    #[test]
    fn reduced_motion_snaps_camera_and_route_fit_does_not_change_selection() {
        let mut view = view_fixture();
        view.reduced_motion = true;
        let point = view.site.rover().unwrap().point;
        view.selected = Some(point);
        view.start = Some(view.site.landmarks[1].point);
        view.end = Some(point);
        let selection = (view.selected, view.start, view.end);
        view.zoom(2.0);
        assert!(view.animation.is_none());
        assert!((view.camera.borrow().zoom - 2.0).abs() < 0.00001);
        view.fit_route(&[selection.1.unwrap(), selection.2.unwrap()]);
        assert!(view.animation.is_none());
        assert_eq!((view.selected, view.start, view.end), selection);
    }

    #[test]
    fn direct_manipulation_cancels_pending_camera_animation() {
        let mut view = view_fixture();
        view.focus_rover();
        assert!(view.animation.is_some());
        view.cancel_animation();
        assert!(view.animation.is_none());
        let current = *view.camera.borrow();
        assert!(!view.advance_animation(Instant::now() + Duration::from_secs(1)));
        assert_eq!(
            CameraPose::of(*view.camera.borrow()),
            CameraPose::of(current)
        );
    }

    #[test]
    fn mars_scale_tracks_zoom_and_local_latitude() {
        let mut camera = Camera {
            width: 1000.,
            height: 1000.,
            ..Default::default()
        };
        let equator = MapBounds {
            west: 0.,
            east: 1.,
            south: -0.5,
            north: 0.5,
        };
        let polar = MapBounds {
            west: 0.,
            east: 1.,
            south: 59.5,
            north: 60.5,
        };
        let scale = kilometers_for_pixels(camera, equator, 100.);
        assert!((scale - 3389.5 * 0.1_f64.to_radians()).abs() < 1e-8);
        assert!((kilometers_for_pixels(camera, polar, 100.) / scale - 0.5).abs() < 0.00001);
        camera.zoom = 2.;
        assert!((kilometers_for_pixels(camera, equator, 100.) - scale / 2.).abs() < 1e-8);
    }
    #[test]
    fn rotated_camera_inverse_anchor_and_route_fit_agree() {
        for angle in [0.0f32, 1., 45., 90., 180., 270., 359.] {
            let mut c = Camera {
                width: 1200.,
                height: 850.,
                zoom: 3.,
                bearing: angle.to_radians(),
                origin_x: 12.,
                origin_y: 32.,
                ..Default::default()
            };
            let p = (0.42, 0.63);
            let pixel = c.screen(p.0, p.1);
            let inverse = c.local(pixel.0, pixel.1);
            assert!((inverse.0 - p.0).abs() < 0.00001 && (inverse.1 - p.1).abs() < 0.00001);
            let anchor = (650., 470.);
            let before = c.local(anchor.0, anchor.1);
            c.rotate_at(0.21, anchor.0, anchor.1);
            c.zoom_at(1.2, anchor.0, anchor.1);
            let after = c.local(anchor.0, anchor.1);
            assert!((before.0 - after.0).abs() < 0.00001 && (before.1 - after.1).abs() < 0.00001);
            let fitted = NormalizedBounds {
                left: 0.2,
                top: 0.25,
                right: 0.7,
                bottom: 0.75,
            }
            .fitted(c);
            for p in [(0.2, 0.25), (0.2, 0.75), (0.7, 0.25), (0.7, 0.75)] {
                let (x, y) = fitted.screen(p.0, p.1);
                assert!(
                    x >= fitted.origin_x
                        && x <= fitted.origin_x + fitted.width
                        && y >= fitted.origin_y
                        && y <= fitted.origin_y + fitted.height
                );
            }
        }
    }
}
