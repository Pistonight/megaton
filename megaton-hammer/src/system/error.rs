//! Error types

use std::process::ExitStatus;

use crate::system;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    // pre-check
    #[error("`Megaton.toml` not found. Please run inside a Megaton project.")]
    NotProject,
    #[error("Cannot find required tool `{0}`. {1}")]
    MissingTool(String, String),
    #[error("Environment variable `{0}` is not set. {1}")]
    MissingEnv(String, String),

    // fs
    #[error("Cannot find `{0}`")]
    NotFound(String),
    #[error("`{0}` already exists")]
    AlreadyExists(String),
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

    // config
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

    // build
    #[error("One or more object files failed to compile. Please check the errors above.")]
    CompileError,
    #[error("Linking failed. Please check the errors above")]
    LinkError,
    #[error("Invalid objdump output `{0}`: {1}")]
    InvalidObjdump(String, String),
    #[error("Objdump exited with status `{0}`")]
    ObjdumpFailed(ExitStatus),
    #[error("Check failed! Check errors above.")]
    CheckError,
    #[error("Failed to convert ELF to NSO!")]
    Elf2NsoError,
    #[error("Npdmtool failed: {0}")]
    NpdmError(ExitStatus),

    #[error("Cannot build toolchain: {0}")]
    BuildToolchain(String),

    #[error("parsing regex: {0}")]
    Regex(#[from] regex::Error),

    #[cfg(windows)]
    #[error("The program is not supported on Windows.")]
    Windows,
}

impl Error {
    pub fn print(&self) {
        system::errorln!("Fatal", "{}", self);
    }
}
