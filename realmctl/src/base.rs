use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Role assigned to each file/directory in the WoW client
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FileRole {
    /// Main game executable
    Executable,
    /// Base immutable data files (common*.MPQ, etc)
    BaseData,
    /// Mutable data files (patches, custom content)
    MutableData,
    /// User-created media (screenshots, videos)
    UserMedia,
    /// User configuration (WTF folder, addons config)
    UserConfig,
    /// Temporary files that can be deleted (Cache, Logs, Errors)
    Ephemeral,
    /// Other files not specifically classified
    Other,
}

/// Manifest describing a WoW base installation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseManifest {
    /// Profile name used for this base
    pub profile: String,
    /// Base directory path
    pub base_path: PathBuf,
    /// Timestamp when base was created
    pub created_at: String,
    /// Map of relative paths to their roles
    pub file_roles: HashMap<String, FileRole>,
    /// Checksums for immutable files (BaseData)
    pub checksums: HashMap<String, String>,
    /// Version/notes
    pub version: Option<String>,
}

/// Profile defining rules for a WoW version
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub version: String,
    pub required_files: Vec<String>,
    pub required_dirs: Vec<String>,
    pub role_rules: Vec<RoleRule>,
    pub warnings: Vec<WarningRule>,
}

/// Rule for assigning roles to files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleRule {
    pub pattern: String,
    pub role: FileRole,
    #[serde(default)]
    pub is_regex: bool,
}

/// Warning rule for problematic paths
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarningRule {
    pub pattern: String,
    pub message: String,
}

impl Profile {
    /// Get the builtin Chromie 3.3.5a profile
    pub fn chromie_335a() -> Self {
        Profile {
            name: "chromie-3.3.5a".to_string(),
            version: "3.3.5a".to_string(),
            required_files: vec![
                "Wow.exe".to_string(),
                "Data/common.MPQ".to_string(),
                "Data/patch.MPQ".to_string(),
                "Data/lichking.MPQ".to_string(),
            ],
            required_dirs: vec!["Data".to_string()],
            role_rules: vec![
                RoleRule {
                    pattern: "Wow.exe".to_string(),
                    role: FileRole::Executable,
                    is_regex: false,
                },
                RoleRule {
                    pattern: r"^Data/common.*\.MPQ$".to_string(),
                    role: FileRole::BaseData,
                    is_regex: true,
                },
                RoleRule {
                    pattern: r"^Data/expansion.*\.MPQ$".to_string(),
                    role: FileRole::BaseData,
                    is_regex: true,
                },
                RoleRule {
                    pattern: r"^Data/lichking.*\.MPQ$".to_string(),
                    role: FileRole::BaseData,
                    is_regex: true,
                },
                RoleRule {
                    pattern: r"^Data/patch.*\.MPQ$".to_string(),
                    role: FileRole::MutableData,
                    is_regex: true,
                },
                RoleRule {
                    pattern: r"^Screenshots($|/)".to_string(),
                    role: FileRole::UserMedia,
                    is_regex: true,
                },
                RoleRule {
                    pattern: r"^WTF($|/)".to_string(),
                    role: FileRole::UserConfig,
                    is_regex: true,
                },
                RoleRule {
                    pattern: r"^Interface($|/)".to_string(),
                    role: FileRole::UserConfig,
                    is_regex: true,
                },
                RoleRule {
                    pattern: r"^Cache($|/)".to_string(),
                    role: FileRole::Ephemeral,
                    is_regex: true,
                },
                RoleRule {
                    pattern: r"^Logs($|/)".to_string(),
                    role: FileRole::Ephemeral,
                    is_regex: true,
                },
                RoleRule {
                    pattern: r"^Errors($|/)".to_string(),
                    role: FileRole::Ephemeral,
                    is_regex: true,
                },
            ],
            warnings: vec![
                WarningRule {
                    pattern: "Cache".to_string(),
                    message: "Cache directory present in base - should be ephemeral".to_string(),
                },
                WarningRule {
                    pattern: "Logs".to_string(),
                    message: "Logs directory present in base - should be ephemeral".to_string(),
                },
                WarningRule {
                    pattern: "Errors".to_string(),
                    message: "Errors directory present in base - should be ephemeral".to_string(),
                },
            ],
        }
    }

    /// Get a builtin Vanilla 1.12 profile
    pub fn vanilla_112() -> Self {
        Profile {
            name: "vanilla-1.12".to_string(),
            version: "1.12".to_string(),
            required_files: vec![
                "WoW.exe".to_string(),
                "realmlist.wtf".to_string(),
            ],
            required_dirs: vec!["Data".to_string(), "WTF".to_string(), "Interface".to_string()],
            role_rules: vec![
                RoleRule {
                    pattern: "WoW.exe".to_string(),
                    role: FileRole::Executable,
                    is_regex: false,
                },
                RoleRule {
                    pattern: r"^Data/.*\.MPQ$".to_string(),
                    role: FileRole::BaseData,
                    is_regex: true,
                },
                RoleRule {
                    pattern: r"^Data/patch.*\.MPQ$".to_string(),
                    role: FileRole::MutableData,
                    is_regex: true,
                },
                RoleRule {
                    pattern: r"^Screenshots($|/)".to_string(),
                    role: FileRole::UserMedia,
                    is_regex: true,
                },
                RoleRule {
                    pattern: r"^WTF($|/)".to_string(),
                    role: FileRole::UserConfig,
                    is_regex: true,
                },
                RoleRule {
                    pattern: r"^Interface($|/)".to_string(),
                    role: FileRole::UserConfig,
                    is_regex: true,
                },
                RoleRule {
                    pattern: r"^Logs($|/)".to_string(),
                    role: FileRole::Ephemeral,
                    is_regex: true,
                },
                RoleRule {
                    pattern: r"^Errors($|/)".to_string(),
                    role: FileRole::Ephemeral,
                    is_regex: true,
                },
                RoleRule {
                    pattern: r"^WDB($|/)".to_string(),
                    role: FileRole::Ephemeral,
                    is_regex: true,
                },
            ],
            warnings: vec![
                WarningRule {
                    pattern: "Logs".to_string(),
                    message: "Logs directory present in base - should be ephemeral".to_string(),
                },
                WarningRule {
                    pattern: "Errors".to_string(),
                    message: "Errors directory present in base - should be ephemeral".to_string(),
                },
            ],
        }
    }

    /// Verify the directory meets requirements
    pub fn verify_requirements(&self, base_dir: &Path) -> Result<()> {
        for file in &self.required_files {
            let path = base_dir.join(file);
            if !path.exists() {
                anyhow::bail!("Required file not found: {}", file);
            }
        }

        for dir in &self.required_dirs {
            let path = base_dir.join(dir);
            if !path.is_dir() {
                anyhow::bail!("Required directory not found: {}", dir);
            }
        }

        Ok(())
    }

    /// Check for warning conditions
    pub fn check_warnings(&self, base_dir: &Path) -> Vec<String> {
        let mut warnings = Vec::new();
        for warning in &self.warnings {
            let path = base_dir.join(&warning.pattern);
            if path.exists() {
                warnings.push(warning.message.clone());
            }
        }
        warnings
    }

    /// Classify a file path according to role rules
    pub fn classify_path(&self, rel_path: &str) -> FileRole {
        for rule in &self.role_rules {
            if rule.is_regex {
                if let Ok(re) = regex::Regex::new(&rule.pattern)
                    && re.is_match(rel_path) {
                        return rule.role.clone();
                    }
            } else if rel_path == rule.pattern
                || rel_path.starts_with(&format!("{}/", rule.pattern))
            {
                return rule.role.clone();
            }
        }
        FileRole::Other
    }
}

/// Scan a directory and build a manifest
pub fn scan_and_build_manifest(base_dir: &Path, profile: &Profile) -> Result<BaseManifest> {
    use std::time::SystemTime;

    let mut file_roles = HashMap::new();
    let mut checksums = HashMap::new();

    // Recursively scan directory
    scan_directory(base_dir, base_dir, profile, &mut file_roles, &mut checksums)?;

    let created_at = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_else(|_| std::time::Duration::new(0, 0))
        .as_secs()
        .to_string();

    Ok(BaseManifest {
        profile: profile.name.clone(),
        base_path: base_dir.to_path_buf(),
        created_at,
        file_roles,
        checksums,
        version: Some(profile.version.clone()),
    })
}

fn scan_directory(
    base_dir: &Path,
    current_dir: &Path,
    profile: &Profile,
    file_roles: &mut HashMap<String, FileRole>,
    checksums: &mut HashMap<String, String>,
) -> Result<()> {
    for entry in std::fs::read_dir(current_dir)? {
        let entry = entry?;
        let path = entry.path();
        let rel_path = path
            .strip_prefix(base_dir)
            .context("Failed to get relative path")?
            .to_string_lossy()
            .to_string();

        if path.is_dir() {
            // Classify directory
            let role = profile.classify_path(&rel_path);
            file_roles.insert(rel_path.clone(), role.clone());

            // Recursively scan subdirectories (skip ephemeral)
            if role != FileRole::Ephemeral {
                scan_directory(base_dir, &path, profile, file_roles, checksums)?;
            }
        } else if path.is_file() {
            let role = profile.classify_path(&rel_path);
            file_roles.insert(rel_path.clone(), role.clone());

            // Compute checksum for BaseData files
            if role == FileRole::BaseData
                && let Ok(hash) = compute_file_hash(&path) {
                    checksums.insert(rel_path, hash);
                }
        }
    }

    Ok(())
}

fn compute_file_hash(path: &Path) -> Result<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = crc32fast::Hasher::new();
    let mut buffer = [0u8; 8192];

    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    Ok(format!("{:08x}", hasher.finalize()))
}

/// Write manifest to disk
pub fn write_manifest(manifest: &BaseManifest, base_dir: &Path) -> Result<()> {
    let manifest_path = base_dir.join("manifest.toml");
    let toml_string = toml::to_string_pretty(manifest)?;
    std::fs::write(manifest_path, toml_string)?;
    Ok(())
}

/// Load manifest from disk
pub fn load_manifest(base_dir: &Path) -> Result<BaseManifest> {
    let manifest_path = base_dir.join("manifest.toml");
    let content = std::fs::read_to_string(manifest_path)?;
    let manifest: BaseManifest = toml::from_str(&content)?;
    Ok(manifest)
}
