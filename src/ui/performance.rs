//! Optional native smoke-test instrumentation.
//!
//! A timed GPUI render callback measures synchronous CPU work in that callback.
//! Its start-to-start interval is **not** GPU presentation time or display FPS.
//! The app's scripted driver records checks against real application state.
//! Nothing is enabled unless TERRAMARS_PROBE specifies an output JSON path or
//! the native command-line driver explicitly calls `configure`.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const MAX_SAMPLES_PER_KIND: usize = 20_000;

thread_local! {
    static RECORDER: RefCell<Option<Recorder>> = RefCell::new(Recorder::from_environment());
}

/// Enables a probe from `--smoke-test` without mutating process-wide environment
/// variables after GPUI/worker threads have started. Call on the UI thread.
pub fn configure(path: impl Into<PathBuf>) {
    RECORDER.with(|recorder| *recorder.borrow_mut() = Some(Recorder::new(path.into())));
}

pub fn enabled() -> bool {
    RECORDER.with(|recorder| recorder.borrow().is_some())
}

/// Marks an actual interaction stage, e.g. home, jezero_map, details, route,
/// mission_log, or gale_map. It does not manufacture frames or test outcomes.
pub fn set_stage(stage: &str) {
    RECORDER.with(|recorder| {
        let mut recorder = recorder.borrow_mut();
        if let Some(recorder) = recorder.as_mut()
            && recorder.stage != stage
        {
            eprintln!("TerraMars probe stage: {stage}");
            recorder.stage = stage.to_owned();
            recorder.previous_starts.clear();
            recorder.transitions.push(json!({
                "stage": stage,
                "elapsed_ms": elapsed_ms(recorder.started.elapsed()),
            }));
        }
    });
}

/// Put `let _timing = performance::render_scope();` at the start of the real
/// app Render implementation. The guard records elapsed CPU wall time on drop.
pub fn render_scope() -> Option<CpuScope> {
    scope("app_render_callback")
}

/// Put this guard inside the actual map canvas/element build or paint callback,
/// rather than around a timer that merely requests a redraw.
pub fn map_scope() -> Option<CpuScope> {
    scope("map_callback")
}

/// An additional named callback may be measured independently. Nested callback
/// durations overlap; consumers must not sum them as a total frame duration.
pub fn scope(kind: &'static str) -> Option<CpuScope> {
    RECORDER.with(|recorder| {
        let mut recorder = recorder.borrow_mut();
        let recorder = recorder.as_mut()?;
        let started = Instant::now();
        let stage = recorder.stage.clone();
        let key = (stage.clone(), kind);
        let interval_ms = recorder
            .previous_starts
            .insert(key, started)
            .map(|previous| elapsed_ms(started.duration_since(previous)));
        Some(CpuScope {
            stage,
            kind,
            started,
            started_ms: elapsed_ms(started.duration_since(recorder.started)),
            interval_ms,
        })
    })
}

/// Records a genuine scripted assertion. `passed` must come from current app
/// state after the action is dispatched, not from the stage's elapsed time.
pub fn check(name: &str, passed: bool, detail: impl Into<String>) {
    RECORDER.with(|recorder| {
        if let Some(recorder) = recorder.borrow_mut().as_mut() {
            recorder.checks.push(json!({
                "name": name,
                "passed": passed,
                "detail": detail.into(),
                "stage": recorder.stage,
                "elapsed_ms": elapsed_ms(recorder.started.elapsed()),
            }));
        }
    });
}

/// Writes the current measurement snapshot. It does not quit GPUI; the native
/// smoke-test driver calls this before exiting after its final state checks.
/// The returned path is None when no probe is enabled.
pub fn finish() -> io::Result<Option<PathBuf>> {
    RECORDER.with(|recorder| {
        let recorder = recorder.borrow();
        let Some(recorder) = recorder.as_ref() else {
            return Ok(None);
        };
        let document = recorder.document();
        if let Some(parent) = recorder.path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(&document).map_err(io::Error::other)?;
        fs::write(&recorder.path, bytes)?;
        Ok(Some(recorder.path.clone()))
    })
}

#[must_use = "Keep the guard alive until the measured callback returns"]
pub struct CpuScope {
    stage: String,
    kind: &'static str,
    started: Instant,
    started_ms: f64,
    interval_ms: Option<f64>,
}

impl Drop for CpuScope {
    fn drop(&mut self) {
        let cpu_ms = elapsed_ms(self.started.elapsed());
        RECORDER.with(|recorder| {
            if let Some(recorder) = recorder.borrow_mut().as_mut() {
                let samples = recorder
                    .samples
                    .entry(self.stage.clone())
                    .or_default()
                    .entry(self.kind)
                    .or_default();
                if samples.rows.len() >= MAX_SAMPLES_PER_KIND {
                    samples.dropped += 1;
                    return;
                }
                samples.rows.push(Sample {
                    started_ms: self.started_ms,
                    callback_cpu_ms: cpu_ms,
                    interval_ms: self.interval_ms,
                });
            }
        });
    }
}

struct Recorder {
    path: PathBuf,
    started: Instant,
    stage: String,
    previous_starts: BTreeMap<(String, &'static str), Instant>,
    samples: BTreeMap<String, BTreeMap<&'static str, Samples>>,
    transitions: Vec<Value>,
    checks: Vec<Value>,
}

impl Recorder {
    fn from_environment() -> Option<Self> {
        env::var_os("TERRAMARS_PROBE")
            .filter(|path| !path.is_empty())
            .map(|path| Self::new(PathBuf::from(path)))
    }

    fn new(path: PathBuf) -> Self {
        Self {
            path,
            started: Instant::now(),
            stage: "startup".into(),
            previous_starts: BTreeMap::new(),
            samples: BTreeMap::new(),
            transitions: vec![json!({ "stage": "startup", "elapsed_ms": 0.0 })],
            checks: Vec::new(),
        }
    }

    fn document(&self) -> Value {
        let mut stages = serde_json::Map::new();
        for (stage, kinds) in &self.samples {
            let mut callbacks = serde_json::Map::new();
            for (kind, samples) in kinds {
                let cpu: Vec<_> = samples.rows.iter().map(|s| s.callback_cpu_ms).collect();
                let intervals: Vec<_> = samples.rows.iter().filter_map(|s| s.interval_ms).collect();
                callbacks.insert(
                    (*kind).into(),
                    json!({
                        "callback_cpu_ms": statistics(&cpu),
                        "callback_start_interval_ms": statistics(&intervals),
                        "dropped_samples": samples.dropped,
                        "raw_samples": samples.rows.iter().map(|sample| json!({
                            "started_ms": sample.started_ms,
                            "callback_cpu_ms": sample.callback_cpu_ms,
                            "previous_same_stage_interval_ms": sample.interval_ms,
                        })).collect::<Vec<_>>(),
                    }),
                );
            }
            stages.insert(stage.clone(), Value::Object(callbacks));
        }
        let all_passed = (!self.checks.is_empty())
            .then(|| self.checks.iter().all(|check| check["passed"] == true));
        json!({
            "schema_version": 1,
            "application": "TerraMars Rust/GPUI",
            "platform": env::consts::OS,
            "architecture": env::consts::ARCH,
            "build_profile": if cfg!(debug_assertions) { "debug" } else { "release" },
            "elapsed_wall_ms": elapsed_ms(self.started.elapsed()),
            "measurement_notes": [
                "callback_cpu_ms measures real synchronous CPU elapsed time inside an instrumented callback, not complete GPU/render/presentation latency.",
                "callback_start_interval_ms measures successive same-kind callback start times within one interaction stage. It is not displayed-frame timing or FPS.",
                "Nested callback durations overlap; do not add app and map timings together.",
                "Idle callbacks and OS scheduling influence intervals. Compare runs only with matching build profile, hardware, stage script, and workload.",
                "State checks are recorded by the native scripted UI driver after real actions, independently of timing samples.",
            ],
            "checks_count": self.checks.len(),
            "all_checks_passed": all_passed,
            "checks": self.checks,
            "stage_transitions": self.transitions,
            "stages": stages,
        })
    }
}

#[derive(Default)]
struct Samples {
    rows: Vec<Sample>,
    dropped: usize,
}

struct Sample {
    started_ms: f64,
    callback_cpu_ms: f64,
    interval_ms: Option<f64>,
}

fn elapsed_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn statistics(values: &[f64]) -> Value {
    if values.is_empty() {
        return json!({ "count": 0, "mean": null, "p50": null, "p95": null, "max": null });
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let percentile = |p: f64| {
        let index = ((sorted.len() as f64 * p).ceil() as usize).saturating_sub(1);
        sorted[index.min(sorted.len() - 1)]
    };
    json!({
        "count": sorted.len(),
        "mean": sorted.iter().sum::<f64>() / sorted.len() as f64,
        "p50": percentile(0.5),
        "p95": percentile(0.95),
        "max": sorted.last(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_records_actual_scopes_and_state_checks_without_gpu_fps_claims() {
        let path =
            env::temp_dir().join(format!("terramars-probe-test-{}.json", std::process::id()));
        configure(&path);
        set_stage("map");
        {
            let _render = render_scope();
            let _map = map_scope();
        }
        {
            let _render = render_scope();
        }
        check("selected actual point", true, "Verified application state");
        assert_eq!(finish().unwrap(), Some(path.clone()));
        let document: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            document["stages"]["map"]["app_render_callback"]["callback_cpu_ms"]["count"],
            2
        );
        assert_eq!(
            document["stages"]["map"]["app_render_callback"]["callback_start_interval_ms"]["count"],
            1
        );
        assert_eq!(
            document["stages"]["map"]["map_callback"]["callback_cpu_ms"]["count"],
            1
        );
        assert_eq!(document["all_checks_passed"], true);
        assert!(document.get("fps").is_none());
        let _ = fs::remove_file(path);
        RECORDER.with(|recorder| *recorder.borrow_mut() = None);
    }

    #[test]
    fn stage_transitions_do_not_create_fake_frame_samples() {
        configure(env::temp_dir().join("terramars-probe-no-frames.json"));
        set_stage("home");
        set_stage("details");
        RECORDER.with(|recorder| {
            let recorder = recorder.borrow();
            let document = recorder.as_ref().unwrap().document();
            assert!(document["stages"].as_object().unwrap().is_empty());
            assert_eq!(document["all_checks_passed"], Value::Null);
        });
        RECORDER.with(|recorder| *recorder.borrow_mut() = None);
    }
}
