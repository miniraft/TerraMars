use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};

/// Planetocentric latitude and east-positive longitude on Mars, in degrees.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub latitude: f64,
    pub longitude: f64,
}

pub type GeographicPoint = Point;

impl Point {
    pub const fn new(latitude: f64, longitude: f64) -> Self {
        Self {
            latitude,
            longitude,
        }
    }

    pub fn validate(self) -> Result<()> {
        ensure!(
            self.latitude.is_finite() && self.longitude.is_finite(),
            "Coordinates must be finite"
        );
        ensure!(
            (-90.0..=90.0).contains(&self.latitude),
            "Latitude must be between −90 and 90 degrees"
        );
        ensure!(
            (-180.0..=360.0).contains(&self.longitude),
            "Longitude must be between −180 and 360 degrees east"
        );
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MapBounds {
    pub west: f64,
    pub south: f64,
    pub east: f64,
    pub north: f64,
}

impl MapBounds {
    pub fn center(self) -> Point {
        Point::new(
            (self.north + self.south) / 2.0,
            (self.east + self.west) / 2.0,
        )
    }

    pub fn contains(self, point: Point) -> bool {
        point.latitude >= self.south
            && point.latitude <= self.north
            && point.longitude >= self.west
            && point.longitude <= self.east
    }

    pub fn validate(self) -> Result<()> {
        Point::new(self.south, self.west).validate()?;
        Point::new(self.north, self.east).validate()?;
        ensure!(
            self.south < self.north && self.west < self.east,
            "Invalid map bounds"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceLink {
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub credit: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Landmark {
    pub id: String,
    pub name: String,
    pub point: Point,
    pub kind: String,
    pub description: String,
    pub source_url: String,
    #[serde(default)]
    pub observed_at: Option<String>,
    #[serde(default)]
    pub sol: Option<i64>,
    #[serde(default)]
    pub elevation_meters: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    pub key: String,
    pub label: String,
    pub category: String,
    pub unit: String,
    pub coverage: String,
    pub note: String,
    pub source_url: String,
    #[serde(default)]
    pub value: Option<f64>,
    #[serde(default)]
    pub observed_at: Option<String>,
    #[serde(default)]
    pub sol: Option<i64>,
}

impl Observation {
    pub fn available(&self) -> bool {
        self.value.is_some_and(f64::is_finite)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MissionSite {
    pub id: String,
    pub name: String,
    pub subtitle: String,
    pub description: String,
    pub latitude: f64,
    pub longitude: f64,
    pub diameter_km: f64,
    pub rover_name: String,
    pub map_asset: String,
    #[serde(default)]
    pub topographic_asset: Option<String>,
    pub map_bounds: MapBounds,
    pub map_credit: String,
    pub map_source_url: String,
    pub landmarks: Vec<Landmark>,
    pub observations: Vec<Observation>,
    pub sources: Vec<SourceLink>,
    pub traverse: Vec<Point>,
    pub retrieved_at: String,
}

impl MissionSite {
    pub fn center(&self) -> Point {
        Point::new(self.latitude, self.longitude)
    }

    pub fn rover(&self) -> Option<&Landmark> {
        self.landmarks.iter().find(|place| place.kind == "rover")
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.id == "jezero" || self.id == "gale",
            "Unknown crater identifier"
        );
        self.center().validate()?;
        self.map_bounds.validate()?;
        ensure!(
            self.diameter_km.is_finite() && self.diameter_km > 0.0,
            "Invalid crater diameter"
        );
        ensure!(self.rover().is_some(), "Missing rover waypoint");
        for landmark in &self.landmarks {
            landmark.point.validate()?;
            if landmark.elevation_meters.is_some_and(|v| !v.is_finite()) {
                bail!("Non-finite landmark elevation");
            }
        }
        for point in &self.traverse {
            point.validate()?;
        }
        for observation in &self.observations {
            ensure!(
                !observation.value.is_some_and(|v| !v.is_finite()),
                "Non-finite observation"
            );
        }
        Ok(())
    }
}

/// Loads the original NASA/USGS-derived bundled snapshot, unchanged by the UI.
pub fn catalog() -> Result<Vec<MissionSite>> {
    #[derive(Deserialize)]
    struct Catalog {
        sites: Vec<MissionSite>,
    }
    let catalog: Catalog = serde_json::from_str(include_str!("../../assets/data/missions.json"))
        .context("Could not parse the bundled Mars mission catalog")?;
    ensure!(
        catalog.sites.len() == 2,
        "Expected Jezero and Gale datasets"
    );
    for site in &catalog.sites {
        site.validate()
            .with_context(|| format!("Invalid {} dataset", site.id))?;
    }
    Ok(catalog.sites)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_catalog_has_georeferenced_rover_positions_and_provenance() {
        let sites = catalog().unwrap();
        assert_eq!(
            sites.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            ["jezero", "gale"]
        );
        for site in sites {
            assert!(site.map_bounds.contains(site.rover().unwrap().point));
            assert!(site.traverse.len() > 100);
            assert!(site.sources.iter().any(|s| s.url.contains("nasa.gov")));
            assert!(site.observations.iter().any(Observation::available));
            assert!(
                site.observations
                    .iter()
                    .filter(|o| o.available())
                    .all(|o| !o.source_url.is_empty())
            );
        }
    }

    #[test]
    fn coordinates_reject_non_finite_or_off_planet_latitudes() {
        assert!(Point::new(91.0, 77.0).validate().is_err());
        assert!(Point::new(18.0, f64::NAN).validate().is_err());
        assert!(Point::new(-4.5, 137.0).validate().is_ok());
    }
}
