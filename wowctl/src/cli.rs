use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};

use crate::{launch, load_config, write_realmlist};

/// WoW Client Manager - manage multiple WoW clients with shared resources
#[derive(Parser)]
#[command(name = "realmctl")]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Launch a WoW workspace
    Launch {
        /// Workspace name to launch (as in your config file)
        workspace: String,
        /// Path to your config.toml
        #[arg(long, default_value = "~/.config/realmctl/config.toml")]
        config: String,
    },
    /// Initialize a base WoW installation for workspace creation
    InitBase {
        /// Path to the WoW directory to use as base
        path: PathBuf,
        /// Profile to use (e.g., chromie-3.3.5a)
        #[arg(long, default_value = "chromie-3.3.5a")]
        profile: String,
    },
    /// Create a new workspace from a base installation
    Create {
        /// Name of the workspace
        workspace: String,
        /// Path to the base installation (must have manifest.toml)
        #[arg(long)]
        base: String,
        /// Sharing rules (format: key=value, e.g., screenshots=global)
        #[arg(long = "share", value_name = "KEY=VALUE")]
        share: Vec<String>,
        /// Workspace root directory
        #[arg(long, default_value = "~/.local/share/wow_workspaces")]
        workspace_root: String,
    },
    /// Clean ephemeral files (cache, logs) from a workspace
    Clean {
        /// Workspace name to clean (as in your config file)
        workspace: String,
        /// Path to your config.toml
        #[arg(long, default_value = "~/.config/realmctl/config.toml")]
        config: String,
        /// Also clean WDB cache files
        #[arg(long)]
        wdb: bool,
    },
    /// Repair a workspace's shared links and directories
    Fix {
        /// Workspace name to fix (as in your config file)
        workspace: String,
        /// Path to your config.toml
        #[arg(long, default_value = "~/.config/realmctl/config.toml")]
        config: String,
    },
}

impl Cli {
    pub fn run(self) -> Result<()> {
        match self.command {
            Commands::Launch { workspace, config } => {
                cmd_launch(&workspace, &config)?;
            }
            Commands::InitBase { path, profile } => {
                cmd_init_base(&path, &profile)?;
            }
            Commands::Create {
                workspace,
                base,
                share,
                workspace_root,
            } => {
                cmd_create_workspace(&workspace, &base, &share, &workspace_root)?;
            }
            Commands::Clean {
                workspace,
                config,
                wdb,
            } => {
                cmd_clean(&workspace, &config, wdb)?;
            }
            Commands::Fix { workspace, config } => {
                cmd_fix(&workspace, &config)?;
            }
        }
        Ok(())
    }
}

fn cmd_launch(workspace: &str, config_path: &str) -> Result<()> {
    println!("Loading configuration for:\n\t{workspace}");
    let game_cfg = load_config(config_path, workspace)?;

    if let (Some(realmlist), Some(realmlist_rel_path)) =
        (&game_cfg.realmlist, &game_cfg.realmlist_rel_path)
    {
        write_realmlist(&game_cfg.directory, realmlist_rel_path, realmlist)?;
    }

    launch(&game_cfg)?;
    Ok(())
}

fn cmd_init_base(path: &Path, profile_name: &str) -> Result<()> {
    use crate::base::{Profile, scan_and_build_manifest, write_manifest};

    println!("Initializing base at: {}", path.display());
    println!("Using profile: {}", profile_name);

    // Expand tilde in path
    let expanded_path = shellexpand::tilde(&path.to_string_lossy()).to_string();
    let base_dir = PathBuf::from(expanded_path);

    if !base_dir.exists() {
        anyhow::bail!("Directory does not exist: {}", base_dir.display());
    }

    // Load profile
    let profile = match profile_name {
        "chromie-3.3.5a" | "3.3.5a" | "335" | "335a" => Profile::chromie_335a(),
        "vanilla-1.12" | "1.12" | "112" => Profile::vanilla_112(),
        _ => anyhow::bail!("Unknown profile: {}", profile_name),
    };

    println!("\n=== Verifying Requirements ===");
    profile.verify_requirements(&base_dir)?;
    println!("✓ All required files and directories present");

    // Check warnings
    let warnings = profile.check_warnings(&base_dir);
    if !warnings.is_empty() {
        println!("\n⚠ Warnings:");
        for warning in warnings {
            println!("  - {}", warning);
        }
    }

    println!("\n=== Scanning Directory ===");
    let manifest = scan_and_build_manifest(&base_dir, &profile)?;

    println!("Found {} files/directories", manifest.file_roles.len());
    println!(
        "Computed {} checksums for immutable files",
        manifest.checksums.len()
    );

    println!("\n=== Writing Manifest ===");
    write_manifest(&manifest, &base_dir)?;
    println!("✓ Manifest written to {}/manifest.toml", base_dir.display());

    println!("\n✓ Base initialization complete!");

    Ok(())
}

fn cmd_create_workspace(
    name: &str,
    base: &str,
    share_args: &[String],
    workspace_root: &str,
) -> Result<()> {
    use crate::workspace::{SharingStrategy, create_workspace, default_sharing_rules};

    println!("Creating workspace: {name}");
    println!("Base: {base}");

    // Expand paths
    let expanded_base = shellexpand::tilde(base).to_string();
    let base_path = PathBuf::from(expanded_base);

    let expanded_root = shellexpand::tilde(workspace_root).to_string();
    let ws_root = PathBuf::from(expanded_root);

    // Parse sharing rules
    let mut sharing_rules = default_sharing_rules();
    for arg in share_args {
        let parts: Vec<&str> = arg.split('=').collect();
        if parts.len() == 2 {
            let key = parts[0].to_string();
            let value = match parts[1] {
                "global" => SharingStrategy::Global,
                "base" => SharingStrategy::Base,
                "workspace" => SharingStrategy::Workspace,
                _ => anyhow::bail!("Invalid sharing strategy: {}", parts[1]),
            };
            sharing_rules.insert(key, value);
        }
    }

    println!("\nSharing rules:");
    for (key, value) in &sharing_rules {
        println!("  {} = {:?}", key, value);
    }

    println!("\n=== Creating Workspace ===");
    let config = create_workspace(name, &base_path, &ws_root, sharing_rules)?;

    println!(
        "✓ Workspace created at: {}",
        config.workspace_path.display()
    );
    println!("\nYou can now launch this workspace by updating your config.toml:");
    println!("[{}]", name);
    println!("directory = \"{}\"", config.workspace_path.display());
    println!("# ... other settings ...");

    Ok(())
}

fn cmd_fix(workspace: &str, config_path: &str) -> Result<()> {
    println!("Fixing workspace: {}", workspace);

    let game_cfg = load_config(config_path, workspace)?;

    // Perform fix/repair operations on the workspace
    crate::workspace::fix_workspace(&game_cfg.directory)?;

    println!("\n✓ Fix operations completed (no user data was overridden)");
    Ok(())
}

fn cmd_clean(workspace: &str, config_path: &str, clean_wdb: bool) -> Result<()> {
    println!("Cleaning workspace: {}", workspace);

    let game_cfg = load_config(config_path, workspace)?;
    let workspace_dir = &game_cfg.directory;

    let mut cleaned_items = Vec::new();

    // Clean Cache directory
    let cache_dir = workspace_dir.join("Cache");
    if cache_dir.exists() {
        match std::fs::remove_dir_all(&cache_dir) {
            Ok(_) => {
                cleaned_items.push("Cache");
                println!("✓ Removed Cache directory");
            }
            Err(e) => {
                eprintln!("✗ Failed to remove Cache: {}", e);
            }
        }
    }

    // Clean Logs directory
    let logs_dir = workspace_dir.join("Logs");
    if logs_dir.exists() {
        match std::fs::remove_dir_all(&logs_dir) {
            Ok(_) => {
                cleaned_items.push("Logs");
                println!("✓ Removed Logs directory");
            }
            Err(e) => {
                eprintln!("✗ Failed to remove Logs: {}", e);
            }
        }
    }

    // Clean Errors directory
    let errors_dir = workspace_dir.join("Errors");
    if errors_dir.exists() {
        match std::fs::remove_dir_all(&errors_dir) {
            Ok(_) => {
                cleaned_items.push("Errors");
                println!("✓ Removed Errors directory");
            }
            Err(e) => {
                eprintln!("✗ Failed to remove Errors: {}", e);
            }
        }
    }

    // Clean WDB cache files if requested
    if clean_wdb {
        let data_dir = workspace_dir.join("Data");
        if data_dir.exists()
            && let Ok(entries) = std::fs::read_dir(&data_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Some(ext) = path.extension()
                        && ext == "wdb" {
                            match std::fs::remove_file(&path) {
                                Ok(_) => {
                                    println!(
                                        "✓ Removed WDB cache: {}",
                                        path.file_name().unwrap().to_string_lossy()
                                    );
                                    cleaned_items.push("WDB files");
                                }
                                Err(e) => {
                                    eprintln!("✗ Failed to remove {}: {}", path.display(), e);
                                }
                            }
                        }
                }
            }

        // Also check locale directories for WDB files
        if let Ok(entries) = std::fs::read_dir(&data_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir()
                    && let Some(dir_name) = path.file_name() {
                        let dir_name = dir_name.to_string_lossy();
                        // Check if it's a locale directory (enUS, enGB, etc.)
                        if dir_name.len() == 4 && dir_name.chars().all(|c| c.is_alphabetic())
                            && let Ok(locale_entries) = std::fs::read_dir(&path) {
                                for locale_entry in locale_entries.flatten() {
                                    let locale_path = locale_entry.path();
                                    if let Some(ext) = locale_path.extension()
                                        && ext == "wdb" {
                                            match std::fs::remove_file(&locale_path) {
                                                Ok(_) => {
                                                    println!(
                                                        "✓ Removed WDB cache: {}/{}",
                                                        dir_name,
                                                        locale_path
                                                            .file_name()
                                                            .unwrap()
                                                            .to_string_lossy()
                                                    );
                                                }
                                                Err(e) => {
                                                    eprintln!(
                                                        "✗ Failed to remove {}: {}",
                                                        locale_path.display(),
                                                        e
                                                    );
                                                }
                                            }
                                        }
                                }
                            }
                    }
            }
        }
    }

    if cleaned_items.is_empty() {
        println!("\nNo files to clean (workspace is already clean)");
    } else {
        println!("\n✓ Workspace cleaned successfully!");
    }

    Ok(())
}
