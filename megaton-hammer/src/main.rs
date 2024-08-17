use std::io::IsTerminal;

use clap::Parser;
use megaton_hammer::MegatonHammer;
fn main() {
    if !std::io::stdout().is_terminal() {
        megaton_hammer::system::disable_colors();
    }
    let cli = MegatonHammer::parse();
    if cli.options.verbose {
        megaton_hammer::system::enable_verbose();
    }
    let result = match &cli.command {
        Some(x) => x.run(&cli),
        None => cli.build(),
    };
    if let Err(e) = result {
        e.print();
        std::process::exit(1);
    }
}
