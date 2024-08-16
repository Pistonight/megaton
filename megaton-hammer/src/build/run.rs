//! The megaton build command

use std::collections::HashMap;
use std::io::BufWriter;
use std::path::Path;
use std::rc::Rc;
use std::time::Instant;

use serde_json::{json, Value};
use walkdir::WalkDir;

use crate::build::{
    load_compile_commands, BuildPhase, CheckPhase, Config, Paths, SourceResult
};
use crate::build::config::Module;
use crate::system::{self, ChildBuilder, Error, PathExt};
use crate::Options;

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

    let paths = Paths::new(root, profile, &config.module.name)?;
    let paths = Rc::new(paths);

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
    let compiler = BuildPhase::new(&paths, &entry, &build)?;
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
            objects.push(cc.output.clone());
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

    let check_phase = config.check.as_ref().map(|x| {
        let paths = Rc::clone(&paths);
        let check = x.get_profile(profile);
        CheckPhase::new(paths, Rc::new(check)).load_symbols()
    });

    if !compile_errors.is_empty() {
        for line in compile_errors {
            system::errorln!("Error", "{}", line);
        }
        return Err(Error::CompileError);
    }
    let elf_name = format!("{}.elf", config.module.name);

    // compile_commands not empty means sources were removed
    // link flags can change if megaton toml changed
    let mut needs_linking = objects_changed || !compile_commands.is_empty() || megaton_toml_changed || !paths.elf.exists();
    // objects can be newer than elf even if not changed
    if !needs_linking {
        let elf_mtime = system::get_modified_time(&paths.elf)?;
        for object in &objects {
            let mtime = match system::get_modified_time(object) {
                Ok(mtime) => mtime,
                Err(_) => {
                    needs_linking = true;
                    break;
                }
            };
            if mtime > elf_mtime {
                needs_linking = true;
                break;
            }
        }
    }
    // LD scripts can change
    if !needs_linking {
        let elf_mtime = system::get_modified_time(&paths.elf)?;
        for ldscript in &build.ldscripts {
            let ldscript = paths.root.join(ldscript);
            let mtime = match system::get_modified_time(&ldscript) {
                Ok(mtime) => mtime,
                Err(_) => {
                    needs_linking = true;
                    break;
                }
            };
            if mtime > elf_mtime {
                needs_linking = true;
                break;
            }
        }
        
    }
    // TODO: libs can change

    if needs_linking {
        system::infoln!("Linking", "{}", elf_name);
        let result = compiler.link(&objects, &paths.elf)?;
        if !result.success {
            for line in result.error {
                system::errorln!("Error", "{}", line);
            }
            return Err(Error::LinkError);
        }
        system::verboseln!("Linked", "{}", elf_name);
    }

    let mut needs_nso = needs_linking || !paths.nso.exists();
    // elf can be newer if check failed
    if !needs_nso {
        let elf_mtime = system::get_modified_time(&paths.elf)?;
        let nso_mtime = system::get_modified_time(&paths.nso)?;
        if elf_mtime > nso_mtime {
            needs_nso = true;
        }
    }
    // symbol files can change
    if !needs_nso {
        if let Some(check_phase) = check_phase.as_ref() {
            match check_phase.as_ref() {
                Ok(check_phase) => {
                    let nso_mtime = system::get_modified_time(&paths.nso)?;
                    for symbol in &check_phase.config.symbols {
                        let symbol = paths.root.join(symbol);
                        let mtime = match system::get_modified_time(&symbol) {
                            Ok(mtime) => mtime,
                            Err(_) => {
                                needs_nso = true;
                                break;
                            }
                        };
                        if mtime > nso_mtime {
                            needs_nso = true;
                            break;
                        }
                    }
                },
                Err(_) => {
                    needs_nso = true;
                }
            }
        }
    }
    if needs_nso {
        let nso_name = format!("{}.nso", config.module.name);
        if let Some(check_phase) = check_phase {
            system::infoln!("Checking", "{}", elf_name);
            let check_phase = check_phase?;
            let missing_symbols = check_phase.check_symbols()?;
            let mut check_ok = true;
            if !missing_symbols.is_empty() {
                system::errorln!("Error", "There are unresolved symbols:");
                system::errorln!("Error", "");
                for symbol in missing_symbols.iter().take(10) {
                    system::errorln!("Error", "  {}", symbol);
                }
                if missing_symbols.len() > 10 {
                    system::errorln!("Error", "  ... ({} more)", missing_symbols.len() - 10);
                }
                system::errorln!("Error", "");
                system::errorln!(
                    "Error",
                    "Found {} unresolved symbols!",
                    missing_symbols.len()
                );
                let missing_symbols = missing_symbols.join("\n");
                let missing_symbols_path = paths.target.join("missing_symbols.txt");
                system::write_file(&missing_symbols_path, &missing_symbols)?;
                system::hintln!(
                    "Hint",
                    "Include the symbols in the linker scripts, or add them to the `ignore` section."
                );
                system::hintln!(
                    "Saved",
                    "All missing symbols to `{}`",
                    paths.from_root(missing_symbols_path)?.display()
                );
                check_ok = false;
            }
            let bad_instructions = check_phase.check_instructions()?;
            if !bad_instructions.is_empty() {
                system::errorln!("Error", "There are unsupported/disallowed instructions:");
                system::errorln!("Error", "");
                for inst in bad_instructions.iter().take(10) {
                    system::errorln!("Error", "  {}", inst);
                }
                if bad_instructions.len() > 10 {
                    system::errorln!(
                        "Error",
                        "  ... ({} more)",
                        bad_instructions.len() - 10
                    );
                }
                system::errorln!("Error", "");
                system::errorln!(
                    "Error",
                    "Found {} disallowed instructions!",
                    bad_instructions.len()
                );

                let output = bad_instructions
                    .join("\n");
                let output_path = paths.target.join("disallowed_instructions.txt");
                system::write_file(
                    &output_path,
                    &output,
                )?;
                system::hintln!(
                    "Saved",
                    "All disallowed instructions to {}",
                    paths.from_root(output_path)?.display()
                );
                check_ok = false;
            }
            if !check_ok {
                return Err(Error::CheckError);
            }
            system::infoln!("Checked", "{} looks good to me", elf_name);
            system::infoln!("Creating", "{}", nso_name);

            let status = ChildBuilder::new(&paths.elf2nso)
                .args([&paths.elf, &paths.nso])
                .silent()
                .spawn()?.wait()?;
            if !status.success() {
                return Err(Error::Elf2NsoError);
            }
        }
    }

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

