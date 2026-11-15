# TerraMars

A native **Rust + GPUI Community Edition** desktop explorer for **Jezero Crater** and **Gale Crater** on Mars. The shared application targets **Linux, macOS and Windows**, using real NASA imagery, rover tracks, sourced observations and Mars-coordinate route planning. It keeps the dark liquid-glass appearance, bundled **Inter** body text, **Manrope** headings, large crater cards and gently moving NASA panorama.

## Run the Rust app

Install Rust with [rustup](https://rustup.rs/). The repository pins Rust **1.100.0**, including rustfmt and Clippy, in `rust-toolchain.toml`.

On Ubuntu/Debian, install GPUI's native development libraries:

```sh
sudo apt update
sudo apt install build-essential clang cmake pkg-config \
  libfontconfig1-dev libfreetype6-dev libwayland-dev \
  libx11-xcb-dev libxcb-shape0-dev libxcb-xfixes0-dev \
  libxkbcommon-dev libxkbcommon-x11-dev libvulkan-dev \
  libssl-dev libasound2-dev libzstd-dev
```

Linux needs a working graphics driver with Vulkan support. GPUI supports both Wayland and X11; use the desktop session you normally run. On macOS, install full Xcode and its command-line tools, open Xcode once, and select its developer directory if necessary:

```sh
sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer
xcrun --find metal
```

On Windows, install Rust's MSVC toolchain and Visual Studio Build Tools with **Desktop development with C++**, CMake and a Windows SDK. The shared renderer uses GPUI Community Edition 0.3.0: WGPU on Linux, Direct3D on Windows and Metal on macOS. See the [GPUI CE repository](https://github.com/gpui-ce/gpui-ce) for platform requirements.

From this directory on any supported host:

```sh
cargo run --locked --release
```

On this Linux workspace, `./run.sh` launches the existing release executable directly, without Cargo or build caches. After source changes, use `./run.sh --build` to rebuild and launch; it uses the project-local Rust installation when available. Other arguments are forwarded to TerraMars. A normal Rust installation can use the Cargo command directly.

The first build downloads and compiles GPUI and its dependencies, so it takes substantially longer than later launches. For subsequent launches, run `target/release/terramars` on Linux/macOS or `target\release\terramars.exe` on Windows. Release mode is the appropriate build for judging interactive performance.

## Explore and plan

- **Home** contains the header and two crater cards over a softly scrolling real Mars panorama. **Mission log** is the other navigation destination.
- Pan and zoom the georeferenced maps, inspect landmarks and last recorded rover positions, and switch optical CTX imagery or MOLA shaded relief. Recorded traverses and calculated solar context remain distinct from observed terrain hazards.
- Map selection opens location details and the compact bottom route planner. Start begins empty; selecting a map point fills destination B. Choosing an endpoint does not move the location being inspected.
- Select endpoints from the map, the landmark catalog or exact Mars coordinates. Set an assumed travel speed/mode and calculate a direct estimate or an available recorded-rover-corridor alternative. Successful plans are automatically added to the local mission log.
- Switch the header between Earth UTC and calculated Mars time, or between **°C · km** and **K · m** display units. On Home, Mars time is Airy Mean Time; in a crater, it is local mean solar time. The date uses Mars Year and Mars Sol Date, not rover mission sols.
- Expand source-backed condition fields to inspect their dates, coverage, notes and source links. Missing climate fields show separately labeled **Predicted** values from MCD 6.2 where the model supports them; geology, radiation and operational gaps remain **Not modeled**.
- Refresh NASA records through the app. Each source updates independently; a failed source retains its previous record. Dates remain the original observation dates. Updated observations are held for the current session; offline launches start from the bundled snapshot.
- Export planned geometry and assumptions as a Mars-coordinate GeoJSON-style file. The export explicitly identifies Mars rather than assigning Earth WGS84 coordinates.

Panels begin hidden in the explorer. The route planner stays along the bottom edge. The app retains the requested removals: no Earth-packet importer, crew-location feature, science-objective input or Landmarks tab. Named map markers remain selectable. Preferences and route plans persist locally; historical legacy fields are retained without activating removed features.

## Real data and its limits

Bundled maps include full-resolution **MRO CTX orbital imagery** (3496 × 3496 for Jezero and 4370 × 4370 for Gale) and **MGS MOLA color shaded relief**. Maps work offline at every zoom, without online tile downloads. The catalog, route planning, numerical terrain screening and climate model also remain available offline. Coordinates use Mars planetocentric latitude and east-positive longitude, not Earth Web Mercator.

| Bundled record | Jezero / Perseverance | Gale / Curiosity |
| --- | --- | --- |
| Weather observation | 2024-04-27, sol 1133 | 2026-08-25, sol 4995 |
| Daily air temperature low / high | −79.3 / −24.7 °C | −71 / −5 °C |
| Pressure | 778.9 Pa | 777 Pa |
| Last recorded rover location | Source sol 2022 | Source sol 5073 |

These are historical station observations, not live forecasts or readings at every clicked point. Rover records supply mission sols without UTC observation timestamps; retrieval time is never substituted for measurement time. Crater centers and diameters follow USGS nomenclature.

Routes use Mars's 3389.5 km mean radius. ETA uses the entered speed and excludes rests, sampling and unmeasured terrain delays. Recorded rover tracks do not establish present astronaut traversability. MOLA image colors are not a numeric elevation model. No route is presented as operational EVA clearance.

Source URLs, imagery bounds, credits, checksums and instrument limitations are in [DATA_SOURCES.md](docs/DATA_SOURCES.md). [FEATURE_MATRIX.md](docs/FEATURE_MATRIX.md) accounts for the research brief, including explicit gaps: no live storm feed, ice/cave inventory, terrain shadows, communications blackout zones or moon ground tracks.

## UI controls and climate predictions

Motion stays on; the motion option and microphone button were removed. Home / Mission log are text navigation with orange active state. Metric / Scientific use a two-part control, and Visuals cards have switches at the right. The clock's arrow indicates that it switches Mars / Earth time. Map controls provide zoom and fit. A glass compass cycles N/E/S/W; two-finger rotation uses native OS gesture events where available, with Alt+scroll as a fallback.

The regional **Predicted** values use real, credited [Mars Climate Database 6.2](https://www-mars.lmd.jussieu.fr/mcd_python/) model exports, interpolated by Mars solar longitude and local true solar time. The model is regional climatology, not live weather, a storm warning, or a validated mission forecast. Measurements keep their original dates and provenance. See [CLIMATE_MODEL.md](docs/CLIMATE_MODEL.md).

The red home backdrop is a warm decorative crop of NASA’s Curiosity panorama PIA23623. Its edges are blended periodically and the image is lightly blurred offline using `tools/prepare_panorama.py` (3-pixel radius, reduced from 10). The same sharper image is used in Mission log. Buttons, panels and the overlay header use GPUI CE’s genuine GPU backdrop blur with light translucent tints. No refraction is applied. Inter and Manrope use bundled static font faces for consistent weights.

Button content uses one measured line with an explicit line height. Icon-only buttons omit empty text elements; long labels scroll within their available width. Home returns to the preserved explorer after visiting Mission log. The crater bubble’s back arrow returns to region selection. Info toggles the information/layers panel; saved routes open in a closable preview inside Mission log without changing the explorer.

Transient notifications disappear five seconds after each new message, including repeated route-log confirmations. Mission history remains saved.

## Keep disk usage small

The runnable app and its packaged download are tens of megabytes. Rust build artifacts, compiler installations and dependency caches can take many gigabytes during development; they are not needed to run the app. After building and packaging, remove generated build output with:

```sh
python3 tools/clean_workspace.py
```

Add `--toolchains` to also discard downloaded project-local compilers and dependency caches. The next build downloads them again and needs internet access. The cleanup retains the release executable, packaged archives, source, bundled assets, preferences and mission history. Run `./run.sh` to launch without rebuilding. Development/test incremental compilation is disabled to limit future cache growth.

## Validate and package

```sh
cargo fmt --package terramars -- --check
cargo clippy --locked --release --all-targets -- -D warnings
cargo test --locked --release --all-targets
cargo build --locked --release --bin terramars
python3 tools/package_desktop.py --platform linux
```

Use `--platform macos` or `--platform windows` when packaging a binary built on that host. The packager requires Python 3.10 or later and uses only the standard library. Images, fonts and the initial catalog are embedded in the executable; package archives also include source documentation, provenance snapshots, font licenses and the optional voice bridge.

App icons come from `assets/icons/app_logo`: Windows and Linux use `logo_small.png`; macOS uses `logo.png`, converted to `.icns` with the built-in macOS `sips` and `iconutil` tools during packaging. Windows builds generate and embed a multi-size `.ico` automatically.

[Desktop builds](.github/workflows/desktop-builds.yml) now runs **Rust** formatting, Clippy, tests and release builds on matching GitHub-hosted Linux, macOS and Windows machines. It uploads a native archive for each target. Push the project, including `Cargo.lock`, `rust-toolchain.toml` and assets, then open **Actions → Desktop builds**. A local Linux build alone does not verify macOS or Windows.

Download and extract the matching `TerraMars-*` artifact. Linux contains an executable; Windows contains `terramars.exe`; macOS contains `TerraMars.app`. The macOS bundle receives an ad-hoc signature, not Developer ID signing or notarization. Public installers and Windows signing are not configured.

## Code layout

```text
src/main.rs                  Native GPUI application entry point
src/ui/                      Shared shell, glass surfaces, map and interactions
src/domain/                  Models, Mars clock, units, route planning and storage
src/services/                NASA refresh, local voice bridge and route export
assets/maps/                 Real CTX / MOLA maps, panorama and geographic manifest
assets/data/                 Sourced observations, landmarks and rover snapshots
assets/fonts/                Inter / Manrope and SIL Open Font Licenses
tools/voice_bridge.py         Optional offline microphone command bridge
tools/package_desktop.py      Native archive / macOS application packaging
docs/                        Data sources, evidence limits and migration notes
```

Architecture and migration notes: [RUST_MIGRATION.md](docs/RUST_MIGRATION.md).

Native pointer regressions on a Linux desktop with X11 / XWayland:

```sh
python3 tools/test_desktop_pointer.py
python3 tools/test_desktop_pointer.py --size 800x600 --skip-refresh
```

These drive actual OS mouse/keyboard events in isolated temporary mission storage, checking routing controls, scroll isolation, switches, popup dismissal and navigation.

## Terrain-aware navigation

Red terrain exclusions come from actual numerical NASA PDS MOLA elevations, using an explicit 15° regional slope planning policy. Click a zone to see its slope, explanation and source. All new routes avoid these cells; hiding the visual does not disable the route screen. Small obstacles, sand traps and terrain finer than ~463 m remain unassessed. See [terrain data and routing](docs/TERRAIN_ROUTING.md).

The planner includes projected duration and arrival time. Its compact body scrolls, while the header/expand control stays visible. Export and rover-focus buttons are removed; Fit route occupies the route footer. Mission log shares the dimmed, preblurred Mars panorama. Header navigation stays centered at wide widths and moves left when the wordmark is hidden on smaller windows. Page and pane entry transitions are brief eased fades/slides.

The shared GPUI renderer has narrowly scoped GPU-image-rotation and native-gesture extensions, documented in [GPUI_PATCHES.md](vendor/GPUI_PATCHES.md). macOS and Windows artifacts require successful native Actions builds; local interaction checks run on Linux.
