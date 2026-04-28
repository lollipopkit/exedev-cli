use std::{
    env,
    fmt::Display,
    io::{self, IsTerminal},
};

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RED_BOLD: &str = "\x1b[1;31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const BLUE: &str = "\x1b[34m";
const MAGENTA: &str = "\x1b[35m";
const CYAN: &str = "\x1b[36m";
const CYAN_BOLD: &str = "\x1b[1;36m";

fn color_enabled(is_terminal: bool) -> bool {
    match env::var("EXEDEV_COLOR").ok().as_deref() {
        Some("always") | Some("1") | Some("true") => true,
        Some("never") | Some("0") | Some("false") => false,
        _ => is_terminal && env::var_os("NO_COLOR").is_none(),
    }
}

fn stdout_color() -> bool {
    color_enabled(io::stdout().is_terminal())
}

fn stderr_color() -> bool {
    color_enabled(io::stderr().is_terminal())
}

fn paint(value: impl Display, code: &str, enabled: bool) -> String {
    if enabled {
        format!("{code}{value}{RESET}")
    } else {
        value.to_string()
    }
}

pub(crate) fn heading(value: impl Display) -> String {
    paint(value, CYAN_BOLD, stdout_color())
}

pub(crate) fn label(value: impl Display) -> String {
    paint(value, BOLD, stdout_color())
}

pub(crate) fn vm(value: impl Display) -> String {
    paint(value, MAGENTA, stdout_color())
}

pub(crate) fn role(value: impl Display) -> String {
    paint(value, BLUE, stdout_color())
}

pub(crate) fn command(value: impl Display) -> String {
    paint(value, CYAN, stdout_color())
}

pub(crate) fn success(value: impl Display) -> String {
    paint(value, GREEN, stdout_color())
}

pub(crate) fn warn(value: impl Display) -> String {
    paint(value, YELLOW, stdout_color())
}

pub(crate) fn error(value: impl Display) -> String {
    paint(value, RED_BOLD, stderr_color())
}

pub(crate) fn muted(value: impl Display) -> String {
    paint(value, DIM, stdout_color())
}

pub(crate) fn stderr_block(value: impl Display) -> String {
    paint(value, YELLOW, stderr_color())
}
