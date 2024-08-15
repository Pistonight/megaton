//! Macros for performing environment checks
use std::path::{Path, PathBuf};

use crate::system::{Error, PathExt};

/// Check if a binary exists, or return an error
macro_rules! check_tool {
    ($tool:literal) => {{
        which::which($tool).map_err(|_| {
            $crate::system::Error::MissingTool(
                $tool.to_string(),
                format!("Please ensure it is installed in the system."),
            )
        })
    }};
    ($tool:literal, $package:literal) => {{
        which::which($tool).map_err(|_| {
            $crate::system::Error::MissingTool(
                $tool.to_string(),
                format!("Please ensure `{}` is installed in the system.", $package),
            )
        })
    }};
    ($tool:expr, $package:literal) => {{
        let os_str: &std::ffi::OsStr = $tool.as_ref();
        which::which(os_str).map_err(|_| {
            $crate::system::Error::MissingTool(
                $tool.to_string_lossy().into_owned(),
                format!("Please ensure `{}` is installed in the system.", $package),
            )
        })
    }};
    ($tool:literal, $abs_path_backup:literal, $package:literal) => {{
        match which::which($tool) {
            Ok(x) => Ok(x),
            Err(_) => which::which($abs_path_backup).map_err(|_| {
                $crate::system::Error::MissingTool(
                    $tool.to_string(),
                    format!("Please ensure `{}` is installed in the system, or `{}` can be found in PATH", $package, $tool),
                )
            })
        }
    }};
}
pub(crate) use check_tool;

/// Check and get an environment variable, or return an error
macro_rules! check_env {
    ($env:literal, $message:literal) => {{
        let x = std::env::var($env).unwrap_or_default();
        if x.is_empty() {
            Err($crate::system::Error::MissingEnv(
                $env.to_string(),
                $message.to_string(),
            ))
        } else {
            Ok(x)
        }
    }};
}
pub(crate) use check_env;

/// Check if OS is supported
pub fn check_os() -> Result<(), Error> {
    #[cfg(windows)]
    {
        return Err(Error::Windows);
    }
    Ok(())
}

/// Find the directory that contains Megaton.toml
pub fn find_root(dir: &str) -> Result<PathBuf, Error> {
    let cwd = Path::new(dir).canonicalize2()?;
    let mut root: &Path = cwd.as_path();
    while !root.join("Megaton.toml").exists() {
        root = root.parent().ok_or(Error::NotProject)?;
    }
    Ok(root.to_path_buf())
}
