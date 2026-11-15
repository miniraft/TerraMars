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
                "Inter font\n\n",
                include_str!("../assets/fonts/Inter-OFL.txt"),
                "\n\nManrope font\n\n",
                include_str!("../assets/fonts/Manrope-OFL.txt"),
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
