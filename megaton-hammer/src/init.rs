use std::path::Path;

use crate::error::Error;
use crate::{infoln, stdio};

pub fn init(path: &str) -> Result<(), Error> {
    let path = Path::new(path);
    if !path.exists() {
        return Err(Error::NotFound(path.display().to_string()));
    }
    let clangd_path = path.join(".clangd");
    if clangd_path.exists() {
        return Err(Error::AlreadyExists(clangd_path.display().to_string()));
    }
    let megaton_path = path.join("Megaton.toml");
    if megaton_path.exists() {
        return Err(Error::AlreadyExists(megaton_path.display().to_string()));
    }
    stdio::write_file(&clangd_path, include_str!("../clangd.template.yaml"))?;
    infoln!("Created", "{}", clangd_path.display());
    stdio::write_file(&megaton_path, include_str!("../Megaton.template.toml"))?;
    infoln!("Created", "{}", megaton_path.display());

    let gitignore_path = path.join(".gitignore");
    if !gitignore_path.exists() {
        stdio::write_file(&gitignore_path, include_str!("../gitignore.template"))?;
        infoln!("Created", "{}", gitignore_path.display());
    }

    Ok(())
}
