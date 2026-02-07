use std::collections::HashMap;
use std::ffi::CString;

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString};

use crate::script_context::ScriptContext;
use crate::styled::{LineStyle, StyledLine};

pub struct PythonEngine {
    loaded_modules: HashMap<String, Py<PyAny>>,
    on_draw_module: Option<Py<PyAny>>,
}

fn pyany_to_styled_lines(py: Python<'_>, val: &Bound<'_, PyAny>) -> Vec<StyledLine> {
    if let Ok(list) = val.cast::<PyList>() {
        return list
            .iter()
            .flat_map(|item| pyany_to_styled_line(py, &item))
            .collect();
    }
    pyany_to_styled_line(py, val)
}

fn pyany_to_styled_line(_py: Python<'_>, val: &Bound<'_, PyAny>) -> Vec<StyledLine> {
    if let Ok(s) = val.cast::<PyString>() {
        let text = s.to_string();
        return text
            .lines()
            .map(|l| StyledLine::plain(l.to_string()))
            .collect();
    }

    if let Ok(dict) = val.cast::<PyDict>() {
        let text = dict
            .get_item("text")
            .ok()
            .flatten()
            .and_then(|v| v.extract::<String>().ok())
            .unwrap_or_default();
        let fg_color = dict
            .get_item("fg_color")
            .ok()
            .flatten()
            .and_then(|v| v.extract::<String>().ok());
        let bg_color = dict
            .get_item("bg_color")
            .ok()
            .flatten()
            .and_then(|v| v.extract::<String>().ok());
        let font_size = dict
            .get_item("font_size")
            .ok()
            .flatten()
            .and_then(|v| v.extract::<f32>().ok());

        let style = LineStyle {
            fg_color,
            bg_color,
            font_size,
        };
        return vec![StyledLine::styled(text, style)];
    }

    vec![StyledLine::plain(val.to_string())]
}

fn context_to_pydict<'py>(py: Python<'py>, ctx: &ScriptContext) -> Bound<'py, PyDict> {
    let dict = PyDict::new(py);
    let _ = dict.set_item("cpu_usage", ctx.cpu_usage);
    let _ = dict.set_item("cpu_count", ctx.cpu_count);
    let _ = dict.set_item("cpu_per_core", &ctx.cpu_per_core);
    let _ = dict.set_item("mem_used", ctx.mem_used);
    let _ = dict.set_item("mem_total", ctx.mem_total);
    let _ = dict.set_item("mem_usage_pct", ctx.mem_usage_pct);
    let _ = dict.set_item("swap_used", ctx.swap_used);
    let _ = dict.set_item("swap_total", ctx.swap_total);
    let _ = dict.set_item("hostname", &ctx.hostname);
    let _ = dict.set_item("uptime_seconds", ctx.uptime_seconds);
    let _ = dict.set_item("os_name", &ctx.os_name);
    let _ = dict.set_item("kernel_version", &ctx.kernel_version);

    let disks: Vec<Bound<'py, PyDict>> = ctx
        .disks
        .iter()
        .map(|d| {
            let dd = PyDict::new(py);
            let _ = dd.set_item("mount_point", &d.mount_point);
            let _ = dd.set_item("total_bytes", d.total_bytes);
            let _ = dd.set_item("available_bytes", d.available_bytes);
            dd
        })
        .collect();
    let _ = dict.set_item("disks", disks);

    let networks: Vec<Bound<'py, PyDict>> = ctx
        .networks
        .iter()
        .map(|n| {
            let nd = PyDict::new(py);
            let _ = nd.set_item("interface", &n.interface);
            let _ = nd.set_item("rx_bytes", n.rx_bytes);
            let _ = nd.set_item("tx_bytes", n.tx_bytes);
            nd
        })
        .collect();
    let _ = dict.set_item("networks", networks);

    dict
}

fn styled_lines_to_pylist<'py>(py: Python<'py>, lines: &[StyledLine]) -> Bound<'py, PyList> {
    let items: Vec<Bound<'py, PyDict>> = lines
        .iter()
        .map(|l| {
            let d = PyDict::new(py);
            let _ = d.set_item("text", &l.text);
            if let Some(fg) = &l.style.fg_color {
                let _ = d.set_item("fg_color", fg);
            }
            if let Some(bg) = &l.style.bg_color {
                let _ = d.set_item("bg_color", bg);
            }
            if let Some(fs) = l.style.font_size {
                let _ = d.set_item("font_size", fs);
            }
            d
        })
        .collect();
    PyList::new(py, &items).expect("failed to create PyList")
}

fn to_cstring(s: &str) -> CString {
    CString::new(s).unwrap_or_else(|_| CString::new("rustky_script").unwrap())
}

impl PythonEngine {
    pub fn new() -> Self {
        Self {
            loaded_modules: HashMap::new(),
            on_draw_module: None,
        }
    }

    pub fn load_file(&mut self, path: &str) -> Result<(), String> {
        Python::attach(|py| {
            let code =
                std::fs::read_to_string(path).map_err(|e| format!("failed to read {path}: {e}"))?;

            // Add parent directory to sys.path so imports work
            if let Some(parent) = std::path::Path::new(path).parent() {
                let sys = py.import("sys").map_err(|e| format!("import sys: {e}"))?;
                let sys_path = sys
                    .getattr("path")
                    .map_err(|e| format!("sys.path: {e}"))?;
                let parent_str = parent.to_string_lossy().to_string();
                sys_path
                    .call_method1("insert", (0, &parent_str))
                    .map_err(|e| format!("sys.path.insert: {e}"))?;
            }

            let code_cstr = to_cstring(&code);
            let path_cstr = to_cstring(path);
            let name_cstr = to_cstring(&module_name_from_path(path));

            let module = PyModule::from_code(py, &code_cstr, &path_cstr, &name_cstr)
                .map_err(|e| format!("python compile error for {path}: {e}"))?;

            self.loaded_modules
                .insert(path.to_string(), module.into_any().unbind());
            Ok(())
        })
    }

    pub fn load_on_draw_hook(&mut self, path: &str) -> Result<(), String> {
        Python::attach(|py| {
            let code =
                std::fs::read_to_string(path).map_err(|e| format!("failed to read {path}: {e}"))?;

            let code_cstr = to_cstring(&code);
            let path_cstr = to_cstring(path);
            let name_cstr = to_cstring("rustky_on_draw");

            let module = PyModule::from_code(py, &code_cstr, &path_cstr, &name_cstr)
                .map_err(|e| format!("python on_draw compile error: {e}"))?;

            self.on_draw_module = Some(module.into_any().unbind());
            Ok(())
        })
    }

    pub fn execute_module(
        &self,
        file_path: &str,
        function: &str,
        ctx: &ScriptContext,
    ) -> Vec<StyledLine> {
        let Some(module) = self.loaded_modules.get(file_path) else {
            return vec![StyledLine::plain(format!(
                "[python: {file_path} not loaded]"
            ))];
        };

        Python::attach(|py| {
            let ctx_dict = context_to_pydict(py, ctx);
            let module_ref = module.bind(py);

            match module_ref.call_method1(function, (ctx_dict,)) {
                Ok(result) => pyany_to_styled_lines(py, &result),
                Err(e) => vec![StyledLine::plain(format!("[python error: {e}]"))],
            }
        })
    }

    pub fn run_on_draw_hook(
        &self,
        lines: Vec<StyledLine>,
        ctx: &ScriptContext,
    ) -> Vec<StyledLine> {
        let Some(module) = &self.on_draw_module else {
            return lines;
        };

        Python::attach(|py| {
            let ctx_dict = context_to_pydict(py, ctx);
            let lines_list = styled_lines_to_pylist(py, &lines);
            let module_ref = module.bind(py);

            match module_ref.call_method1("on_draw", (lines_list, ctx_dict)) {
                Ok(result) => pyany_to_styled_lines(py, &result),
                Err(e) => {
                    eprintln!("python on_draw hook error: {e}");
                    lines
                }
            }
        })
    }
}

fn module_name_from_path(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "rustky_script".into())
}
