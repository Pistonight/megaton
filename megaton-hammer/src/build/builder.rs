//! Build flags processing

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::ChildStderr;

use filetime::FileTime;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::build::Paths;
use crate::build::config::Build;
use crate::system::{self, ChildBuilder, ChildProcess, Error, PathExt};

pub struct Builder<'a> {
    paths: &'a Paths,
    c_flags: Vec<String>,
    cpp_flags: Vec<String>,
    s_flags: Vec<String>,
    ld_flags: Vec<String>,
}

const DEFAULT_COMMON: &[&str] = &[
    "-march=armv8-a+crc+crypto",
    "-mtune=cortex-a57",
    "-mtp=soft",
    "-fPIC",
    "-fvisibility=hidden",
];

const DEFAULT_C: &[&str] = &[
    "-g",
    "-Wall",
    "-Werror",
    "-fdiagnostics-color=always",
    "-ffunction-sections",
    "-fdata-sections",
    "-O3",
];

const DEFAULT_CPP: &[&str] = &[
    "-fno-rtti",
    "-fno-exceptions",
    "-fno-asynchronous-unwind-tables",
    "-fno-unwind-tables",
    "-fpermissive",
    "-std=c++20",
];

const DEFAULT_S: &[&str] = &["-g"];

const DEFAULT_LD: &[&str] = &[
    "-g",
    "-nostartfiles",
    "-nodefaultlibs",
    "-Wl,--shared",
    "-Wl,--export-dynamic",
    "-Wl,-z,nodynamic-undefined-weak",
    "-Wl,--gc-sections",
    "-Wl,--build-id=sha1",
    "-Wl,--nx-module-name",
    "-Wl,--exclude-libs=ALL",
];

macro_rules! create_flags {
    ($field: expr, $default: expr) => {
        match $field {
            None => $default.iter().map(|x| x.to_string()).collect_vec(),
            Some(flags) => {
                let mut v = vec![];
                for flag in flags {
                    if flag == "<default>" {
                        v.extend($default.iter().map(|x| x.to_string()));
                    } else {
                        v.push(flag.clone());
                    }
                }
                v
            }
        }
    };
    ($field: expr, $default: ident extends $base: expr) => {
        match $field {
            None => $base
                .iter()
                .cloned()
                .chain($default.iter().map(|x| x.to_string()))
                .collect_vec(),
            Some(flags) => {
                let mut v = $base.clone();
                for flag in flags {
                    if flag == "<default>" {
                        v.extend($default.iter().map(|x| x.to_string()));
                    } else {
                        v.push(flag.clone());
                    }
                }
                v
            }
        }
    }
}

impl<'a> Builder<'a> {
    pub fn new(
        paths: &'a Paths, 
        entry: &str, 
        build: &Build
    ) -> Result<Self, Error> {
        let flags = &build.flags;
        let common = create_flags!(&flags.common, DEFAULT_COMMON);
        let mut includes = Vec::with_capacity(build.includes.len());
        for dir in &build.includes {
            let path = paths.root.join(dir).canonicalize2()?;
            includes.push(format!("-I{}", path.display()));
        }
        let mut c_flags = create_flags!(&flags.c, DEFAULT_C extends common);
        let mut cpp_flags = create_flags!(&flags.cxx, DEFAULT_CPP extends c_flags);
        let s_flags = create_flags!(&flags.as_, DEFAULT_S extends cpp_flags);
        c_flags.extend(includes.iter().cloned());
        cpp_flags.extend(includes.into_iter());

        let mut ld_flags = create_flags!(&flags.ld, DEFAULT_LD extends common);

        ld_flags.push(format!("-Wl,-init={}", entry));
        ld_flags.push(format!("-Wl,--version-script={}", paths.verfile.display()));
        for libpath in &build.libpaths {
            ld_flags.push(format!("-L{}", paths.root.join(libpath).canonicalize2()?.display()));
        }
        for lib in &build.libraries {
            ld_flags.push(format!("-l{}", lib));
        }
        for ldscript in &build.ldscripts {
            let path = paths.root.join(ldscript).canonicalize2()?;
            ld_flags.push(format!("-Wl,-T,{}", path.display()));
        }

        Ok(Self {
            paths,
            c_flags,
            cpp_flags,
            s_flags,
            ld_flags,
        })
    }

    fn create_command(
        &self,
        s_type: SourceType,
        source: String,
        d_file: String,
        o_file: String,
    ) -> CompileCommand {
        let arguments = match s_type {
            SourceType::C => std::iter::once(self.paths.make_c.display().to_string())
                .chain(
                    [
                        "-MMD".to_string(),
                        "-MP".to_string(),
                        "-MF".to_string(),
                        d_file,
                    ]
                    .into_iter(),
                )
                .chain(self.c_flags.iter().cloned())
                .chain(
                    [
                        "-c".to_string(),
                        "-o".to_string(),
                        o_file.clone(),
                        source.clone(),
                    ]
                    .into_iter(),
                )
                .collect_vec(),
            SourceType::Cpp => std::iter::once(self.paths.make_cpp.display().to_string())
                .chain(
                    [
                        "-MMD".to_string(),
                        "-MP".to_string(),
                        "-MF".to_string(),
                        d_file,
                    ]
                    .into_iter(),
                )
                .chain(self.cpp_flags.iter().cloned())
                .chain(
                    [
                        "-c".to_string(),
                        "-o".to_string(),
                        o_file.clone(),
                        source.clone(),
                    ]
                    .into_iter(),
                )
                .collect_vec(),
            SourceType::S => std::iter::once(self.paths.make_cpp.display().to_string())
                .chain(
                    [
                        "-MMD".to_string(),
                        "-MP".to_string(),
                        "-MF".to_string(),
                        d_file,
                        "-x".to_string(),
                        "assembler-with-cpp".to_string(),
                    ]
                    .into_iter(),
                )
                .chain(self.s_flags.iter().cloned())
                .chain(
                    [
                        "-c".to_string(),
                        "-o".to_string(),
                        o_file.clone(),
                        source.clone(),
                    ]
                    .into_iter(),
                )
                .collect_vec(),
        };

        CompileCommand {
            directory: "/".to_string(),
            arguments,
            file: source,
            output: o_file,
        }
    }

    pub fn process_source(
        &self,
        source_path: &Path,
        cc_possibly_changed: bool,
        compile_commands: &mut HashMap<String, CompileCommand>,
    ) -> Result<SourceResult, Error> {
        let source = source_path.display().to_string();
        let (source_type, base, ext) = match get_source_type(&source) {
            Some(x) => x,
            None => {
                return Ok(SourceResult::NotSource);
            }
        };
        let hashed = source_hashed(&source, base, ext);
        let o_path = self.paths.target_o.join(&format!("{}.o", hashed));
        let o_file = o_path.display().to_string();
        let d_path = self.paths.target_o.join(&format!("{}.d", hashed));
        let d_file = d_path.display().to_string();
        if !o_path.exists() {
            // output doesn't exist
            let cc = self.create_command(source_type, source, d_file, o_file);
            return Ok(SourceResult::NeedCompile(cc));
        }
        let o_mtime = system::get_modified_time(&o_path)?;
        // d file changed? (source included in d_file)
        if !are_deps_up_to_date(&d_path, o_mtime)? {
            let cc = self.create_command(source_type, source, d_file, o_file);
            return Ok(SourceResult::NeedCompile(cc));
        }
        // dependencies didn't change. Only rebuild if compile command changed
        if !cc_possibly_changed {
            return Ok(SourceResult::UpToDate(o_file));
        }
        let cc = self.create_command(source_type, source, d_file, o_file);
        match compile_commands.remove(&cc.file) {
            Some(old_cc) => {
                if old_cc == cc {
            Ok(SourceResult::UpToDate(cc.output))
                } else {
            Ok(SourceResult::NeedCompile(cc))
                }
            }
            None => {
                // no previous command found, (never built), need build
            Ok(SourceResult::NeedCompile(cc))
            }
        }
    }

    pub fn link_start(&self, objects: &[String], elf: &Path) -> Result<BuildTask, Error> {
        // use CXX for linking
        let child = ChildBuilder::new(&self.paths.make_cpp)
            .args(self.ld_flags.iter().chain(
                objects.iter()
            ).chain(
                [
                    "-o".to_string(),
                    elf.display().to_string(),
                ].iter()
            ))
            .silence_stdout()
            .pipe_stderr()
            .spawn()?;
        system::verboseln!("Running", "{}", child.command());
        Ok(BuildTask { child })
    }
}

pub enum SourceResult {
    NotSource,
    UpToDate(String),
    NeedCompile(CompileCommand),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompileCommand {
    #[serde(default)]
    directory: String,
    pub arguments: Vec<String>,
    pub file: String,
    pub output: String,
}


pub struct LinkResult {
    pub success: bool,
    pub error: Vec<String>,
}

impl CompileCommand {
    pub fn start(&self) -> Result<BuildTask, Error> {
        system::verboseln!("Running", "{}", self.arguments.join(" "));
        let child = ChildBuilder::new(&self.arguments[0])
            .args(&self.arguments[1..])
            .silence_stdout()
            .pipe_stderr()
            .spawn()?;
        Ok(BuildTask { child })
    }
}

pub struct BuildTask {
    child: ChildProcess,
}

impl BuildTask {
    pub fn wait(self)  -> Result<BuildResult, Error> {
        let mut child = self.child;
        let error = child.take_stderr();
        let status = child.wait()?;
        Ok(BuildResult {
            success: status.success(),
            error,
        })
    }
}

pub struct BuildResult {
    pub success: bool,
    pub error: Option<BufReader<ChildStderr>>,
}

pub fn load_compile_commands(cc_json: &Path, map: &mut HashMap<String, CompileCommand>) {
    system::verboseln!("Loading", "{}", cc_json.display());
    if !cc_json.exists() {
        return;
    }
    let file = match system::open(&cc_json) {
        Ok(file) => BufReader::new(file),
        Err(_) => {
            return;
        }
    };
    let ccs: Vec<CompileCommand> = match serde_json::from_reader(file) {
        Ok(ccs) => ccs,
        Err(_) => return,
    };
    for cc in ccs {
        map.insert(cc.file.clone(), cc);
    }
}

pub enum SourceType {
    C,
    Cpp,
    S,
}

impl SourceType {
    pub fn from_ext(ext: &str) -> Option<Self> {
        match ext {
            ".c" => Some(Self::C),
            ".cpp" | ".cc" | ".cxx" | ".c++" => Some(Self::Cpp),
            ".s" | ".asm" => Some(Self::S),
            _ => None,
        }
    }
}

fn get_source_type(source: &str) -> Option<(SourceType, &str, &str)> {
    let dot = source.rfind('.').unwrap_or_else(|| source.len());
    let ext = &source[dot..];
    let source_type = SourceType::from_ext(ext)?;
    let slash = source.rfind(|c| c == '/' || c == '\\').unwrap_or(0);
    let base = &source[slash+1..dot];
    if base.is_empty() {
        return None;
    }
    Some((source_type, base, ext))
}

fn source_hashed(source: &str, base: &str, ext: &str) -> String {
    let mut hasher = DefaultHasher::default();
    source.hash(&mut hasher);
    let hash = hasher.finish();
    format!("{}-{:016x}{}", base, hash, ext)
}

fn are_deps_up_to_date(d_path: &Path, o_mtime: FileTime) -> Result<bool, Error> {
    // (very strong) assumptions of the depfiles:
    // - the first rule is what we care about (the target)
    // - the first line is just the target
    if !d_path.exists() {
        return Ok(false);
    }
    let lines = BufReader::new(system::open(d_path)?).lines();
    for line in lines.skip(1) {
        // skip the <target>: \ line
        let line = match line {
            Ok(x) => x,
            Err(_) => return Ok(false),
        };
        let part = line.trim().trim_end_matches('\\').trim_end();
        if part.ends_with(':') {
            break;
        }
        let d_mtime = match system::get_modified_time(part) {
            Ok(x) => x,
            Err(_) => return Ok(false),
        };
        if d_mtime > o_mtime {
            return Ok(false);
        }
    }
    Ok(true)
}
