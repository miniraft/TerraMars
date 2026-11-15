//! Public NASA map/outreach feeds. Retrieval time is not observation time.

use std::collections::BTreeSet;
use std::io::Read;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use chrono::{NaiveDate, Utc};
use reqwest::blocking::Client;
use serde_json::Value;

use crate::domain::{Landmark, MissionSite, Point};

pub const MAX_DOWNLOAD_BYTES: u64 = 8 * 1024 * 1024;
pub const SOURCE_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone)]
pub struct RefreshResult {
    pub site: MissionSite,
    pub updated_sources: Vec<String>,
    pub warnings: Vec<String>,
}

impl RefreshResult {
    pub fn has_updates(&self) -> bool {
        !self.updated_sources.is_empty()
    }
}

/// Blocking, bounded downloads; call this on a background worker. A failed
/// source retains its prior data independently of the other source updates.
pub fn refresh_site(original: &MissionSite) -> RefreshResult {
    let mut result = RefreshResult {
        site: original.clone(),
        updated_sources: Vec::new(),
        warnings: Vec::new(),
    };
    let (mission, weather_category) = match original.id.as_str() {
        "jezero" => ("M20", "mars2020"),
        "gale" => ("MSL", "msl"),
        _ => {
            result
                .warnings
                .push("NASA refresh supports Jezero and Gale only.".into());
            return result;
        }
    };
    let client = match Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(SOURCE_TIMEOUT)
        .user_agent("TerraMars/0.2 (public NASA mission-data viewer)")
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            result
                .warnings
                .push(format!("NASA HTTP client could not start: {error}"));
            return result;
        }
    };
    let sources = [
        (
            "Rover waypoint",
            format!(
                "https://mars.nasa.gov/mmgis-maps/{mission}/Layers/json/{mission}_waypoints_current.json"
            ),
        ),
        (
            "Recorded traverse",
            format!(
                "https://mars.nasa.gov/mmgis-maps/{mission}/Layers/json/{mission}_traverse.json"
            ),
        ),
        (
            "Historical weather",
            format!(
                "https://mars.nasa.gov/rss/api/?feed=weather&category={weather_category}&feedtype=json"
            ),
        ),
    ];
    // The three downloads overlap; each retains its own 15-second deadline.
    let downloads = std::thread::scope(|scope| {
        let handles: Vec<_> = sources
            .iter()
            .map(|(name, url)| {
                let client = client.clone();
                scope.spawn(move || (*name, url.clone(), fetch(&client, url)))
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join())
            .collect::<Vec<_>>()
    });
    for download in downloads {
        let (name, url, payload) = match download {
            Ok(download) => download,
            Err(_) => {
                result
                    .warnings
                    .push("A NASA download worker failed. Previous data retained.".into());
                continue;
            }
        };
        let payload = match payload {
            Ok(payload) => payload,
            Err(error) => {
                result.warnings.push(format!(
                    "{name}: {error:#}. Previous data retained. Source: {url}"
                ));
                continue;
            }
        };
        let mut candidate = result.site.clone();
        let mut source_warnings = Vec::new();
        let parsed = match name {
            "Rover waypoint" => apply_rover(&mut candidate, &payload, &url, &mut source_warnings),
            "Recorded traverse" => apply_traverse(&mut candidate, &payload, &mut source_warnings),
            _ => apply_weather(&mut candidate, &payload, &url, &mut source_warnings),
        };
        match parsed {
            Ok(()) => {
                result.site = candidate;
                result.updated_sources.push(name.into());
                result.warnings.extend(source_warnings);
            }
            Err(error) => result.warnings.push(format!("{name}: invalid or unsupported NASA response ({error:#}). Previous data retained. Source: {url}")),
        }
    }
    if result.has_updates() {
        result.site.retrieved_at = Utc::now().format("%Y-%m-%d").to_string();
    }
    result
}

fn fetch(client: &Client, url: &str) -> Result<Value> {
    let response = client
        .get(url)
        .header("Accept", "application/json")
        .send()
        .with_context(|| format!("Could not fetch {url}"))?
        .error_for_status()?;
    if response
        .content_length()
        .is_some_and(|size| size > MAX_DOWNLOAD_BYTES)
    {
        bail!("Response exceeds the 8 MiB limit");
    }
    let mut bytes = Vec::new();
    response
        .take(MAX_DOWNLOAD_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_DOWNLOAD_BYTES {
        bail!("Response exceeds the 8 MiB limit");
    }
    let value: Value = serde_json::from_slice(&bytes)?;
    if !value.is_object() {
        bail!("Expected a JSON object");
    }
    Ok(value)
}

fn number(value: &Value) -> Option<f64> {
    let value = value.as_f64().or_else(|| value.as_str()?.parse().ok())?;
    value.is_finite().then_some(value)
}

fn coordinate(latitude: f64, longitude: f64) -> Result<Point> {
    if !latitude.is_finite()
        || !longitude.is_finite()
        || !(-90.0..=90.0).contains(&latitude)
        || !(-180.0..=360.0).contains(&longitude)
    {
        bail!("Out-of-range Mars coordinate");
    }
    Ok(Point {
        latitude,
        longitude,
    })
}

fn latest_feature(value: &Value) -> Result<&Value> {
    value["features"]
        .as_array()
        .context("No rover features")?
        .iter()
        .filter(|feature| feature["properties"].is_object())
        .max_by(|a, b| {
            number(&a["properties"]["sol"])
                .unwrap_or(-1.0)
                .total_cmp(&number(&b["properties"]["sol"]).unwrap_or(-1.0))
        })
        .context("No valid rover feature")
}

fn apply_rover(
    site: &mut MissionSite,
    value: &Value,
    source: &str,
    warnings: &mut Vec<String>,
) -> Result<()> {
    let properties = &latest_feature(value)?["properties"];
    let point = coordinate(
        number(&properties["lat"]).context("Missing rover latitude")?,
        number(&properties["lon"]).context("Missing rover longitude")?,
    )?;
    let sol = number(&properties["sol"]).context("Missing mission sol")?;
    if sol < 0.0 || sol.fract() != 0.0 || sol > i64::MAX as f64 {
        bail!("Invalid mission sol");
    }
    let sol = sol as i64;
    let altitude = number(&properties["elev_geoid"]).filter(|value| *value > -9000.0);
    let old_rover = site
        .landmarks
        .iter()
        .find(|landmark| landmark.kind == "rover")
        .context("Mission has no rover record")?;
    let rover = Landmark {
        id: old_rover.id.clone(),
        name: site.rover_name.clone(),
        point,
        kind: "rover".into(),
        description: "Latest published NASA end-of-drive localization. Mission sol is the observation timestamp; the source does not supply a UTC observation date. Not live spacecraft telemetry.".into(),
        source_url: source.into(),
        observed_at: None,
        sol: Some(sol),
        elevation_meters: altitude,
    };
    for landmark in &mut site.landmarks {
        if landmark.kind == "rover" {
            *landmark = rover.clone();
        }
    }
    for observation in &mut site.observations {
        let value = match observation.key.as_str() {
            "elevation" => altitude,
            "rover_tilt" => number(&properties["tilt"]),
            _ => None,
        };
        if let Some(value) = value {
            observation.value = Some(value);
            observation.observed_at = None;
            observation.sol = Some(sol);
            observation.coverage = "Published rover waypoint".into();
            observation.source_url = source.into();
        }
    }
    if !contains(site, point) {
        warnings.push(format!("{} sol {sol} is outside bundled map coverage. Its real coordinate is retained; extend imagery before mapping this position.", site.rover_name));
    }
    Ok(())
}

fn contains(site: &MissionSite, point: Point) -> bool {
    let b = &site.map_bounds;
    point.latitude >= b.south
        && point.latitude <= b.north
        && point.longitude >= b.west
        && point.longitude <= b.east
}

fn apply_traverse(site: &mut MissionSite, value: &Value, warnings: &mut Vec<String>) -> Result<()> {
    let features = value["features"]
        .as_array()
        .context("Missing traverse features")?;
    let mut points = Vec::new();
    for feature in features {
        let geometry = &feature["geometry"];
        match geometry["type"].as_str() {
            Some("LineString") => add_line(&geometry["coordinates"], &mut points)?,
            Some("MultiLineString") => {
                for line in geometry["coordinates"]
                    .as_array()
                    .context("Invalid multi-line traverse")?
                {
                    add_line(line, &mut points)?;
                }
            }
            _ => bail!("Unexpected traverse geometry type"),
        }
    }
    if points.len() < 2 {
        bail!("Too few traverse points");
    }
    let outside = points
        .iter()
        .filter(|point| !contains(site, **point))
        .count();
    if outside > 0 {
        warnings.push(format!("{outside} published traverse vertices are outside bundled imagery. Actual coordinates are retained; off-map terrain is not available."));
    }
    site.traverse = points;
    Ok(())
}

fn add_line(value: &Value, points: &mut Vec<Point>) -> Result<()> {
    for value in value.as_array().context("Invalid traverse line")? {
        let vertex = value.as_array().context("Invalid traverse vertex")?;
        if vertex.len() < 2 {
            bail!("Traverse vertex needs longitude and latitude");
        }
        let point = coordinate(
            number(&vertex[1]).context("Invalid latitude")?,
            number(&vertex[0]).context("Invalid longitude")?,
        )?;
        if points.last() != Some(&point) {
            points.push(point);
        }
        if points.len() > 200_000 {
            bail!("Traverse exceeds 200,000 vertices");
        }
    }
    Ok(())
}

fn apply_weather(
    site: &mut MissionSite,
    value: &Value,
    source: &str,
    warnings: &mut Vec<String>,
) -> Result<()> {
    let raw = value
        .get("sols")
        .or_else(|| value.get("soles"))
        .and_then(Value::as_array)
        .context("Missing weather records")?;
    let mut records: Vec<_> = raw.iter().filter(|record| record.is_object()).collect();
    records.sort_by(|a, b| {
        number(&b["sol"])
            .unwrap_or(-1.0)
            .total_cmp(&number(&a["sol"]).unwrap_or(-1.0))
    });
    let mut updated = 0;
    let mut dates = BTreeSet::new();
    let coverage = if site.id == "jezero" {
        "Historical MEDA rover observation"
    } else {
        "Historical REMS rover observation"
    };
    for observation in &mut site.observations {
        let field = match observation.key.as_str() {
            "air_min" => "min_temp",
            "air_max" => "max_temp",
            "surface_min" => "min_gts_temp",
            "surface_max" => "max_gts_temp",
            "pressure" => "pressure",
            "wind_speed" => "wind_speed",
            "wind_direction" => "wind_direction",
            _ => continue,
        };
        for record in &records {
            let Some(value) = number(&record[field]) else {
                continue;
            };
            let Some(date) = record["terrestrial_date"].as_str() else {
                continue;
            };
            let Some(sol) = number(&record["sol"]) else {
                continue;
            };
            if NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err()
                || sol < 0.0
                || sol.fract() != 0.0
                || sol > i64::MAX as f64
                || value <= -9000.0
                || (observation.unit == "°C" && value < -273.15)
                || (observation.unit == "Pa" && value < 0.0)
                || (observation.key == "wind_speed" && value < 0.0)
            {
                continue;
            }
            observation.value = Some(value);
            observation.observed_at = Some(date.into());
            observation.sol = Some(sol as i64);
            observation.coverage = coverage.into();
            observation.note = "Latest valid value for this field in the fetched NASA public outreach feed. Rover-site context only; not a measurement at an arbitrary selected map pin, a current reading, or a forecast. Rover orientation, shadows, and sampling affect these readings; use calibrated PDS products for scientific operations.".into();
            observation.source_url = source.into();
            dates.insert(date.to_owned());
            updated += 1;
            break;
        }
    }
    if updated == 0 {
        bail!("No valid dated environmental values");
    }
    warnings.push(format!("Weather source observation dates: {}. Fetching a source does not make historical measurements current.", dates.into_iter().collect::<Vec<_>>().join(", ")));
    Ok(())
}

/// Source endpoints are fixed by mission and never taken from an untrusted
/// packet or map click.
pub fn source_urls(site_id: &str) -> Result<[String; 3]> {
    let (mission, category) = match site_id {
        "jezero" => ("M20", "mars2020"),
        "gale" => ("MSL", "msl"),
        _ => return Err(anyhow!("Unsupported crater")),
    };
    Ok([
        format!(
            "https://mars.nasa.gov/mmgis-maps/{mission}/Layers/json/{mission}_waypoints_current.json"
        ),
        format!("https://mars.nasa.gov/mmgis-maps/{mission}/Layers/json/{mission}_traverse.json"),
        format!("https://mars.nasa.gov/rss/api/?feed=weather&category={category}&feedtype=json"),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn site(id: &str) -> MissionSite {
        let catalog: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/data/missions.json"
        )))
        .unwrap();
        serde_json::from_value(
            catalog["sites"]
                .as_array()
                .unwrap()
                .iter()
                .find(|site| site["id"] == id)
                .unwrap()
                .clone(),
        )
        .unwrap()
    }

    #[test]
    fn source_sol_is_not_replaced_with_retrieval_timestamp() {
        let mut site = site("jezero");
        let source: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/data/jezero_waypoint.geojson"
        )))
        .unwrap();
        apply_rover(
            &mut site,
            &source,
            "https://mars.nasa.gov/source",
            &mut vec![],
        )
        .unwrap();
        let rover = site
            .landmarks
            .iter()
            .find(|landmark| landmark.kind == "rover")
            .unwrap();
        assert_eq!(rover.sol, Some(2022));
        assert_eq!(rover.observed_at, None);
        assert_eq!(rover.point.latitude, 18.43559434);
    }

    #[test]
    fn dated_real_weather_and_missing_fields_are_preserved() {
        let snapshots: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/data/weather_snapshots.json"
        )))
        .unwrap();
        let mut site = site("jezero");
        let mut warnings = vec![];
        let data = json!({ "sols": snapshots["jezero"]["records"] });
        apply_weather(
            &mut site,
            &data,
            "https://mars.nasa.gov/weather",
            &mut warnings,
        )
        .unwrap();
        let pressure = site
            .observations
            .iter()
            .find(|o| o.key == "pressure")
            .unwrap();
        assert_eq!(pressure.value, Some(778.9));
        assert_eq!(pressure.observed_at.as_deref(), Some("2024-04-27"));
        assert!(
            site.observations
                .iter()
                .find(|o| o.key == "wind_speed")
                .unwrap()
                .value
                .is_none()
        );
    }

    #[test]
    fn non_numeric_missing_data_are_never_zero() {
        assert_eq!(number(&json!("--")), None);
        assert_eq!(number(&Value::Null), None);
        assert_eq!(number(&json!("777")), Some(777.0));
    }
}
