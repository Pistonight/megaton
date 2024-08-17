//! File System Utilities
use filetime::FileTime;
use std::fs::File;
use std::path::{Path, PathBuf};

use crate::system::{self, Error};

/// Convenience wrapper for std::fs::remove_dir_all
pub fn remove_directory<P>(path: P) -> Result<(), Error>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    system::verboseln!("Removing", "{}", path.display());
    if !path.exists() {
        return Ok(());
    }
    std::fs::remove_dir_all(path).map_err(|e| Error::RemoveDirectory(path.display().to_string(), e))
}

/// Convenience wrapper for std::fs::create_dir_all
pub fn ensure_directory<P>(path: P) -> Result<(), Error>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    if path.exists() {
        return Ok(());
    }
    system::verboseln!("Creating", "{}", path.display());
    std::fs::create_dir_all(path).map_err(|e| Error::CreateDirectory(path.display().to_string(), e))
}

/// Convenience wrapper for std::fs::remove_file
pub fn remove_file<P>(path: P) -> Result<(), Error>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    system::verboseln!("Removing", "{}", path.display());
    if !path.exists() {
        return Ok(());
    }
    std::fs::remove_file(path).map_err(|e| Error::RemoveFile(path.display().to_string(), e))
}

/// Convenience wrapper for std::fs::rename
pub fn rename_file<P, Q>(from: P, to: Q) -> Result<(), Error>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    let from = from.as_ref();
    let to = to.as_ref();
    system::verboseln!("Renaming", "{} --> {}", from.display(), to.display());
    std::fs::rename(from, to)
        .map_err(|e| Error::RenameFile(from.display().to_string(), to.display().to_string(), e))
}

/// Convenience wrapper for std::fs::read_to_string
pub fn read_file<P>(path: P) -> Result<String, Error>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    std::fs::read_to_string(path).map_err(|e| Error::ReadFile(path.display().to_string(), e))
}

/// Wrapper for File::open
pub fn open<P>(path: P) -> Result<File, Error>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    File::open(path).map_err(|e| Error::ReadFile(path.display().to_string(), e))
}

/// Convenience wrapper for std::fs::write
pub fn write_file<P, S>(path: P, content: S) -> Result<(), Error>
where
    P: AsRef<Path>,
    S: AsRef<[u8]>,
{
    let path = path.as_ref();
    std::fs::write(path, content).map_err(|e| Error::WriteFile(path.display().to_string(), e))
}

pub fn create<P>(path: P) -> Result<File, Error>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    File::create(path).map_err(|e| Error::WriteFile(path.display().to_string(), e))
}

/// Replace file extension
pub fn replace_ext<P>(path: P, ext: &str) -> PathBuf
where
    P: AsRef<Path>,
{
    let mut path = path.as_ref().to_path_buf();
    path.set_extension(ext);
    path
}

/// Result for checking if an output is up-to-date
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UpToDate {
    Yes,
    Outdated,
    NotFound,
}

impl UpToDate {
    pub fn is_yes(self) -> bool {
        self == Self::Yes
    }
}

/// Check if a file is up-to-date based on its modification time
pub fn is_up_to_date<P>(path: P, time: FileTime) -> Result<UpToDate, Error>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    if !path.exists() {
        return Ok(UpToDate::NotFound);
    }

    let t = get_modified_time(path)?;
    if t >= time {
        Ok(UpToDate::Yes)
    } else {
        Ok(UpToDate::Outdated)
    }
}

/// Get the modified time for a file.
///
/// Returns None if the file does not exist or an error occurs
pub fn get_modified_time<P>(path: P) -> Result<FileTime, Error>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    if !path.exists() {
        return Err(Error::NotFound(path.display().to_string()));
    }

    path.metadata()
        .map(|x| FileTime::from_last_modification_time(&x))
        .map_err(|e| Error::ReadFile(path.display().to_string(), e))
}

/// Set the modified time for a file
pub fn set_modified_time<P>(path: P, time: FileTime) -> Result<(), Error>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    filetime::set_file_mtime(path, time)
        .map_err(|e| Error::SetModifiedTime(path.display().to_string(), e))
}

pub trait PathExt {
    /// Wrapper for std::path::canonicalize, but maps the error to our own
    fn canonicalize2(&self) -> Result<PathBuf, Error>;

    /// Get the relative path from base to self. Base must be an absolute path.
    /// Will error if self or base does not exist.
    fn with_base<P>(&self, base: P) -> Result<PathBuf, Error>
    where
        P: AsRef<Path>;
}

impl<P> PathExt for P
where
    P: AsRef<Path>,
{
    fn canonicalize2(&self) -> Result<PathBuf, Error> {
        dunce::canonicalize(self)
            .map_err(|x| Error::InvalidPath(self.as_ref().display().to_string(), x))
    }

    fn with_base<PBase>(&self, base: PBase) -> Result<PathBuf, Error>
    where
        PBase: AsRef<Path>,
    {
        let path = self.as_ref();
        let base = base.as_ref();
        assert!(base.is_absolute());
        Ok(pathdiff::diff_paths(path, base).unwrap_or(path.to_path_buf()))
    }
}
