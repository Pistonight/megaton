use std::collections::BTreeSet;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::mpsc;

use filetime::FileTime;
use regex::Regex;

use crate::build::Paths;
use crate::build::config::Check;
use crate::system::{self, ChildBuilder, Error, Executer, PathExt, Task};

pub fn load_checker(paths: &Paths, config: Check, executer: &Executer) -> Result<Checker, Error> {
    let mut tasks = Vec::with_capacity(config.symbols.len());
    let (send, recv) = mpsc::channel();
    for path in &config.symbols {
        let path = paths.root.join(path).canonicalize2()?;
        let file = BufReader::new(system::open(&path)?);
        let id = paths.from_root(&path)?.display().to_string();
        let send = send.clone();
        let task = executer.execute(move || {
            process_objdump_syms(
                &id,
                file.lines().flatten(),
                send,
            )?;
            Ok(())
        });
        tasks.push(task);
    }

    Ok(Checker {
        data: CheckData::new(paths, config),
        tasks,
        recv: Some(recv),
    })
}

pub struct Checker {
    data: CheckData,
    tasks: Vec<Task<Result<(), Error>>>,
    recv: Option<mpsc::Receiver<String>>,
}

impl Checker {
    pub fn are_syms_newer_than(&self, paths: &Paths, m_time: FileTime) -> bool {
        for symbol in &self.data.config.symbols {
            let symbol = paths.root.join(symbol);
            let sym_mtime = match system::get_modified_time(&symbol) {
                Ok(sym_mtime) => sym_mtime,
                Err(_) => {
                    return true;
                }
            };
            if sym_mtime > m_time {
                return true;
            }
        }
        false
    }

    pub fn check_symbols(&mut self, executer: &Executer) -> Result<CheckSymbolTask, Error> {
        // run objdump -T
        let mut child = ChildBuilder::new(&self.data.objdump)
            .args(system::args!["-T", self.data.elf])
            .piped()
            .spawn()?;
        let elf_symbols = child.take_stdout().ok_or(Error::ObjdumpFailed)?;
        let (elf_send, elf_recv) = mpsc::channel();
        let dump_task = executer.execute(move || {
            process_objdump_syms(
                "(output of `objdump -T`)",
                elf_symbols.lines().flatten(),
                elf_send,
            )
        });
        let ignore = std::mem::take(&mut self.data.config.ignore);
        let recv = self.recv.take().unwrap();
        let check_task = executer.execute(move || {
            let mut loaded_symbols = BTreeSet::new();
            while let Ok(symbol) = recv.recv() {
                loaded_symbols.insert(symbol);
            }
            let mut missing_symbols = vec![];
            while let Ok(symbol) = elf_recv.recv() {
                if ignore.contains(&symbol) {
                    continue;
                }
                if !loaded_symbols.contains(&symbol) {
                    missing_symbols.push(symbol);
                }
            }
            missing_symbols
        });
        let wait_task = executer.execute(move || {
            child.dump_stderr("Error");
            let status = child.wait()?;
            if !status.success() {
                return Err(Error::ObjdumpFailed);
            }
            Ok(())
        });

        Ok(CheckSymbolTask {
            dump_task,
            check_task,
            wait_task,
            load_tasks: std::mem::take(&mut self.tasks),
        })
        
    }

    pub fn check_instructions(&self, executer: &Executer) -> Result<CheckInstructionTask, Error> {
        let mut child = ChildBuilder::new(&self.data.objdump)
            .args(system::args!["-d", self.data.elf])
            .piped()
            .spawn()?;
        let elf_instructions = child.take_stdout().ok_or(Error::ObjdumpFailed)?;
        let (elf_send, elf_recv) = mpsc::channel();
        let dump_task = executer.execute(move || {
            process_objdump_insts(
                elf_instructions.lines().flatten(),
                elf_send,
            );
        });

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
        let extra = &self.data.config.disallowed_instructions;
        if !extra.is_empty() {
            disallowed_regexes.reserve_exact(extra.len());
            for s in extra {
                disallowed_regexes.push(Regex::new(s)?);
            }
        }
        let check_task = executer.execute(move || {
            let mut output = vec![];
            while let Ok(inst) = elf_recv.recv() {
                for regex in &disallowed_regexes {
                    if regex.is_match(&inst.1) {
                        output.push(format!("{}: {}", inst.0, inst.1));
                        break;
                    }
                }
            }
            output
        });
        let wait_task = executer.execute(move || {
            child.dump_stderr("Error");
            let status = child.wait()?;
            if !status.success() {
                return Err(Error::ObjdumpFailed);
            }
            Ok(())
        });

        Ok(CheckInstructionTask {
            dump_task,
            wait_task,
            check_task,
        })
    }
}

struct CheckData {
    objdump: PathBuf,
    elf: PathBuf,
    config: Check,
}

impl CheckData {
    pub fn new(paths: &Paths, config: Check) -> Self {
        Self {
            objdump: paths.objdump.clone(),
            elf: paths.elf.clone(),
            config,
        }
    }
}

pub struct CheckSymbolTask {
    dump_task: Task<Result<(), Error>>,
    check_task: Task<Vec<String>>,
    wait_task: Task<Result<(), Error>>,
    load_tasks: Vec<Task<Result<(), Error>>>,
}

impl CheckSymbolTask {
    pub fn wait(self) -> Result<Vec<String>, Error> {
        for task in self.load_tasks {
            task.wait()?;
        }
        self.dump_task.wait()?;
        self.wait_task.wait()?;
        let result = self.check_task.wait();
        Ok(result)
    }
}

pub struct CheckInstructionTask {
    dump_task: Task<()>,
    wait_task: Task<Result<(), Error>>,
    check_task: Task<Vec<String>>,
}

impl CheckInstructionTask {
    pub fn wait(self) -> Result<Vec<String>, Error> {
        self.dump_task.wait();
        self.wait_task.wait()?;
        let result = self.check_task.wait();
        Ok(result)
    }
}

/// Parse the output of objdump -T
fn process_objdump_syms<Iter, Str>(
    id: &str,
    raw_symbols: Iter,
    send: mpsc::Sender<String>,
) -> Result<(), Error>
where
    Iter: IntoIterator<Item = Str>,
    Str: AsRef<str>,
{
    system::verboseln!("Loading", "{}", id);
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
        send.send(symbol.to_string()).unwrap();
    }

    system::verboseln!("Loaded", "{}", id);
    Ok(())
}

/// Parse the output of objdump --disassemble
///
/// Returns a list of (address, instructions)
fn process_objdump_insts<Iter, Str>(
    raw_instructions: Iter,
    send: mpsc::Sender<(String, String)>,
) 
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
        .for_each(|inst| {
            send.send(inst).unwrap();
        });
}
