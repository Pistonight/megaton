//! Error types

use crate::errorln;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Cannot find required tool `{0}`. {1}")]
    MissingTool(String, String),
    #[error("Environment variable `{0}` is not set. {1}")]
    MissingEnv(String, String),
    #[error("Cannot read file `{0}`: {1}")]
    ReadConfig(String, std::io::Error),
    #[error("Cannot create directory `{0}`: {1}")]
    CreateDirectory(String, std::io::Error),
    #[error("Cannot parse config file: {0}")]
    ParseConfig(String),
    #[error("No entry point specified in the config. Please specify `entry` in the `make` section")]
    NoEntryPoint,
}

impl Error {
    pub fn print(&self) {
        errorln!("Fatal", "{}", self);
    }
}