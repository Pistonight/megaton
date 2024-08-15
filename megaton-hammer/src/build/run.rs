//! The megaton build command

use std::cell::LazyCell;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::Instant;

use serde_json::{json, Value};
use walkdir::WalkDir;

use crate::build::{
    are_deps_up_to_date, load_compile_commands, CompileCommand, Compiler, Config, Paths, SourceResult,
};
use crate::system::{self, ChildBuilder, Error, PathExt};
use crate::Options;

use super::Module;

/// Run megaton build
pub fn run(dir: &str, options: &Options) -> Result<(), Error> {
    let start_time = Instant::now();

    let root = system::find_root(dir)?;
    let megaton_toml = root.join("Megaton.toml");
    let config = Config::from_path(&megaton_toml)?;
    let profile = match (options.profile.as_str(), &config.module.default_profile) {
        ("none", Some(p)) if p == "" => {
            // default-profile = "" means to disallow no profile
            return Err(Error::NoProfile);
        }
        ("none", Some(p)) => p,
        ("none", None) => "none",
        (profile, _) => profile,
    };

    let paths = Paths::new(root, profile)?;

    system::infoln!("Building", "{} (profile `{profile}`)", config.module.name);
    system::ensure_directory(&paths.target_o)?;
    let megaton_toml_mtime = system::get_modified_time(&megaton_toml)?;
    let npdm_json = paths.target.join("main.npdm.json");
    let megaton_toml_changed = !system::is_up_to_date(&npdm_json, megaton_toml_mtime)?.is_yes();
    if megaton_toml_changed {
        system::infoln!("Creating", "main.npdm");
        create_npdm(&paths, &config.module, &npdm_json, &paths.target.join("main.npdm"))?;
        system::set_modified_time(npdm_json, megaton_toml_mtime)?;
        system::verboseln!("Created", "main.npdm");
    }

    let build = config.build.get_profile(profile);
    let entry = build.entry.as_ref().ok_or(Error::NoEntryPoint)?;

    let cc_possibly_changed = megaton_toml_changed;
    let mut compile_commands = HashMap::new();
    let mut new_compile_commands = Vec::new();
    if cc_possibly_changed {
        load_compile_commands(&paths.cc_json, &mut compile_commands);
    }
    let compiler = Compiler::new(&paths, &entry, &build)?;
    let mut objects_changed = false;
    let mut objects = Vec::new();
    let mut compile_errors = Vec::new();

    for source_dir in &build.sources {
        let source_dir = paths.root.join(source_dir).canonicalize2()?;
        for entry in WalkDir::new(source_dir).into_iter().flatten() {
            let source_path = entry.path();
            let cc = compiler.process_source(
                source_path,
                cc_possibly_changed,
                &mut compile_commands,
            )?;
            let cc = match cc {
                SourceResult::NotSource => continue,
                SourceResult::UpToDate(o_file) => {
                    system::verboseln!("Skipped", "{}", source_path.from_base(&paths.root)?.display());
                    objects.push(o_file);
                    continue;
                }
                SourceResult::NeedCompile(cc) => cc
            };
            objects_changed = true;
            system::infoln!("Compiling", "{}", source_path.from_base(&paths.root)?.display());
            system::verboseln!("Running", "{}", cc.arguments.join(" "));
            let result = cc.invoke()?;
            if !result.success {
                compile_errors.extend(result.error);
            }
            system::verboseln!("Compiled", "{}", source_path.from_base(&paths.root)?.display());
            new_compile_commands.push(cc);
        }
    }

    if megaton_toml_changed {
        system::infoln!("Creating", "verfile");
        create_verfile(&paths, &entry)?;
        system::verboseln!("Created", "verfile");
    }

    // if compiled, save cc_json
    if objects_changed {
        system::verboseln!("Saving", "compile_commands.json");
        let file = BufWriter::new(system::create(&paths.cc_json)?);
        serde_json::to_writer_pretty(file, &new_compile_commands).map_err(|e| Error::ParseJson(paths.cc_json.display().to_string(), e))?;
        system::verboseln!("Saved", "compile_commands.json");
    }

    let elf_name = format!("{}.elf", config.module.name);
    let elf_path = paths.target.join(&elf_name);
    // compile_commands not empty means sources were removed
    // link flags can change if megaton toml changed
    // TODO: LD scripts can change
    // TODO: libs can change
    let needs_linking = objects_changed || !compile_commands.is_empty() || megaton_toml_changed || !elf_path.exists();

    if !compile_errors.is_empty() {
        for line in compile_errors {
            system::errorln!("Error", "{}", line);
        }
        return Err(Error::CompileError);
    }

    if needs_linking {
        system::infoln!("Linking", "{}", elf_name);
        let result = compiler.link(&objects, &elf_path)?;
        if !result.success {
            for line in result.error {
                system::errorln!("Error", "{}", line);
            }
            return Err(Error::LinkError);
        }
        system::verboseln!("Linked", "{}", elf_name);
    }

    // if mt changed
    // or if nso doesn't exist
    // or if elf was relinked
    // or if syms changed
    // need nso

    let elapsed = start_time.elapsed();
    system::infoln!("Finished", 
        "{} (profile `{profile}`) in {:.2}s",
        config.module.name,
        elapsed.as_secs_f32()
    );

    Ok(())
}

fn create_npdm(paths: &Paths, module: &Module, npdm_json: &Path, main_npdm: &Path) -> Result<(), Error> {
    let mut npdm_data: Value = serde_json::from_str(include_str!("../../template/main.npdm.json")).unwrap();
    npdm_data["title_id"] = json!(format!("0x{}", module.title_id_hex()));
    let npdm_data = serde_json::to_string_pretty(&npdm_data).expect("fail to serialize npdm data");
    system::write_file(npdm_json, &npdm_data)?;
    let npdm_status = ChildBuilder::new(&paths.npdmtool)
        .args(system::args![&npdm_json, &main_npdm])
        .silent()
        .spawn()?
        .wait()?;
    if !npdm_status.success() {
        return Err(Error::NpdmError(npdm_status));
    }
    Ok(())
}

fn create_verfile(paths: &Paths, entry: &str) -> Result<(), Error> {
    let verfile_data = format!("{}{}{}", include_str!("../../template/verfile.before"),entry,include_str!("../../template/verfile.after"));
    system::write_file(&paths.verfile, &verfile_data)?;
    Ok(())
}

