use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::styled::LineStyle;

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub general: General,
    pub window: Window,
    pub modules: Vec<Module>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct General {
    pub update_interval_ms: u64,
    pub font: String,
    pub font_size: f32,
    pub fg_color: String,
    pub bg_color: String,
    pub scripts_dir: Option<String>,
    #[cfg(feature = "rhai-scripting")]
    pub on_draw_rhai: Option<String>,
    #[cfg(feature = "python-scripting")]
    pub on_draw_python: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Window {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub transparent: bool,
    pub always_on_top: bool,
    pub decoration: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Module {
    Cpu {
        #[serde(default = "default_label")]
        label: String,
        #[serde(default)]
        show_per_core: bool,
    },
    Memory {
        #[serde(default = "default_label_mem")]
        label: String,
    },
    Disk {
        #[serde(default = "default_mount")]
        mount_point: String,
    },
    Network {
        #[serde(default = "default_iface")]
        interface: String,
    },
    Uptime,
    Hostname,
    Time {
        #[serde(default = "default_time_format")]
        format: String,
    },
    Text {
        content: String,
    },
    Exec {
        command: String,
        label: Option<String>,
        #[serde(default)]
        style: Option<LineStyle>,
    },
    #[cfg(feature = "rhai-scripting")]
    Rhai {
        code: Option<String>,
        file: Option<String>,
        function: String,
    },
    #[cfg(feature = "python-scripting")]
    Python {
        file: String,
        function: String,
    },
}

fn default_label() -> String {
    "CPU".into()
}
fn default_label_mem() -> String {
    "MEM".into()
}
fn default_mount() -> String {
    "/".into()
}
fn default_iface() -> String {
    "eth0".into()
}
fn default_time_format() -> String {
    "%Y-%m-%d %H:%M:%S".into()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: General::default(),
            window: Window::default(),
            modules: vec![
                Module::Hostname,
                Module::Uptime,
                Module::Time {
                    format: default_time_format(),
                },
                Module::Cpu {
                    label: default_label(),
                    show_per_core: false,
                },
                Module::Memory {
                    label: default_label_mem(),
                },
                Module::Disk {
                    mount_point: default_mount(),
                },
            ],
        }
    }
}

impl Default for General {
    fn default() -> Self {
        Self {
            update_interval_ms: 1000,
            font: "monospace".into(),
            font_size: 12.0,
            fg_color: "#ffffff".into(),
            bg_color: "#000000aa".into(),
            scripts_dir: None,
            #[cfg(feature = "rhai-scripting")]
            on_draw_rhai: None,
            #[cfg(feature = "python-scripting")]
            on_draw_python: None,
        }
    }
}

impl Default for Window {
    fn default() -> Self {
        Self {
            x: 20,
            y: 40,
            width: 320,
            height: 600,
            transparent: true,
            always_on_top: true,
            decoration: false,
        }
    }
}

impl Config {
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("rustky")
            .join("config.toml")
    }

    #[allow(dead_code)]
    pub fn scripts_dir(&self) -> PathBuf {
        if let Some(ref dir) = self.general.scripts_dir {
            PathBuf::from(shellexpand(dir))
        } else {
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("~/.config"))
                .join("rustky")
                .join("scripts")
        }
    }

    #[allow(dead_code)]
    pub fn resolve_script_path(&self, path: &str) -> PathBuf {
        let expanded = shellexpand(path);
        let p = PathBuf::from(&expanded);
        if p.is_absolute() {
            p
        } else {
            self.scripts_dir().join(p)
        }
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        match std::fs::read_to_string(&path) {
            Ok(contents) => match toml::from_str(&contents) {
                Ok(cfg) => cfg,
                Err(e) => {
                    eprintln!("warning: failed to parse {}: {e}", path.display());
                    eprintln!("falling back to defaults");
                    Self::default()
                }
            },
            Err(_) => {
                eprintln!("no config found at {}, using defaults", path.display());
                Self::default()
            }
        }
    }

    pub fn generate_default_toml() -> String {
        toml::to_string_pretty(&Config::default()).expect("failed to serialize default config")
    }
}

#[allow(dead_code)]
fn shellexpand(s: &str) -> String {
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest).to_string_lossy().into_owned();
        }
    }
    s.to_string()
}
