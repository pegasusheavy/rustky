# AGENTS.md

## Project Overview

**rustky** is a conky-like system monitor for Wayland, written in Rust. It renders system stats (CPU, memory, disk, network, uptime, hostname, time) as text onto a wlr-layer-shell surface using Skia for rendering. It supports custom modules via shell commands, Rhai scripts, and Python scripts.

## Architecture

```
src/
  main.rs              — Entry point, CLI arg handling, wires config → renderer → monitor → wayland
  config.rs            — TOML config loading/parsing, module definitions (serde-based)
  monitor.rs           — System data collection via sysinfo; maps Module variants to StyledLines
  render.rs            — Skia-based text rendering to RGBA pixel buffers (supports per-line styling)
  styled.rs            — StyledLine + LineStyle types for per-line color/font overrides
  script_context.rs    — ScriptContext struct — system data snapshot passed to script engines
  wayland.rs           — Wayland client (smithay-client-toolkit), layer shell surface, calloop event loop, script engine dispatch
  scripting/
    mod.rs             — cfg-gated module declarations
    rhai_engine.rs     — Rhai scripting engine (compile, execute, on_draw hook)
    python_engine.rs   — Python (PyO3) scripting engine (load, execute, on_draw hook)
```

**Data flow:** `Config` defines which modules to display → `Monitor::collect()` gathers live data per module as `Vec<StyledLine>` → script engines (Rhai/Python) execute scripted modules and on_draw hooks → `Renderer::render_styled_lines()` draws styled text to pixels → `wayland::RustkyState::draw()` copies pixels into a wl_shm buffer and commits to the surface.

## Module Types

- **Built-in** (always available): `cpu`, `memory`, `disk`, `network`, `uptime`, `hostname`, `time`, `text`
- **Exec** (always available): runs a shell command via `sh -c`, supports optional label and per-line style
- **Rhai** (requires `rhai-scripting` feature): inline code or file-based, calls a named function with system data in scope
- **Python** (requires `python-scripting` feature): file-based, calls a named function with system data as dict argument

## Scripting

Scripts receive system data (CPU, memory, disk, network, hostname, uptime, etc.) and return either:
- A plain string (rendered as default styled line)
- A styled dict/map with `text`, optional `fg_color`, `bg_color`, `font_size`
- An array/list of the above

**on_draw hooks** receive all collected lines and can transform them before rendering.

## Key Dependencies

- **smithay-client-toolkit** + **calloop** — Wayland client and event loop
- **wayland-protocols-wlr** — wlr-layer-shell for desktop overlay positioning
- **skia-rs** / **skia-rs-canvas** — Local path dependency (`../skia-rs/`) for 2D rendering
- **sysinfo** — System metrics
- **serde** + **toml** — Config parsing
- **rhai** (optional) — Embedded scripting engine
- **pyo3** (optional) — Python bindings

## Feature Flags

- `rhai-scripting` — Enables Rhai script modules and on_draw hooks
- `python-scripting` — Enables Python script modules and on_draw hooks

## Conventions

- **Rust edition 2024**
- Config lives at `~/.config/rustky/config.toml`; falls back to compiled defaults on missing/invalid config
- Scripts directory defaults to `~/.config/rustky/scripts/`; configurable via `scripts_dir` in `[general]`
- Modules are defined as a tagged enum (`Module`) with `#[serde(tag = "type")]`
- The font is hardcoded to `/usr/share/fonts/TTF/DejaVuSansMono.ttf` via `include_bytes!`
- Pixel format conversion: Skia outputs RGBA premultiplied, Wayland expects ARGB8888 (BGRA in LE) — the swizzle happens in `RustkyState::draw()`
- No async runtime; uses calloop's synchronous event loop with timer-based refresh
- Feature-gated code uses `#[cfg(feature = "...")]` at both the module and item level

## Building

```sh
cargo build                                          # base (no scripting)
cargo build --features rhai-scripting                # with Rhai
cargo build --features python-scripting              # with Python
cargo build --features rhai-scripting,python-scripting  # both
```

Requires `skia-rs` to be checked out at `../skia-rs/` (path dependency). Requires Wayland development libraries and a compositor that supports wlr-layer-shell.

## Config

Run `rustky --default-config` to dump default TOML config to stdout. See `examples/config.toml` for a full example with all module types including exec, rhai, and python.
