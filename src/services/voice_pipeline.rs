//! Embedding of the [`vox`](https://github.com/mrtozner/vox) local voice AI
//! framework into TerraMars.
//!
//! This module is gated behind the `voice-vox` Cargo feature so that the
//! default build compiles without vox's heavy native dependencies (ONNX
//! Runtime, cpal, rodio, ...). Enable it with:
//!
//! ```toml
//! [features]
//! default = []
//! voice-vox = ["dep:vox", "dep:tokio", "dep:tracing"]
//! vox = { git = "https://github.com/mrtozner/vox", optional = true,
//!   features = ["whisper", "silero", "kokoro", "tts"] }
//! tokio = { version = "1", optional = true, features = ["full"] }
//! tracing = { version = "0.1", optional = true }
//! ```
//!
//! Then `cargo build --features voice-vox`. Models (silero_vad.onnx,
//! ggml-tiny.en.bin, kokoro-v1.0.onnx, voices.bin) are expected in a
//! `models/` directory next to the executable, or in `VOX_MODELS_DIR`.

#[cfg(feature = "voice-vox")]
use std::path::PathBuf;
#[cfg(feature = "voice-vox")]
use std::sync::Arc;
#[cfg(feature = "voice-vox")]
use std::thread;

use anyhow::Result;
#[cfg(feature = "voice-vox")]
use anyhow::Context;

#[cfg(feature = "voice-vox")]
use tokio::sync::mpsc;

use super::voice::VoiceCommandQueue;
#[cfg(feature = "voice-vox")]
use super::voice::VoiceCommand;

/// Resolve the vox models directory: `VOX_MODELS_DIR` env, then a sibling
/// `models/` of the running executable.
#[cfg(feature = "voice-vox")]
fn models_dir() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("VOX_MODELS_DIR") {
        let p = PathBuf::from(dir);
        std::fs::create_dir_all(&p).ok();
        return Ok(p);
    }
    if let Ok(exe) = std::env::current_exe() {
        let mut cursor = exe.parent().map(|p| p.to_path_buf());
        while let Some(dir) = cursor {
            let candidate = dir.join("models");
            if candidate.exists() {
                return Ok(candidate);
            }
            cursor = dir.parent().map(|p| p.to_path_buf());
        }
    }
    std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|p| p.to_path_buf()))
        .map(|p| p.join("models"))
        .context("could not resolve vox models directory")
}

/// Confirmation phrases spoken back for recognized commands (TTS).
#[cfg(feature = "voice-vox")]
fn confirmation(cmd: &VoiceCommand) -> &'static str {
    match cmd {
        VoiceCommand::PlanRoute => "Route planned.",
        VoiceCommand::SpeakRoute => "Reading the route.",
        VoiceCommand::SpeakConditions => "Reading the conditions.",
        VoiceCommand::StartHere => "Start point set.",
        VoiceCommand::EndHere => "End point set.",
        VoiceCommand::SwapEndpoints => "Endpoints swapped.",
        VoiceCommand::ModeOnFoot => "Travel mode: on foot.",
        VoiceCommand::ModeRover => "Travel mode: rover.",
        VoiceCommand::SpeedUp => "Speed increased.",
        VoiceCommand::SpeedDown => "Speed decreased.",
        VoiceCommand::SetSpeed(_) => "Travel speed set.",
        VoiceCommand::UnitsMetric => "Units: metric.",
        VoiceCommand::UnitsScientific => "Units: scientific.",
        VoiceCommand::TimeUtc => "Clock: UTC.",
        VoiceCommand::TimeAmt => "Clock: Mars local time.",
        VoiceCommand::OpenInfo => "Opening info panel.",
        VoiceCommand::ShowConditions => "Showing conditions.",
        VoiceCommand::ShowVisuals => "Showing visuals.",
        VoiceCommand::OpenJezero => "Opening Jezero Crater.",
        VoiceCommand::OpenGale => "Opening Gale Crater.",
        VoiceCommand::OpenMissionLog => "Opening mission log.",
        VoiceCommand::ZoomIn => "Zooming in.",
        VoiceCommand::ZoomOut => "Zooming out.",
        VoiceCommand::BearingNorth => "Bearing north.",
        VoiceCommand::BearingEast => "Bearing east.",
        VoiceCommand::BearingSouth => "Bearing south.",
        VoiceCommand::BearingWest => "Bearing west.",
        VoiceCommand::ScrollUp => "Scrolling up.",
        VoiceCommand::ScrollDown => "Scrolling down.",
        VoiceCommand::ScrollToAirLow => "Scrolling to air low.",
        VoiceCommand::ScrollToAirHigh => "Scrolling to air high.",
        VoiceCommand::ToggleTerrain => "Toggling MOLA terrain relief.",
        VoiceCommand::ToggleLandmarks => "Toggling sourced landmarks.",
        VoiceCommand::ToggleRover => "Toggling rover position.",
        VoiceCommand::ToggleTraverse => "Toggling recorded traverse.",
        VoiceCommand::ToggleTerminator => "Toggling day night shading.",
        VoiceCommand::ToggleDangers => "Toggling danger zones.",
        VoiceCommand::StopListening => "Stopping voice control.",
        VoiceCommand::Unknown(_) => "",
    }
}

/// Handle to the running vox pipeline.
#[cfg(feature = "voice-vox")]
pub struct VoicePipeline {
    speak_tx: Option<mpsc::UnboundedSender<String>>,
}

#[cfg(feature = "voice-vox")]
impl VoicePipeline {
    /// Start the voice pipeline on background threads.
    ///
    /// - One thread runs the vox listen loop (mic → VAD → Whisper STT) and
    ///   pushes recognized [`VoiceCommand`]s onto `queue`.
    /// - One thread runs a speak consumer (TTS Kokoro + audio playback) fed by
    ///   `speak()`.
    pub fn start(queue: VoiceCommandQueue) -> Result<Self> {
        let models_dir = models_dir()?;

        // One runtime purely to load the async Kokoro backend up front.
        let rt = tokio::runtime::Runtime::new().context("create tokio runtime")?;
        let tts = rt.block_on(vox::tts::KokoroBackend::new(
            &models_dir.join("kokoro-v1.0.onnx"),
            &models_dir.join("voices.bin"),
        ))
        .with_context(|| format!("load Kokoro TTS from {}", models_dir.display()))?;
        let player = vox::audio::AudioPlayer::new()
            .context("initialize audio playback device")?;
        drop(rt);

        let tts_arc = Arc::new(tts);
        let player_arc = Arc::new(player);

        // VAD + Whisper are synchronous to construct.
        let vad = vox::vad::SileroVad::new(models_dir.join("silero_vad.onnx"))
            .with_context(|| format!("load silero VAD from {}", models_dir.display()))?;
        let stt = vox::stt::WhisperBackend::from_model(models_dir.join("ggml-tiny.en.bin"))
            .with_context(|| format!("load Whisper STT from {}", models_dir.display()))?;

        let (speak_tx, speak_rx) = mpsc::unbounded_channel::<String>();

        // ---- Speak consumer thread -------------------------------------
        let speak_tts = tts_arc.clone();
        let speak_player = player_arc.clone();
        thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("tokio runtime for speak");
            rt.block_on(async move {
                let mut rx = speak_rx;
                while let Some(text) = rx.recv().await {
                    if text.trim().is_empty() {
                        continue;
                    }
                    let request = vox::types::TtsRequest {
                        text,
                        voice: None,
                        seed: None,
                    };
                    match speak_tts.synthesize(&request).await {
                        Ok(output) => {
                            let _ = speak_player.play_blocking(&output.audio);
                        }
                        Err(error) => log::warn!(error = %error, "TTS synthesis failed"),
                    }
                }
            });
        });

        // ---- Listen thread (mic → VAD → STT → commands) ----------------
        // Confirmations are spoken via `speak_tx` (the speak consumer thread),
        // so the listen loop does not need its own TTS backend.
        let listen_queue = queue.clone();
        let listen_speak_tx = speak_tx.clone();
        thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("tokio runtime for vox listen");
            rt.block_on(async move {
                let vox = vox::Vox::builder()
                    .vad(vad)
                    .stt(stt)
                    .on_utterance(move |result, _ctx| {
                        let cmd = super::voice::parse_command(&result.text);
                        let speak_text = confirmation(&cmd).to_string();
                        listen_queue.push(cmd.clone());
                        let _ = listen_speak_tx.send(speak_text);
                    })
                    .build()
                    .expect("build vox pipeline");
                let _ = vox.listen().await;
            });
        });

        Ok(Self {
            speak_tx: Some(speak_tx),
        })
    }

    /// Ask the pipeline to speak `text` aloud (non-blocking).
    pub fn speak(&self, text: impl Into<String>) {
        if let Some(tx) = &self.speak_tx {
            let _ = tx.send(text.into());
        }
    }
}

/// Stub when the `voice-vox` feature is disabled: the app still builds and
/// voice commands can be wired up, but no audio pipeline runs. Starting it
/// must fail so the UI reports that voice control is unavailable instead of
/// silently enabling a non-functional toggle.
#[cfg(not(feature = "voice-vox"))]
pub struct VoicePipeline;

#[cfg(not(feature = "voice-vox"))]
impl VoicePipeline {
    pub fn start(_queue: VoiceCommandQueue) -> Result<Self> {
        anyhow::bail!(
            "voice control requires the `voice-vox` feature and model files in `models/`; \
             build with `cargo build --features voice-vox`"
        );
    }
    pub fn speak(&self, _text: impl Into<String>) {}
}
