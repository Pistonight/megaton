//! Utilities and wrappers for std io, fs, path and process.

use std::io::{BufReader, BufRead};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio, ExitStatus, ChildStderr, ChildStdout, ChildStdin};
use std::ffi::OsStr;
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};

use crate::error::Error;
use crate::{errorln, hintln};

use filetime::FileTime;

/// Convenience macro for building an argument list
macro_rules! args {
    ($($arg:expr),* $(,)?) => {
        {
            let args: Vec<&std::ffi::OsStr> = vec![$($arg.as_ref()),*];
            args
        }
    };
}
pub (crate) use args;

/// Check if a binary exists, or return an error
macro_rules! check_tool {
    ($tool:literal) => {
        {
            which::which($tool).map_err(|_| {
                Error::MissingTool(
                    $tool.to_string(),
                    format!("Please ensure it is installed in the system.")
                )
            })
        }
    };
    ($tool:literal, $package:literal) => {
        {
            which::which($tool).map_err(|_| {
                Error::MissingTool(
                    $tool.to_string(),
                    format!("Please ensure {} is installed in the system.", $package)
                )
            })
        }
    };
    ($tool:expr, $package:literal) => {
        {
            let os_str: &std::ffi::OsStr = $tool.as_ref();
            which::which(os_str).map_err(|_| {
                Error::MissingTool(
                    $tool.to_string_lossy().into_owned(),
                    format!("Please ensure {} is installed in the system.", $package)
                )
            })
        }
    };
}
pub (crate) use check_tool;

/// Check and get an environment variable, or return an error
macro_rules! check_env {
    ($env:literal, $message:literal) => {
        {
            let x = std::env::var($env).unwrap_or_default();
            if x.is_empty() {
                Err(Error::MissingEnv(
                    $env.to_string(),
                    $message.to_string()
                ))
            } else {
                Ok(x)
            }
        }
    };
}
pub (crate) use check_env;

/// Convenience wrapper around `Command` for building a child process
pub struct ChildBuilder {
    arg0: String,
    command: Command,
}

impl ChildBuilder {
    pub fn new<S>(arg0: S) -> Self
    where
        S: AsRef<OsStr>,
    {
        Self {
            arg0: arg0.as_ref().to_string_lossy().to_string(),
            command: Command::new(arg0),
        }
    }

    #[inline]
    pub fn current_dir<P>(mut self, dir: P) -> Self
    where
        P: AsRef<Path>,
    {
        self.command.current_dir(dir);
        self
    }

    /// Set args as in `Command`
    #[inline]
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.command.args(args);
        self
    }

    /// Set stdin to pipe
    #[inline]
    pub fn pipe_stdin(mut self) -> Self {
        self.command.stdin(Stdio::piped());
        self
    }

    /// Set stdout to pipe
    #[inline]
    pub fn pipe_stdout(mut self) -> Self {
        self.command.stdout(Stdio::piped());
        self
    }

    /// Set stderr to pipe
    #[inline]
    pub fn pipe_stderr(mut self) -> Self {
        self.command.stderr(Stdio::piped());
        self
    }

    /// Set stdout and stderr to pipe
    #[inline]
    pub fn piped(self) -> Self {
        self.pipe_stdout().pipe_stderr()
    }

    /// Set stdout to null
    #[inline]
    pub fn silence_stdout(mut self) -> Self {
        self.command.stdout(Stdio::null());
        self
    }

    /// Set stderr to null
    #[inline]
    pub fn silence_stderr(mut self) -> Self {
        self.command.stderr(Stdio::null());
        self
    }

    /// Set stdout and stderr to null
    #[inline]
    pub fn silent(self) -> Self {
        self.silence_stdout().silence_stderr()
    }

    pub fn spawn(mut self) -> Result<ChildProcess, Error> {
        // we don't care about escaping it properly, just for debugging
        let args_str = self.command
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(" ");
        let command_str = format!("{} {}", self.arg0, args_str);
        let child = self.command.spawn().map_err(|e| Error::SpawnChild(command_str.clone(), e))?;
        Ok(ChildProcess { command_str, child })
    }
}

/// Convenience wrapper around `Child` for a spawned process
pub struct ChildProcess {
    command_str: String,
    child: Child,
}

impl ChildProcess {
    pub fn take_stdin(&mut self) -> ChildStdin {
        self.child.stdin.take().expect("stdin is not piped! Need to call `pipe_stdin` on the builder!")
    }
    /// Take the stdout of the child process and wrap it in a `BufReader`
    pub fn take_stdout(&mut self) -> Option<BufReader<ChildStdout>> {
        self.child.stdout.take().map(BufReader::new)
    }

    /// Take the stderr of the child process and wrap it in a `BufReader`
    pub fn take_stderr(&mut self) -> Option<BufReader<ChildStderr>> {
        self.child.stderr.take().map(BufReader::new)
    }

    /// Take output with extra settings
    pub fn take_output(&mut self) -> TermIter {
        let (send, recv) = mpsc::channel();

        let mut handles = vec![];

        if let Some(stdout) = self.take_stdout() {
            let send = send.clone();
            let handle = thread::spawn(move || {
                for line in TermLines::new(stdout).flatten() {
                    if send.send(TermOut::Stdout(line)).is_err() {
                        break;
                    }
                }
            });

            handles.push(handle);
        }

        if let Some(stderr) = self.take_stderr() {
            let handle = thread::spawn(move || {
                for line in TermLines::new(stderr).flatten() {
                    if send.send(TermOut::Stderr(line)).is_err() {
                        break;
                    }
                }
            });

            handles.push(handle);
        }

        TermIter {
            recv,
            join_handles: handles,
        }

    }


    /// Wait for the child process to exit
    pub fn wait(mut self) -> Result<ExitStatus, Error> {
        let status = self.child.wait().map_err(|e| Error::WaitForChild(self.command_str.clone(), e))?;
        Ok(status)
    }

    /// Take the stderr, and dump it using `errorln!`
    pub fn dump_stderr(&mut self, prefix: &str) {
        if let Some(stderr) = self.take_stderr() {
            for line in TermLines::new(stderr).flatten() {
                errorln!(prefix, "{line}");
            }
        }
    }

    /// Take the stdout, and dump it using `hintln!`
    pub fn dump_stdout(&mut self, prefix: &str) {
        if let Some(stderr) = self.take_stdout() {
            for line in TermLines::new(stderr).flatten() {
                hintln!(prefix, "{line}");
            }
        }
    }

    /// Dump with extra settings
    pub fn dump(&mut self, stdout_prefix: Option<&str>, stderr_prefix: Option<&str>, step: usize) {
        for msg in self.take_output().step_by(step) {
            match msg {
                TermOut::Stdout(line) => {
                    if let Some(prefix) = stdout_prefix {
                        hintln!(prefix, "{line}");
                    }
                },
                TermOut::Stderr(line) => {
                    if let Some(prefix) = stderr_prefix {
                        errorln!(prefix, "{line}");
                    }
                },
            }

        }

    }
}

#[derive(Debug)]
pub struct TermIter {
    recv: Receiver<TermOut>,
    join_handles: Vec<JoinHandle<()>>,
}

impl Iterator for TermIter {
    type Item = TermOut;

    fn next(&mut self) -> Option<Self::Item> {
        match self.recv.recv() {
            Ok(x) => Some(x),
            Err(_) => {
                while let Some(handle) = self.join_handles.pop() {
                    let _ = handle.join();
                }
                None
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TermOut {
    Stdout(String),
    Stderr(String),
}

impl AsRef<str> for TermOut {
    fn as_ref(&self) -> &str {
        match self {
            TermOut::Stdout(x) => x,
            TermOut::Stderr(x) => x,
        }
    }
}

impl Into<String> for TermOut {
    fn into(self) -> String {
        match self {
            TermOut::Stdout(x) => x,
            TermOut::Stderr(x) => x,
        }
    }
}

/// Wrapper for reader that buffers the output until CR or LF
#[derive(Debug)]
pub struct TermLines<R> where R: BufRead {
    read: R,
    buffer: Vec<u8>,
}

impl<R> TermLines<R> where R: BufRead {
    pub fn new(read: R) -> Self {
        Self {
            read,
            buffer: Vec::new(),
        }
    }
}

impl<R> Iterator for TermLines<R> where R: BufRead {
    type Item = std::io::Result<String>;

    fn next(&mut self) -> Option<Self::Item> {
        self.buffer.clear();
        let mut buf: [u8; 1] = [0];

        loop {
            if let Err(e) = self.read.read_exact(&mut buf) {
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    return None;
                }
                return Some(Err(e));
            }

            let c = buf[0];
            if c == b'\n' || c == b'\r' {
                return Some(Ok(String::from_utf8_lossy(&self.buffer).into_owned()));
            }
            self.buffer.push(c);
        }
    }

}

/// Convenience wrapper for std::fs::remove_file
pub fn remove_file<P>(path: P) -> Result<(), Error>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    if !path.exists() {
        return Ok(());
    }
    std::fs::remove_file(path).map_err(|e| Error::RemoveFile(path.display().to_string(), e))
}

pub fn rename_file<P, Q>(from: P, to: Q) -> Result<(), Error>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    let from = from.as_ref();
    let to = to.as_ref();
    std::fs::rename(from, to).map_err(|e| Error::RenameFile(from.display().to_string(), to.display().to_string(), e))
}

/// Convenience wrapper for std::fs::remove_dir_all
pub fn remove_directory<P>(path: P) -> Result<(), Error>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    if !path.exists() {
        return Ok(());
    }
    std::fs::remove_dir_all(path).map_err(|e| Error::RemoveDirectory(path.display().to_string(), e))
}

/// Convenience wrapper for std::fs::read_to_string
pub fn read_file<P>(path: P) -> Result<String, Error>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    std::fs::read_to_string(path).map_err(|e| Error::ReadFile(path.display().to_string(), e))
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

/// Get the modified time for a file. Returns None if the file does not exist or an error occurs
pub fn get_modified_time<P>(path: P) -> Result<FileTime, Error>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    if !path.exists() {
        return Err(Error::NotFound(path.display().to_string()));
    }

    path.metadata().map(|x|FileTime::from_last_modification_time(&x)).map_err(|e| Error::ReadFile(path.display().to_string(), e))
}

/// Set the modified time for a file
pub fn set_modified_time<P>(path: P, time: FileTime) -> Result<(), Error>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    filetime::set_file_mtime(path, time).map_err(|e| Error::SetModifiedTime(path.display().to_string(), e))
}

pub fn ensure_directory<P>(path: P) -> Result<(), Error>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    if !path.exists() {
        std::fs::create_dir_all(path).map_err(|e| Error::CreateDirectory(path.display().to_string(), e))?;
    }
    Ok(())
}

pub trait PathExt {
    /// Wrapper for std::path::canonicalize, but maps the error to our own
    fn canonicalize2(&self) -> Result<PathBuf, Error>;

    /// Get the relative path from base to self. Base must be an absolute path.
    /// Will error if self or base does not exist.
    fn from_base<P>(&self, base: P) -> Result<PathBuf, Error>
    where
        P: AsRef<Path>;

}

impl<P> PathExt for P where P: AsRef<Path> {
    fn canonicalize2(&self) -> Result<PathBuf, Error>
    {
        let path = self.as_ref();
        path.canonicalize().map_err(|x| Error::InvalidPath(path.display().to_string(), x))
    }

    fn from_base<PBase>(&self, base: PBase) -> Result<PathBuf, Error>
    where
        PBase: AsRef<Path>,
    {
        let path = self.as_ref();
        let base = base.as_ref();
        assert!(base.is_absolute());
        Ok(pathdiff::diff_paths(path, base).unwrap_or(path.to_path_buf()))
    }
}

macro_rules! root_rel {
    ($paths:ident.$member:ident) => {
        $paths.$member.from_base(&$paths.root)
    };
}
pub(crate) use root_rel;

