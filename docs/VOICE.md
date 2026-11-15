# Legacy offline voice utilities

The microphone button and voice input have been removed from TerraMars. The utilities below are retained for reference and protocol tests; the current desktop app does not connect to the bridge.

## Optional offline voice bridge

Microphone recognition uses the local [Vosk recognizer](https://alphacephei.com/vosk/) through [sounddevice](https://python-sounddevice.readthedocs.io/). No audio is uploaded or saved. Install Python 3.10 or later and, in a virtual environment:

```sh
python -m venv .venv
# Linux / macOS:
source .venv/bin/activate
# Windows PowerShell instead:
# .venv\Scripts\Activate.ps1
python -m pip install -r tools/voice-requirements.txt
```

Download and unpack an English model from [the Vosk model catalog](https://alphacephei.com/vosk/models). The speech model is not bundled. On Linux, sounddevice also requires the PortAudio runtime (`sudo apt install libportaudio2`). On macOS and Windows the pip installation normally supplies PortAudio; allow microphone access when the operating system asks.

Start the bridge in its own terminal:

```sh
python tools/voice_bridge.py --model /absolute/path/to/unpacked/model
```

The bridge binds only to `127.0.0.1:8765`. `GET /health` checks readiness. `POST /listen` with optional JSON `{"seconds":6}` records 2–10 seconds and returns `{"text":"zoom in","engine":"vosk","offline":true}`. `POST /cancel` stops an ongoing recording at the next audio-queue interval (up to approximately 250 ms); a cancelled listening response contains empty text and `"cancelled":true`. Errors return `{"error":"..."}` with a non-200 status. Enable the app's microphone control to listen continuously in short intervals; stop with the button or the supported stop phrase. The app never executes arbitrary spoken shell commands. Touch and mouse controls remain available if the bridge or microphone is absent.

Supported phrases are **zoom in**, **zoom out**, **show terrain**, **show imagery**, **plan route**, **start here**, **end here**, and **stop listening**. Recognition quality depends on the selected model and microphone. This prototype does not implement certified EVA voice control.

