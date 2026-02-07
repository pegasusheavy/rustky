use std::collections::HashMap;

use rhai::{Array, Dynamic, Engine, Map, Scope, AST};

use crate::script_context::ScriptContext;
use crate::styled::{LineStyle, StyledLine};

pub struct RhaiEngine {
    engine: Engine,
    compiled_files: HashMap<String, AST>,
    compiled_inline: HashMap<String, AST>,
    on_draw_ast: Option<AST>,
}

fn dynamic_to_styled_lines(val: Dynamic) -> Vec<StyledLine> {
    if val.is_array() {
        let arr = val.into_array().unwrap_or_default();
        arr.into_iter().flat_map(dynamic_to_styled_line).collect()
    } else {
        dynamic_to_styled_line(val)
    }
}

fn dynamic_to_styled_line(val: Dynamic) -> Vec<StyledLine> {
    if val.is_string() {
        let s = val.into_string().unwrap_or_default();
        return s.lines().map(|l| StyledLine::plain(l.to_string())).collect();
    }

    if val.is_map() {
        let map = val.cast::<Map>();
        let text = map
            .get("text")
            .and_then(|v| v.clone().into_string().ok())
            .unwrap_or_default();
        let fg_color = map
            .get("fg_color")
            .and_then(|v| v.clone().into_string().ok());
        let bg_color = map
            .get("bg_color")
            .and_then(|v| v.clone().into_string().ok());
        let font_size = map
            .get("font_size")
            .and_then(|v| v.as_float().ok().map(|f| f as f32));

        let style = LineStyle {
            fg_color,
            bg_color,
            font_size,
        };
        return vec![StyledLine::styled(text, style)];
    }

    vec![StyledLine::plain(val.to_string())]
}

fn context_to_scope(ctx: &ScriptContext) -> Scope<'static> {
    let mut scope = Scope::new();
    scope.push("cpu_usage", ctx.cpu_usage);
    scope.push("cpu_count", ctx.cpu_count as i64);
    scope.push(
        "cpu_per_core",
        ctx.cpu_per_core
            .iter()
            .map(|&v| Dynamic::from(v))
            .collect::<Array>(),
    );
    scope.push("mem_used", ctx.mem_used as i64);
    scope.push("mem_total", ctx.mem_total as i64);
    scope.push("mem_usage_pct", ctx.mem_usage_pct);
    scope.push("swap_used", ctx.swap_used as i64);
    scope.push("swap_total", ctx.swap_total as i64);
    scope.push("hostname", ctx.hostname.clone());
    scope.push("uptime_seconds", ctx.uptime_seconds as i64);
    scope.push(
        "os_name",
        ctx.os_name.clone().unwrap_or_default(),
    );
    scope.push(
        "kernel_version",
        ctx.kernel_version.clone().unwrap_or_default(),
    );

    // Disks as array of maps
    let disks: Array = ctx
        .disks
        .iter()
        .map(|d| {
            let mut m = Map::new();
            m.insert("mount_point".into(), Dynamic::from(d.mount_point.clone()));
            m.insert("total_bytes".into(), Dynamic::from(d.total_bytes as i64));
            m.insert(
                "available_bytes".into(),
                Dynamic::from(d.available_bytes as i64),
            );
            Dynamic::from(m)
        })
        .collect();
    scope.push("disks", disks);

    // Networks as array of maps
    let networks: Array = ctx
        .networks
        .iter()
        .map(|n| {
            let mut m = Map::new();
            m.insert("interface".into(), Dynamic::from(n.interface.clone()));
            m.insert("rx_bytes".into(), Dynamic::from(n.rx_bytes as i64));
            m.insert("tx_bytes".into(), Dynamic::from(n.tx_bytes as i64));
            Dynamic::from(m)
        })
        .collect();
    scope.push("networks", networks);

    scope
}

impl RhaiEngine {
    pub fn new() -> Self {
        let mut engine = Engine::new();

        // Register a `styled(text, style_map)` helper
        engine.register_fn("styled", |text: &str, style: Map| -> Dynamic {
            let mut m = Map::new();
            m.insert("text".into(), Dynamic::from(text.to_string()));
            if let Some(v) = style.get("fg_color") {
                m.insert("fg_color".into(), v.clone());
            }
            if let Some(v) = style.get("bg_color") {
                m.insert("bg_color".into(), v.clone());
            }
            if let Some(v) = style.get("font_size") {
                m.insert("font_size".into(), v.clone());
            }
            Dynamic::from(m)
        });

        // Convenience: `styled(text, fg_color_string)`
        engine.register_fn("styled", |text: &str, fg: &str| -> Dynamic {
            let mut m = Map::new();
            m.insert("text".into(), Dynamic::from(text.to_string()));
            m.insert("fg_color".into(), Dynamic::from(fg.to_string()));
            Dynamic::from(m)
        });

        Self {
            engine,
            compiled_files: HashMap::new(),
            compiled_inline: HashMap::new(),
            on_draw_ast: None,
        }
    }

    pub fn compile_file(&mut self, path: &str) -> Result<(), String> {
        let ast = self
            .engine
            .compile_file(path.into())
            .map_err(|e| format!("rhai compile error for {path}: {e}"))?;
        self.compiled_files.insert(path.to_string(), ast);
        Ok(())
    }

    pub fn compile_inline(&mut self, key: &str, code: &str) -> Result<(), String> {
        let ast = self
            .engine
            .compile(code)
            .map_err(|e| format!("rhai compile error for inline '{key}': {e}"))?;
        self.compiled_inline.insert(key.to_string(), ast);
        Ok(())
    }

    pub fn load_on_draw_hook(&mut self, path: &str) -> Result<(), String> {
        let ast = self
            .engine
            .compile_file(path.into())
            .map_err(|e| format!("rhai on_draw compile error: {e}"))?;
        self.on_draw_ast = Some(ast);
        Ok(())
    }

    pub fn execute_module(
        &self,
        key: &str,
        function: &str,
        ctx: &ScriptContext,
        is_file: bool,
    ) -> Vec<StyledLine> {
        let ast = if is_file {
            self.compiled_files.get(key)
        } else {
            self.compiled_inline.get(key)
        };

        let Some(ast) = ast else {
            return vec![StyledLine::plain(format!("[rhai: {key} not compiled]"))];
        };

        let mut scope = context_to_scope(ctx);

        let result = self
            .engine
            .call_fn::<Dynamic>(&mut scope, ast, function, ());

        match result {
            Ok(val) => dynamic_to_styled_lines(val),
            Err(e) => vec![StyledLine::plain(format!("[rhai error: {e}]"))],
        }
    }

    pub fn run_on_draw_hook(
        &self,
        lines: Vec<StyledLine>,
        ctx: &ScriptContext,
    ) -> Vec<StyledLine> {
        let Some(ref ast) = self.on_draw_ast else {
            return lines;
        };

        let mut scope = context_to_scope(ctx);

        // Convert lines to Rhai array of maps
        let lines_array: Array = lines
            .iter()
            .map(|l| {
                let mut m = Map::new();
                m.insert("text".into(), Dynamic::from(l.text.clone()));
                if let Some(ref fg) = l.style.fg_color {
                    m.insert("fg_color".into(), Dynamic::from(fg.clone()));
                }
                if let Some(ref bg) = l.style.bg_color {
                    m.insert("bg_color".into(), Dynamic::from(bg.clone()));
                }
                if let Some(fs) = l.style.font_size {
                    m.insert("font_size".into(), Dynamic::from(fs as f64));
                }
                Dynamic::from(m)
            })
            .collect();

        let result =
            self.engine
                .call_fn::<Dynamic>(&mut scope, ast, "on_draw", (lines_array,));

        match result {
            Ok(val) => dynamic_to_styled_lines(val),
            Err(e) => {
                eprintln!("rhai on_draw hook error: {e}");
                lines
            }
        }
    }
}
