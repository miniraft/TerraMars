//! Optional offline voice control for TerraMars.
//!
//! Two layers live here:
//! 1. A command vocabulary + parser that turns recognized speech into
//!    [`VoiceCommand`]s the app can execute (pure, no audio, testable).
//! 2. An optional loopback HTTP bridge client (retained from the archived
//!    Vosk bridge) plus the wiring to embed the [`vox`](https://github.com/mrtozner/vox)
//!    voice AI framework behind the `voice-vox` feature.
//!
//! Microphone audio is handled entirely by the local vox pipeline or the
//! archived `tools/voice_bridge.py`; nothing is uploaded or sent to a remote
//! service from here.

use std::collections::VecDeque;
use std::io::Read;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use reqwest::Url;
use reqwest::blocking::{Client, RequestBuilder};
use serde::Deserialize;
use serde_json::{Value, json};

pub const DEFAULT_BRIDGE: &str = "http://127.0.0.1:8765";
const MAX_RESPONSE_BYTES: u64 = 64 * 1024;

// ---------------------------------------------------------------------------
// Command vocabulary
// ---------------------------------------------------------------------------

/// A single recognized voice command, ready for the app to execute.
#[derive(Clone, Debug, PartialEq)]
pub enum VoiceCommand {
    /// Plan a route between the currently set start and destination.
    PlanRoute,
    /// Speak the currently selected route out loud (TTS).
    SpeakRoute,
    /// Speak the conditions at the currently selected point (TTS).
    SpeakConditions,
    /// Set the start point to the currently inspected map point.
    StartHere,
    /// Set the end point to the currently inspected map point.
    EndHere,
    /// Swap start and end.
    SwapEndpoints,
    /// Travel mode: on foot.
    ModeOnFoot,
    /// Travel mode: rover.
    ModeRover,
    /// Increase travel speed by 0.5 km/h.
    SpeedUp,
    /// Decrease travel speed by 0.5 km/h.
    SpeedDown,
    /// Set travel speed to exactly `km/h` (must be finite, > 0, <= 15).
    SetSpeed(f64),
    /// Units: metric.
    UnitsMetric,
    /// Units: scientific.
    UnitsScientific,
    /// Clock: Earth UTC.
    TimeUtc,
    /// Clock: Mars local time (AMT when no site, LMST at a site).
    TimeAmt,
    /// Open the info / conditions panel.
    OpenInfo,
    /// Switch the detail view to conditions.
    ShowConditions,
    /// Switch the detail view to visuals (map layers).
    ShowVisuals,
    /// Open Jezero Crater.
    OpenJezero,
    /// Open Gale Crater.
    OpenGale,
    /// Open the mission log.
    OpenMissionLog,
    /// Zoom the map in.
    ZoomIn,
    /// Zoom the map out.
    ZoomOut,
    /// Set map bearing to north.
    BearingNorth,
    /// Set map bearing to east.
    BearingEast,
    /// Set map bearing to south.
    BearingSouth,
    /// Set map bearing to west.
    BearingWest,
    /// Scroll the focused detail panel up.
    ScrollUp,
    /// Scroll the focused detail panel down.
    ScrollDown,
    /// Scroll the conditions panel to the "Air low" record.
    ScrollToAirLow,
    /// Scroll the conditions panel to the "Air high" record.
    ScrollToAirHigh,
    /// Toggle the MOLA terrain relief layer.
    ToggleTerrain,
    /// Toggle sourced landmarks.
    ToggleLandmarks,
    /// Toggle rover position.
    ToggleRover,
    /// Toggle recorded rover traverse.
    ToggleTraverse,
    /// Toggle day/night shading.
    ToggleTerminator,
    /// Toggle danger zones (steep terrain).
    ToggleDangers,
    /// Stop the continuous listening loop.
    StopListening,
    /// Any other recognized phrase that maps to a status hint but no state change.
    Unknown(String),
}

/// Shared queue of recognized commands for the foreground UI to drain.
///
/// The voice pipeline (background task) pushes commands here; TerraMars
/// drains and executes them on the GPUI event loop where it owns `Context`.
#[derive(Clone, Default)]
pub struct VoiceCommandQueue {
    inner: Arc<Mutex<VecDeque<VoiceCommand>>>,
}

impl VoiceCommandQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a command recognized by the voice pipeline.
    pub fn push(&self, command: VoiceCommand) {
        if let Ok(mut q) = self.inner.lock() {
            q.push_back(command);
        }
    }

    /// Drain all pending commands into `out`.
    pub fn drain(&self, out: &mut Vec<VoiceCommand>) {
        if let Ok(mut q) = self.inner.lock() {
            while let Some(cmd) = q.pop_front() {
                out.push(cmd);
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.inner.lock().map(|q| q.is_empty()).unwrap_or(true)
    }
}

impl Default for VoiceCommand {
    fn default() -> Self {
        VoiceCommand::Unknown(String::new())
    }
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// Normalize free-form speech into lowercase alphanumeric words.
fn normalize(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c.is_whitespace() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Strip a single leading/trailing polite frame like "please ..." / "... please".
fn frame(text: &str) -> &str {
    let t = text.trim_start_matches("please ").trim_end_matches(" please");
    t.trim_start_matches("can you ").trim_end_matches(" please")
}

/// Extract the first trailing number from a phrase, if any.
fn first_number(text: &str) -> Option<f64> {
    text.split_whitespace()
        .filter_map(|w| w.parse::<f64>().ok())
        .next()
}

/// Parse a transcript into a command. Exact phrases and keyword matches both
/// work, so "zoom in", "please zoom in", and "zoom in please" all match.
pub fn parse_command(text: &str) -> VoiceCommand {
    let raw = normalize(text);
    let framed = frame(&raw);

    // Phrase matches (longest / most specific first). Each arm returns so a
    // phrase like "speed up" wins before the numeric-speed check below.
    match framed {
        "plan route" => return VoiceCommand::PlanRoute,
        "speak route" | "read route" | "say the route" => return VoiceCommand::SpeakRoute,
        "speak conditions" | "read conditions" | "say the conditions" => {
            return VoiceCommand::SpeakConditions
        }
        "start here" => return VoiceCommand::StartHere,
        "end here" => return VoiceCommand::EndHere,
        "swap" | "swap endpoints" => return VoiceCommand::SwapEndpoints,
        "on foot" => return VoiceCommand::ModeOnFoot,
        "rover" | "rover mode" => return VoiceCommand::ModeRover,
        "speed up" | "faster" | "increase speed" => return VoiceCommand::SpeedUp,
        "slow down" | "slower" | "decrease speed" => return VoiceCommand::SpeedDown,
        "metric" | "switch to metric" | "use metric" => return VoiceCommand::UnitsMetric,
        "scientific" | "switch to scientific" | "use scientific" => {
            return VoiceCommand::UnitsScientific
        }
        "utc" | "switch to utc" | "earth time" | "use utc" => return VoiceCommand::TimeUtc,
        "amt" | "mars time" | "local mars time" | "switch to amt" | "use amt" => {
            return VoiceCommand::TimeAmt
        }
        "open info" | "info tab" | "show info" | "conditions tab" => return VoiceCommand::OpenInfo,
        "show conditions" | "conditions" => return VoiceCommand::ShowConditions,
        "show visuals" | "visuals" | "layers" => return VoiceCommand::ShowVisuals,
        "open jezero" | "open jezero crater" | "jezero crater" => return VoiceCommand::OpenJezero,
        "open gale" | "open gale crater" | "gale crater" => return VoiceCommand::OpenGale,
        "open mission log" | "mission log" => return VoiceCommand::OpenMissionLog,
        "zoom in" | "zoom" => return VoiceCommand::ZoomIn,
        "zoom out" | "zoom out please" => return VoiceCommand::ZoomOut,
        "north" | "bearing north" | "direction north" => return VoiceCommand::BearingNorth,
        "east" | "bearing east" | "direction east" => return VoiceCommand::BearingEast,
        "south" | "bearing south" | "direction south" => return VoiceCommand::BearingSouth,
        "west" | "bearing west" | "direction west" => return VoiceCommand::BearingWest,
        "scroll up" | "scroll upwards" => return VoiceCommand::ScrollUp,
        "scroll down" | "scroll downwards" => return VoiceCommand::ScrollDown,
        "scroll to air low" | "air low" => return VoiceCommand::ScrollToAirLow,
        "scroll to air high" | "air high" => return VoiceCommand::ScrollToAirHigh,
        "toggle terrain" | "mola terrain" | "mola relief" => return VoiceCommand::ToggleTerrain,
        "toggle landmarks" | "sourced landmarks" => return VoiceCommand::ToggleLandmarks,
        "toggle rover" | "rover position" => return VoiceCommand::ToggleRover,
        "toggle traverse" | "recorded traverse" => return VoiceCommand::ToggleTraverse,
        "toggle night" | "day night" | "terminator" => return VoiceCommand::ToggleTerminator,
        "toggle dangers" | "steep terrain" | "danger zones" => return VoiceCommand::ToggleDangers,
        "stop listening" | "stop" | "cancel" => return VoiceCommand::StopListening,
        _ => {}
    }

    // Speed with explicit value: "speed 5", "set speed to 4 kmh".
    if framed.starts_with("speed ")
        || framed.starts_with("set speed")
        || framed.starts_with("change speed")
    {
        if let Some(kmh) = first_number(&framed) {
            return VoiceCommand::SetSpeed(kmh);
        }
        return VoiceCommand::Unknown(framed.to_owned());
    }

    // Keyword fallback for loose speech (e.g. "what are the conditions").
    // Exact phrases above take priority, so "speak conditions" / "read
    // conditions" keep their own commands.
    if framed.split_whitespace().any(|word| word == "conditions") {
        return VoiceCommand::ShowConditions;
    }

    VoiceCommand::Unknown(framed.to_owned())
}

// ---------------------------------------------------------------------------
// Loopback bridge client (archived Vosk bridge, retained for reference)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Deserialize)]
pub struct VoiceHealth {
    pub ok: bool,
    pub ready: bool,
    pub engine: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct VoiceTranscript {
    pub text: String,
    pub engine: String,
    pub offline: bool,
    pub cancelled: bool,
}

impl VoiceTranscript {
    pub fn command(&self) -> VoiceCommand {
        if self.cancelled {
            VoiceCommand::StopListening
        } else {
            parse_command(&self.text)
        }
    }
}

/// Blocking health probe; call from a background worker.
pub fn health(base_url: &str) -> Result<VoiceHealth> {
    let client = local_client(Duration::from_secs(3))?;
    let value = response_json(client.get(endpoint(base_url, "health")?))?;
    let health: VoiceHealth = serde_json::from_value(value).context("invalid health")?;
    Ok(health)
}

/// Record a command interval through the optional local bridge.
pub fn listen_for(base_url: &str, seconds: u8) -> Result<VoiceTranscript> {
    if !(2..=10).contains(&seconds) {
        bail!("Listening intervals must be between 2 and 10 seconds");
    }
    let client = local_client(Duration::from_secs(15))?;
    let value = response_json(
        client
            .post(endpoint(base_url, "listen")?)
            .json(&json!({ "seconds": seconds })),
    )?;
    let mut t: VoiceTranscript = serde_json::from_value(value).context("invalid transcript")?;
    t.text = t.text.trim().to_string();
    Ok(t)
}

/// Interrupt a concurrent `/listen`.
pub fn cancel(base_url: &str) -> Result<()> {
    let client = local_client(Duration::from_secs(2))?;
    let _ = response_json(client.post(endpoint(base_url, "cancel")?).json(&json!({})))?;
    Ok(())
}

fn local_client(timeout: Duration) -> Result<Client> {
    Ok(Client::builder()
        .connect_timeout(Duration::from_secs(1))
        .timeout(timeout)
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("TerraMars local offline voice client")
        .build()?)
}

fn endpoint(base_url: &str, path: &str) -> Result<Url> {
    let mut url = Url::parse(base_url).context("Invalid voice bridge URL")?;
    let host = url.host_str().context("Voice bridge URL needs a host")?;
    let host = host.trim_start_matches('[').trim_end_matches(']');
    let is_loopback =
        host == "localhost" || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback());
    if !is_loopback || !matches!(url.scheme(), "http" | "https") {
        bail!("The offline voice bridge must use a loopback HTTP(S) address");
    }
    if !url.username().is_empty() || url.password().is_some() {
        bail!("Voice bridge URLs must not contain credentials");
    }
    url.set_path(&format!("/{path}"));
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}

fn response_json(request: RequestBuilder) -> Result<Value> {
    let response = request
        .send()
        .context("Voice bridge unavailable. Start tools/voice_bridge.py and check its microphone/model setup")?;
    let status = response.status();
    let mut bytes = Vec::new();
    response.take(MAX_RESPONSE_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_RESPONSE_BYTES {
        bail!("Voice bridge response exceeds 64 KiB");
    }
    let value: Value = serde_json::from_slice(&bytes).context("Voice bridge returned invalid JSON")?;
    if !status.is_success() {
        bail!(
            "Voice bridge: {} (HTTP {status})",
            value["error"].as_str().unwrap_or("request failed")
        );
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_and_mode_commands() {
        assert_eq!(parse_command("plan route"), VoiceCommand::PlanRoute);
        assert_eq!(parse_command("start here"), VoiceCommand::StartHere);
        assert_eq!(parse_command("end here"), VoiceCommand::EndHere);
        assert_eq!(parse_command("swap"), VoiceCommand::SwapEndpoints);
        assert_eq!(parse_command("on foot"), VoiceCommand::ModeOnFoot);
        assert_eq!(parse_command("rover mode"), VoiceCommand::ModeRover);
    }

    #[test]
    fn units_and_time_commands() {
        assert_eq!(parse_command("metric"), VoiceCommand::UnitsMetric);
        assert_eq!(parse_command("scientific"), VoiceCommand::UnitsScientific);
        assert_eq!(parse_command("utc"), VoiceCommand::TimeUtc);
        assert_eq!(parse_command("amt"), VoiceCommand::TimeAmt);
    }

    #[test]
    fn speed_and_speak() {
        assert_eq!(parse_command("speed 5"), VoiceCommand::SetSpeed(5.0));
        assert_eq!(parse_command("set speed to 4 kmh"), VoiceCommand::SetSpeed(4.0));
        assert_eq!(parse_command("faster"), VoiceCommand::SpeedUp);
        assert_eq!(parse_command("speak route"), VoiceCommand::SpeakRoute);
        assert_eq!(parse_command("speak conditions"), VoiceCommand::SpeakConditions);
    }

    #[test]
    fn camera_and_scroll() {
        assert_eq!(parse_command("zoom in"), VoiceCommand::ZoomIn);
        assert_eq!(parse_command("zoom out"), VoiceCommand::ZoomOut);
        assert_eq!(parse_command("east"), VoiceCommand::BearingEast);
        assert_eq!(parse_command("north"), VoiceCommand::BearingNorth);
        assert_eq!(parse_command("scroll up"), VoiceCommand::ScrollUp);
        assert_eq!(parse_command("scroll down"), VoiceCommand::ScrollDown);
        assert_eq!(parse_command("scroll to air low"), VoiceCommand::ScrollToAirLow);
        assert_eq!(parse_command("scroll to air high"), VoiceCommand::ScrollToAirHigh);
    }

    #[test]
    fn layers_and_sites() {
        assert_eq!(parse_command("toggle terrain"), VoiceCommand::ToggleTerrain);
        assert_eq!(parse_command("mola relief"), VoiceCommand::ToggleTerrain);
        assert_eq!(parse_command("rover position"), VoiceCommand::ToggleRover);
        assert_eq!(parse_command("open jezero"), VoiceCommand::OpenJezero);
        assert_eq!(parse_command("open jezero crater"), VoiceCommand::OpenJezero);
        assert_eq!(parse_command("jezero crater"), VoiceCommand::OpenJezero);
        assert_eq!(parse_command("open gale"), VoiceCommand::OpenGale);
        assert_eq!(parse_command("open gale crater"), VoiceCommand::OpenGale);
        assert_eq!(parse_command("gale crater"), VoiceCommand::OpenGale);
        assert_eq!(parse_command("open mission log"), VoiceCommand::OpenMissionLog);
        assert_eq!(parse_command("open info"), VoiceCommand::OpenInfo);
    }

    #[test]
    fn polite_frames_and_punctuation_are_tolerated() {
        assert_eq!(
            parse_command("Please zoom in!"),
            VoiceCommand::ZoomIn
        );
        assert_eq!(
            parse_command("can you switch to scientific please"),
            VoiceCommand::UnitsScientific
        );
        assert_eq!(
            parse_command("what are the conditions"),
            VoiceCommand::ShowConditions
        );
    }

    #[test]
    fn unknown_phrases_are_flagged() {
        match parse_command("open the pod bay doors") {
            VoiceCommand::Unknown(t) if !t.is_empty() => {}
            other => panic!("expected unknown, got {other:?}"),
        }
        assert!(matches!(
            parse_command("rm -rf anything"),
            VoiceCommand::Unknown(_)
        ));
    }

    #[test]
    fn queue_push_and_drain() {
        let q = VoiceCommandQueue::new();
        q.push(VoiceCommand::ZoomIn);
        q.push(VoiceCommand::SpeakRoute);
        let mut out = Vec::new();
        q.drain(&mut out);
        assert_eq!(out, vec![VoiceCommand::ZoomIn, VoiceCommand::SpeakRoute]);
        assert!(q.is_empty());
    }
}
