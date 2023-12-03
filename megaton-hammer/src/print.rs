use std::io::Write;
use std::{cell::RefCell, io::IsTerminal};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

static mut ENABLED: bool = true;
#[inline]
pub fn set_enabled(enabled: bool) {
    unsafe {
        ENABLED = enabled;
    }
}

#[inline]
pub fn is_enabled() -> bool {
    unsafe { ENABLED }
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

pub(crate) fn info_color() -> ColorSpec {
    let mut x = ColorSpec::new();
    x.set_fg(Some(Color::Green)).set_bold(true);
    x
}

pub(crate) fn hint_color() -> ColorSpec {
    let mut x = ColorSpec::new();
    x.set_fg(Some(Color::Yellow)).set_bold(true);
    x
}

pub(crate) fn error_color() -> ColorSpec {
    let mut x = ColorSpec::new();
    x.set_fg(Some(Color::Red)).set_bold(true);
    x
}

#[macro_export]
macro_rules! infoln {
    ($status:expr, $($args:tt)*) => {
        {
            use $crate::print::*;
            if is_enabled() {
                let status = { $status };
                print_status_tag(&info_color(), status);
                println!($($args)*);
            }
        }
    };
}

#[macro_export]
macro_rules! errorln {
    ($status:expr, $($args:tt)*) => {
        {
            use $crate::print::*;
            if is_enabled() {
                let status = { $status };
                print_status_tag(&error_color(), status);
                println!($($args)*);
            }
        }
    };
}

#[macro_export]
macro_rules! hintln {
    ($status:expr, $($args:tt)*) => {
        {
            use $crate::print::*;
            if is_enabled() {
                let status = { $status };
                print_status_tag(&hint_color(), status);
                println!($($args)*);
            }
        }
    };
}
