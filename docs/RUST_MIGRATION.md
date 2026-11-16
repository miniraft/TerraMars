# Rust / GPUI migration

The active desktop application is the Cargo project at the repository root. It uses the same Rust/GPUI source on Linux, macOS and Windows. The retired Flutter project and its generated build files have been removed. Shared assets remain in `assets/`, and the Rust app can still import earlier mission history from local application storage.

## Application boundaries

- `src/ui/` owns GPUI rendering, map interactions, navigation and finite UI transitions. The window title is **TerraMars**. Images and bundled Orbitron/Departure Mono fonts are compiled into the application.
- `src/domain/` owns portable models, NASA Mars24 calculations, unit conversion, Mars-sphere route estimates and local preferences / route logs. It has no dependency on a platform's UI toolkit.
- `src/services/` owns source-preserving NASA refresh and Mars-coordinate route export. Blocking network operations belong on worker threads, not the rendering thread.
- `assets/` retains the real source snapshots, map imagery, exact geographic bounds and credits from the earlier app. The rewrite does not replace these with decorative or generated maps.

The inspected map location and the two route endpoints are separate state. Changing a route endpoint must not select that endpoint for inspection or move the camera. Selecting a map point prefills endpoint B when the planner opens. A successful plan is recorded automatically; the log represents plans rather than completed journeys.

Removed user-facing features stay removed: Earth packet import / watched inbox, crew location, the location panel's Landmarks tab and science objectives. Unknown fields in historical saved state can be preserved without reactivating those features.

## Rendering and motion

GPUI sends rendering work to its GPU-backed platform renderer. Full-resolution bundled map images decode once and are reused while the view transforms for pan and zoom; no map tiles are downloaded at runtime. Numerical terrain and climate grids remain bundled for planning. A lightly preblurred version of the real NASA panorama supplies the home background, avoiding a large live blur pass on every frame. Its 3-pixel offline blur preserves more terrain detail than the previous 10-pixel blur. GPUI Community Edition 0.3.0 applies genuine GPU backdrop blur to the translucent panels, buttons and header. The map extends underneath the header. The decorative panorama alone is blurred offline; it has no runtime blur filter.

Motion is always enabled and has no preference switch. It communicates state: brief eased panel reveals, pressed / hover feedback and clear route-planning transitions. The background panorama is the ambient animation; field calculations and source refresh are separate from its frame cadence. A Rust rewrite alone does not establish a measured speedup: use a release build and compare actual interaction on the target graphics driver.

## Native builds

`Cargo.lock` fixes dependency resolution and `rust-toolchain.toml` fixes the compiler and required components. The desktop workflow is `.github/workflows/desktop-builds.yml`.

The active UI uses `gpui-ce` and the matching `gpui_ce_platform` backend at 0.3.0. The old xattr compatibility patch is no longer in Cargo dependency resolution. The active GPUI CE sources have local extensions for native rotation input and rotating atlas images on the GPU; see `vendor/GPUI_PATCHES.md`.

Each native runner executes formatting checks, Clippy, Rust tests and a release build. Clippy and tests use the release profile so the large GPUI dependency graph is reused for the final binary. Linux installs the Wayland / X11, font and Vulkan development dependencies. macOS checks the selected Xcode Metal tools. GPUI CE uses the WGPU renderer on Linux, Direct3D on Windows and Metal on macOS; the legacy FXC build step is removed.

The matrix builds Linux x86_64 and ARM64, macOS Intel x86_64 and Apple Silicon ARM64, and Windows x86_64 and ARM64 on native runners. Every Cargo check, test and build specifies its Rust target; packaging reads from `target/<target>/release`. The ARM64 runner labels are `ubuntu-24.04-arm` and `windows-11-arm`; Intel macOS uses `macos-15-intel`.

Intel macOS builds target Catalina **10.15**, the floor supported by GPUI's native backend (which uses Metal's `supportsFamily:` API). Apple Silicon builds target **11.0**, the first macOS release for that architecture. `.cargo/config.toml` defaults `MACOSX_DEPLOYMENT_TARGET` to 10.15 for local builds, and CI explicitly selects the floor for each macOS architecture. Packaging uses the same setting for `LSMinimumSystemVersion`, with an 11.0 floor on ARM64. These are deployment targets; running on the oldest supported OS still requires testing on that OS, since the hosted runners use macOS 15.

`tools/package_desktop.py` creates one file per platform: a Linux AppImage built from an AppDir with `appimagetool`, a Windows ZIP containing `TerraMars.exe` plus documentation, and a macOS `.dmg` containing `TerraMars.app` and an Applications shortcut. The workflow uploads each file directly with `actions/upload-artifact@v7` and `archive: false`. Filenames include the platform and CPU architecture. On Linux, mark the AppImage executable and run it; it still uses system desktop libraries and a compatible graphics driver. On macOS, mount the DMG and drag the app into Applications. Assets and font/GPUI license notices are embedded in the executable; run it with `--export-licenses <file>` to save those notices. Documentation and provenance snapshots remain in the repository (and in the macOS bundle). The macOS bundle identifier is `com.miniraft.terramars` and the display name is **TerraMars**. macOS packaging applies an ad-hoc signature. Developer ID signing / notarization and Windows signing remain separate release work.

A host build validates that host. The GitHub matrix must finish successfully before macOS and Windows artifacts can be considered compiled and tested; this migration does not claim those builds were executed locally on Linux.

## Scientific continuity

The scientific boundaries remain in [DATA_SOURCES.md](DATA_SOURCES.md) and [FEATURE_MATRIX.md](FEATURE_MATRIX.md). In particular:

- Historical station weather is dated and geographically scoped; a selected pixel does not acquire a weather measurement.
- Mars Sol Date, Mars Year and local solar time are calculated planetary quantities; rover mission sols remain source metadata.
- Orbital imagery is projected in Mars coordinates. Route export must not identify it as Earth WGS84.
- Route ETA is distance divided by the user's assumed speed. Neither a rover corridor nor a colored MOLA image establishes a safe EVA route or a sampled slope.
- Source failures retain the previous record. Missing observations remain distinct from source-backed regional climate predictions, and retrieval time does not become observation time.

The microphone button is a placeholder that reports voice control as coming soon; no audio API is linked into the application. Regional MCD predictions and their limits are documented in [CLIMATE_MODEL.md](CLIMATE_MODEL.md).

## Local verification

On Linux, the optimized Rust app builds successfully and passes the automated tests covering NASA Mars24, map camera bounds and animation, real-source parsing, units, routes, persistent logs, export, and embedded icon registration. Formatting and Clippy pass with warnings denied for application code. 

The native GPU smoke driver passed all 12 checks on both X11 and Wayland, including the 800×600 compact layout on X11. It opens both regions and checks selection, endpoint independence, automatic route logging, an explicit route alternative, saved-log reload, Mars export scientific units, and pinch/rotation dispatch through the native GPUI window. It uses an isolated temporary mission directory so diagnostics do not modify the user's history:

```sh
cargo run --locked --release -- --smoke-test /tmp/terramars-gpui-smoke.json
# Also exercise the minimum desktop layout:
cargo run --locked --release -- --smoke-test /tmp/terramars-gpui-small.json --window-size 800x600
```

Real XTest mouse/keyboard regressions pass 32 checks at 1200×850 and 33 checks at 800×600. They cover units, time switching, endpoint picking by landmark/coordinates/map, B autofill, route logging, layer switches, panel scroll isolation and collapse controls. They also check that opening/closing a saved route stays in Mission log, that Home restores the explorer state, and that the crater back arrow returns to region selection. Run `python3 tools/test_desktop_pointer.py` or add `--size 800x600 --skip-refresh`.

The JSON report records real render/paint callback durations and callback intervals. These are CPU measurements; they do not measure GPU presentation latency or establish display FPS. No benchmark instrumentation runs during ordinary use. macOS and Windows require successful native Actions builds before their artifacts are considered verified.
