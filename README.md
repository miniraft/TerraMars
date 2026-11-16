# TerraMars

Mars exploration app where you can plan routes and see current Mars conditions.

## Which build to install

Pick the file that matches your OS and CPU. Download it from a successful **Desktop builds** run in GitHub Actions (tagged releases also publish a signed macOS DMG).

| Your machine | File |
| --- | --- |
| Mac with Apple Silicon (M1/M2/M3/M4…) | `TerraMars-macos-arm64.dmg` |
| Mac with Intel CPU | `TerraMars-macos-x86_64.dmg` |
| Mac, either CPU (tagged releases) | `TerraMars-macos-universal.dmg` |
| Linux x86_64 | `TerraMars-linux-x86_64.AppImage` |
| Linux ARM64 | `TerraMars-linux-arm64.AppImage` |
| Windows x86_64 | `TerraMars-windows-x86_64.zip` |
| Windows ARM64 | `TerraMars-windows-arm64.zip` |

macOS minimums: Catalina 10.15 (Intel) and Big Sur 11.0 (Apple Silicon).

## Running it

- **macOS** — open the DMG, drag `TerraMars.app` into Applications, and launch it from Launchpad.
- **Linux** — `chmod +x TerraMars-linux-*.AppImage` then run the file. System graphics and desktop libraries are still required.
- **Windows** — extract the ZIP and run `TerraMars.exe` from inside the `TerraMars` folder, leaving the documentation and resource folders beside it. The archives are unsigned, so SmartScreen may still ask for confirmation.

Assets and license notices are embedded. Run the executable with `--export-licenses <file>` to save the bundled font and GPUI notices. See [native build details](docs/RUST_MIGRATION.md#native-builds).

## Features

- **Explorer map** — georeferenced orbital Mars maps with CTX imagery or MOLA colour relief, drag to pan, scroll to zoom, Alt+scroll or pinch to rotate, a compass that snaps in 90° steps, and a live scale bar in kilometres.
- **Route planner** — set a start and destination (map click, sourced landmark, or typed coordinates), switch between on-foot and rover travel, pick an assumed speed, and get a distance, ETA and risk note. Routes detour around slopes steeper than 15°.
- **Layered overlays** — sourced landmarks, recorded rover traverses, planned route with A/B markers, danger-zone labels, and a day/night terminator.
- **Conditions and science panels** — terrain warnings, predicted regional weather (air and surface temperature, wind, dust, sunlight), historical station records, solar geometry with Mars date and season, and eight expandable observation categories split into recorded vs. predicted values.
- **Mission log** — every planned route is saved with its crater, timestamp, distance and duration; click an entry to reopen it in a full-window map preview.
- **Live clock** — header clock toggles between Earth UTC and Mars local time, with the current Mars sol and season.
- **NASA data** — refresh pulls the latest rover waypoint, traverse and historical weather for each crater; failed sources keep their previous data and warn you instead of showing invented numbers.
- **Two regions** — Jezero and Gale craters, each with its own landmark set, traverse and observations; Valles Marineris is coming soon.
- **Local state** — units, speed, clock mode and planned routes persist between launches.

## Voice control

Voice commands are **not implemented yet**. The microphone button in the header is a placeholder: selecting it shows a "Voice functionality coming soon" notice. No microphone access is requested and no audio is captured.
