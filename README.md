# TerraMars
Mars exploration app where you can plan routes and see current mars conditions.

Download the file for your platform from a successful **Desktop builds** run in GitHub Actions: a Linux executable, a macOS DMG (drag `TerraMars.app` into Applications), or a Windows EXE. Each download is a single native file, without a ZIP wrapper. On Linux, make the download executable with `chmod +x TerraMars-linux-*`. Builds target the architecture shown in the filename and use system graphics/desktop libraries.

Assets and license notices are embedded. Run the executable with `--export-licenses <file>` to save the bundled font and GPUI notices. See [native build details](docs/RUST_MIGRATION.md#native-builds).

GitHub Actions builds Linux, macOS and Windows for both x86_64 and ARM64. The macOS deployment minimum is Catalina 10.15 for Intel and Big Sur 11.0 for Apple Silicon.

## Voice control

An optional offline voice assistant (vox) can be built with
`cargo build --features voice-vox`. It lets you plan routes, check conditions,
change units/time/map layers, zoom, and scroll by speaking. See
[docs/VOICE_CONTROL.md](docs/VOICE_CONTROL.md) for the command list and model
setup.
