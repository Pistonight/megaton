use std::collections::BTreeSet;
use std::io::{BufRead, BufReader};
use std::rc::Rc;

use regex::Regex;

use crate::build::Paths;
use crate::build::config::Check;
use crate::system::{self, ChildBuilder, Error, PathExt};

/// Check the ELF for issues with symbols and instructions
pub struct CheckPhase {
    paths: Rc<Paths>,
    config: Rc<Check>,
}

pub struct CheckPhaseLoaded {
    paths: Rc<Paths>,
    pub config: Rc<Check>,
    symbols: BTreeSet<String>,
}

impl CheckPhase {
    pub fn new(paths: Rc<Paths>, config: Rc<Check>) -> Self {
        Self { paths, config }
    }
    pub fn load_symbols(self) -> Result<CheckPhaseLoaded, Error> {
        let mut loaded_symbols = BTreeSet::new();
        for path in &self.config.symbols {
            let path = self.paths.root.join(path).canonicalize2()?;
            let lines = BufReader::new(system::open(&path)?).lines().flatten();
            parse_objdump_syms(
                &self.paths.from_root(path)?.display().to_string(),
                lines,
                &mut loaded_symbols,
            )?;
        }
        Ok(CheckPhaseLoaded {
            paths: self.paths,
            config: self.config,
            symbols: loaded_symbols,
        })
    }
}

impl CheckPhaseLoaded {
    pub fn check_symbols(&self) -> Result<Vec<String>, Error> {
        // run objdump -T
        let mut child = ChildBuilder::new(&self.paths.objdump)
            .args(system::args!["-T", self.paths.elf])
            .piped()
            .spawn()?;

        let mut elf_symbols = BTreeSet::new();
        if let Some(stdout) = child.take_stdout() {
            parse_objdump_syms(
                "(output of `objdump -T`)",
                stdout.lines().flatten(),
                &mut elf_symbols,
            )?;
        }

        child.dump_stderr("Error");
        let child_status = child.wait()?;
        if !child_status.success() {
            return Err(Error::ObjdumpFailed(child_status));
        }
        for symbol in &self.config.ignore {
            elf_symbols.remove(symbol);
        }

        let missing_symbols = elf_symbols
            .into_iter()
            .filter(|symbol| !self.symbols.contains(symbol))
            .collect::<Vec<_>>();

        Ok(missing_symbols)
        
    }

    pub fn check_instructions(&self) -> Result<Vec<String>, Error> {
        let mut child = ChildBuilder::new(&self.paths.objdump)
            .args(system::args!["-d", self.paths.elf])
            .piped()
            .spawn()?;
        let elf_instructions = match child.take_stdout() {
            Some(stdout) => parse_objdump_insts(stdout.lines().flatten()),
            None => {
                return Ok(vec!["objdump -d returned no output".to_string()]);
            }
        };
        child.dump_stderr("Error");
        let status = child.wait()?;
        if !status.success() {
            return Err(Error::ObjdumpFailed(status));
        }
        // These instructions will cause console to Instruction Abort
        // (potentially due to permission or unsupported instruction?)
        let mut disallowed_regexes = vec![
            Regex::new(r"^msr\s*spsel")?,
            Regex::new(r"^msr\s*daifset")?,
            Regex::new(r"^mrs\.*daif")?,
            Regex::new(r"^mrs\.*tpidr_el1")?,
            Regex::new(r"^msr\s*tpidr_el1")?,
            Regex::new(r"^hlt")?,
        ];
        let extra = &self.config.disallowed_instructions;
        if !extra.is_empty() {
            disallowed_regexes.reserve_exact(extra.len());
            for s in extra {
                disallowed_regexes.push(Regex::new(s)?);
            }
        }

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
        .map(|(addr, inst)| format!("{}: {}", addr, inst))
        .collect::<Vec<_>>();

        Ok(detected_disallowed_instructions)
    }
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
    let mut iter = raw_symbols.into_iter();
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

    Ok(())
}

/// Parse the output of objdump --disassemble
///
/// Returns a list of (address, instructions)
fn parse_objdump_insts<Iter, Str>(raw_instructions: Iter) -> Vec<(String, String)>
where
    Iter: IntoIterator<Item = Str>,
    Str: AsRef<str>,
{
    raw_instructions
        .into_iter()
        .flat_map(|line| {
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
        })
        .collect()
}
