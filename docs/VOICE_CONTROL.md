# Voice control (vox integration)

TerraMars can be controlled by voice using the local-first
[vox](https://github.com/mrtozner/vox) assistant. Everything runs offline on
your machine: microphone → VAD → Whisper STT → command → Kokoro TTS → speakers.
No audio or text is uploaded anywhere.

## Build

Voice support is optional and gated behind the `voice-vox` feature so the
default build stays light:

```sh
cargo build --features voice-vox
cargo run --features voice-vox
```

Cargo downloads and builds vox from
`https://github.com/mrtozner/vox` (see `Cargo.toml`).

## Models

vox needs model files in a `models/` directory next to the executable, or in
the path given by `VOX_MODELS_DIR`:

- `silero_vad.onnx` — voice activity detection
- `ggml-tiny.en.bin` — Whisper speech-to-text
- `kokoro-v1.0.onnx` + `voices.bin` — Kokoro text-to-speech

Download them from vox (e.g. `vox models download silero-vad`, `vox models
download whisper-tiny.en`, `vox models download kokoro`, plus `kokoro-voices`)
or place them manually.

## Enable

Launch the app, then click the **Voice ○** button in the top-right header to
enable voice control (it becomes **Voice ●**). The first enable starts the local
vox pipeline and opens the microphone. Click again to disable, or say **"stop
listening"**.

## Voice commands

Say any of these naturally; polite frames ("please …", "… please") and
punctuation are ignored.

Route planning
- "plan route" — plan a route from the current start to destination
- "start here" / "end here" — set the inspected map point as start / end
- "swap" — swap start and destination
- "on foot" / "rover" — travel mode
- "faster" / "slower" / "speed up" / "speed down" — adjust by 0.5 km/h
- "speed 5" / "set speed to 4 kmh" — exact speed (0.5–15 km/h)
- "speak route" — read the route out loud (distance, ETA, mode)

Conditions / time / units
- "speak conditions" — read predicted conditions at the selected point
- "show conditions" / "conditions" — conditions tab
- "show visuals" / "visuals" / "layers" — map layers tab
- "metric" / "scientific" — unit system
- "utc" / "amt" — Earth UTC / Mars local time

Map
- "open Jezero" / "open Gale" — open that crater
- "open mission log" — mission log
- "open info" — info / conditions panel
- "zoom in" / "zoom out"
- "north" / "east" / "south" / "west" — compass bearing
- "scroll up" / "scroll down" / "scroll to air low" / "scroll to air high"

Map layers (toggle)
- "toggle terrain" / "mola relief" — MOLA terrain relief
- "toggle landmarks" / "sourced landmarks"
- "toggle rover" / "rover position"
- "toggle traverse" / "recorded traverse"
- "toggle night" / "terminator" — day/night shading
- "toggle dangers" / "steep terrain" — danger zones

Transport
- "stop listening" — disable voice control

## How it maps to TerraMars

Each command executes the same action as the corresponding UI control, so voice
and mouse/keyboard stay in sync: route planning uses `RoutePlanner` (MOLA
screened, ETA at your assumed speed), units use `UnitSystem` (metric vs
scientific), the clock uses `MarsClock` (UTC / AMT / LMST), and TTS reads back
route summaries and conditions via vox Kokoro.

## Notes

- `cargo` and the Rust toolchain (1.100.0) are required to build; they are not
  bundled with the app.
- Recognition quality depends on the microphone and Whisper model.
- Commands are exact-ish phrases; if a phrase isn't recognized, the app shows a
  "Voice command not recognized" status.
- The pipeline is local and offline; nothing leaves your machine.
