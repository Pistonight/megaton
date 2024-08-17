//! The megaton build command

use std::collections::HashMap;
use std::io::{BufRead, BufWriter};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use filetime::FileTime;
use serde_json::{json, Value};
use walkdir::WalkDir;

use crate::build::{
    load_checker, load_compile_commands, Builder, BuildResult, Config, Paths, SourceResult
};
use crate::system::{self, ChildBuilder, Error, Executer, PathExt};
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

    let executer = Arc::new(Executer::new());

    let mut main_npdm_task = None;

    system::infoln!("Building", "{} (profile `{profile}`)", config.module.name);
    system::ensure_directory(&paths.target_o)?;
    let megaton_toml_mtime = system::get_modified_time(&megaton_toml)?;
    let npdm_json = paths.target.join("main.npdm.json");
    let megaton_toml_changed = !system::is_up_to_date(&npdm_json, megaton_toml_mtime)?.is_yes();
    if megaton_toml_changed {
        let target = paths.target.clone();
        let npdmtool = paths.npdmtool.clone();
        let title_id = config.module.title_id_hex();
        let task = executer.execute(move || {
            system::infoln!("Creating", "main.npdm");
            create_npdm(target, npdmtool, title_id, megaton_toml_mtime)?;
            system::verboseln!("Created", "main.npdm");
            Ok::<(), Error>(())
        });

        main_npdm_task = Some(task);
    }

    let build = config.build.get_profile(profile);
    let entry = build.entry.as_ref().ok_or(Error::NoEntryPoint)?;

    let cc_possibly_changed = megaton_toml_changed;
    let mut compile_commands = HashMap::new();
    let mut new_compile_commands = Vec::new();
    if cc_possibly_changed {
        // even though this is blocking
        // this will only load when Megaton.toml changes
        load_compile_commands(&paths.cc_json, &mut compile_commands);
    }
    let builder = Builder::new(&paths, &entry, &build)?;
    // if any .o files were rebuilt
    let mut objects_changed = false;
    // all .o files
    let mut objects = Vec::new();
    let mut cc_tasks = Vec::new();

    // fire off all cc tasks
    for source_dir in &build.sources {
        let source_dir = paths.root.join(source_dir);
        for entry in WalkDir::new(source_dir).into_iter().flatten() {
            let source_path = entry.path();
            let cc = builder.process_source(
                source_path,
                cc_possibly_changed,
                &mut compile_commands,
            )?;
            let cc = match cc {
                SourceResult::NotSource => {
                    // file type not recognized, skip
                    continue;
                },
                SourceResult::UpToDate(o_file) => {
                    system::verboseln!("Skipped", "{}", source_path.from_base(&paths.root)?.display());
                    objects.push(o_file);
                    continue;
                }
                SourceResult::NeedCompile(cc) => cc
            };
            objects_changed = true;
            objects.push(cc.output.clone());
            let source_display = source_path.from_base(&paths.root)?.display().to_string();
            system::verboseln!("Compiling", "{}", source_display);
            let child = cc.start()?;
            let task = executer.execute(move || {
                let result = child.wait()?;
                if !result.success {
                    system::verboseln!("Failed", "{}", source_display);
                }
                system::infoln!("Compiled", "{}", source_display);
                Ok::<BuildResult, Error>(result)
            });
            new_compile_commands.push(cc);
            cc_tasks.push(task);
        }
    }

    let verfile_task = if megaton_toml_changed {
        let verfile = paths.verfile.clone();
        let entry = entry.clone();
        Some(executer.execute(move || {
            system::verboseln!("Creating", "verfile");
            create_verfile(verfile, entry)?;
            system::infoln!("Created", "verfile");
            Ok::<(), Error>(())
        }))
    } else {
        None
    };

    // if compiled, save cc_json
    let save_cc_json_task = if objects_changed || !compile_commands.is_empty() {
        system::verboseln!("Saving", "compile_commands.json");
        let file = BufWriter::new(system::create(&paths.cc_json)?);
        let path_display = paths.cc_json.display().to_string();
        Some(executer.execute(move || {
            serde_json::to_writer_pretty(
                file, 
                &new_compile_commands
            ).map_err(|e| Error::ParseJson(path_display, e))?;
            system::verboseln!("Saved", "compile_commands.json");
            Ok::<(), Error>(())
        }))
    } else {
        None
    };

    // compute if linking is needed

    // compile_commands not empty means sources were removed
    // link flags can change if megaton toml changed
    let mut needs_linking = objects_changed || !compile_commands.is_empty() || megaton_toml_changed || !paths.elf.exists();
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
    // objects can be newer than elf even if not changed
    // note that even if compile is in progress, this works
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
    // TODO: libs can change

    // eagerly load checker if linking is needed and check config exists
    let checker = match (needs_linking, config.check.as_ref()) {
        (true, Some(check)) => {
            let check = check.get_profile(profile);
            Some(load_checker(&paths, check, &executer)?)
        },
        _ => None
    };

    // start joining the cc tasks
    let mut compile_failed = false;
    for t in cc_tasks {
        match t.wait() {
            Err(e) => {
                system::errorln!("Error", "{}", e);
                compile_failed = true;
            },
            Ok(result) => {
                if !result.success {
                    compile_failed = true;
                }
                if let Some(error) = result.error {
                    for line in error.lines().flatten() {
                        system::errorln!("Error", "{}", line);
                    }
                }
            }
        }
    }
    if compile_failed {
        return Err(Error::CompileError);
    }
    
    // linker dependencies
    if needs_linking {
        if let Some(verfile_task) = verfile_task {
            verfile_task.wait()?;
        }
    }

    let elf_name = format!("{}.elf", config.module.name);

    let link_task = if needs_linking {
        system::infoln!("Linking", "{}", elf_name);
        let task = builder.link_start(&objects, &paths.elf)?;
        let elf_name = elf_name.clone();
        let task = executer.execute(move || {
            let result = task.wait()?;
            system::verboseln!("Linked", "{}", elf_name);
            Ok::<BuildResult, Error>(result)
        });
        Some(task)
    } else {
        None
    };

    let mut needs_nso = needs_linking || !paths.nso.exists();
    // symbol files can change
    if !needs_nso {
        if let Some(checker) = checker.as_ref() {
            let nso_mtime = system::get_modified_time(&paths.nso)?;
            needs_nso = checker.are_syms_newer_than(&paths, nso_mtime);
        }
    }
    // elf can be newer if check failed
    if !needs_nso {
        // note we don't need to wait for linker here
        // because if is linking -> needs_linking must be true
        let elf_mtime = system::get_modified_time(&paths.elf)?;
        let nso_mtime = system::get_modified_time(&paths.nso)?;
        if elf_mtime > nso_mtime {
            needs_nso = true;
        }
    }

    // nso dependency
    if let Some(task) = link_task {
        let result = task.wait()?;
        if !result.success {
            if let Some(error) = result.error {
                for line in error.lines().flatten() {
                    system::errorln!("Error", "{}", line);
                }
            }
            return Err(Error::LinkError);
        }
    }

    if needs_nso {
        let nso_name = format!("{}.nso", config.module.name);
        if let Some(mut checker) = checker {
            system::infoln!("Checking", "{}", elf_name);
            let missing_symbols = checker.check_symbols(&executer)?;
            let bad_instructions = checker.check_instructions(&executer)?;
            let missing_symbols = missing_symbols.wait()?;
            let bad_instructions = bad_instructions.wait()?;
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

    if let Some(task) = save_cc_json_task {
        task.wait()?;
    }

    if let Some(task) = main_npdm_task {
        task.wait()?;
    }

    let elapsed = start_time.elapsed();
    system::infoln!("Finished", 
        "{} (profile `{profile}`) in {:.2}s",
        config.module.name,
        elapsed.as_secs_f32()
    );

    Ok(())
}

fn create_npdm(
    target: PathBuf,
    npdmtool: PathBuf,
    title_id: String,
    m_time: FileTime,
) -> Result<(), Error> {
    let mut npdm_data: Value = serde_json::from_str(include_str!("../../template/main.npdm.json")).unwrap();
    npdm_data["title_id"] = json!(format!("0x{}", title_id));
    let npdm_data = serde_json::to_string_pretty(&npdm_data).expect("fail to serialize npdm data");
    let npdm_json = target.join("main.npdm.json");
    system::write_file(&npdm_json, &npdm_data)?;
    system::set_modified_time(&npdm_json, m_time)?;
    let main_npdm = target.join("main.npdm");
    let npdm_status = ChildBuilder::new(npdmtool)
        .args(system::args![&npdm_json, &main_npdm])
        .silent()
        .spawn()?
        .wait()?;
    if !npdm_status.success() {
        return Err(Error::NpdmError(npdm_status));
    }
    Ok(())
}

fn create_verfile(verfile: PathBuf, entry: String) -> Result<(), Error> {
    let verfile_data = format!("{}{}{}", include_str!("../../template/verfile.before"),entry,include_str!("../../template/verfile.after"));
    system::write_file(verfile, &verfile_data)?;
    Ok(())
}

pub fn clean(dir: &str, options: &Options) -> Result<(), Error> {
    let root = system::find_root(dir)?;
    let mut target = root.clone();
    target.push("target");
    target.push("megaton");
    if "none" != &options.profile {
        target.push(&options.profile);
    }
    if !root.exists() {
        system::hintln!("Skipped", "{}", target.from_base(&root)?.display());
        return Ok(());
    }

    system::remove_directory(&target)?;
    system::infoln!("Cleaned", "{}", target.from_base(&root)?.display());
    Ok(())
}

