use clap::Parser;
use megatonhammer::MegatonHammer;
fn main() {
    megatonhammer::print::set_enabled(true);
    let cli = MegatonHammer::parse();
    let result = match &cli.command {
        Some(x) => x.run(),
        None => cli.build()
    };
    if let Err(e) = result {
        e.print();
        std::process::exit(1);
    }
}
