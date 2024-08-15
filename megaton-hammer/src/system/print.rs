//! Print Utilities

use std::cell::RefCell;
use std::io::{IsTerminal, Write};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

static mut VERBOSE: bool = false;

pub fn is_verbose() -> bool {
    unsafe { VERBOSE }
}

pub fn enable_verbose() {
    unsafe { VERBOSE = true }
}

thread_local! {
    static STDOUT: RefCell<StandardStream> = RefCell::new(make_stdout());
}

fn make_stdout() -> StandardStream {
    let color_choice = if std::io::stdout().is_terminal() {
        ColorChoice::Auto
    } else {
        ColorChoice::Never
    };
    StandardStream::stdout(color_choice)
}

pub(crate) fn print_status_tag(color_spec: &ColorSpec, tag: &str) {
    STDOUT.with_borrow_mut(|stdout| {
        let _ = stdout.set_color(color_spec);
        let _ = write!(stdout, "{:>12}", tag);
        let _ = stdout.reset();
        print!(" ");
    });
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
            let status = { $status };
            $crate::system::print_status_tag(&$crate::system::info_color(), status);
            println!($($args)*);
        }
    };
}
pub(crate) use infoln;

macro_rules! errorln {
    ($status:expr, $($args:tt)*) => {
        {
            let status = { $status };
            $crate::system::print_status_tag(&$crate::system::error_color(), status);
            println!($($args)*);
        }
    };
}
pub(crate) use errorln;

macro_rules! hintln {
    ($status:expr, $($args:tt)*) => {
        {
            let status = { $status };
            $crate::system::print_status_tag(&$crate::system::hint_color(), status);
            println!($($args)*);
        }
    };
}
pub(crate) use hintln;

macro_rules! verboseln {
    ($status:expr, $($args:tt)*) => {
        {
            if ($crate::system::is_verbose()) {
                let status = { $status };
                $crate::system::print_status_tag(&$crate::system::hint_color(), status);
                println!($($args)*);
            }
        }
    };
}
pub(crate) use verboseln;
