use clap::Parser;
use realmctl::cli::Cli;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    cli.run()
}
