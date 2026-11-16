use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow, ensure};
use chrono::{DateTime, Utc};
use directories::{BaseDirs, ProjectDirs};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use super::{RouteEstimate, UnitSystem};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub unit_system: UnitSystem,
    pub speed_kmh: f64,
    pub earth_time: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            unit_system: UnitSystem::Metric,
            speed_kmh: 3.0,
            earth_time: false,
            extra: BTreeMap::new(),
        }
    }
}

impl<'de> Deserialize<'de> for Settings {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let mut raw = BTreeMap::<String, Value>::deserialize(deserializer)?;
        let unit_system = match raw.remove("unitSystem").as_ref().and_then(Value::as_str) {
            Some("scientific") => UnitSystem::Scientific,
            _ => UnitSystem::Metric,
        };
        let speed_kmh = raw
            .remove("speedKmh")
            .and_then(|v| v.as_f64())
            .filter(|v| v.is_finite() && *v > 0.0 && *v <= 15.0)
            .unwrap_or(3.0);
        let earth_time = raw
            .remove("earthTime")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        Ok(Self {
            unit_system,
            speed_kmh,
            earth_time,
            extra: raw,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MissionState {
    pub schema_version: u32,
    pub settings: Settings,
    pub planned_routes: Vec<PlannedRouteRecord>,
    /// Earlier visited places and packet fields remain inert but round-trip.
    #[serde(flatten)]
    pub legacy_fields: BTreeMap<String, Value>,
}

impl Default for MissionState {
    fn default() -> Self {
        Self {
            schema_version: 1,
            settings: Settings::default(),
            planned_routes: vec![],
            legacy_fields: BTreeMap::new(),
        }
    }
}

impl MissionState {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == 1,
            "Unsupported saved mission state schema version {}",
            self.schema_version
        );
        for route in &self.planned_routes {
            route.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedRouteRecord {
    pub crater_id: String,
    pub start_label: String,
    pub end_label: String,
    pub mode: String,
    pub route: RouteEstimate,
    pub planned_at: DateTime<Utc>,
}

impl PlannedRouteRecord {
    /// Whether this record describes the same journey as `route`: same crater,
    /// endpoints, travel mode and resulting geometry. Two plans of an identical
    /// journey are the same mission, so the log keeps only one of them.
    pub fn same_journey(
        &self,
        crater_id: &str,
        start: &str,
        end: &str,
        mode: &str,
        route: &RouteEstimate,
    ) -> bool {
        self.crater_id == crater_id
            && self.start_label == start
            && self.end_label == end
            && self.mode == mode
            && self.route.name == route.name
            && self.route.points == route.points
            && (self.route.distance_km - route.distance_km).abs() < f64::EPSILON
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.crater_id == "jezero" || self.crater_id == "gale",
            "Unknown crater in saved route"
        );
        ensure!(
            !self.start_label.trim().is_empty()
                && !self.end_label.trim().is_empty()
                && !self.mode.trim().is_empty(),
            "Saved route is missing its endpoint labels or mode"
        );
        self.route.validate()
    }
}

#[derive(Debug)]
pub struct MissionStore {
    pub directory: PathBuf,
    pub state: MissionState,
}

impl MissionStore {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
            state: MissionState::default(),
        }
    }

    pub fn load(directory: impl Into<PathBuf>) -> Result<Self> {
        let mut store = Self::new(directory);
        store.reload()?;
        Ok(store)
    }

    pub fn default_directory() -> Result<PathBuf> {
        ProjectDirs::from("org", "TerraMars", "TerraMars")
            .map(|dirs| dirs.data_local_dir().join("mission"))
            .ok_or_else(|| anyhow!("Could not locate the local application data directory"))
    }

    pub fn load_default() -> Result<Self> {
        let directory = Self::default_directory()?;
        let mut store = Self::load(directory)?;
        if !store.file_path().exists() {
            for legacy in legacy_state_candidates() {
                if legacy.is_file() {
                    let state: MissionState = serde_json::from_slice(&fs::read(&legacy)?)
                        .with_context(|| {
                            format!(
                                "Could not read earlier Flutter mission history at {}",
                                legacy.display()
                            )
                        })?;
                    state.validate()?;
                    store.state = state;
                    // Copy state into the native application's location;
                    // leave the original Flutter file untouched.
                    store.save()?;
                    break;
                }
            }
        }
        Ok(store)
    }

    pub fn file_path(&self) -> PathBuf {
        self.directory.join("mission_state.json")
    }

    pub fn reload(&mut self) -> Result<()> {
        let path = self.file_path();
        let backup = self.directory.join("mission_state.json.bak");
        let source = if path.is_file() {
            path
        } else if backup.is_file() {
            backup
        } else {
            return Ok(());
        };
        let state: MissionState = serde_json::from_slice(
            &fs::read(&source)
                .with_context(|| format!("Could not open mission history {}", source.display()))?,
        )
        .context("Mission history is not valid JSON; the existing file has not been replaced")?;
        state.validate()?;
        self.state = state;
        Ok(())
    }

    pub fn save(&self) -> Result<()> {
        self.state.validate()?;
        ensure!(
            self.state.settings.speed_kmh.is_finite() && self.state.settings.speed_kmh > 0.0,
            "Travel speed preference must be finite and positive"
        );
        fs::create_dir_all(&self.directory).context("Could not create mission data directory")?;
        let temporary = self
            .directory
            .join(format!("mission_state.{}.tmp", std::process::id()));
        let mut file =
            fs::File::create(&temporary).context("Could not create temporary mission history")?;
        let data = serde_json::to_vec_pretty(&self.state)?;
        file.write_all(&data)?;
        file.sync_all()?;
        drop(file);
        replace_file(&temporary, &self.file_path())
            .context("Could not replace mission history; previous data is preserved")
    }

    /// Removes every saved plan. The previous history is restored if the new
    /// file cannot be written, so a failed clear never loses data silently.
    pub fn clear_routes(&mut self) -> Result<()> {
        let previous = std::mem::take(&mut self.state.planned_routes);
        if let Err(error) = self.save() {
            self.state.planned_routes = previous;
            return Err(error);
        }
        Ok(())
    }

    /// Returns `true` when a new record was written, or `false` when the same
    /// journey is already the newest log entry. Repeating "Plan route" for an
    /// unchanged journey must not append another identical mission.
    pub fn record_route(
        &mut self,
        crater_id: &str,
        start_label: &str,
        end_label: &str,
        mode: &str,
        route: &RouteEstimate,
    ) -> Result<bool> {
        let record = PlannedRouteRecord {
            crater_id: crater_id.into(),
            start_label: start_label.into(),
            end_label: end_label.into(),
            mode: mode.into(),
            route: route.clone(),
            planned_at: Utc::now(),
        };
        record.validate()?;
        if self.state.planned_routes.first().is_some_and(|latest| {
            latest.same_journey(crater_id, start_label, end_label, mode, route)
        }) {
            return Ok(false);
        }
        self.state.planned_routes.insert(0, record);
        if let Err(error) = self.save() {
            self.state.planned_routes.remove(0);
            return Err(error);
        }
        Ok(true)
    }
}

fn legacy_state_candidates() -> Vec<PathBuf> {
    let Some(base) = BaseDirs::new() else {
        return vec![];
    };
    vec![
        base.data_dir()
            .join("com.example.terra_mars/mission/mission_state.json"),
        base.data_dir()
            .join("com.example.terraMars/mission/mission_state.json"),
        base.data_dir()
            .join("com.example/terra_mars/mission/mission_state.json"),
        base.home_dir()
            .join("Library/Application Support/com.example.terraMars/mission/mission_state.json"),
    ]
}

/// Unix rename replaces atomically. Windows std::fs::rename does not replace
/// an existing destination, so use a recoverable backup/restore sequence there.
fn replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    #[cfg(not(target_os = "windows"))]
    {
        fs::rename(source, destination)
    }
    #[cfg(target_os = "windows")]
    {
        if !destination.exists() {
            return fs::rename(source, destination);
        }
        let backup = destination.with_extension("json.bak");
        if backup.exists() {
            fs::remove_file(&backup)?;
        }
        fs::rename(destination, &backup)?;
        match fs::rename(source, destination) {
            Ok(()) => {
                let _ = fs::remove_file(backup);
                Ok(())
            }
            Err(error) => {
                let _ = fs::rename(backup, destination);
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Point, RoutePlanner};

    fn test_directory(name: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "terra_mars_{name}_{}_{}",
            std::process::id(),
            stamp
        ))
    }

    #[test]
    fn route_history_and_preferences_survive_reopening_and_repeated_saves() {
        let directory = test_directory("history");
        let mut store = MissionStore::load(&directory).unwrap();
        let route = RoutePlanner::plan(
            Point::new(18.4, 77.5),
            Point::new(18.41, 77.51),
            3.0,
            &[],
            &[],
        )
        .unwrap()
        .remove(0);
        store.state.settings.unit_system = UnitSystem::Scientific;
        store
            .record_route(
                "jezero",
                "Coordinate A",
                "Coordinate B",
                "EVA traverse",
                &route,
            )
            .unwrap();
        store.save().unwrap();
        let reopened = MissionStore::load(&directory).unwrap();
        assert_eq!(reopened.state.settings.unit_system, UnitSystem::Scientific);
        assert_eq!(reopened.state.planned_routes.len(), 1);
        assert_eq!(reopened.state.planned_routes[0].crater_id, "jezero");
        assert_eq!(reopened.state.planned_routes[0].route.points, route.points);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn repeating_an_unchanged_route_does_not_append_a_duplicate_mission() {
        let directory = test_directory("duplicate");
        let mut store = MissionStore::load(&directory).unwrap();
        let route = RoutePlanner::plan(
            Point::new(18.4, 77.5),
            Point::new(18.41, 77.51),
            3.0,
            &[],
            &[],
        )
        .unwrap()
        .remove(0);
        let log = |store: &mut MissionStore| {
            store
                .record_route("jezero", "Start", "Destination", "On foot", &route)
                .unwrap()
        };
        assert!(log(&mut store), "first plan is a new mission");
        assert!(!log(&mut store), "repeated plan is the same mission");
        assert!(!log(&mut store), "spamming the button stays at one mission");
        assert_eq!(store.state.planned_routes.len(), 1);
        // A different journey is still recorded normally.
        assert!(
            store
                .record_route("jezero", "Start", "Elsewhere", "On foot", &route)
                .unwrap()
        );
        assert_eq!(store.state.planned_routes.len(), 2);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn earlier_flutter_fields_round_trip_without_reactivating_them() {
        let directory = test_directory("legacy");
        fs::create_dir_all(&directory).unwrap();
        let original = serde_json::json!({
            "schemaVersion": 1, "settings": {"speedKmh":"bad", "unitSystem":"scientific", "extraPreference": true},
            "plannedRoutes": [], "visited": [{"legacy":"keep"}], "packets": [{"legacy":"keep too"}]
        });
        fs::write(
            directory.join("mission_state.json"),
            serde_json::to_vec(&original).unwrap(),
        )
        .unwrap();
        let store = MissionStore::load(&directory).unwrap();
        assert_eq!(store.state.settings.speed_kmh, 3.0);
        store.save().unwrap();
        let saved: Value = serde_json::from_slice(&fs::read(store.file_path()).unwrap()).unwrap();
        assert_eq!(saved["visited"], original["visited"]);
        assert_eq!(saved["packets"], original["packets"]);
        assert_eq!(saved["settings"]["extraPreference"], true);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn invalid_saved_route_or_schema_does_not_overwrite_existing_file() {
        let directory = test_directory("invalid");
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("mission_state.json");
        fs::write(&path, b"{\"schemaVersion\":99}").unwrap();
        assert!(MissionStore::load(&directory).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "{\"schemaVersion\":99}");
        fs::remove_dir_all(directory).unwrap();
    }
}
