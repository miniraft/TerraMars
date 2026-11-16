#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod ui;

fn main() -> anyhow::Result<()> {
    // Keep redistribution notices inside the standalone executable and make them
    // accessible even on Windows, where the GUI executable has no console.
    let mut arguments = std::env::args_os().skip(1);
    if arguments.next().as_deref() == Some(std::ffi::OsStr::new("--export-licenses")) {
        let destination = arguments
            .next()
            .ok_or_else(|| anyhow::anyhow!("Usage: terramars --export-licenses <file>"))?;
        std::fs::write(
            destination,
            concat!(
                "Orbitron font\n\n",
                include_str!("../assets/fonts/Orbitron/OFL.txt"),
                "\n\nDeparture Mono font\n\n",
                include_str!("../assets/fonts/DepartureMono-1.500/LICENSE"),
                "\n\nGPUI Community Edition\n\n",
                include_str!("../assets/licenses/GPUI-CE-LICENSE-APACHE.txt"),
                "\n\nGPUI modifications\n\n",
                include_str!("../vendor/GPUI_PATCHES.md"),
            ),
        )?;
        return Ok(());
    }
    env_logger::init();
    ui::run()
}
