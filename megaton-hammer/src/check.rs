use std::collections::BTreeSet;
use std::io::BufRead;

use crate::config::Check;
use crate::error::Error;
use crate::{errorln, hintln, infoln, Paths};
use crate::stdio::{self, args, root_rel, PathExt, ChildBuilder};

/// Check ELF for issues with symbols and instructions
pub fn check_elf(paths: &Paths, check: &Check) -> Result<(), Error>
{
    infoln!("Checking", "{}", root_rel!(paths.elf)?.display());

    let mut child = ChildBuilder::new(&paths.objdump)
        .args(args!["-T", paths.elf])
        .piped()
        .spawn()?;

    let mut elf_symbols = BTreeSet::new();
    if let Some(stdout) = child.take_stdout() {
        parse_objdump_syms("(output of `objdump -T`)", stdout.lines().flatten(), &mut elf_symbols)?;
    }

    child.dump_stderr("Error");

    let mut child = ChildBuilder::new(&paths.objdump)
        .args(args!["-d", paths.elf])
        .piped()
        .spawn()?;

    let elf_instructions = match child.take_stdout() {
        Some(stdout) => parse_objdump_insts("(output of `objdump -d`)", stdout.lines().flatten()),
        None => {
            errorln!("Error", "Failed to get output of `objdump -d`");
            return Err(Error::CheckError);
        }
    };
    child.dump_stderr("Error");
    let status = child.wait()?;
    if !status.success() {
        return Err(Error::CheckError);
    }

    let result = run_checks(paths, check, elf_symbols, elf_instructions);
    if result.is_err() {
        // rename the binary so checks are not bypassed the next time.
        // since we only check when the make output is modified
        let elf_bad_path = paths.elf.with_extension("bad.elf");
        stdio::rename_file(&paths.elf, &elf_bad_path)?;
        infoln!("Renamed", "ELF to {}", paths.from_root(elf_bad_path)?.display());
    }

    result
}

fn run_checks(
    paths: &Paths,
    check: &Check,
    mut elf_symbols: BTreeSet<String>,
    elf_instructions: Vec<(String, String)>,
) -> Result<(), Error> {
    for symbol in &check.ignore {
        elf_symbols.remove(symbol);
    }

    let mut loaded_symbols = BTreeSet::new();
    for path in &check.symbols {
        let path = paths.root.join(path).canonicalize2()?;
        let file_content = stdio::read_file(&path)?;
        parse_objdump_syms(&paths.from_root(path)?.display().to_string(), file_content.lines(), &mut loaded_symbols)?;
    }

    let missing_symbols = elf_symbols
        .into_iter()
        .filter(|symbol| !loaded_symbols.contains(symbol))
        .collect::<Vec<_>>();
    if !missing_symbols.is_empty() {
        errorln!("Error", "There are unresolved symbols:");
        errorln!("Error", "");
        for symbol in missing_symbols.iter().take(10) {
            errorln!("Error", "  {}", symbol);
        }
        if missing_symbols.len() > 10 {
            errorln!("Error", "  ... ({} more)", missing_symbols.len() - 10);
        }
        errorln!("Error", "");
        errorln!(
            "Error",
            "Found {} unresolved symbols!",
            missing_symbols.len()
        );
        let missing_symbols = missing_symbols.join("\n");
        let missing_symbols_path = paths.make.join("missing_symbols.txt");
        stdio::write_file(&missing_symbols_path, &missing_symbols)?;
        hintln!(
            "Hint",
            "Include the symbols in the linker scripts, or add them to the `ignore` section."
        );
        hintln!(
            "Saved",
            "All missing symbols to `{}`",
            paths.from_root(missing_symbols_path)?.display()
        );
        return Err(Error::CheckError);
    }

    infoln!("Checked", "All symbols can be resolved!");

    if !check.disallowed_instructions.is_empty() {
        let mut disallowed_regexes = Vec::with_capacity(check.disallowed_instructions.len());
        for s in &check.disallowed_instructions {
            match regex::Regex::new(s) {
                Ok(regex) => disallowed_regexes.push(regex),
                Err(e) => {
                    errorln!("Error", "Invalid regex: {}", e);
                    return Err(Error::CheckError);
                }
            }
        }
        infoln!("Parsed", "{} disallowed instruction patterns", disallowed_regexes.len());

        let detected_disallowed_instructions = elf_instructions
            .iter()
            .filter(|inst| {
                for regex in &disallowed_regexes {
                    if regex.is_match(&inst.1) {
                        return true;
                    }
                }
                false
            })
            .collect::<Vec<_>>();

        if !detected_disallowed_instructions.is_empty() {
            errorln!("Error", "There are disallowed instructions:");
            errorln!("Error", "");
            for (addr, inst) in detected_disallowed_instructions.iter().take(10) {
                errorln!("Error", "  {addr}: {inst}");
            }
            if detected_disallowed_instructions.len() > 10 {
                errorln!("Error", "  ... ({} more)", detected_disallowed_instructions.len() - 10);
            }
            errorln!("Error", "");
            errorln!(
                "Error",
                "Found {} disallowed instructions!",
                detected_disallowed_instructions.len()
            );

            let detected_disallowed_instructions = detected_disallowed_instructions
                .iter()
                .map(|(addr, inst)| format!("{}: {}", addr, inst))
                .collect::<Vec<_>>()
                .join("\n");
            let disallowed_instructions_path = paths.make.join("disallowed_instructions.txt");
            stdio::write_file(&disallowed_instructions_path, &detected_disallowed_instructions)?;
            hintln!(
                "Saved",
                "All disallowed instructions to {}",
                paths.from_root(disallowed_instructions_path)?.display()
            );

            return Err(Error::CheckError);
        }

        infoln!("Checked", "No disallowed instructions found!");

    }

    Ok(())
}

/// Parse the output of objdump -T
fn parse_objdump_syms<Iter, Str>(
    id: &str,
    raw_symbols: Iter,
    output: &mut BTreeSet<String>,
) -> Result<(), Error>
where
    Iter: IntoIterator<Item = Str>,
    Str: AsRef<str>,
{
    infoln!("Parsing", "{}", id);
    let mut iter = raw_symbols.into_iter();
    let old_size = output.len();
    while let Some(line) = iter.next() {
        if line.as_ref() == "DYNAMIC SYMBOL TABLE:" {
            break;
        }
    }

    // Example
    // # 0000000000000000      DF *UND*	0000000000000000 nnsocketGetPeerName
    //                   ^ spaces      ^ this is a tag

    while let Some(line) = iter.next() {
        let line = line.as_ref();
        if line.len() <= 25 {
            continue;
        }
        let symbol = match line[25..].splitn(2, ' ').skip(1).next() {
            Some(symbol) => symbol,
            None => {
                return Err(Error::InvalidObjdump(
                    id.to_string(),
                    format!("invalid line: {}", line),
                ))
            }
        };
        output.insert(symbol.to_string());
    }

    if output.len() == old_size {
        hintln!("Warning", "No symbols found in `{}`", id);
    }

    Ok(())
}

/// Parse the output of objdump --disassemble
///
/// Returns a list of (address, instructions)
fn parse_objdump_insts<Iter, Str>(id: &str, raw_instructions: Iter) -> Vec<(String, String)>
where
    Iter: IntoIterator<Item = Str>,
    Str: AsRef<str>,
{
    infoln!("Parsing", "{}", id);

    raw_instructions.into_iter().flat_map(|line| {
        let line = line.as_ref();
        // Example
        // 0000000000000000 <__code_start__>:
        //        0:	14000008 	b	20 <entrypoint>
        //        4:	0001a6e0 	.word	0x0001a6e0
        //        8:	d503201f 	nop
        //          ^ tab       _^ tab
        let mut parts = line.splitn(2, ":\t");
        let addr = parts.next()?.to_string();
        let bytes_and_asm = parts.next()?;
        let mut parts = bytes_and_asm.splitn(2, " \t");
        let _bytes = parts.next()?;
        //14000008 	b	20 <entrypoint>
        let inst = parts.next()?;
        //b	20 <entrypoint>
        Some((addr, inst.to_string()))
    }).collect()
}
