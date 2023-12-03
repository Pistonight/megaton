use std::collections::BTreeSet;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

use crate::config::Check;
use crate::error::Error;
use crate::{errorln, hintln, infoln};

pub fn check_symbols<SRoot, SBinary, SObjDump>(
    root: SRoot,
    binary: SBinary,
    objdump: SObjDump,
    check: &Check,
) -> Result<(), Error>
where
    SRoot: AsRef<Path>,
    SBinary: AsRef<Path>,
    SObjDump: AsRef<Path>,
{
    let binary = binary.as_ref();
    infoln!("Checking", "{}", binary.display());

    let binary_path = binary.display().to_string();
    let args = vec!["-T", &binary_path];
    let command = format!("{} {}", objdump.as_ref().display(), args.join(" "));

    let mut child = Command::new(objdump.as_ref())
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Error::Subprocess(command.clone(), "cannot spawn child".to_string(), e))?;

    let mut elf_symbols = BTreeSet::new();
    if let Some(stdout) = child.stdout.take() {
        let stdout = BufReader::new(stdout).lines().flatten();
        parse_objdump_syms("(elf objdump output)", stdout, &mut elf_symbols)?;
    }

    if let Some(stderr) = child.stderr.take() {
        let stderr = BufReader::new(stderr);
        for line in stderr.lines() {
            if let Ok(line) = line {
                errorln!("Error", "{}", line);
            }
        }
    }

    let status = child
        .wait()
        .map_err(|e| Error::Subprocess(command.clone(), "cannot wait for child".to_string(), e))?;
    if !status.success() {
        return Err(Error::CheckError);
    }

    std::fs::remove_file(binary).map_err(|e| Error::AccessFile(binary.display().to_string(), e))?;

    for symbol in &check.ignore {
        elf_symbols.remove(symbol);
    }

    let mut loaded_symbols = BTreeSet::new();
    for path in &check.symbols {
        let file_content = std::fs::read_to_string(root.as_ref().join(path))
            .map_err(|e| Error::AccessFile(path.to_string(), e))?;
        parse_objdump_syms(&path, file_content.lines(), &mut loaded_symbols)?;
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
        hintln!(
            "Hint",
            "Include the symbols in the linker scripts, or add them to the `ignore` section."
        );
        return Err(Error::CheckError);
    }

    infoln!("Checked", "All symbols can be resolved!");

    Ok(())
}

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
