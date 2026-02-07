mod config;
mod monitor;
mod render;
mod script_context;
mod styled;
mod wayland;

#[cfg(any(feature = "rhai-scripting", feature = "python-scripting"))]
mod scripting;

use config::Config;
use monitor::Monitor;
use render::Renderer;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--default-config") {
        print!("{}", Config::generate_default_toml());
        return;
    }

    let cfg = Config::load();
    eprintln!("rustky: loaded config, {} modules", cfg.modules.len());

    let renderer = Renderer::new(
        cfg.general.font_size,
        &cfg.general.fg_color,
        &cfg.general.bg_color,
    );

    let monitor = Monitor::new();

    wayland::run(cfg, renderer, monitor);
}
