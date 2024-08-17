//! Print Utilities

use std::io::Write;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, StandardStreamLock, WriteColor};

static mut VERBOSE: bool = false;
static mut COLOR: bool = true;

pub fn is_verbose() -> bool {
    unsafe { VERBOSE }
}

pub fn enable_verbose() {
    unsafe { VERBOSE = true }
}

pub fn disable_colors() {
    unsafe { COLOR = false }
}

pub fn stdout() -> StandardStream {
    let color = if unsafe { COLOR } {
        ColorChoice::Auto
    } else {
        ColorChoice::Never
    };
    StandardStream::stdout(color)
}

pub(crate) fn print_status_tag(stdout: &mut StandardStreamLock, color_spec: &ColorSpec, tag: &str) {
    let _ = stdout.set_color(color_spec);
    let _ = write!(stdout, "{:>12} ", tag);
    let _ = stdout.reset();
}

pub fn info_color() -> ColorSpec {
    let mut x = ColorSpec::new();
    x.set_fg(Some(Color::Green)).set_bold(true);
    x
}

pub fn hint_color() -> ColorSpec {
    let mut x = ColorSpec::new();
    x.set_fg(Some(Color::Yellow)).set_bold(true);
    x
}

pub fn error_color() -> ColorSpec {
    let mut x = ColorSpec::new();
    x.set_fg(Some(Color::Red)).set_bold(true);
    x
}

macro_rules! infoln {
    ($status:expr, $($args:tt)*) => {
        {
            use std::io::Write;
            let stdout = $crate::system::stdout();
            let mut stdout = stdout.lock();
            let status = { $status };
            $crate::system::print_status_tag(&mut stdout, &$crate::system::info_color(), status);
            let _ = writeln!(&mut stdout, $($args)*);
        }
    };
}
pub(crate) use infoln;

macro_rules! errorln {
    ($status:expr, $($args:tt)*) => {
        {
            use std::io::Write;
            let stdout = $crate::system::stdout();
            let mut stdout = stdout.lock();
            let status = { $status };
            $crate::system::print_status_tag(&mut stdout, &$crate::system::error_color(), status);
            let _ = writeln!(&mut stdout, $($args)*);
        }
    };
}
pub(crate) use errorln;

macro_rules! hintln {
    ($status:expr, $($args:tt)*) => {
        {
            use std::io::Write;
            let stdout = $crate::system::stdout();
            let mut stdout = stdout.lock();
            let status = { $status };
            $crate::system::print_status_tag(&mut stdout, &$crate::system::hint_color(), status);
            let _ = writeln!(&mut stdout, $($args)*);
        }
    };
}
pub(crate) use hintln;

macro_rules! verboseln {
    ($status:expr, $($args:tt)*) => {
        {
            if ($crate::system::is_verbose()) {
                use std::io::Write;
                let stdout = $crate::system::stdout();
                let mut stdout = stdout.lock();
                let status = { $status };
                $crate::system::print_status_tag(&mut stdout, &$crate::system::hint_color(), status);
                let _ = writeln!(&mut stdout, $($args)*);
            }
        }
    };
}
pub(crate) use verboseln;
