use clap::Parser;
use wow_version_switcher::{launch, load_config, write_realmlist};

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

    let game_cfg = load_config(&args.config, &args.game)?;

    if let (Some(realmlist), Some(realmlist_rel_path)) =
        (&game_cfg.realmlist, &game_cfg.realmlist_rel_path)
    {
        write_realmlist(&game_cfg.directory, realmlist_rel_path, realmlist)?;
    }

    launch(&game_cfg).expect("Failed to launch game");
    Ok(())
}
