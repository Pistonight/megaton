//! Build-related paths

use std::path::{Path, PathBuf};

use crate::system::{self, Error, PathExt};

/// Paths used by the program. All paths are absolute
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Paths {
    /// Root directory of the project (where Megaton.toml is, set by `--dir/-C`)
    pub root: PathBuf,

    /// The gcc compiler binary
    pub make_c: PathBuf,

    /// The g++ compiler binary
    pub make_cpp: PathBuf,

    /// The npdmtool executable ($DEVKITPRO/tools/bin/npdmtool)
    pub npdmtool: PathBuf,

    /// The aarch64-none-elf-objdump executable ($DEVKITPRO/devkitA64/bin/aarch64-none-elf-objdump)
    pub objdump: PathBuf,

    /// The elf2nso executable ($DEVKITPRO/tools/bin/elf2nso)
    pub elf2nso: PathBuf,

    /// The target directory for megaton (<root>/target/megaton/<profile>)
    pub target: PathBuf,

    /// The object file output directory (<root>/target/megaton/<profile>/o)
    pub target_o: PathBuf,

    /// The version script file for linker
    pub verfile: PathBuf,

    /// The compile_commands.json file
    pub cc_json: PathBuf,

    // /// The home directory of megaton. Read from the MEAGTON_HOME environment variable.
    // pub megaton_home: PathBuf,

    // /// The toolchain directory for megaton. ($MEGATON_HOME/toolchain)
    // pub toolchain: PathBuf,

    // /// The template json file for generating the npdm file. ($MEGATON_HOME/toolchain/npdm-template.json)
    // pub npdm_template_json: PathBuf,
    /// The target ELF (target/megaton/<profile>/<name>.elf)
    pub elf: PathBuf,

    /// The target NSO
    pub nso: PathBuf,
}

macro_rules! check_dkp_tool {
    ($dkp:ident, $tool:literal, $path:literal) => {{
        match which::which($tool).ok() {
            Some(x) => x,
            None => {
                let mut p = match $dkp.as_ref() {
                    None => {
                        $dkp = Some(get_devkitpro_path()?);
                        $dkp.as_ref().cloned().unwrap()
                    }
                    Some(x) => x.clone(),
                }
                .join($path);
                p.push($tool);
                system::check_tool!(p, "devkitPro")?
            }
        }
    }};
}

impl Paths {
    pub fn new(root: PathBuf, profile: &str, module_name: &str) -> Result<Self, Error> {
        let mut devkitpro = None;
        let make_c = check_dkp_tool!(devkitpro, "aarch64-none-elf-gcc", "devkitA64/bin");
        let make_cpp = check_dkp_tool!(devkitpro, "aarch64-none-elf-g++", "devkitA64/bin");
        let objdump = check_dkp_tool!(devkitpro, "aarch64-none-elf-objdump", "devkitA64/bin");
        let elf2nso = check_dkp_tool!(devkitpro, "elf2nso", "tools/bin");
        let npdmtool = check_dkp_tool!(devkitpro, "npdmtool", "tools/bin");

        // let megaton_home = check_env!(
        //     "MEGATON_HOME",
        //     "Please set MEGATON_HOME to the local path to the megaton repository on your system."
        // )?;
        // let megaton_home = megaton_home.canonicalize2()?;
        // let toolchain = megaton_home.join("toolchain").canonicalize2()?;
        // let runtime = megaton_home.join("runtime").canonicalize2()?;
        // let npdm_template_json = runtime.join("npdm-template.json").canonicalize2()?;

        let mut target = root.join("target/megaton");
        target.push(profile);
        let target_o = target.join("o");
        let verfile = target.join("verfile");
        let cc_json = target.join("compile_commands.json");
        let elf = target.join(format!("{}.elf", module_name));
        let nso = target.join(format!("{}.nso", module_name));

        Ok(Self {
            root,
            make_c,
            make_cpp,
            npdmtool,
            objdump,
            elf2nso,
            target,
            target_o,
            verfile,
            cc_json,
            elf,
            nso,
        })
    }

    /// Get the path as relative from root
    pub fn from_root<P>(&self, path: P) -> Result<PathBuf, Error>
    where
        P: AsRef<Path>,
    {
        path.with_base(&self.root)
    }
}

fn get_devkitpro_path() -> Result<PathBuf, Error> {
    system::check_env!("DEVKITPRO", "Please refer to https://devkitpro.org/wiki/devkitPro_pacman#customising-existing-pacman-install to configure the environment variables.")?.canonicalize2()
}
