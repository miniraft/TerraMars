use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

use super::Point;

/// NASA volumetric mean radius (km); a map estimate is not terrain clearance.
pub const MARS_RADIUS_KM: f64 = 3389.5;

pub struct RoutePlanner;

impl RoutePlanner {
    pub fn distance_km(a: Point, b: Point) -> f64 {
        let lat1 = a.latitude.to_radians();
        let lat2 = b.latitude.to_radians();
        let d_lat = lat2 - lat1;
        let d_lon = (b.longitude - a.longitude).to_radians();
        let h = (d_lat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (d_lon / 2.0).sin().powi(2);
        2.0 * MARS_RADIUS_KM * h.clamp(0.0, 1.0).sqrt().asin()
    }

    pub fn plan(
        start: Point,
        end: Point,
        speed_kmh: f64,
        traverse: &[Point],
        hazards: &[HazardZone],
    ) -> Result<Vec<RouteEstimate>> {
        ensure!(
            speed_kmh.is_finite() && speed_kmh > 0.0,
            "Travel speed must be finite and positive"
        );
        start.validate()?;
        end.validate()?;
        for point in traverse {
            point.validate()?;
        }
        for hazard in hazards {
            hazard.validate()?;
        }
        if hazards
            .iter()
            .any(|zone| inside(start, &zone.polygon) || inside(end, &zone.polygon))
        {
            return Ok(vec![]);
        }
        let mut routes = Vec::new();
        if clear(start, end, hazards) {
            routes.push(estimate(
                "Direct reconnaissance",
                vec![start, end],
                speed_kmh,
                false,
                !hazards.is_empty(),
            ));
        } else if let Some(points) = detour(start, end, hazards) {
            routes.push(estimate(
                "Imported keep-out detour",
                points,
                speed_kmh,
                false,
                true,
            ));
        }
        if traverse.len() > 1 {
            let a = nearest_index(start, traverse);
            let b = nearest_index(end, traverse);
            if a != b {
                let mut points = vec![start];
                if a <= b {
                    points.extend_from_slice(&traverse[a..=b]);
                } else {
                    points.extend(traverse[b..=a].iter().rev().copied());
                }
                points.push(end);
                if points
                    .windows(2)
                    .all(|pair| clear(pair[0], pair[1], hazards))
                {
                    routes.push(estimate(
                        "Recorded rover corridor",
                        points,
                        speed_kmh,
                        true,
                        !hazards.is_empty(),
                    ));
                }
            }
        }
        Ok(routes)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteEstimate {
    pub name: String,
    pub points: Vec<Point>,
    pub distance_km: f64,
    pub duration_seconds: u64,
    pub assumed_speed_kmh: f64,
    #[serde(default)]
    pub uses_observed_traverse: bool,
    #[serde(default)]
    pub avoids_imported_hazards: bool,
    #[serde(default)]
    pub avoids_terrain_hazards: bool,
}

impl RouteEstimate {
    pub fn risk_note(&self) -> &'static str {
        if self.avoids_terrain_hazards {
            "Avoids mapped MOLA slope exclusions (15° at ~463 m scale). Small rocks, sand, ledges, radiation and life-support limits remain unassessed."
        } else if self.uses_observed_traverse {
            "Follows published rover samples with unverified connectors. No slope, roughness, radiation, suit, or life-support clearance is available."
        } else {
            "Geometric estimate only. No slope, roughness, radiation, suit, or life-support clearance is available."
        }
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(self.points.len() >= 2, "A route requires two endpoints");
        ensure!(
            self.distance_km.is_finite() && self.distance_km >= 0.0,
            "Invalid route distance"
        );
        ensure!(
            self.assumed_speed_kmh.is_finite() && self.assumed_speed_kmh > 0.0,
            "Invalid route travel speed"
        );
        ensure!(!self.name.trim().is_empty(), "Missing route name");
        for point in &self.points {
            point.validate()?;
        }
        Ok(())
    }
}

/// Retained domain capability for externally supplied exclusion geometry. No
/// mission packet or imported hazard control is exposed by the current app.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HazardZone {
    pub id: String,
    pub label: String,
    pub polygon: Vec<Point>,
    pub source_url: String,
}

impl HazardZone {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (3..=100).contains(&self.polygon.len()),
            "Keep-out polygons need 3–100 vertices"
        );
        ensure!(
            !self.id.trim().is_empty() && !self.label.trim().is_empty(),
            "Keep-out zone requires an identifier and label"
        );
        ensure!(
            self.source_url.starts_with("https://") || self.source_url.starts_with("http://"),
            "Keep-out zone requires a source URL"
        );
        for point in &self.polygon {
            point.validate()?;
        }
        Ok(())
    }
}

pub(crate) fn estimate(
    name: &str,
    points: Vec<Point>,
    speed_kmh: f64,
    uses_traverse: bool,
    has_hazards: bool,
) -> RouteEstimate {
    let distance_km: f64 = points
        .windows(2)
        .map(|pair| RoutePlanner::distance_km(pair[0], pair[1]))
        .sum();
    RouteEstimate {
        name: name.into(),
        points,
        distance_km,
        duration_seconds: (distance_km / speed_kmh * 3600.0).ceil() as u64,
        assumed_speed_kmh: speed_kmh,
        uses_observed_traverse: uses_traverse,
        avoids_imported_hazards: has_hazards,
        avoids_terrain_hazards: false,
    }
}

pub(crate) fn nearest_index(point: Point, line: &[Point]) -> usize {
    line.iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            RoutePlanner::distance_km(point, **a).total_cmp(&RoutePlanner::distance_km(point, **b))
        })
        .map_or(0, |(index, _)| index)
}

fn inside(point: Point, polygon: &[Point]) -> bool {
    let mut result = false;
    let mut j = polygon.len() - 1;
    for i in 0..polygon.len() {
        let (a, b) = (polygon[i], polygon[j]);
        if (a.latitude > point.latitude) != (b.latitude > point.latitude)
            && point.longitude
                < (b.longitude - a.longitude) * (point.latitude - a.latitude)
                    / (b.latitude - a.latitude)
                    + a.longitude
        {
            result = !result;
        }
        j = i;
    }
    result
}

fn cross(a: Point, b: Point, c: Point) -> f64 {
    (b.longitude - a.longitude) * (c.latitude - a.latitude)
        - (b.latitude - a.latitude) * (c.longitude - a.longitude)
}

fn intersects(a: Point, b: Point, c: Point, d: Point) -> bool {
    if a.longitude.max(b.longitude) < c.longitude.min(d.longitude)
        || c.longitude.max(d.longitude) < a.longitude.min(b.longitude)
        || a.latitude.max(b.latitude) < c.latitude.min(d.latitude)
        || c.latitude.max(d.latitude) < a.latitude.min(b.latitude)
    {
        return false;
    }
    cross(a, b, c) * cross(a, b, d) <= 0.0 && cross(c, d, a) * cross(c, d, b) <= 0.0
}

fn clear(a: Point, b: Point, hazards: &[HazardZone]) -> bool {
    for zone in hazards {
        if inside(a, &zone.polygon) || inside(b, &zone.polygon) {
            return false;
        }
        for i in 0..zone.polygon.len() {
            if intersects(
                a,
                b,
                zone.polygon[i],
                zone.polygon[(i + 1) % zone.polygon.len()],
            ) {
                return false;
            }
        }
    }
    true
}

fn detour(start: Point, end: Point, hazards: &[HazardZone]) -> Option<Vec<Point>> {
    let mut nodes = vec![start, end];
    for zone in hazards {
        let lat = zone.polygon.iter().map(|p| p.latitude).sum::<f64>() / zone.polygon.len() as f64;
        let lon = zone.polygon.iter().map(|p| p.longitude).sum::<f64>() / zone.polygon.len() as f64;
        for point in &zone.polygon {
            let (dy, dx) = (point.latitude - lat, point.longitude - lon);
            let length = dx.hypot(dy);
            if length > 0.0 {
                // Computational clearance, never a measured safety buffer.
                nodes.push(Point::new(
                    point.latitude + dy / length * 0.0002,
                    point.longitude + dx / length * 0.0002,
                ));
            }
        }
    }
    if nodes.len() > 250 {
        return None;
    }
    let mut distances = vec![f64::INFINITY; nodes.len()];
    let mut previous = vec![None; nodes.len()];
    let mut visited = vec![false; nodes.len()];
    distances[0] = 0.0;
    for _ in 0..nodes.len() {
        let next = (0..nodes.len())
            .filter(|i| !visited[*i])
            .min_by(|a, b| distances[*a].total_cmp(&distances[*b]))?;
        if !distances[next].is_finite() {
            break;
        }
        if next == 1 {
            break;
        }
        visited[next] = true;
        for i in 0..nodes.len() {
            if visited[i] || !clear(nodes[next], nodes[i], hazards) {
                continue;
            }
            let candidate = distances[next] + RoutePlanner::distance_km(nodes[next], nodes[i]);
            if candidate < distances[i] {
                distances[i] = candidate;
                previous[i] = Some(next);
            }
        }
    }
    if !distances[1].is_finite() {
        return None;
    }
    let mut indices = vec![];
    let mut at = Some(1);
    while let Some(i) = at {
        indices.push(i);
        at = previous[i];
    }
    indices.reverse();
    Some(indices.into_iter().map(|i| nodes[i]).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mars_distance_and_eta_use_the_explicit_speed_assumption() {
        let (a, b) = (Point::new(0.0, 0.0), Point::new(0.0, 1.0));
        let distance = RoutePlanner::distance_km(a, b);
        assert!((distance - MARS_RADIUS_KM * std::f64::consts::PI / 180.0).abs() < 1e-9);
        let route = RoutePlanner::plan(a, b, 3.0, &[], &[]).unwrap().remove(0);
        assert!((route.duration_seconds as f64 - distance / 3.0 * 3600.0).abs() <= 1.0);
        assert!(route.risk_note().contains("Geometric estimate"));
        assert!(RoutePlanner::plan(a, b, 0.0, &[], &[]).is_err());
    }

    #[test]
    fn observed_corridor_preserves_real_samples_and_reverses_as_needed() {
        let line = [
            Point::new(0.0, 0.0),
            Point::new(0.1, 0.5),
            Point::new(0.0, 1.0),
        ];
        let routes = RoutePlanner::plan(line[2], line[0], 3.0, &line, &[]).unwrap();
        assert_eq!(routes.len(), 2);
        assert!(routes[1].uses_observed_traverse);
        assert!(routes[1].points.contains(&line[1]));
        assert!(routes[1].distance_km > routes[0].distance_km);
        assert!(routes[1].risk_note().contains("unverified connectors"));
    }

    #[test]
    fn polygon_intersections_detour_and_block_inside_endpoints() {
        let zone = HazardZone {
            id: "test".into(),
            label: "Synthetic test geometry".into(),
            source_url: "https://example.org/test".into(),
            polygon: vec![
                Point::new(-0.01, 0.4),
                Point::new(-0.01, 0.6),
                Point::new(0.01, 0.6),
                Point::new(0.01, 0.4),
            ],
        };
        let routes = RoutePlanner::plan(
            Point::new(0.0, 0.0),
            Point::new(0.0, 1.0),
            3.0,
            &[],
            std::slice::from_ref(&zone),
        )
        .unwrap();
        assert_eq!(routes.len(), 1);
        assert!(routes[0].points.len() > 2);
        assert!(routes[0].points.windows(2).all(|p| clear(
            p[0],
            p[1],
            std::slice::from_ref(&zone)
        )));
        assert!(
            RoutePlanner::plan(
                Point::new(0.0, 0.5),
                Point::new(0.0, 1.0),
                3.0,
                &[],
                std::slice::from_ref(&zone)
            )
            .unwrap()
            .is_empty()
        );
        let grazing = RoutePlanner::plan(
            Point::new(-0.01, 0.0),
            Point::new(-0.01, 1.0),
            3.0,
            &[],
            &[zone],
        )
        .unwrap();
        assert!(grazing.iter().all(|r| r.name != "Direct reconnaissance"));
    }
}
