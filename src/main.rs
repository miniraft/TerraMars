#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod ui;

fn main() -> anyhow::Result<()> {
    env_logger::init();
    ui::run()
}
