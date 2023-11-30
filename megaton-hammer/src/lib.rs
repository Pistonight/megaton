use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};

pub mod config;
pub use config::MegatonConfig;
pub mod make;

pub mod error;
use error::Error;

pub mod print;

/// CLI entry point
#[derive(Debug, Clone, Default, PartialEq, Parser)]
#[command(author, version, about)]
pub struct MegatonHammer {

    /// The project directory.
    ///
    /// If specified, megaton will run as if invoked from this directory.
    #[clap(short('C'), long, default_value = ".")]
    pub dir: String,

    /// The subcommand
    #[clap(subcommand)]
    pub command: Option<MegatonCommand>,

    /// Build options
    #[clap(flatten)]
    pub options: BuildOptions,
}

#[derive(Debug, Clone, PartialEq, Subcommand)]
pub enum MegatonCommand {
    /// Remove the outputs
    Clean,
}

#[derive(Debug, Clone, Default, PartialEq, Parser)]
pub struct BuildOptions {
    /// Build the project in release mode.
    ///
    /// This will pass `--release` to cargo and may
    /// change some default flags passed to C compilers.
    #[clap(short, long)]
    pub release: bool,

    /// Specify the build profile to use.
    ///
    /// Different profiles for `cargo`, `make` and `check` can be defined
    /// in the Megaton.toml file under `cargo.profiles`, `make.profiles` and
    /// `check.profiles` respectively.
    #[clap(short, long, default_value = "none")]
    pub profile: String,

    /// Suppress output
    #[clap(short, long)]
    pub quiet: bool,
}

impl MegatonHammer {
    /// Invoke `self.command`
    pub fn invoke(&self) -> Result<(), Error> {
        match &self.command {
            Some(MegatonCommand::Clean) => self.clean(),
            None => self.build(),
        }
    }
    /// Invoke the build command
    pub fn build(&self) -> Result<(), Error> {
        #[cfg(target_os = "windows")]
        {
            warnln!("Warning", "You are using Windows. There is a high chance the tool does not work. Please consider using WSL or a Linux environment to save yourself from troubles.");
        }
        ensure_sys_util("make")?;
        ensure_sys_util("objdump")?;
        if which::which("npdmtool").is_err() {
            return Err(Error::MissingTool("npdmtool".to_string(), "Please ensure devkitPro is installed in the system.".to_string()));
        }

        if std::env::var("DEVKITPRO").unwrap_or_default().is_empty() {
            return Err(Error::MissingEnv("DEVKITPRO".to_string(), "Please ensure devkitPro is installed in the system.".to_string()));
        }

        let root_dir = Path::new(&self.dir);
        let megaton_toml_path = root_dir.join("Megaton.toml");
        infoln!("Loading", "{}", megaton_toml_path.display());
        let config = MegatonConfig::from_path(&megaton_toml_path)?;
        let flavor = if self.options.release { "release" } else { "debug" };
        let profile = &self.options.profile;

        infoln!("Building", "{} ({flavor}, profile `{profile}`)", config.module.name);
        let target_dir = root_dir.join("target/megaton").join(flavor).join(profile);
        let makefile = config.create_makefile(&self)?;
        let make_dir = target_dir.join("make");
        let build_dir = make_dir.join("build");
        if !build_dir.exists() {
            std::fs::create_dir_all(&build_dir)
                .map_err(|e| Error::CreateDirectory(build_dir.display().to_string(), e))?;
            infoln!("Created", "`{}`", build_dir.display());
        }
        let makefile_path = make_dir.join("build.mk");
        let mut need_new_makefile = true;
        if makefile_path.exists() {
            if let Ok(old_makefile) = std::fs::read_to_string(&makefile_path) {
                if old_makefile == makefile {
                    need_new_makefile = false;
                }
            }
        }
        if need_new_makefile {
            std::fs::write(&makefile_path, makefile)
                .map_err(|e| Error::CreateDirectory(makefile_path.display().to_string(), e))?;
            infoln!("Created", "`{}`", makefile_path.display());
        }

        Ok(())
    }

    /// Invoke the clean command
    pub fn clean(&self) -> Result<(), Error> {
        let target_dir = self.target_dir();
        if target_dir.exists() {
            if std::fs::remove_dir_all(&target_dir).is_err() {
                hintln!("Warning", "Failed to remove `{}`. Please remove it manually.", target_dir.display());
            }
            infoln!("Cleaned", "`{}`", target_dir.display());
        }

        Ok(())
    }

    pub fn target_dir(&self) -> PathBuf {
        Path::new(&self.dir).join("target/megaton")
    }
}


fn ensure_sys_util(bin: &str) -> Result<(), Error> {
    which::which(bin).map_err(|_| Error::MissingTool(bin.to_string(), "Please ensure it is installed in the system.".to_string()))?;
    Ok(())
}
