use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::SystemTime,
};

use clap::{Parser, Subcommand};

pub mod config;
pub use config::MegatonConfig;
pub mod check;
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
        which::which("make").map_err(|_| {
            Error::MissingTool(
                "make".to_string(),
                "Please ensure it is installed in the system.".to_string(),
            )
        })?;

        let env_dev_kit_pro = std::env::var("DEVKITPRO").unwrap_or_default();
        if env_dev_kit_pro.is_empty() {
            return Err(Error::MissingEnv(
                "DEVKITPRO".to_string(),
                "Please ensure devkitPro is installed in the system.".to_string(),
            ));
        }
        let npdmtool = Path::new(&env_dev_kit_pro).join("tools/bin/npdmtool");
        if which::which(&npdmtool).is_err() {
            return Err(Error::MissingTool(
                "npdmtool".to_string(),
                "Please ensure devkitPro is installed in the system.".to_string(),
            ));
        }
        let objdump = Path::new(&env_dev_kit_pro).join("devkitA64/bin/aarch64-none-elf-objdump");
        if which::which(&objdump).is_err() {
            return Err(Error::MissingTool(
                "aarch64-none-elf-objdump".to_string(),
                "Please ensure devkitPro is installed in the system.".to_string(),
            ));
        }

        let mut dkp_bin_path = Path::new(&env_dev_kit_pro).join("devkitA64/bin").display().to_string();
        if !dkp_bin_path.ends_with('/') {
            dkp_bin_path.push('/');
        }

        let root_dir = Path::new(&self.dir);
        let megaton_toml_path = root_dir.join("Megaton.toml");
        infoln!("Loading", "{}", megaton_toml_path.display());
        let config = MegatonConfig::from_path(&megaton_toml_path)?;
        let flavor = if self.options.release {
            "release"
        } else {
            "debug"
        };
        let profile = &self.options.profile;

        infoln!(
            "Building",
            "{} ({flavor}, profile `{profile}`)",
            config.module.name
        );
        let target_dir = root_dir.join("target/megaton").join(flavor).join(profile);
        let makefile = config.create_makefile(&self)?;
        let make_dir = target_dir.join("make");
        let build_dir = make_dir.join("build");
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
            if !make_dir.exists() {
                std::fs::create_dir_all(&make_dir)
                    .map_err(|e| Error::AccessDirectory(make_dir.display().to_string(), e))?;
                infoln!("Created", "`{}`", make_dir.display());
            }
            std::fs::write(&makefile_path, makefile)
                .map_err(|e| Error::AccessDirectory(makefile_path.display().to_string(), e))?;
            infoln!("Saved", "`{}`", makefile_path.display());
            if build_dir.exists() {
                std::fs::remove_dir_all(&build_dir)
                    .map_err(|e| Error::AccessDirectory(build_dir.display().to_string(), e))?;
            }
        }
        if !build_dir.exists() {
            std::fs::create_dir_all(&build_dir)
                .map_err(|e| Error::AccessDirectory(build_dir.display().to_string(), e))?;
            infoln!("Created", "`{}`", build_dir.display());
        }

        // build ELF
        let elf_target = format!("{}.elf", config.module.name);
        let elf_path = build_dir.join(&elf_target);
        let elf_modified_time = get_modified_time(&elf_path);
        if elf_modified_time.is_none() && elf_path.exists() {
            std::fs::remove_file(&elf_path)
                .map_err(|e| Error::AccessFile(elf_path.display().to_string(), e))?;
        }
        make::invoke_make(
            &root_dir,
            &build_dir,
            "../build.mk",
            &elf_target,
            &dkp_bin_path,
            true,
        )?;
        let new_elf_modified_time = get_modified_time(&elf_path);
        if new_elf_modified_time.is_none() {
            return Err(Error::MakeError);
        }
        if new_elf_modified_time != elf_modified_time {
            if let Some(check_config) = &config.check {
                let check = check_config.get_profile(profile);
                check::check_symbols(root_dir, &elf_path, &objdump, &check)?;
            }
        }

        let nso_target = format!("{}.nso", config.module.name);
        make::invoke_make(
            &root_dir,
            &build_dir,
            "../build.mk",
            &nso_target,
            &dkp_bin_path,
            false,
        )?;

        let app_json_path = target_dir.join("npdm-app.json");
        let app_json = include_str!("./template.json")
            .replace("TITLE_ID_PLACEHOLDER", &config.module.title_id_hex());
        std::fs::write(&app_json_path, app_json)
            .map_err(|e| Error::AccessFile(app_json_path.display().to_string(), e))?;

        let args = vec![
            app_json_path.display().to_string(),
            target_dir.join("main.npdm").display().to_string(),
        ];
        let command = format!("{} {}", npdmtool.display().to_string(), args.join(" "));
        let mut child = Command::new(npdmtool)
            .args(&args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| Error::Subprocess(command.clone(), "cannot spawn child".to_string(), e))?;
        let status = child.wait().map_err(|e| {
            Error::Subprocess(command.clone(), "cannot wait for child".to_string(), e)
        })?;
        if !status.success() {
            return Err(Error::NpdmError(status));
        }
        infoln!("Created", "main.npdm");

        Ok(())
    }

    /// Invoke the clean command
    pub fn clean(&self) -> Result<(), Error> {
        let target_dir = self.target_dir();
        if target_dir.exists() {
            if std::fs::remove_dir_all(&target_dir).is_err() {
                hintln!(
                    "Warning",
                    "Failed to remove `{}`. Please remove it manually.",
                    target_dir.display()
                );
            }
            infoln!("Cleaned", "`{}`", target_dir.display());
        }

        Ok(())
    }

    pub fn target_dir(&self) -> PathBuf {
        Path::new(&self.dir).join("target/megaton")
    }
}

fn get_modified_time(path: &Path) -> Option<SystemTime> {
    if !path.exists() {
        return None;
    }
    path.metadata().and_then(|m| m.modified()).ok()
}
