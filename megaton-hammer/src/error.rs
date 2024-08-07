//! Error types

use std::process::ExitStatus;

use crate::errorln;

#[derive(Debug, thiserror::Error)]
pub enum Error {

    #[error("Cannot find required tool `{0}`. {1}")]
    MissingTool(String, String),
    #[error("Environment variable `{0}` is not set. {1}")]
    MissingEnv(String, String),

    // fs
    #[error("Cannot find `{0}`")]
    NotFound(String),
    #[error("Cannot find `{0}`. {1}")]
    NotFoundWithMessage(String, String),
    #[error("Invalid path `{0}`: {1}")]
    InvalidPath(String, std::io::Error),
    #[error("Cannot read file `{0}`: {1}")]
    ReadFile(String, std::io::Error),
    #[error("Cannot rename file `{0}` to `{1}`: {2}")]
    RenameFile(String, String, std::io::Error),
    #[error("Cannot write file `{0}`: {1}")]
    WriteFile(String, std::io::Error),
    #[error("Cannot remove file `{0}`: {1}")]
    RemoveFile(String, std::io::Error),
    #[error("Cannot create directory `{0}`: {1}")]
    CreateDirectory(String, std::io::Error),
    #[error("Cannot remove directory `{0}`: {1}")]
    RemoveDirectory(String, std::io::Error),
    #[error("Cannot set modified time for file `{0}`: {1}")]
    SetModifiedTime(String, std::io::Error),

    // process
    #[error("error spawning `{0}`: {1}")]
    SpawnChild(String, std::io::Error),
    #[error("error executing `{0}`: {1}")]
    WaitForChild(String, std::io::Error),

    #[error("Cannot parse config file: {0}")]
    ParseConfig(String),
    #[error("Please specify a profile with `--profile`")]
    NoProfile,
    #[error("Cannot parse `{0}`: {1}")]
    ParseJson(String, serde_json::Error),
    #[error(
        "No entry point specified in the config. Please specify `entry` in the `make` section"
    )]
    NoEntryPoint,
    #[error("Make failed! Check errors above.")]
    MakeError,
    #[error("Invalid objdump output `{0}`: {1}")]
    InvalidObjdump(String, String),
    #[error("Check failed! Check errors above.")]
    CheckError,
    #[error("Npdmtool failed: {0}")]
    NpdmError(ExitStatus),

    #[error("Cannot build toolchain: {0}")]
    BuildToolchain(String),

    
    #[cfg(windows)]
    #[error("The program is not supported on Windows.")]
    Windows,
}

impl Error {
    pub fn print(&self) {
        errorln!("Fatal", "{}", self);
    }
}
