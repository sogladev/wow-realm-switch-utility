use clap::Parser;
use wowctl::cli::Cli;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    cli.run()
}
