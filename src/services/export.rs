//! Mars route export. Geometry uses longitude/latitude order, but this is a
//! Mars-specific GeoJSON-style document, not Earth RFC 7946 / EPSG:4326 data.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::{Value, json};

use crate::domain::{MissionSite, RouteEstimate};

pub fn route_geojson(
    site: &MissionSite,
    route: &RouteEstimate,
    mode: &str,
    objective: &str,
) -> Value {
    json!({
        "type": "FeatureCollection",
        "name": "TerraMars reconnaissance plan",
        "body": "Mars",
        "coordinateReferenceSystem": "Mars planetocentric latitude / east-positive longitude, degrees; not Earth EPSG:4326 or RFC 7946 WGS84",
        "coordinateOrder": ["longitude", "latitude"],
        "features": [{
            "type": "Feature",
            "properties": {
                "crater": site.id,
                "mode": mode,
                "scienceObjective": objective,
                "name": route.name,
                "distanceKm": route.distance_km,
                "estimatedSeconds": route.duration_seconds,
                "assumedSpeedKmh": route.assumed_speed_kmh,
                "createdAt": Utc::now().to_rfc3339(),
                "terrainVerified": false,
                "lifeSupportVerified": false,
                "usesObservedTraverse": route.uses_observed_traverse,
                "avoidsImportedHazards": route.avoids_imported_hazards,
                "riskNote": route.risk_note(),
                "sources": site.sources.iter().map(|source| source.url.as_str()).collect::<Vec<_>>(),
                "sourceRetrievalDate": site.retrieved_at,
            },
            "geometry": {
                "type": "LineString",
                "coordinates": route.points.iter().map(|point| [point.longitude, point.latitude]).collect::<Vec<_>>(),
            },
        }],
    })
}

/// Writes only to the path explicitly chosen by the caller's save dialog.
/// The UI should describe this as a Mars route, not an Earth GPS/GPX track.
pub fn write_route(
    path: &Path,
    site: &MissionSite,
    route: &RouteEstimate,
    mode: &str,
    objective: &str,
) -> Result<()> {
    route
        .validate()
        .context("Cannot export an invalid Mars route")?;
    let bytes = serde_json::to_vec_pretty(&route_geojson(site, route, mode, objective))?;
    fs::write(path, bytes)
        .with_context(|| format!("Could not write Mars route to {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Point;

    #[test]
    fn exported_route_keeps_mars_body_and_lon_lat_order() {
        let catalog: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/data/missions.json"
        )))
        .unwrap();
        let site: MissionSite = serde_json::from_value(catalog["sites"][0].clone()).unwrap();
        let route = RouteEstimate {
            name: "Test route".into(),
            points: vec![
                Point {
                    latitude: 18.44,
                    longitude: 77.45,
                },
                Point {
                    latitude: 18.43,
                    longitude: 77.44,
                },
            ],
            distance_km: 1.0,
            duration_seconds: 1200,
            assumed_speed_kmh: 3.0,
            uses_observed_traverse: false,
            avoids_imported_hazards: false,
            avoids_terrain_hazards: false,
        };
        let value = route_geojson(&site, &route, "EVA traverse", "test objective");
        assert_eq!(value["body"], "Mars");
        assert_eq!(
            value["features"][0]["geometry"]["coordinates"][0],
            json!([77.45, 18.44])
        );
        assert_eq!(value["features"][0]["properties"]["terrainVerified"], false);
    }
}
