use clap::Parser;
use wow_version_switcher::{write_realmlist, launch, load_config};

/// Wow version switcher
#[derive(Parser)]
struct Args {
    /// Which game key to launch (as in your config file)
    game: String,
    /// Path to your config.toml
    #[arg(long, default_value = "~/.config/wow_version_switcher/config.toml")]
    config: String,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    println!("Loading configuration for:\n\t{}", args.game);

    let game_cfg = load_config( &args.config, &args.game)?;

    write_realmlist(&game_cfg.directory, &game_cfg.realmlist_rel_path, &game_cfg.realmlist)?;
    launch(&game_cfg).expect("Failed to launch game");
    Ok(())
}