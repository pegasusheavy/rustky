use std::process::Command;

use sysinfo::{Disks, Networks, System};

use crate::config::Module;
use crate::script_context::{DiskInfo, NetworkInfo, ScriptContext};
use crate::styled::StyledLine;

pub struct Monitor {
    sys: System,
    disks: Disks,
    networks: Networks,
}

impl Monitor {
    pub fn new() -> Self {
        Self {
            sys: System::new_all(),
            disks: Disks::new_with_refreshed_list(),
            networks: Networks::new_with_refreshed_list(),
        }
    }

    pub fn refresh(&mut self) {
        self.sys.refresh_all();
        self.disks.refresh(true);
        self.networks.refresh(true);
    }

    #[allow(dead_code)]
    pub fn snapshot(&self) -> ScriptContext {
        let cpu_per_core: Vec<f64> = self
            .sys
            .cpus()
            .iter()
            .map(|cpu| cpu.cpu_usage() as f64)
            .collect();

        let disks: Vec<DiskInfo> = self
            .disks
            .list()
            .iter()
            .map(|d| DiskInfo {
                mount_point: d.mount_point().to_string_lossy().into_owned(),
                total_bytes: d.total_space(),
                available_bytes: d.available_space(),
            })
            .collect();

        let networks: Vec<NetworkInfo> = self
            .networks
            .list()
            .iter()
            .map(|(name, data)| NetworkInfo {
                interface: name.clone(),
                rx_bytes: data.total_received(),
                tx_bytes: data.total_transmitted(),
            })
            .collect();

        ScriptContext {
            cpu_usage: self.sys.global_cpu_usage() as f64,
            cpu_count: self.sys.cpus().len(),
            cpu_per_core,
            mem_used: self.sys.used_memory(),
            mem_total: self.sys.total_memory(),
            mem_usage_pct: if self.sys.total_memory() > 0 {
                self.sys.used_memory() as f64 / self.sys.total_memory() as f64 * 100.0
            } else {
                0.0
            },
            swap_used: self.sys.used_swap(),
            swap_total: self.sys.total_swap(),
            disks,
            networks,
            hostname: System::host_name().unwrap_or_else(|| "unknown".into()),
            uptime_seconds: System::uptime(),
            os_name: System::name(),
            kernel_version: System::kernel_version(),
        }
    }

    pub fn collect(&self, module: &Module) -> Vec<StyledLine> {
        match module {
            Module::Cpu {
                label,
                show_per_core,
            } => {
                if *show_per_core {
                    self.sys
                        .cpus()
                        .iter()
                        .enumerate()
                        .map(|(i, cpu)| {
                            StyledLine::plain(format!("  core {i}: {:.1}%", cpu.cpu_usage()))
                        })
                        .collect()
                } else {
                    let avg = self.sys.global_cpu_usage();
                    vec![StyledLine::plain(format!("{label}: {avg:.1}%"))]
                }
            }
            Module::Memory { label } => {
                let used = self.sys.used_memory() as f64 / 1_073_741_824.0;
                let total = self.sys.total_memory() as f64 / 1_073_741_824.0;
                let pct = if total > 0.0 {
                    used / total * 100.0
                } else {
                    0.0
                };
                vec![StyledLine::plain(format!(
                    "{label}: {used:.1}/{total:.1} GiB ({pct:.0}%)"
                ))]
            }
            Module::Disk { mount_point } => {
                for disk in self.disks.list() {
                    if disk.mount_point().to_string_lossy() == mount_point.as_str() {
                        let total = disk.total_space() as f64 / 1_073_741_824.0;
                        let avail = disk.available_space() as f64 / 1_073_741_824.0;
                        let used = total - avail;
                        return vec![StyledLine::plain(format!(
                            "DISK {mount_point}: {used:.1}/{total:.1} GiB"
                        ))];
                    }
                }
                vec![StyledLine::plain(format!("DISK {mount_point}: not found"))]
            }
            Module::Network { interface } => {
                for (name, data) in self.networks.list() {
                    if name == interface {
                        let rx = data.total_received() as f64 / 1_048_576.0;
                        let tx = data.total_transmitted() as f64 / 1_048_576.0;
                        return vec![StyledLine::plain(format!(
                            "NET {interface}: rx {rx:.1} MiB / tx {tx:.1} MiB"
                        ))];
                    }
                }
                vec![StyledLine::plain(format!("NET {interface}: not found"))]
            }
            Module::Uptime => {
                let secs = System::uptime();
                let h = secs / 3600;
                let m = (secs % 3600) / 60;
                vec![StyledLine::plain(format!("UPTIME: {h}h {m}m"))]
            }
            Module::Hostname => {
                let name = System::host_name().unwrap_or_else(|| "unknown".into());
                vec![StyledLine::plain(format!("HOST: {name}"))]
            }
            Module::Time { format } => {
                let now = chrono::Local::now();
                vec![StyledLine::plain(now.format(format).to_string())]
            }
            Module::Text { content } => vec![StyledLine::plain(content.clone())],
            Module::Exec {
                command,
                label,
                style,
            } => {
                let output = Command::new("sh")
                    .arg("-c")
                    .arg(command)
                    .output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                    .unwrap_or_else(|e| format!("exec error: {e}"));
                let text = if let Some(lbl) = label {
                    format!("{lbl}: {output}")
                } else {
                    output
                };
                if let Some(s) = style {
                    vec![StyledLine::styled(text, s.clone())]
                } else {
                    vec![StyledLine::plain(text)]
                }
            }
            #[cfg(feature = "rhai-scripting")]
            Module::Rhai { .. } => {
                // Rhai modules are executed by the scripting engine in wayland.rs
                vec![StyledLine::plain("[rhai: not executed]".into())]
            }
            #[cfg(feature = "python-scripting")]
            Module::Python { .. } => {
                // Python modules are executed by the scripting engine in wayland.rs
                vec![StyledLine::plain("[python: not executed]".into())]
            }
        }
    }
}
