use clap::Parser;
use megatonhammer::MegatonHammer;
fn main() {
    megatonhammer::print::set_enabled(true);
    let cli = MegatonHammer::parse();
    println!("{:?}", cli);
    if let Err(e) = cli.invoke() {
        e.print();
        std::process::exit(1);
    }
}

