mod animation;
mod app;
mod assets;
mod clock;
mod interaction;
mod map;
mod performance;
mod style;

pub fn run() -> anyhow::Result<()> {
    app::run()
}
