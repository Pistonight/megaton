use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use serde_json::{Value, json};

pub mod config;
pub mod check;
pub mod make;
pub mod error;

pub mod print;
pub mod stdio;
pub mod toolchain;

use crate::stdio::{args, root_rel, ChildBuilder, PathExt, check_env, check_tool};
use crate::error::Error;

pub use config::MegatonConfig;

/// CLI entry point
#[derive(Debug, Clone, Default, PartialEq, Parser)]
#[command(author, version, about)]
pub struct MegatonHammer {
    /// Set the project root (where Megaton.toml is)
    ///
    /// Defaults to the current working directory
    #[clap(short('C'), long, default_value = ".")]
    pub dir: String,

    /// Subcommand
    #[clap(subcommand)]
    pub command: Option<MegatonCommand>,

    /// Build options
    #[clap(flatten)]
    pub options: BuildOptions,
}

#[derive(Debug, Clone, PartialEq, Subcommand)]
pub enum MegatonCommand {
    /// Build the toolchain
    Toolchain,
}

impl MegatonCommand {
    pub fn run(&self) -> Result<(), Error> {
        match self {
            Self::Toolchain => toolchain::build(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Parser)]
pub struct BuildOptions {
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

/// Paths used by the program. All paths are absolute
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Paths {
    /// Current working directory
    pub cwd: PathBuf,

    /// Root directory of the project (where Megaton.toml is, set by `--dir/-C`)
    pub root: PathBuf,

    /// The devkitPro installation. Read from the `DEVKITPRO` environment variable.
    pub devkitpro: PathBuf,
    
    /// The compiler toolchain. ($DEVKITPRO/devkitA64/bin)
    pub devkita64_bin: PathBuf,

    /// The npdmtool executable ($DEVKITPRO/tools/bin/npdmtool)
    pub npdmtool: PathBuf,

    /// The aarch64-none-elf-objdump executable ($DEVKITPRO/devkitA64/bin/aarch64-none-elf-objdump)
    pub objdump: PathBuf,

    /// Megaton.toml in the root directory
    pub megaton_toml: PathBuf,

    /// The target directory for megaton (<root>/target/megaton/<profile>)
    pub target: PathBuf,

    /// The home directory of megaton. Read from the MEAGTON_HOME environment variable.
    pub megaton_home: PathBuf,

    /// The toolchain directory for megaton. ($MEGATON_HOME/toolchain)
    pub toolchain: PathBuf,

    /// The template json file for generating the npdm file. ($MEGATON_HOME/toolchain/npdm-template.json)
    pub npdm_template_json: PathBuf,

    /// The make build directory
    pub make: PathBuf,

    /// The makefile
    pub makefile: PathBuf,

    /// The target ELF
    pub elf: PathBuf,

    /// The target NSO
    pub nso: PathBuf,

    /// The compile_commands.json file
    pub cc_json: PathBuf,

}

impl Paths {
    pub fn new(root: &str) -> Result<Self, Error> {
        let cwd = std::env::current_dir().map_err(|e| Error::InvalidPath(".".to_string(), e))?;
        let cwd = cwd.canonicalize2()?;
        let root = root.canonicalize2()?;

        let devkitpro = check_env!("DEVKITPRO", "Please refer to https://devkitpro.org/wiki/devkitPro_pacman#customising-existing-pacman-install to configure the environment variables.")?;
        let devkitpro = devkitpro.canonicalize2()?;
        let devkita64_bin = devkitpro.join("devkitA64/bin").canonicalize2()?;
        let npdmtool = devkitpro.join("tools/bin/npdmtool").canonicalize2()?;
        let objdump = devkita64_bin.join("aarch64-none-elf-objdump").canonicalize2()?;
        check_tool!(npdmtool, "devkitPro")?;
        check_tool!(objdump, "devkitPro")?;

        let megaton_home = check_env!("MEGATON_HOME", "Please set MEGATON_HOME to the local path to the megaton repository on your system.")?;
        let megaton_home = megaton_home.canonicalize2()?;
        let toolchain = megaton_home.join("toolchain").canonicalize2()?;
        let runtime = megaton_home.join("runtime").canonicalize2()?;
        let npdm_template_json = runtime.join("npdm-template.json").canonicalize2()?;

        let megaton_toml = root.join("Megaton.toml");
        if !megaton_toml.exists() {
            return Err(Error::NotFoundWithMessage(
                megaton_toml.display().to_string(),
                "Please create the `Megaton.toml` in the project root.".to_string()
            ));
        }
        let megaton_toml = megaton_toml.canonicalize2()?;

        Ok(Self {
            cwd,
            root,
            devkitpro,
            devkita64_bin,
            npdmtool,
            objdump,
            megaton_toml,
            megaton_home,
            toolchain,
            npdm_template_json,
            ..Default::default()
        })
    }

    pub fn prepare_profile(mut self, profile: &str) -> Result<Self, Error> {
        let target = self.root.join("target/megaton").join(profile);
        if !target.exists() {
            stdio::ensure_directory(&target)?;
            infoln!("Created", "{}", target.display());
        }
        let target = target.canonicalize2()?;

        self.target = target;

        Ok(self)
    }

    pub fn pre_makefile(mut self) -> Result<Self, Error> {
        let make = self.target.join("make");
        if !make.exists() {
            stdio::ensure_directory(&make)?;
            infoln!("Created", "{}", make.display());
        }
        let make = make.canonicalize2()?;

        self.make = make;
        self.makefile = self.target.join("makefile");
        Ok(self)
    }

    pub fn pre_make(mut self, module_name: &str) -> Result<Self, Error> {
        self.makefile = self.makefile.canonicalize2()?;
        self.elf = self.make.join(format!("{module_name}.elf"));
        self.nso = self.make.join(format!("{module_name}.nso"));
        self.cc_json = self.make.join("compile_commands.json");

        Ok(self)
    }

    /// Get the path as relative from root
    pub fn from_root<P>(&self, path: P) -> Result<PathBuf, Error>
    where
        P: AsRef<Path>,
    {
        path.from_base(&self.root)
    }
}


impl MegatonHammer {
    /// Build the project
    pub fn build(&self) -> Result<(), Error> {
        // === pre-conditions ===
        // Check if projects, envs, tools are setup correctly
        check_os()?;
        // check_tool!("git")?;
        check_tool!("make")?;
        // check_tool!("cargo", "Rust")?;
        // check_tool!("rustc", "Rust")?;

        let paths = Paths::new(&self.dir)?;


        infoln!("Loading", "{}", root_rel!(paths.megaton_toml)?.display());
        let config = MegatonConfig::from_path(&paths.megaton_toml)?;
        let mut profile = &self.options.profile;
        if profile == "none" {
            if config.module.no_default_profile {
                return Err(Error::NoProfile);
            }
            if let Some(default_profile) = &config.module.default_profile {
                profile = default_profile;
            }
        };
        let paths = paths.prepare_profile(profile)?;

        // === pre-build ===
        // Build meta files and detect if clean is needed
        infoln!(
            "Building",
            "{} (profile `{profile}`)",
            config.module.name
        );
        let megaton_toml_modified_time = stdio::get_modified_time(&paths.megaton_toml)?;
        let npdm_app_json_path = paths.target.join("npdm-app.json");
        let need_clean = match stdio::get_modified_time(&npdm_app_json_path) {
            Ok(time) if time < megaton_toml_modified_time => true,
            Err(_) => true,
            _ => false,
        };
        if need_clean {
            hintln!("Cleaning", "{}", root_rel!(paths.target)?.display());
            stdio::remove_directory(&paths.target)?;
            stdio::ensure_directory(&paths.target)?;

            // npdm-app.json
            infoln!("Loading", "{}", root_rel!(paths.npdm_template_json)?.display());
            let npdm_data = stdio::read_file(&paths.npdm_template_json)?;
            let mut npdm_data: Value = serde_json::from_str(&npdm_data)
                .map_err(|e| Error::ParseJson(paths.npdm_template_json.display().to_string(), e))?;

            npdm_data["title_id"] = json!(format!("0x{}", config.module.title_id_hex()));
            let npdm_data = serde_json::to_string_pretty(&npdm_data).expect("fail to serialize npdm data");
            stdio::write_file(&npdm_app_json_path, npdm_data)?;
            let npdm_app_json_path = npdm_app_json_path.canonicalize2()?;
            infoln!("Created", "{}", paths.from_root(&npdm_app_json_path)?.display());

            let npdm_path = paths.target.join("main.npdm");
            let npdm_status = ChildBuilder::new(&paths.npdmtool)
                .args(args![
                    &npdm_app_json_path,
                    &npdm_path
                ]).silent().spawn()?.wait()?;
            if !npdm_status.success() {
                return Err(Error::NpdmError(npdm_status));
            }
            let npdm_path = npdm_path.canonicalize2()?;
            infoln!("Created", "{}", paths.from_root(npdm_path)?.display());

        }

        // === build ===
        // run cargo (if needed)
        // run make
        let paths = paths.pre_makefile()?;
        let makefile = config.create_makefile(&paths, &self)?;
        let mut need_new_makefile = true;
        if paths.makefile.exists() {
            if let Ok(old_makefile) = stdio::read_file(&paths.makefile) {
                if old_makefile == makefile {
                    need_new_makefile = false;
                }
            }
        }
        if need_new_makefile {
            if paths.make.exists() {
                hintln!("Cleaning", "{}", root_rel!(paths.make)?.display());
                stdio::remove_directory(&paths.make)?;
                stdio::ensure_directory(&paths.make)?;
            }
            stdio::write_file(&paths.makefile, makefile)?;
            infoln!("Saved", "{}", root_rel!(paths.makefile)?.display());
        }

        let paths = paths.pre_make(&config.module.name)?;
        let elf_modified_time = stdio::get_modified_time(&paths.elf).ok();
        infoln!("Making", "{}", root_rel!(paths.elf)?.display());
        make::make_elf(&paths)?;
        infoln!("Made", "{}", root_rel!(paths.elf)?.display());
        let new_elf_modified_time = stdio::get_modified_time(&paths.elf).ok();
        if new_elf_modified_time.is_none() {
            return Err(Error::MakeError);
        }
        if new_elf_modified_time != elf_modified_time {
            // === check ===
            if let Some(check_config) = &config.check {
                let check = check_config.get_profile(profile);
                check::check_elf(&paths, &check)?;
            }
        }

        infoln!("Making", "{}", root_rel!(paths.nso)?.display());
        make::make_nso(&paths)?;
        infoln!(
            "Finished",
            "{} (profile `{profile}`)",
            config.module.name
        );


        Ok(())
    }

}

pub fn check_os() -> Result<(), Error> {
    #[cfg(windows)]
    {
        return Err(Error::Windows);
    }
    Ok(())
}
