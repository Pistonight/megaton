//! Script for building custom rustc toolchain and libraries

use std::io::{Write, BufRead};
use std::path::Path;

use crate::{infoln, hintln};
use crate::stdio::{self, check_tool, check_env, PathExt, ChildBuilder, args, ChildProcess};
use crate::error::Error;

pub fn build() -> Result<(), Error> {
    infoln!("Building", "Megaton toolchain");

    let megaton_home = check_env!("MEGATON_HOME", 
    "Please set MEGATON_HOME to the root of your local megaton repository")?;
    let megaton_home = megaton_home.canonicalize2()?;
    hintln!("Path", "MEGATON_HOME = {}", megaton_home.display());
    
    let toolchain_path = megaton_home.join("toolchain").canonicalize2()?;
    hintln!("Path", "Toolchain = {}", toolchain_path.display());
    
    check_tool!("rustup", "Rust")?;
    check_tool!("rustc", "Rust")?;
    check_tool!("git")?;
    check_tool!("ninja")?;

    setup_rustc_repo(&toolchain_path)?;
    build_rustc(&toolchain_path)?;




    todo!();
}

fn setup_rustc_repo(toolchain_path: &Path) -> Result<(), Error> {
    infoln!("Cloning", "rustc");
    let rustc_path = toolchain_path.join("rustc");
    if rustc_path.exists() {
        hintln!("Removing", "{}", rustc_path.display());
        stdio::remove_directory(&rustc_path)?;
    }
    let mut clone_command = ChildBuilder::new("git")
        .args(args![
            "clone", 
            "https://github.com/rust-lang/rust", 
            rustc_path,
            "--depth",
            "1",
            "--progress"

        ])
        .piped().spawn()?;

    clone_command.dump(None, Some("Git"), 5);
    let status = clone_command.wait()?;
    if !status.success() {
        return Err(Error::BuildToolchain("Failed to clone rustc".to_string()));
    }
    infoln!("Cloned", "rustc");

    let mut setup_command = ChildBuilder::new("./x")
        .current_dir(&rustc_path)
        .args(args![
            "setup",
        ])
        .pipe_stdin()
        .piped().spawn()?;

    let setup_input = stdio::read_file(toolchain_path.join("rustc-setup.txt"))?;
    if setup_command.take_stdin().write_all(setup_input.as_bytes()).is_err() {
        return Err(Error::BuildToolchain("Failed to write rustc setup input".to_string()));
    }

    stream_rustc_output(&mut setup_command);

    let status = setup_command.wait()?;
    if !status.success() {
        return Err(Error::BuildToolchain("Failed to setup rustc".to_string()));
    }

    let config_path = rustc_path.join("config.toml");
    let mut config_toml = stdio::read_file(&config_path)?;
    let mut rustc_command = ChildBuilder::new("rustc")
        .args(args![
            "-vV",
        ])
        .piped().spawn()?;

    let mut host_triple = None;
    match rustc_command.take_stdout() {
        Some(stdout) => {
            for line in stdout.lines().flatten() {
                if let Some(line) = line.strip_prefix("host: ") {
                    host_triple = Some(line.trim().to_string());
                    break;
                }
            }
        }
        _ => return Err(Error::BuildToolchain("Failed to get rustc host triple".to_string())),
    }

    let host_triple = host_triple.ok_or(Error::BuildToolchain("Failed to get rustc host triple".to_string()))?;
    config_toml.push_str(&format!(r#"
[build]
build-stage = 1
host = ["{0}"]
target = ["{0}", "aarch64-unknown-hermit", "aarch64-nintendo-switch-freestanding"]
"#, host_triple));
    stdio::write_file(&config_path, config_toml)?;

    infoln!("Configured", "rustc");

    Ok(())
}

fn build_rustc(toolchain_path: &Path) -> Result<(), Error> {
    infoln!("Building", "rustc");
    let rustc_path = toolchain_path.join("rustc");
    let mut build_command = ChildBuilder::new("./x")
        .current_dir(&rustc_path)
        .args(args![
            "build",
            "--stage",
            "1",
            "library",
        ]).spawn()?;

    stream_rustc_output(&mut build_command);

    let status = build_command.wait()?;
    if !status.success() {
        return Err(Error::BuildToolchain("Failed to build rustc".to_string()));
    }

    let link_command = ChildBuilder::new("rustup")
        .current_dir(&rustc_path)
        .args(args![
            "toolchain",
            "link",
            "megaton",
            "build/host/stage1",
        ]).spawn()?.wait()?;
    if !link_command.success() {
        return Err(Error::BuildToolchain("Failed to link rustc build artifacts".to_string()));
    }
    infoln!("Linked", "rustc build artifacts");
    Ok(())
}

fn stream_rustc_output(command: &mut ChildProcess) {
    for msg in command.take_output() {
        let msg: &str = msg.as_ref();
        let mut parts = msg.trim_start().splitn(2, ' ');
        if let Some(status) = parts.next() {
            if let Some(message) = parts.next() {
                if status.eq_ignore_ascii_case("downloading") {
                    infoln!("Downloading", "{}", message);
                } else if status.eq_ignore_ascii_case("extracting") {
                    infoln!("Extracting", "{}", message);
                } else if status.eq_ignore_ascii_case("building") {
                    infoln!("Building", "{}", message);
                } else if status.eq_ignore_ascii_case("compiling") {
                    infoln!("Compiling", "{}", message);
                } else if status.eq_ignore_ascii_case("finished") {
                    infoln!("Finished", "{}", message);
                }
            }
        }
    }
}
