//! Optional offline Vosk bridge. Only loopback URLs are accepted; audio is
//! handled by tools/voice_bridge.py and never sent to a remote service here.

use std::io::Read;
use std::net::IpAddr;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use reqwest::Url;
use reqwest::blocking::{Client, RequestBuilder};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const DEFAULT_BRIDGE: &str = "http://127.0.0.1:8765";
const MAX_RESPONSE_BYTES: u64 = 64 * 1024;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VoiceHealth {
    pub ok: bool,
    pub ready: bool,
    #[serde(default)]
    pub engine: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VoiceTranscript {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub engine: String,
    #[serde(default)]
    pub offline: bool,
    #[serde(default)]
    pub cancelled: bool,
}

impl VoiceTranscript {
    pub fn intent(&self) -> Option<VoiceIntent> {
        if self.cancelled {
            None
        } else {
            parse_intent(&self.text)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceIntent {
    ZoomIn,
    ZoomOut,
    ShowTerrain,
    ShowImagery,
    PlanRoute,
    StartHere,
    EndHere,
    StopListening,
}

/// The supported intents are UI actions, never shell or system commands.
/// Exact command phrases with optional polite framing avoid acting on unrelated
/// prose such as "do not plan route".
pub fn parse_intent(text: &str) -> Option<VoiceIntent> {
    let normalized = text
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let normalized = normalized
        .trim_start_matches("please ")
        .trim_end_matches(" please");
    match normalized {
        "zoom in" => Some(VoiceIntent::ZoomIn),
        "zoom out" => Some(VoiceIntent::ZoomOut),
        "show terrain" => Some(VoiceIntent::ShowTerrain),
        "show imagery" => Some(VoiceIntent::ShowImagery),
        "plan route" => Some(VoiceIntent::PlanRoute),
        "start here" => Some(VoiceIntent::StartHere),
        "end here" => Some(VoiceIntent::EndHere),
        "stop listening" => Some(VoiceIntent::StopListening),
        _ => None,
    }
}

/// Blocking health probe; call from a background worker.
pub fn health(base_url: &str) -> Result<VoiceHealth> {
    let client = local_client(Duration::from_secs(3))?;
    let value = response_json(client.get(endpoint(base_url, "health")?))?;
    let health: VoiceHealth = serde_json::from_value(value)?;
    if !health.ok || !health.ready {
        bail!(
            "Local voice bridge is not ready. Start tools/voice_bridge.py with an unpacked Vosk model."
        );
    }
    Ok(health)
}

/// Records a five-second command interval through the optional local bridge.
/// Continuous hands-free use repeats this call on a worker until cancelled.
pub fn listen(base_url: &str) -> Result<VoiceTranscript> {
    listen_for(base_url, 5)
}

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
    serde_json::from_value(value).context("Invalid local voice transcript")
}

/// A separate short request interrupts a concurrent /listen in the bridge.
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
    let response = request.send().context("Voice bridge unavailable. Start tools/voice_bridge.py and check its microphone/model setup")?;
    let status = response.status();
    let mut bytes = Vec::new();
    response
        .take(MAX_RESPONSE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_RESPONSE_BYTES {
        bail!("Voice bridge response exceeds 64 KiB");
    }
    let value: Value =
        serde_json::from_slice(&bytes).context("Voice bridge returned invalid JSON")?;
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
    fn known_phrases_are_actions_and_arbitrary_text_is_not() {
        assert_eq!(parse_intent("Please ZOOM IN!"), Some(VoiceIntent::ZoomIn));
        assert_eq!(
            parse_intent("stop listening"),
            Some(VoiceIntent::StopListening)
        );
        assert_eq!(parse_intent("do not plan route"), None);
        assert_eq!(parse_intent("rm -rf anything"), None);
        assert_eq!(parse_intent(""), None);
    }

    #[test]
    fn voice_addresses_cannot_escape_loopback_or_redirect() {
        assert!(endpoint(DEFAULT_BRIDGE, "listen").is_ok());
        assert!(endpoint("http://[::1]:8765", "health").is_ok());
        assert!(endpoint("https://example.com", "listen").is_err());
        assert!(endpoint("http://127.0.0.1.evil.example", "listen").is_err());
        assert!(endpoint("http://name:password@127.0.0.1:8765", "listen").is_err());
    }

    #[test]
    fn cancelled_transcripts_never_trigger_an_action() {
        let result = VoiceTranscript {
            text: "plan route".into(),
            engine: "vosk".into(),
            offline: true,
            cancelled: true,
        };
        assert_eq!(result.intent(), None);
    }
}
