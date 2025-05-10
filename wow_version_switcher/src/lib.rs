use serde::Deserialize;
mod clipboard;

#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    pub directory: std::path::PathBuf,
    #[serde(default = "default_executable")]
    pub executable: String,
    pub realmlist_rel_path: String,
    pub launch_cmd: Option<String>,
    pub realmlist: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub clear_cache: Option<bool>,
}

fn default_executable() -> String {
    "Wow.exe".to_string()
}

/// Load the whole config file (TOML)
pub fn load_config(path_str: &String, game: &String) -> std::io::Result<Config> {
    let config_path = shellexpand::tilde(path_str).to_string();
    let config_path = std::path::PathBuf::from(config_path);

    let s = std::fs::read_to_string(config_path).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Config file not found: {}", path_str),
        )
    })?;

    let configs: std::collections::HashMap<String, Config> = toml::from_str(&s).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Failed to parse config file",
        )
    })?;

    let config = configs
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(game))
        .map(|(_, value)| value)
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Config with key '{}' not found (case-insensitive)", game),
            )
        })?;

    let mut config = config.clone();

    // Expand ~ in the directory path
    // $HOME, $USER are NOT expanded
    config.directory = std::path::PathBuf::from(
        shellexpand::tilde(&config.directory.to_string_lossy()).to_string(),
    );

    Ok(config.clone())
}

/// Overwrite the realmlist file to point at the desired server
pub fn write_realmlist(
    game_folder: &std::path::Path,
    rel_path: &str,
    realmlist: &str,
) -> std::io::Result<()> {
    let realmlist_path = game_folder.join(rel_path);
    let realmlist_fmt = format!("set realmlist to {}", realmlist);
    std::fs::write(&realmlist_path, &realmlist_fmt).map_err(|e| {
        eprintln!(
            "Realmlist not writable, check path: {}",
            realmlist_path.display()
        );
        e
    })?;
    println!("Realmlist set to:\n\t{}", realmlist_fmt);
    Ok(())
}

/// Verifies the integrity of a game installation by checking for required files and directories.
/// @todo: implement other client versions besides 3.3.5a
#[allow(dead_code)]
pub fn verify_game_integrity(game_dir: &std::path::Path) -> Result<bool, std::io::Error> {
    let required_files = ["Battle.net.dll", "Data/lichking.MPQ", "Data/patch-3.MPQ"];
    let required_dirs = ["Data"];

    // Check required directories
    for dir in required_dirs.iter() {
        let dir_path = game_dir.join(dir);
        if !dir_path.is_dir() {
            println!("Missing required directory: {}", dir);
            return Ok(false);
        }
    }

    // Check required files
    for file in required_files.iter() {
        let file_path = game_dir.join(file);
        if !file_path.is_file() {
            println!("Missing required file: {}", file);
            return Ok(false);
        }
    }

    Ok(true)
}

fn clear_cache(game_dir: &std::path::Path) -> std::io::Result<()> {
    let cache_dir = game_dir.join("Cache");
    match cache_dir.try_exists() {
        Ok(true) => {
            println!("Cache directory exists, removing...");
            std::fs::remove_dir_all(&cache_dir)?;
        }
        Ok(false) => {
            println!("Cache directory does not exist, nothing to remove.");
        }
        Err(e) => {
            eprintln!("Failed to check if cache directory exists: {}", e);
            return Err(e);
        }
    }
    Ok(())
}

/// Launches the game executable
/// On Linux, it supports launching the game using a custom command or Wine with a local `.wine` configuration.
/// On Windows, it directly runs the executable.
pub fn launch(config: &Config) -> std::io::Result<()> {
    // Clear cache if specified
    if config.clear_cache == Some(true) {
        clear_cache(&config.directory)?;
    }

    // Verify executable exists
    let executable_path = config.directory.join(
        &config.executable
            .clone()
    );
    if !executable_path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Executable not found: {}", executable_path.display()),
        ));
    }

    // Write username and password
    if let Some(user) = &config.username {
        println!("Account Name:\n\t{}", user);
    }
    if let Some(password) = &config.password {
        println!("Password:\n\t{}", password);
        clipboard::to_clipboard(&password).unwrap();
    }

    // Launch the game
    match std::env::consts::OS {
        "linux" => {
            let command: String = config.launch_cmd.clone().unwrap_or_else(|| {
                let wine_prefix_path = config.directory.join(".wine");
                format!(
                    "WINEPREFIX=\"{}\" wine \"{}\"",
                    wine_prefix_path.to_string_lossy(),
                    executable_path.to_string_lossy()
                )
            });
            println!("Launching with command:\n\t{}", command);
            std::process::Command::new("setsid")
                .arg("sh")
                .arg("-c")
                .arg(command)
                .spawn()?;
        }
        "windows" => {
            std::process::Command::new(executable_path).spawn()?;
        }
        _ => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Unsupported platform",
            ));
        }
    }
    Ok(())
}
