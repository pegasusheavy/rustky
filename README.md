# rustky

A modern, GPU-accelerated system monitor for Wayland — like conky, but in Rust.

![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)

rustky renders live system stats as a transparent overlay on your Wayland desktop using wlr-layer-shell. It is configured entirely through a single TOML file and supports custom modules via shell commands, [Rhai](https://rhai.rs/) scripts, and [Python](https://www.python.org/).

## Features

- **Wayland-native** — uses wlr-layer-shell (sway, Hyprland, KDE 6, etc.)
- **Skia rendering** — subpixel anti-aliased text, per-line colors and font sizes
- **Scrollable** — mouse wheel scrolling when content exceeds the window
- **Modular** — built-in modules for CPU, memory, disk, network, uptime, hostname, and time
- **Shell commands** — run any command and display its output (`type = "exec"`)
- **Rhai scripting** — inline expressions or script files with full access to system data
- **Python scripting** — PyO3-powered modules for complex logic
- **on_draw hooks** — Rhai/Python functions that transform all lines before each frame
- **Transparent background** — ARGB with configurable alpha
- **Lightweight** — single binary, ~2 MB installed, no runtime dependencies beyond Wayland and a font
- **systemd integration** — ships with a user service file

## Screenshot

> *Place a screenshot at `assets/screenshot.png` and uncomment:*
<!-- ![rustky screenshot](assets/screenshot.png) -->

## Installation

### Arch Linux (PKGBUILD)

From the project root:

```sh
makepkg -si
```

This installs the binary to `/usr/bin/rustky`, a systemd user service, and an example config.

### From source

Requires Rust **nightly** (edition 2024) and Wayland development headers.

```sh
# Base build — built-in modules + exec only
cargo build --release

# With Rhai scripting
cargo build --release --features rhai-scripting

# With Python scripting
cargo build --release --features python-scripting

# Everything
cargo build --release --features rhai-scripting,python-scripting
```

### Dependencies

| Type | Packages |
|------|----------|
| Runtime | `wayland`, `ttf-dejavu` |
| Build | `cargo` (nightly), `wayland-protocols` |
| Optional | `python` (for `python-scripting` feature) |

## Usage

```sh
# Run directly
rustky

# Dump the default config to stdout
rustky --default-config

# Run as a systemd user service
systemctl --user enable --now rustky
```

## Configuration

rustky looks for its config at `~/.config/rustky/config.toml`. Generate a starting point with:

```sh
rustky --default-config > ~/.config/rustky/config.toml
```

### General settings

```toml
[general]
update_interval_ms = 1000       # refresh rate in milliseconds
font = "monospace"              # font family (display only — rendering uses DejaVu Sans Mono)
font_size = 14.0                # default font size in points
fg_color = "#c0caf5"            # default foreground (hex RGB or RGBA)
bg_color = "#1a1b26cc"          # window background (hex RGBA for transparency)
# scripts_dir = "~/.config/rustky/scripts/"
# on_draw_rhai = "on_draw.rhai"
# on_draw_python = "on_draw.py"
```

### Window

```toml
[window]
x = 20                          # margin from anchored edge (right)
y = 40                          # margin from top
width = 340
height = 500
transparent = true
always_on_top = true
decoration = false
```

### Modules

Modules are rendered top-to-bottom in the order they appear. Each `[[modules]]` block defines one line (or group of lines) on the overlay.

#### Built-in modules

```toml
[[modules]]
type = "hostname"

[[modules]]
type = "uptime"

[[modules]]
type = "time"
format = "%a %Y-%m-%d %H:%M:%S"

[[modules]]
type = "text"
content = "── Section Header ──"
style = { fg_color = "#0078d7" }

[[modules]]
type = "cpu"
label = "CPU"
show_per_core = false           # set true to show each core individually

[[modules]]
type = "memory"
label = "RAM"

[[modules]]
type = "disk"
mount_point = "/"

[[modules]]
type = "network"
interface = "eno1"
```

#### Shell commands

```toml
[[modules]]
type = "exec"
command = "sensors coretemp-isa-0000 2>/dev/null | awk '/Package/{print $4}'"
label = "TEMP"
style = { fg_color = "#ff6d00" }
```

#### Rhai scripts (requires `rhai-scripting` feature)

Inline:

```toml
[[modules]]
type = "rhai"
function = "status"
code = '''
fn status() {
    let pct = cpu_usage;
    if pct > 80.0 {
        styled(`CPU: ${pct}%`, #{ fg_color: "#ff0000" })
    } else {
        `CPU: ${pct}%`
    }
}
'''
```

File-based:

```toml
[[modules]]
type = "rhai"
file = "my_module.rhai"         # resolved relative to scripts_dir
function = "render"
```

#### Python scripts (requires `python-scripting` feature)

```toml
[[modules]]
type = "python"
file = "my_module.py"           # resolved relative to scripts_dir
function = "render"
```

A Python module function receives system data as a dict and returns a string, a dict, or a list:

```python
def render(ctx):
    pct = ctx["cpu_usage"]
    if pct > 80:
        return {"text": f"CPU: {pct:.1f}%", "fg_color": "#ff0000"}
    return f"CPU: {pct:.1f}%"
```

### Per-line styling

Any module that supports a `style` table accepts:

| Key | Type | Description |
|-----|------|-------------|
| `fg_color` | `"#RRGGBB"` or `"#RRGGBBAA"` | Text color |
| `bg_color` | `"#RRGGBB"` or `"#RRGGBBAA"` | Line background color |
| `font_size` | `f32` | Override font size for this line |

### on_draw hooks

An `on_draw` hook is a script function called after all modules have been collected but before rendering. It receives the full list of styled lines and the system context, and returns a (possibly modified) list.

```toml
[general]
on_draw_rhai = "on_draw.rhai"       # file in scripts_dir
# on_draw_python = "on_draw.py"
```

```javascript
// on_draw.rhai
fn on_draw(lines) {
    // Append a footer
    lines.push(`──── ${timestamp()} ────`);
    lines
}
```

### Script context

Both Rhai and Python scripts receive a snapshot of current system data:

| Field | Type | Description |
|-------|------|-------------|
| `cpu_usage` | `f64` | Total CPU usage (0–100) |
| `cpu_count` | `usize` | Number of logical cores |
| `cpu_per_core` | `[f64]` | Per-core usage |
| `mem_used` | `u64` | Used memory in bytes |
| `mem_total` | `u64` | Total memory in bytes |
| `mem_usage_pct` | `f64` | Memory usage percentage |
| `swap_used` | `u64` | Used swap in bytes |
| `swap_total` | `u64` | Total swap in bytes |
| `hostname` | `str` | System hostname |
| `uptime_seconds` | `u64` | Uptime in seconds |
| `os_name` | `str?` | OS name |
| `kernel_version` | `str?` | Kernel version |
| `disks` | `[{mount_point, total_bytes, available_bytes}]` | Disk info |
| `networks` | `[{interface, rx_bytes, tx_bytes}]` | Network info |

## Architecture

```
config.toml
    │
    ▼
┌─────────┐    ┌───────────┐    ┌──────────┐    ┌─────────┐
│  Config  │───▶│  Monitor  │───▶│ Renderer │───▶│ Wayland │
│  (TOML)  │    │ (sysinfo) │    │  (Skia)  │    │ (SCTK)  │
└─────────┘    └───────────┘    └──────────┘    └─────────┘
                    │                ▲
                    ▼                │
              ┌───────────┐    StyledLine[]
              │ Scripting │────────┘
              │ Rhai / Py │
              └───────────┘
```

- **Config** parses `~/.config/rustky/config.toml` and defines the module list
- **Monitor** collects system data via `sysinfo` and executes shell commands
- **Scripting** engines (optional) run Rhai/Python modules and on_draw hooks
- **Renderer** draws styled text lines to a Skia surface
- **Wayland** manages the layer-shell surface, buffer swaps, and the calloop event loop

## License

MIT
