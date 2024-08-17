use clap::{Parser, Subcommand};

pub mod build;

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

    /// Subcommand
    #[clap(subcommand)]
    pub command: Option<MegatonCommand>,

    /// Build options
    #[clap(flatten)]
    pub options: Options,
}

#[derive(Debug, Clone, PartialEq, Subcommand)]
pub enum MegatonCommand {
    // Clean project outputs
    Clean,
    // /// Init a project - generate Megaton.toml, .clangd, etc
    // Init,
    // /// Build the toolchain
    // Toolchain,
}

impl MegatonCommand {
    pub fn run(&self, args: &MegatonHammer) -> Result<(), Error> {
        match self {
            Self::Clean => build::clean(&args.dir, &args.options),
            // Self::Toolchain => toolchain::build(),
            // Self::Init => init::init(&args.dir),
        }
    }
}

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

impl MegatonHammer {
    /// Build the project
    pub fn build(&self) -> Result<(), Error> {
        build::run(&self.dir, &self.options)
    }
}
