//! Integration with `make` build tool.
//!
//! Megaton puts the artifacts in the `./target/megaton/<flavor>/<profile>/make` directory:
//! - `build.mk`: The Makefile
//! - `build`: The build output directory

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::stdio::{args, ChildBuilder, self, PathExt, root_rel};
use crate::{hintln, errorln, infoln, MegatonConfig, MegatonHammer, Paths};

macro_rules! format_makefile_template {
    ($($args:tt)*) => {
        format!(
r###"
# GENERATED BY MEGATON HAMMER
include $(DEVKITPRO)/libnx/switch_rules

MEGATON_MODULE_NAME := {MEGATON_MODULE_NAME}
MEGATON_MODULE_ENTRY := {MEGATON_MODULE_ENTRY}
MEGATON_MODULE_TITLE_ID := 0x{MEGATON_MODULE_TITLE_ID}
MEGATON_ROOT := {MEGATON_ROOT}

TARGET := $(MEGATON_MODULE_NAME)
VERFILE := verfile

DEFAULT_ARCH_FLAGS := \
    -march=armv8-a+crc+crypto \
    -mtune=cortex-a57 \
    -mtp=soft \
    -fPIC \
    -fvisibility=hidden \

DEFAULT_CFLAGS := \
    -D__SWITCH__ \
    -g \
    -Wall \
    -Werror \
    -fdiagnostics-color=always \
    -ffunction-sections \
    -fdata-sections \
    -fvisibility=hidden \
    -O3 \

DEFAULT_CXXFLAGS := \
    -fno-rtti \
    -fno-exceptions \
    -fno-asynchronous-unwind-tables \
    -fno-unwind-tables \
    -fpermissive \
    -std=c++20 \

DEFAULT_ASFLAGS := -g
DEFAULT_LDFLAGS := \
    -g \
    -Wl,-Map,$(TARGET).map \
    -nostartfiles \
    -nodefaultlibs \
    -Wl,--shared \
    -Wl,--export-dynamic \
    -Wl,-z,nodynamic-undefined-weak \
    -Wl,--gc-sections \
    -Wl,--build-id=sha1 \
    -Wl,--nx-module-name \
    -Wl,-init=$(MEGATON_MODULE_ENTRY) \
    -Wl,--version-script=$(VERFILE) \
    -Wl,--exclude-libs=ALL \

DEFAULT_LIBS := -u malloc

{EXTRA_SECTION}

SOURCES          := $(SOURCES) {SOURCES}
ALL_SOURCE_DIRS  := $(ALL_SOURCE_DIRS) $(foreach dir,$(SOURCES),$(shell find $(dir) -type d))
VPATH            := $(VPATH) $(ALL_SOURCE_DIRS)

INCLUDES         := $(INCLUDES) {INCLUDES}
LIBDIRS          := $(LIBDIRS) $(PORTLIBS) $(LIBNX)
INCLUDE_FLAGS    := $(foreach dir,$(INCLUDES),-I$(dir)) $(foreach dir,$(LIBDIRS),-I$(dir)/include)

DEFINES          := $(DEFINES) {DEFINES}

ARCH_FLAGS       := $(ARCH_FLAGS) {ARCH_FLAGS}
CFLAGS           := $(CFLAGS) $(ARCH_FLAGS) $(DEFINES) $(INCLUDE_FLAGS) {CFLAGS}
CXXFLAGS         := $(CFLAGS) $(CXXFLAGS) {CXXFLAGS}
ASFLAGS          := $(ASFLAGS) $(ARCH_FLAGS) {ASFLAGS}

LD_SCRIPTS       := {LD_SCRIPTS}
LD_SCRIPTS_FLAGS := $(foreach ld,$(LD_SCRIPTS),-Wl,-T,$(ld))
LD               := $(CXX)
LDFLAGS          := $(LDFLAGS) $(ARCH_FLAGS) $(LD_SCRIPTS_FLAGS) {LDFLAGS}
LIBS             := $(LIBS) {LIBS}
LIBPATHS         := $(LIBPATHS) $(foreach dir,$(LIBDIRS),-L$(dir)/lib) 

DEPSDIR          ?= .
CFILES           := $(CFILES) $(foreach dir,$(ALL_SOURCE_DIRS),$(notdir $(wildcard $(dir)/*.c)))
CPPFILES         := $(CPPFILES) $(foreach dir,$(ALL_SOURCE_DIRS),$(notdir $(wildcard $(dir)/*.cpp)))
SFILES           := $(SFILES) $(foreach dir,$(ALL_SOURCE_DIRS),$(notdir $(wildcard $(dir)/*.s)))
OFILES           := $(CPPFILES:.cpp=.o) $(CFILES:.c=.o) $(SFILES:.s=.o)
DFILES           := $(OFILES:.o=.d)

.PHONY: nso elf
nso: $(TARGET).nso
elf: $(TARGET).elf

$(TARGET).nso: $(TARGET).elf
$(TARGET).elf: $(OFILES) $(LD_SCRIPTS) $(VERFILE)
$(VERFILE):
	@echo $(VERFILE)
	@echo "{{" > $(VERFILE)
	@echo "    global:" >> $(VERFILE)
	@echo "        $(MEGATON_MODULE_ENTRY);" >> $(VERFILE)
	@echo "    local: *;" >> $(VERFILE)
	@echo "}};" >> $(VERFILE)

-include $(DFILES)

"###,
        $($args)*
        )
    };
}

macro_rules! default_or_empty {
    ($make:ident, $default:expr) => {
        if $make.no_default_flags.unwrap_or_default() {
            ""
        } else {
            $default
        }
    };
}

impl MegatonConfig {
    /// Create the Makefile content from the config
    pub fn create_makefile(&self, paths: &Paths, cli: &MegatonHammer) -> Result<String, Error> {
        let mut root = paths.root.display().to_string();
        if !root.ends_with('/') {
            root.push('/');
        }

        let make = self.make.get_profile(&cli.options.profile);

        let entry = make.entry.as_ref().ok_or(Error::NoEntryPoint)?;

        let extra_section = make
            .extra
            .iter()
            .map(|s| format!("{} := {}", s.key, s.val))
            .collect::<Vec<_>>()
            .join("\n");

        let sources = make
            .sources
            .iter()
            .map(|s| format!("$(MEGATON_ROOT){s}"))
            .collect::<Vec<_>>()
            .join(" ");
        let includes = make
            .includes
            .iter()
            .map(|s| format!("$(MEGATON_ROOT){s}"))
            .collect::<Vec<_>>()
            .join(" ");
        let ld_scripts = make
            .ld_scripts
            .iter()
            .map(|s| format!("$(MEGATON_ROOT){s}"))
            .collect::<Vec<_>>()
            .join(" ");
        let defines = make
            .defines
            .iter()
            .map(|s| format!("-D{s}"))
            .collect::<Vec<_>>()
            .join(" ");

        let makefile = format_makefile_template!(
            MEGATON_MODULE_NAME = self.module.name,
            MEGATON_MODULE_ENTRY = entry,
            MEGATON_MODULE_TITLE_ID = self.module.title_id_hex(),
            MEGATON_ROOT = root,
            EXTRA_SECTION = extra_section,
            SOURCES = sources,
            INCLUDES = includes,
            DEFINES = defines,
            ARCH_FLAGS = default_or_empty!(make, "$(DEFAULT_ARCH_FLAGS)"),
            CFLAGS = default_or_empty!(make, "$(DEFAULT_CFLAGS)"),
            CXXFLAGS = default_or_empty!(make, "$(DEFAULT_CXXFLAGS)"),
            ASFLAGS = default_or_empty!(make, "$(DEFAULT_ASFLAGS)"),
            LD_SCRIPTS = ld_scripts,
            LDFLAGS = default_or_empty!(make, "$(DEFAULT_LDFLAGS)"),
            LIBS = default_or_empty!(make, "$(DEFAULT_LIBS)"),
        );

        Ok(makefile)
    }
}

/// Compiler command for IDE integration. See
/// <https://clang.llvm.org/docs/JSONCompilationDatabase.html>
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompileCommand<'a> {
    /// The working directory of the compilation. All paths specified in the command or file fields must be either absolute or relative to this directory.
    pub directory: Cow<'a, String>,
    /// The main translation unit source processed by this compilation step. This is used by tools as the key into the compilation database. There can be multiple command objects for the same file, for example if the same source file is compiled with different configurations.
    pub file: String,
    /// The compile command as a single shell-escaped string. Arguments may be shell quoted and escaped following platform conventions, with ‘"’ and ‘\’ being the only special characters. Shell expansion is not supported.
    pub command: String,
    /// The name of the output created by this compilation step.
    pub output: String,
}

pub struct CCBuilder {
    /// The prefix to add to the compiler command. Used to make the cc executable absolute
    cc_prefix: String,
    /// Absolute path to build directory
    directory: String,
}

impl CCBuilder {
    pub fn from_paths(paths: &Paths) -> Self {
        let mut cc_prefix = paths.devkita64_bin.display().to_string();
        if !cc_prefix.ends_with('/') {
            cc_prefix.push('/');
        }
        let directory = paths.make.display().to_string();
        Self {
            cc_prefix,
            directory,
        }
    }

    pub fn build(&self, command: &str) -> CompileCommand {
        // hopefully there are no spaces in the source paths...:)
        let mut iter = command.split_whitespace();
        let mut file = String::new();
        let mut output = String::new();
        while let Some(arg) = iter.next() {
            match arg {
                "-c" => {
                    if let Some(arg) = iter.next() {
                        file = arg.to_string();
                    }
                }
                "-o" => {
                    if let Some(arg) = iter.next() {
                        output = arg.to_string();
                    }
                }
                _ => {}
            }
        }
        CompileCommand {
            directory: Cow::Borrowed(&self.directory),
            file,
            command: format!("{}{command}", self.cc_prefix),
            output,
        }
    }
}

pub fn make_elf(paths: &Paths, verbose: bool) -> Result<(), Error> {
    invoke_make(paths, "elf", true, verbose)
}

pub fn make_nso(paths: &Paths, verbose: bool) -> Result<(), Error> {
    invoke_make(paths, "nso", false, verbose)
}

fn invoke_make(
    paths: &Paths,
    target: &str,
    save_compile_commands: bool,
    verbose: bool,
) -> Result<(), Error>
{
    let j_flag = format!("-j{}", num_cpus::get());
    let mut child = ChildBuilder::new("make")
        .args(args![
            "--no-print-directory",
            "V=1",
            &j_flag,
            "-C",
            &paths.make,
            "-f",
            "../makefile",
            target
        ]).piped().spawn()?;

    // load existing compile commands, since make may not execute all the targets
    let mut compile_commands = BTreeMap::new();
    if save_compile_commands && paths.cc_json.exists() {
        if let Ok(cc_json) = stdio::read_file(&paths.cc_json) {
            if let Ok(cc_vec) = serde_json::from_str::<Vec<CompileCommand>>(&cc_json) {
                for command in cc_vec {
                    compile_commands.insert(command.file.clone(), command);
                }
            }
        }
    }

    let cc_builder = CCBuilder::from_paths(paths);

    if let Some(stdout) = child.take_stdout() {
        let stdout = BufReader::new(stdout);
        for line in stdout.lines() {
            if let Ok(line) = line {
                if verbose {
                    hintln!("Verbose", "{}", line);
                }

                // hide some outputs
                if line.starts_with("built ...") {
                    continue;
                }
                if line.ends_with("up to date.") {
                    continue;
                }
                if line.starts_with("aarch64-none-elf-") {
                    // compiler command
                    let compile_command = cc_builder.build(&line);
                    if let Ok(file_path) = paths.from_root(&compile_command.file) {
                        infoln!("Compiling", "{}", file_path.display());
                    }
                    compile_commands.insert(compile_command.file.clone(), compile_command);
                    continue;
                }
                if let Some(line) = line.strip_prefix("linking ") {
                    infoln!("Linking", "{}", line);
                }
            }
        }
    }

    if let Some(stderr) = child.take_stderr() {
        let stderr = BufReader::new(stderr);
        for line in stderr.lines() {
            if let Ok(line) = line {
                if verbose {
                    hintln!("Verbose", "{}", line);
                }
                // clean the error output
                if line.starts_with("make: ***") {
                    continue;
                }
                if line == "compilation terminated." {
                    continue;
                }
                errorln!("Error", "{}", line);
            }
        }
    }

    let status = child.wait()?;
    if !status.success() {
        return Err(Error::MakeError);
    }

    if save_compile_commands {
        let vec = compile_commands.into_values().collect::<Vec<_>>();

        match serde_json::to_string_pretty(&vec) {
            Err(e) => {
                errorln!("Error", "Failed to serialize compiler commands: {}", e);
            }
            Ok(json) => {
                stdio::write_file(&paths.cc_json, &json)?;
                infoln!("Saved", "{}", root_rel!(paths.cc_json)?.display());
            }
        }
    }

    Ok(())
}
