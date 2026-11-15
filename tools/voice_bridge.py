#!/usr/bin/env python3
"""Optional local microphone bridge for TerraMars desktop (Vosk + sounddevice).

Nothing is uploaded. The GPUI client calls POST /listen and receives text.
Run: python tools/voice_bridge.py --model /path/to/unpacked/vosk-model
"""

from __future__ import annotations

import argparse
import json
import os
import queue
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


def make_handler(model, sounddevice, recognizer_type):
    microphone_lock = threading.Lock()
    cancel_recording = threading.Event()

    class Handler(BaseHTTPRequestHandler):
        def respond(self, status, payload):
            body = json.dumps(payload).encode("utf-8")
            self.send_response(status)
            self.send_header("Content-Type", "application/json; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(body)

        def do_GET(self):
            if self.path == "/health":
                self.respond(200, {"ok": True, "ready": True, "engine": "vosk"})
            else:
                self.respond(404, {"error": "Unknown endpoint"})

        def do_POST(self):
            if self.path not in ("/listen", "/cancel"):
                self.respond(404, {"error": "Unknown endpoint"})
                return
            # Native desktop requests do not carry Origin. Reject browser
            # callers so an unrelated website cannot activate the microphone.
            if self.headers.get("Origin"):
                self.respond(403, {"error": "Browser microphone requests are not accepted"})
                return
            if self.path == "/cancel":
                cancel_recording.set()
                self.respond(200, {"ok": True, "cancelled": True})
                return
            try:
                length = int(self.headers.get("Content-Length", "0"))
                if length < 0 or length > 512:
                    self.respond(413, {"error": "Request too large"})
                    return
                options = json.loads(self.rfile.read(length) or b"{}")
                duration = float(options.get("seconds", 6))
                if not 2 <= duration <= 10:
                    raise ValueError("seconds must be between 2 and 10")
            except (ValueError, TypeError, AttributeError) as error:
                self.respond(400, {"error": str(error)})
                return

            if not microphone_lock.acquire(blocking=False):
                self.respond(409, {"error": "Microphone is already listening"})
                return
            try:
                cancel_recording.clear()
                audio = queue.Queue()

                def callback(data, _frames, _time, status):
                    if status:
                        print(f"Microphone: {status}", flush=True)
                    audio.put(bytes(data))

                # Match the physical microphone's native sample rate. Vosk
                # receives that rate, avoiding platform-specific resampling.
                device = sounddevice.query_devices(kind="input")
                sample_rate = int(device["default_samplerate"])
                recognizer = recognizer_type(model, sample_rate)
                parts = []
                deadline = time.monotonic() + duration
                with sounddevice.RawInputStream(
                    samplerate=sample_rate,
                    blocksize=4096,
                    dtype="int16",
                    channels=1,
                    callback=callback,
                ):
                    while time.monotonic() < deadline and not cancel_recording.is_set():
                        try:
                            block = audio.get(timeout=0.25)
                        except queue.Empty:
                            continue
                        if recognizer.AcceptWaveform(block):
                            parts.append(json.loads(recognizer.Result()).get("text", ""))
                parts.append(json.loads(recognizer.FinalResult()).get("text", ""))
                cancelled = cancel_recording.is_set()
                text = "" if cancelled else " ".join(part for part in parts if part).strip()
                self.respond(200, {
                    "text": text, "engine": "vosk", "offline": True, "cancelled": cancelled
                })
            except Exception as error:
                self.respond(503, {"error": f"Microphone recognition failed: {error}"})
            finally:
                microphone_lock.release()

    return Handler


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", default=os.environ.get("TERRAMARS_VOSK_MODEL"))
    parser.add_argument("--port", type=int, default=8765)
    args = parser.parse_args()
    if not args.model or not Path(args.model).is_dir():
        parser.error("Provide --model pointing to an unpacked Vosk speech model directory")
    try:
        import sounddevice
        from vosk import KaldiRecognizer, Model
    except ImportError as error:
        parser.error(f"Missing optional dependency: {error}. Install tools/voice-requirements.txt")

    model = Model(args.model)
    server = ThreadingHTTPServer(
        ("127.0.0.1", args.port), make_handler(model, sounddevice, KaldiRecognizer)
    )
    print(f"TerraMars offline voice bridge: http://127.0.0.1:{args.port}", flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
