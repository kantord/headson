use std::io::IsTerminal as _;

use crate::serialization::types::ColorMode;

// ANSI SGR fragments
const RESET: &str = "\u{001b}[0m";
const BOLD_BLUE: &str = "\u{001b}[1;34m";
const GREEN: &str = "\u{001b}[32m";
const DARK_GRAY: &str = "\u{001b}[90m";

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ColorRole {
    Key,
    String,
}

pub fn wrap_role<S: Into<String>>(
    s: S,
    role: ColorRole,
    enabled: bool,
) -> String {
    let s: String = s.into();
    if !enabled {
        return s;
    }
    let prefix = match role {
        ColorRole::Key => BOLD_BLUE,
        ColorRole::String => GREEN,
    };
    let mut out = String::with_capacity(8 + 8 + s.len());
    out.push_str(prefix);
    out.push_str(&s);
    out.push_str(RESET);
    out
}

pub fn omission_marker(enabled: bool) -> &'static str {
    if enabled {
        "\u{001b}[90m…\u{001b}[0m"
    } else {
        "…"
    }
}

pub fn color_comment<S: Into<String>>(body: S, enabled: bool) -> String {
    if !enabled {
        return body.into();
    }
    let b = body.into();
    let mut out = String::with_capacity(b.len() + 8 + 4);
    out.push_str(DARK_GRAY);
    out.push_str(&b);
    out.push_str(RESET);
    out
}

fn env_bool(var: &str) -> Option<bool> {
    std::env::var_os(var).map(|v| {
        let s = v.to_string_lossy();
        !(s == "0" || s.is_empty())
    })
}

struct ColorEnv {
    force: bool,            // CLICOLOR_FORCE=1
    force_color: bool,      // FORCE_COLOR=1
    no_color: bool,         // NO_COLOR present
    dumb: bool,             // TERM=dumb
    clicolor: Option<bool>, // CLICOLOR=0/1
    is_tty: bool,
}

fn read_color_env() -> ColorEnv {
    let term_dumb = std::env::var_os("TERM")
        .map(|t| t.to_string_lossy() == "dumb")
        .unwrap_or(false);
    ColorEnv {
        force: matches!(env_bool("CLICOLOR_FORCE"), Some(true)),
        force_color: matches!(env_bool("FORCE_COLOR"), Some(true)),
        no_color: std::env::var_os("NO_COLOR").is_some(),
        dumb: term_dumb,
        clicolor: env_bool("CLICOLOR"),
        is_tty: std::io::stdout().is_terminal(),
    }
}

fn auto_color_enabled(env: &ColorEnv) -> bool {
    if env.force || env.force_color {
        return true;
    }
    if env.no_color || env.dumb {
        return false;
    }
    if let Some(b) = env.clicolor {
        return b && env.is_tty;
    }
    env.is_tty
}

pub fn resolve_color_enabled(mode: ColorMode) -> bool {
    match mode {
        ColorMode::On => true,
        ColorMode::Off => false,
        ColorMode::Auto => auto_color_enabled(&read_color_env()),
    }
}
