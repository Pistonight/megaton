//! Error types

use std::process::ExitStatus;

use crate::errorln;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Cannot find required tool `{0}`. {1}")]
    MissingTool(String, String),
    #[error("Environment variable `{0}` is not set. {1}")]
    MissingEnv(String, String),
    #[error("Cannot access file `{0}`: {1}")]
    AccessFile(String, std::io::Error),
    #[error("Cannot access directory `{0}`: {1}")]
    AccessDirectory(String, std::io::Error),
    #[error("Cannot parse config file: {0}")]
    ParseConfig(String),
    #[error(
        "No entry point specified in the config. Please specify `entry` in the `make` section"
    )]
    NoEntryPoint,
    #[error("error executing `{0}`: `{1}`: {2}")]
    Subprocess(String, String, std::io::Error),
    #[error("Make failed! Check errors above.")]
    MakeError,
    #[error("Invalid objdump output `{0}`: {1}")]
    InvalidObjdump(String, String),
    #[error("Check failed! Check errors above.")]
    CheckError,
    #[error("Npdmtool failed: {0}")]
    NpdmError(ExitStatus),
}

impl Error {
    pub fn print(&self) {
        errorln!("Fatal", "{}", self);
    }
}
