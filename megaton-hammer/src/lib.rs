use std::path::{Path, PathBuf};
use std::time::Instant;

use clap::{Parser, Subcommand};
use serde_json::{json, Value};

pub mod build;
// pub mod make;

// pub mod init;
// pub mod toolchain;

pub mod system;

// use crate::stdio::{args, check_env, check_tool, root_rel, ChildBuilder, PathExt};
use crate::system::Error;

/// CLI entry point
#[derive(Debug, Clone, Default, PartialEq, Parser)]
#[command(author, version, about)]
pub struct MegatonHammer {
    /// Set the project root (where Megaton.toml is)
    ///
    /// Defaults to the current working directory
    #[clap(short('C'), long, default_value = ".")]
    pub dir: String,

    // /// Subcommand
    // #[clap(subcommand)]
    // pub command: Option<MegatonCommand>,

    /// Build options
    #[clap(flatten)]
    pub options: Options,
}

#[derive(Debug, Clone, PartialEq, Subcommand)]
pub enum MegatonCommand {
    // /// Init a project - generate Megaton.toml, .clangd, etc
    // Init,
    // /// Build the toolchain
    // Toolchain,
}

// impl MegatonCommand {
//     pub fn run(&self, args: &MegatonHammer) -> Result<(), Error> {
//         match self {
//             Self::Toolchain => toolchain::build(),
//             Self::Init => init::init(&args.dir),
//         }
//     }
// }

#[derive(Debug, Clone, Default, PartialEq, Parser)]
pub struct Options {
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

    /// Print verbose output from commands
    #[clap(short, long)]
    pub verbose: bool,
}

// impl Paths {
//     pub fn prepare_profile(mut self, profile: &str) -> Result<Self, Error> {
//         let target = self.root.join("target/megaton").join(profile);
//         if !target.exists() {
//             stdio::ensure_directory(&target)?;
//             infoln!("Created", "{}", target.display());
//         }
//         let target = target.canonicalize2()?;
//
//         self.target = target;
//
//         Ok(self)
//     }
//
//     pub fn pre_makefile(mut self) -> Result<Self, Error> {
//         let make = self.target.join("make");
//         if !make.exists() {
//             stdio::ensure_directory(&make)?;
//             infoln!("Created", "{}", make.display());
//         }
//         let make = make.canonicalize2()?;
//         let sources = self.target.join("sources");
//         if !sources.exists() {
//             stdio::ensure_directory(&sources)?;
//             infoln!("Created", "{}", sources.display());
//         }
//         let sources = sources.canonicalize2()?;
//
//         self.make = make;
//         self.makefile = self.target.join("makefile");
//         self.sources = sources;
//         Ok(self)
//     }
//
//     pub fn pre_make(mut self, module_name: &str) -> Result<Self, Error> {
//         self.makefile = self.makefile.canonicalize2()?;
//         self.elf = self.make.join(format!("{module_name}.elf"));
//         self.nso = self.make.join(format!("{module_name}.nso"));
//         self.cc_json = self.make.join("compile_commands.json");
//
//         Ok(self)
//     }
//
//     /// Get the path as relative from root
//     pub fn from_root<P>(&self, path: P) -> Result<PathBuf, Error>
//     where
//         P: AsRef<Path>,
//     {
//         path.from_base(&self.root)
//     }
// }

impl MegatonHammer {
    /// Build the project
    pub fn build(&self) -> Result<(), Error> {
        build::run(&self.dir, &self.options)
//         let start_time = Instant::now();
//         // === pre-conditions ===
//         // Check if projects, envs, tools are setup correctly
//         check_os()?;
//         // check_tool!("git")?;
//         // check_tool!("make")?;
//         // check_tool!("cargo", "Rust")?;
//         // check_tool!("rustc", "Rust")?;
//
//         let paths = Paths::new(&self.dir)?;
//
//         infoln!("Loading", "{}", root_rel!(paths.megaton_toml)?.display());
//         let config = MegatonConfig::from_path(&paths.megaton_toml)?;
//         let mut profile = &self.options.profile;
//         if profile == "none" {
//             if config.module.no_default_profile {
//                 return Err(Error::NoProfile);
//             }
//             if let Some(default_profile) = &config.module.default_profile {
//                 profile = default_profile;
//             }
//         };
//         let paths = paths.prepare_profile(profile)?;
//
//         // === pre-build ===
//         // Build meta files and detect if clean is needed
//         infoln!("Building", "{} (profile `{profile}`)", config.module.name);
//         let megaton_toml_modified_time = stdio::get_modified_time(&paths.megaton_toml)?;
//         let npdm_app_json_path = paths.target.join("npdm-app.json");
//         let need_clean = self.options.clean
//             || match stdio::get_modified_time(&npdm_app_json_path) {
//                 Ok(time) if time < megaton_toml_modified_time => true,
//                 Err(_) => true,
//                 _ => false,
//             };
//         if need_clean {
//             hintln!("Cleaning", "{}", root_rel!(paths.target)?.display());
//             stdio::remove_directory(&paths.target)?;
//             stdio::ensure_directory(&paths.target)?;
//
//             // npdm-app.json
//             infoln!(
//                 "Loading",
//                 "{}",
//                 root_rel!(paths.npdm_template_json)?.display()
//             );
//             let npdm_data = stdio::read_file(&paths.npdm_template_json)?;
//             let mut npdm_data: Value = serde_json::from_str(&npdm_data)
//                 .map_err(|e| Error::ParseJson(paths.npdm_template_json.display().to_string(), e))?;
//
//             npdm_data["title_id"] = json!(format!("0x{}", config.module.title_id_hex()));
//             let npdm_data =
//                 serde_json::to_string_pretty(&npdm_data).expect("fail to serialize npdm data");
//             stdio::write_file(&npdm_app_json_path, npdm_data)?;
//             let npdm_app_json_path = npdm_app_json_path.canonicalize2()?;
//             infoln!(
//                 "Created",
//                 "{}",
//                 paths.from_root(&npdm_app_json_path)?.display()
//             );
//
//             let npdm_path = paths.target.join("main.npdm");
//             let npdm_status = ChildBuilder::new(&paths.npdmtool)
//                 .args(args![&npdm_app_json_path, &npdm_path])
//                 .silent()
//                 .spawn()?
//                 .wait()?;
//             if !npdm_status.success() {
//                 return Err(Error::NpdmError(npdm_status));
//             }
//             let npdm_path = npdm_path.canonicalize2()?;
//             infoln!("Created", "{}", paths.from_root(npdm_path)?.display());
//         }
//
//         // === build ===
//         // run cargo (if needed)
//         // run make
//         let paths = paths.pre_makefile()?;
//         // check sources
//         let makefile = config.create_makefile(&paths, &self)?;
//         let mut need_new_makefile = true;
//         if paths.makefile.exists() {
//             if let Ok(old_makefile) = stdio::read_file(&paths.makefile) {
//                 if old_makefile == makefile {
//                     need_new_makefile = false;
//                 }
//             }
//         }
//         if need_new_makefile {
//             if paths.make.exists() {
//                 hintln!("Cleaning", "{}", root_rel!(paths.make)?.display());
//                 stdio::remove_directory(&paths.make)?;
//                 stdio::ensure_directory(&paths.make)?;
//             }
//             stdio::write_file(&paths.makefile, makefile)?;
//             infoln!("Saved", "{}", root_rel!(paths.makefile)?.display());
//         }
//
//         let paths = paths.pre_make(&config.module.name)?;
//         let elf_modified_time = stdio::get_modified_time(&paths.elf).ok();
//         make::make_elf(&paths, self.options.verbose)?;
//         let new_elf_modified_time = stdio::get_modified_time(&paths.elf).ok();
//         if new_elf_modified_time.is_none() {
//             return Err(Error::MakeError);
//         }
//         if new_elf_modified_time != elf_modified_time {
//             // === check ===
//             if let Some(check_config) = &config.check {
//                 let check = check_config.get_profile(profile);
//                 check::check_elf(&paths, &check)?;
//             }
//         }
//
//         make::make_nso(&paths, self.options.verbose)?;
//         let elapsed = start_time.elapsed();
//         infoln!(
//             "Finished",
//             "{} (profile `{profile}`) in {:.2}s",
//             config.module.name,
//             elapsed.as_secs_f32()
//         );
//
//         Ok(())
    }
}
