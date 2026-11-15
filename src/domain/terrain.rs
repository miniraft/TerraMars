//! Regional, source-backed MOLA slope exclusions. Not a small-obstacle map.
use super::{
    Point, RouteEstimate, RoutePlanner,
    routes::{MARS_RADIUS_KM, estimate},
};
use anyhow::{Result, ensure};
use serde::Deserialize;
use std::{
    cmp::Ordering,
    collections::{BinaryHeap, VecDeque},
    sync::Arc,
};

#[derive(Clone, Deserialize)]
pub struct TerrainMetadata {
    pub source_url: String,
    pub columns: usize,
    pub rows: usize,
    pub west: f64,
    pub north: f64,
    pub degrees_per_cell: f64,
    pub nominal_resolution_m: f64,
    pub slope_limit_degrees: f64,
}
#[derive(Clone, Debug)]
pub struct DangerZone {
    pub id: usize,
    pub peak: Point,
    pub max_slope: f64,
    pub cells: usize,
}
/// Horizontal cell runs for a compact, exact overlay of the blocked raster.
#[derive(Clone)]
pub struct DangerRun {
    pub row: usize,
    pub first: usize,
    pub end: usize,
}
pub struct TerrainGrid {
    pub metadata: TerrainMetadata,
    pub zones: Vec<DangerZone>,
    pub runs: Vec<DangerRun>,
    slopes: Vec<f32>,
    regions: Vec<usize>,
}
impl TerrainGrid {
    pub fn bundled(id: &str) -> Result<Arc<Self>> {
        let (meta, raw): (&str, &[u8]) = match id {
            "jezero" => (
                include_str!("../../assets/data/terrain/jezero.json"),
                include_bytes!("../../assets/data/terrain/jezero.i16be"),
            ),
            "gale" => (
                include_str!("../../assets/data/terrain/gale.json"),
                include_bytes!("../../assets/data/terrain/gale.i16be"),
            ),
            _ => anyhow::bail!("No numerical terrain coverage for {id}"),
        };
        let metadata: TerrainMetadata = serde_json::from_str(meta)?;
        let elevations = raw
            .as_chunks::<2>()
            .0
            .iter()
            .map(|v| i16::from_be_bytes([v[0], v[1]]))
            .collect::<Vec<_>>();
        Self::new(metadata, &elevations).map(Arc::new)
    }
    fn new(metadata: TerrainMetadata, elevations: &[i16]) -> Result<Self> {
        let (w, h) = (metadata.columns, metadata.rows);
        ensure!(
            w >= 3 && h >= 3 && elevations.len() == w * h,
            "Invalid terrain dimensions"
        );
        ensure!(
            metadata.degrees_per_cell > 0.
                && metadata.slope_limit_degrees > 0.
                && metadata.slope_limit_degrees < 90.,
            "Invalid terrain scale/policy"
        );
        let mut slopes = vec![f32::NAN; w * h];
        let dy = metadata.degrees_per_cell.to_radians() * MARS_RADIUS_KM * 1000.;
        for y in 1..h - 1 {
            let latitude = metadata.north - (y as f64 + 0.5) * metadata.degrees_per_cell;
            let dx = dy * latitude.to_radians().cos();
            for x in 1..w - 1 {
                let i = y * w + x;
                let z = [
                    elevations[i],
                    elevations[i - 1],
                    elevations[i + 1],
                    elevations[i - w],
                    elevations[i + w],
                ];
                if z.contains(&i16::MIN) {
                    continue;
                }
                let gx = (f64::from(z[2]) - f64::from(z[1])) / (2. * dx);
                let gy = (f64::from(z[4]) - f64::from(z[3])) / (2. * dy);
                slopes[i] = gx.hypot(gy).atan().to_degrees() as f32;
            }
        }
        let mut grid = Self {
            metadata,
            zones: vec![],
            runs: vec![],
            slopes,
            regions: vec![usize::MAX; w * h],
        };
        for y in 1..h - 1 {
            let mut x = 1;
            while x < w - 1 {
                if grid.blocked(y * w + x) {
                    let first = x;
                    while x < w - 1 && grid.blocked(y * w + x) {
                        x += 1;
                    }
                    grid.runs.push(DangerRun {
                        row: y,
                        first,
                        end: x,
                    });
                } else {
                    x += 1;
                }
            }
        }
        for i in 0..w * h {
            if !grid.steep(i) || grid.regions[i] != usize::MAX {
                continue;
            }
            let id = grid.zones.len();
            let mut queue = VecDeque::from([i]);
            grid.regions[i] = id;
            let mut peak = i;
            let mut cells = 0;
            while let Some(at) = queue.pop_front() {
                cells += 1;
                if grid.slopes[at] > grid.slopes[peak] {
                    peak = at;
                }
                let (x, y) = (at % w, at / w);
                for (dx, dy) in [
                    (-1, 0),
                    (1, 0),
                    (0, -1),
                    (0, 1),
                    (-1, -1),
                    (1, -1),
                    (-1, 1),
                    (1, 1),
                ] {
                    let (nx, ny) = (x as isize + dx, y as isize + dy);
                    if nx < 0 || ny < 0 || nx >= w as isize || ny >= h as isize {
                        continue;
                    }
                    let n = ny as usize * w + nx as usize;
                    if grid.steep(n) && grid.regions[n] == usize::MAX {
                        grid.regions[n] = id;
                        queue.push_back(n);
                    }
                }
            }
            grid.zones.push(DangerZone {
                id,
                peak: grid.center(peak),
                max_slope: f64::from(grid.slopes[peak]),
                cells,
            });
        }
        Ok(grid)
    }
    fn steep(&self, i: usize) -> bool {
        self.slopes[i].is_finite() && f64::from(self.slopes[i]) >= self.metadata.slope_limit_degrees
    }
    fn blocked(&self, i: usize) -> bool {
        !self.slopes[i].is_finite() || self.steep(i)
    }
    pub fn center(&self, i: usize) -> Point {
        self.corner(
            (i % self.metadata.columns) as f64 + 0.5,
            (i / self.metadata.columns) as f64 + 0.5,
        )
    }
    pub fn corner(&self, x: f64, y: f64) -> Point {
        Point::new(
            self.metadata.north - y * self.metadata.degrees_per_cell,
            self.metadata.west + x * self.metadata.degrees_per_cell,
        )
    }
    fn cell(&self, p: Point) -> Option<usize> {
        let x = ((p.longitude - self.metadata.west) / self.metadata.degrees_per_cell).floor();
        let y = ((self.metadata.north - p.latitude) / self.metadata.degrees_per_cell).floor();
        if !x.is_finite()
            || !y.is_finite()
            || x < 0.
            || y < 0.
            || x >= self.metadata.columns as f64
            || y >= self.metadata.rows as f64
        {
            None
        } else {
            Some(y as usize * self.metadata.columns + x as usize)
        }
    }
    pub fn danger(&self, p: Point) -> Option<(&DangerZone, f64)> {
        let i = self.cell(p)?;
        self.zones
            .get(self.regions[i])
            .map(|z| (z, f64::from(self.slopes[i])))
    }
    pub fn traversable(&self, p: Point) -> bool {
        self.cell(p).is_some_and(|i| !self.blocked(i))
    }
    /// Supercover traversal: exact cell crossings, including both sides at corners.
    pub fn clear(&self, a: Point, b: Point) -> bool {
        let (Some(ai), Some(bi)) = (self.cell(a), self.cell(b)) else {
            return false;
        };
        if self.blocked(ai) || self.blocked(bi) {
            return false;
        }
        let w = self.metadata.columns;
        let (mut x, mut y) = ((ai % w) as isize, (ai / w) as isize);
        let (bx, by) = ((bi % w) as isize, (bi / w) as isize);
        let ax = (a.longitude - self.metadata.west) / self.metadata.degrees_per_cell;
        let ay = (self.metadata.north - a.latitude) / self.metadata.degrees_per_cell;
        let dx = (b.longitude - a.longitude) / self.metadata.degrees_per_cell;
        let dy = (a.latitude - b.latitude) / self.metadata.degrees_per_cell;
        let (sx, sy) = (if dx > 0. { 1 } else { -1 }, if dy > 0. { 1 } else { -1 });
        let step_x = if dx == 0. {
            f64::INFINITY
        } else {
            1. / dx.abs()
        };
        let step_y = if dy == 0. {
            f64::INFINITY
        } else {
            1. / dy.abs()
        };
        let mut tx = if dx == 0. {
            f64::INFINITY
        } else {
            ((x as f64 + if sx > 0 { 1. } else { 0. }) - ax) / dx
        };
        let mut ty = if dy == 0. {
            f64::INFINITY
        } else {
            ((y as f64 + if sy > 0 { 1. } else { 0. }) - ay) / dy
        };
        let on_vertical_boundary = dx == 0. && (ax - ax.round()).abs() < 1e-10;
        let on_horizontal_boundary = dy == 0. && (ay - ay.round()).abs() < 1e-10;
        let base_free = |x: isize, y: isize| {
            x >= 0
                && y >= 0
                && x < w as isize
                && y < self.metadata.rows as isize
                && !self.blocked(y as usize * w + x as usize)
        };
        let free = |x: isize, y: isize| {
            base_free(x, y)
                && (!on_vertical_boundary || base_free(x - 1, y))
                && (!on_horizontal_boundary || base_free(x, y - 1))
        };
        if !free(x, y) {
            return false;
        }
        for _ in 0..self.metadata.columns + self.metadata.rows + 4 {
            if x == bx && y == by {
                return true;
            }
            if (tx - ty).abs() < 1e-10 {
                if !free(x + sx, y) || !free(x, y + sy) {
                    return false;
                }
                x += sx;
                y += sy;
                tx += step_x;
                ty += step_y;
            } else if tx < ty {
                x += sx;
                tx += step_x;
            } else {
                y += sy;
                ty += step_y;
            }
            if !free(x, y) {
                return false;
            }
        }
        false
    }
    pub fn plan(
        &self,
        start: Point,
        end: Point,
        speed: f64,
        traverse: &[Point],
    ) -> Result<Vec<RouteEstimate>> {
        ensure!(
            self.traversable(start),
            "Starting point is in a steep-terrain exclusion or outside assessed terrain. Choose another start."
        );
        ensure!(
            self.traversable(end),
            "Destination is in a steep-terrain exclusion or outside assessed terrain. Choose another destination."
        );
        ensure!(speed.is_finite() && speed > 0., "Invalid travel speed");
        let direct = self.clear(start, end);
        let points = if direct {
            vec![start, end]
        } else {
            self.detour(start,end).ok_or_else(||anyhow::anyhow!("No route around the mapped terrain exclusions within this region. Choose different endpoints."))?
        };
        let mut route = estimate(
            if direct {
                "Terrain-screened direct"
            } else {
                "Terrain avoidance"
            },
            points,
            speed,
            false,
            false,
        );
        route.avoids_terrain_hazards = true;
        let mut routes = vec![route];
        for mut route in RoutePlanner::plan(start, end, speed, traverse, &[])?
            .into_iter()
            .filter(|r| r.uses_observed_traverse)
        {
            if route.points.windows(2).all(|p| self.clear(p[0], p[1])) {
                route.avoids_terrain_hazards = true;
                routes.push(route);
            }
        }
        Ok(routes)
    }
    fn detour(&self, start: Point, end: Point) -> Option<Vec<Point>> {
        let (start_i, end_i) = (self.cell(start)?, self.cell(end)?);
        let w = self.metadata.columns;
        let h = self.metadata.rows;
        let mut costs = vec![f64::INFINITY; w * h];
        let mut previous = vec![usize::MAX; w * h];
        let mut visited = vec![false; w * h];
        let mut heap = BinaryHeap::new();
        costs[start_i] = 0.;
        heap.push(Node {
            index: start_i,
            score: 0.,
        });
        while let Some(Node { index: i, .. }) = heap.pop() {
            if visited[i] {
                continue;
            }
            visited[i] = true;
            if i == end_i {
                break;
            }
            let (x, y) = ((i % w) as isize, (i / w) as isize);
            for (dx, dy) in [
                (-1, 0),
                (1, 0),
                (0, -1),
                (0, 1),
                (-1, -1),
                (1, -1),
                (-1, 1),
                (1, 1),
            ] {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 1 || ny < 1 || nx >= w as isize - 1 || ny >= h as isize - 1 {
                    continue;
                }
                let n = ny as usize * w + nx as usize;
                if self.blocked(n) {
                    continue;
                }
                if dx != 0
                    && dy != 0
                    && (self.blocked(y as usize * w + nx as usize)
                        || self.blocked(ny as usize * w + x as usize))
                {
                    continue;
                }
                let cost = costs[i] + RoutePlanner::distance_km(self.center(i), self.center(n));
                if cost < costs[n] {
                    costs[n] = cost;
                    previous[n] = i;
                    heap.push(Node {
                        index: n,
                        score: cost + RoutePlanner::distance_km(self.center(n), self.center(end_i)),
                    });
                }
            }
        }
        if !costs[end_i].is_finite() {
            return None;
        }
        let mut indices = vec![end_i];
        while *indices.last()? != start_i {
            let p = previous[*indices.last()?];
            if p == usize::MAX {
                return None;
            }
            indices.push(p);
        }
        indices.reverse();
        let mut path = vec![start];
        path.extend(indices.into_iter().map(|i| self.center(i)));
        path.push(end);
        let mut smooth = vec![start];
        let mut i = 0;
        while i < path.len() - 1 {
            let mut next = path.len() - 1;
            while next > i + 1 && !self.clear(path[i], path[next]) {
                next -= 1;
            }
            if !self.clear(path[i], path[next]) {
                return None;
            }
            smooth.push(path[next]);
            i = next;
        }
        Some(smooth)
    }
}
#[derive(Clone, Copy)]
struct Node {
    index: usize,
    score: f64,
}
impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.score == other.score
    }
}
impl Eq for Node {}
impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .score
            .total_cmp(&self.score)
            .then_with(|| self.index.cmp(&other.index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_mola_has_exclusions_and_flat_landing_corridors() {
        for id in ["jezero", "gale"] {
            let g = TerrainGrid::bundled(id).unwrap();
            assert!(!g.zones.is_empty());
            let site = super::super::catalog()
                .unwrap()
                .into_iter()
                .find(|s| s.id == id)
                .unwrap();
            let a = site.landmarks[0].point;
            let b = site.landmarks[1].point;
            let routes = g.plan(a, b, 3., &site.traverse).unwrap();
            assert!(
                routes
                    .iter()
                    .all(|r| r.points.windows(2).all(|p| g.clear(p[0], p[1])))
            );
            assert!(g.plan(g.zones[0].peak, b, 3., &[]).is_err());
        }
    }
    #[test]
    fn detour_does_not_cut_blocked_cells_or_diagonal_corners() {
        let m = TerrainMetadata {
            source_url: "synthetic test".into(),
            columns: 20,
            rows: 20,
            west: 0.,
            north: 1.,
            degrees_per_cell: 0.01,
            nominal_resolution_m: 463.,
            slope_limit_degrees: 15.,
        };
        let mut g = TerrainGrid::new(m, &[0; 400]).unwrap();
        for y in 5..15 {
            g.slopes[y * 20 + 10] = 30.;
        }
        let (a, b) = (g.center(10 * 20 + 4), g.center(10 * 20 + 16));
        assert!(!g.clear(a, b));
        let r = g.plan(a, b, 3., &[]).unwrap();
        assert!(r[0].points.len() > 2);
        assert!(r[0].points.windows(2).all(|p| g.clear(p[0], p[1])));
        assert!(!g.clear(g.center(0), b));
        assert!(
            !g.clear(g.corner(10., 5.), g.corner(10., 15.)),
            "A boundary-grazing line must not cut a blocked cell"
        );
    }
    #[test]
    fn actual_jezero_slopes_change_the_route_geometry() {
        let g = TerrainGrid::bundled("jezero").unwrap();
        let mut verified = false;
        for zone in &g.zones {
            let a = Point::new(zone.peak.latitude, zone.peak.longitude - 0.035);
            let b = Point::new(zone.peak.latitude, zone.peak.longitude + 0.035);
            if !g.traversable(a) || !g.traversable(b) || g.clear(a, b) {
                continue;
            }
            if let Ok(routes) = g.plan(a, b, 3., &[]) {
                let r = &routes[0];
                assert!(r.points.len() > 2);
                assert!(r.points.windows(2).all(|p| g.clear(p[0], p[1])));
                assert!(r.distance_km > RoutePlanner::distance_km(a, b));
                verified = true;
                break;
            }
        }
        assert!(
            verified,
            "Need a real terrain-exclusion crossing regression"
        );
    }
}
