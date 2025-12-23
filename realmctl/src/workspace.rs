use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::base::{BaseManifest, FileRole};

/// Sharing strategy for workspace files
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SharingStrategy {
    /// Shared globally across all workspaces
    Global,
    /// Shared per base (all workspaces using the same base)
    Base,
    /// Unique to this workspace
    Workspace,
}

/// Workspace configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub name: String,
    pub base_name: String,
    pub base_path: PathBuf,
    pub workspace_path: PathBuf,
    pub created_at: String,
    pub sharing_rules: HashMap<String, SharingStrategy>,
}

/// Default sharing rules
pub fn default_sharing_rules() -> HashMap<String, SharingStrategy> {
    let mut rules = HashMap::new();
    rules.insert("screenshots".to_string(), SharingStrategy::Global);
    rules.insert("interface/addons".to_string(), SharingStrategy::Base);
    rules.insert("wtf".to_string(), SharingStrategy::Workspace);
    rules
}

/// Create a new workspace
pub fn create_workspace(
    name: &str,
    base_path: &Path,
    workspace_root: &Path,
    sharing_rules: HashMap<String, SharingStrategy>,
) -> Result<WorkspaceConfig> {
    use std::time::SystemTime;

    // Load base manifest
    let base_manifest = crate::base::load_manifest(base_path)
        .context("Failed to load base manifest - is this a valid base?")?;

    // Create workspace directory
    let workspace_path = workspace_root.join(name);
    if workspace_path.exists() {
        anyhow::bail!("Workspace already exists: {}", workspace_path.display());
    }
    std::fs::create_dir_all(&workspace_path)?;

    // Create shared directories based on strategy
    let global_shared_dir = workspace_root.join(".shared").join("global");
    let per_base_shared_dir = workspace_root.join(".shared").join(&base_manifest.profile);

    std::fs::create_dir_all(&global_shared_dir)?;
    std::fs::create_dir_all(&per_base_shared_dir)?;

    // Link files according to manifest and sharing rules
    link_workspace_files(
        base_path,
        &workspace_path,
        &global_shared_dir,
        &per_base_shared_dir,
        &base_manifest,
        &sharing_rules,
    )?;

    let created_at = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_string();

    let config = WorkspaceConfig {
        name: name.to_string(),
        base_name: base_manifest.profile.clone(),
        base_path: base_path.to_path_buf(),
        workspace_path: workspace_path.clone(),
        created_at,
        sharing_rules,
    };

    // Write workspace config
    let config_path = workspace_path.join("workspace.toml");
    let toml_string = toml::to_string_pretty(&config)?;
    std::fs::write(config_path, toml_string)?;

    Ok(config)
}

fn link_workspace_files(
    base_path: &Path,
    workspace_path: &Path,
    global_shared_dir: &Path,
    per_base_shared_dir: &Path,
    manifest: &BaseManifest,
    sharing_rules: &HashMap<String, SharingStrategy>,
) -> Result<()> {
    // First pass: create shared links for directories
    // Process directories from shallowest to deepest to ensure parents are created first
    let mut dir_entries: Vec<_> = manifest
        .file_roles
        .iter()
        .filter(|(_, role)| matches!(role, FileRole::UserMedia | FileRole::UserConfig))
        .filter(|(rel_path, _)| base_path.join(rel_path).is_dir())
        .collect();

    // Sort by path depth (number of slashes)
    dir_entries.sort_by_key(|(rel_path, _)| rel_path.matches('/').count());

    let mut processed_shared_dirs: Vec<String> = Vec::new();

    for (rel_path, role) in dir_entries {
        let workspace_file = workspace_path.join(rel_path);

        // Skip if a parent directory is already shared
        let should_skip = processed_shared_dirs
            .iter()
            .any(|shared_dir| rel_path.starts_with(&format!("{}/", shared_dir)));

        if should_skip {
            continue;
        }

        match role {
            FileRole::UserMedia => {
                // Check sharing strategy for media directories
                let strategy = determine_strategy(rel_path, sharing_rules, SharingStrategy::Global);
                if !matches!(strategy, SharingStrategy::Workspace) {
                    create_shared_link(
                        rel_path,
                        &workspace_file,
                        global_shared_dir,
                        per_base_shared_dir,
                        strategy,
                    )?;
                    processed_shared_dirs.push(rel_path.to_string());
                } else {
                    create_shared_link(
                        rel_path,
                        &workspace_file,
                        global_shared_dir,
                        per_base_shared_dir,
                        strategy,
                    )?;
                }
            }
            FileRole::UserConfig => {
                // Check sharing strategy for config directories
                let strategy =
                    determine_strategy(rel_path, sharing_rules, SharingStrategy::Workspace);
                if !matches!(strategy, SharingStrategy::Workspace) {
                    create_shared_link(
                        rel_path,
                        &workspace_file,
                        global_shared_dir,
                        per_base_shared_dir,
                        strategy,
                    )?;
                    processed_shared_dirs.push(rel_path.to_string());
                } else {
                    create_shared_link(
                        rel_path,
                        &workspace_file,
                        global_shared_dir,
                        per_base_shared_dir,
                        strategy,
                    )?;
                }
            }
            _ => {}
        }
    }

    // Second pass: create files and other directories
    for (rel_path, role) in &manifest.file_roles {
        let base_file = base_path.join(rel_path);
        let workspace_file = workspace_path.join(rel_path);

        // Skip if already handled in first pass
        if matches!(role, FileRole::UserMedia | FileRole::UserConfig) && base_file.is_dir() {
            continue;
        }

        // Ensure parent directory exists in workspace
        // But don't create it if any ancestor should be a symlink
        if let Some(parent) = workspace_file.parent()
            && parent != workspace_path {
                // Check if any ancestor should be a shared link
                let mut should_create = true;
                let mut current = parent;
                while current != workspace_path {
                    // Check if this directory exists and is a symlink
                    if current.read_link().is_ok() {
                        should_create = false;
                        break;
                    }
                    // Check if this directory should be a shared directory
                    if let Ok(rel) = current.strip_prefix(workspace_path) {
                        let rel_str = rel.to_string_lossy().to_string();
                        // Check if this path matches a sharing rule
                        for key in sharing_rules.keys() {
                            if &rel_str == key || rel_str.starts_with(&format!("{}/", key)) {
                                should_create = false;
                                break;
                            }
                        }
                    }
                    if !should_create {
                        break;
                    }
                    if let Some(p) = current.parent() {
                        current = p;
                    } else {
                        break;
                    }
                }
                if should_create && !parent.exists() {
                    std::fs::create_dir_all(parent)?;
                }
            }

        match role {
            FileRole::BaseData | FileRole::Executable => {
                // Hard link immutable files from base
                if base_file.is_file() && !workspace_file.exists() {
                    std::fs::hard_link(&base_file, &workspace_file)
                        .or_else(|_| {
                            // Fallback to symlink if hard link fails
                            #[cfg(unix)]
                            std::os::unix::fs::symlink(&base_file, &workspace_file)?;
                            #[cfg(windows)]
                            std::os::windows::fs::symlink_file(&base_file, &workspace_file)?;
                            Ok::<(), std::io::Error>(())
                        })
                        .with_context(|| format!("Failed to link {}", rel_path))?;
                }
            }
            FileRole::MutableData => {
                // Copy mutable data to workspace
                if base_file.is_file() && !workspace_file.exists() {
                    std::fs::copy(&base_file, &workspace_file)?;
                }
            }
            FileRole::Ephemeral => {
                // Create empty directories for ephemeral content
                if base_file.is_dir() && !workspace_file.exists() {
                    std::fs::create_dir_all(&workspace_file)?;
                }
            }
            FileRole::Other => {
                // Copy other files
                if base_file.is_file() && !workspace_file.exists() {
                    std::fs::copy(&base_file, &workspace_file)?;
                }
            }
            _ => {}
        }
    }

    Ok(())
}

/// Repair shared directories and symlinks for a workspace
pub fn fix_workspace(workspace_path: &Path) -> Result<()> {
    println!("Verifying workspace: {}", workspace_path.display());

    // Load workspace config
    let config = load_workspace_config(workspace_path)?;

    // Determine workspace root (parent directory)
    let workspace_root = workspace_path
        .parent()
        .context("Failed to determine workspace root (parent directory missing)")?;

    let global_shared_dir = workspace_root.join(".shared").join("global");
    let per_base_shared_dir = workspace_root.join(".shared").join(&config.base_name);

    // Ensure shared roots exist
    if !global_shared_dir.exists() {
        println!("Creating missing global shared root: {}", global_shared_dir.display());
        std::fs::create_dir_all(&global_shared_dir)?;
    }
    if !per_base_shared_dir.exists() {
        println!("Creating missing base shared root: {}", per_base_shared_dir.display());
        std::fs::create_dir_all(&per_base_shared_dir)?;
    }

    // Load base manifest so we can find the paths expected to be shared
    let base_manifest = crate::base::load_manifest(&config.base_path)
        .context("Failed to load base manifest for workspace")?;
    let base_path = &config.base_path;

    // Build list of candidate directories (like in link_workspace_files)
    let mut dir_entries: Vec<_> = base_manifest
        .file_roles
        .iter()
        .filter(|(_, role)| matches!(role, crate::base::FileRole::UserMedia | crate::base::FileRole::UserConfig))
        .filter(|(rel_path, _)| base_path.join(rel_path).is_dir())
        .collect();

    // Sort by path depth
    dir_entries.sort_by_key(|(rel_path, _)| rel_path.matches('/').count());

    for (rel_path, role) in dir_entries {
        let ws_file = workspace_path.join(rel_path);

        let strategy = match role {
            crate::base::FileRole::UserMedia =>
                determine_strategy(rel_path, &config.sharing_rules, SharingStrategy::Global),
            crate::base::FileRole::UserConfig =>
                determine_strategy(rel_path, &config.sharing_rules, SharingStrategy::Workspace),
            _ => SharingStrategy::Workspace,
        };

        match strategy {
            SharingStrategy::Workspace => {
                // Should be a real directory inside workspace
                if ws_file.exists() {
                    if ws_file.read_link().is_ok() {
                        println!("⚠ Expected directory but found a symlink at {}. Leaving as-is.", ws_file.display());
                    } else if ws_file.is_dir() {
                        // OK
                    } else {
                        println!("⚠ Expected directory at {}, but found a file. Leaving as-is.", ws_file.display());
                    }
                } else {
                    println!("Creating missing workspace directory: {}", ws_file.display());
                    std::fs::create_dir_all(&ws_file)?;
                }
            }
            SharingStrategy::Global | SharingStrategy::Base => {
                // Expected to be a symlink to the global or base shared dir
                let target_base = match strategy {
                    SharingStrategy::Global => &global_shared_dir,
                    SharingStrategy::Base => &per_base_shared_dir,
                    _ => unreachable!(),
                };
                let target = target_base.join(rel_path);

                // If workspace path exists
                // Use symlink_metadata to detect dangling symlinks as well
                match std::fs::symlink_metadata(&ws_file) {
                    Ok(meta) => {
                        if meta.file_type().is_symlink() {
                            // Existing symlink (possibly dangling)
                            if let Ok(link_target) = ws_file.read_link() {
                                // Resolve relative links
                                let resolved = if link_target.is_absolute() {
                                    link_target
                                } else {
                                    ws_file.parent().unwrap_or_else(|| Path::new(".")).join(link_target)
                                };

                                if resolved.exists() {
                                    // All good
                                } else {
                                    // Target missing: recreate target directory
                                    println!("Target missing for symlink {} -> {}. Recreating {}.", ws_file.display(), resolved.display(), target.display());
                                    std::fs::create_dir_all(&target)?;
                                }
                            } else {
                                // Shouldn't happen, but treat as dangling; recreate target
                                println!("Dangling symlink detected at {}. Recreating target {}.", ws_file.display(), target.display());
                                std::fs::create_dir_all(&target)?;
                            }
                        } else {
                            // Not a symlink - user replaced symlink with real directory or file
                            println!("⚠ Detected real file/directory at {} which seems to replace an expected symlink. Will NOT overwrite or remove user data.", ws_file.display());
                        }
                    }
                    Err(_) => {
                        // Path doesn't exist - create target and symlink
                        if !target.exists() {
                            println!("Creating missing target shared directory: {}", target.display());
                            std::fs::create_dir_all(&target)?;
                        }
                        // Ensure parent exists
                        if let Some(parent) = ws_file.parent() {
                            if !parent.exists() {
                                std::fs::create_dir_all(parent)?;
                            }
                        }

                        // It's possible that creating the target (under the per-base/global shared dir)
                        // made the corresponding path accessible via an existing parent symlink in the workspace.
                        // If the workspace path now exists, do not attempt to create another symlink (would EEXIST).
                        if ws_file.exists() {
                            #[cfg(test)]
                            println!("  -> workspace path {} already exists after creating target, skipping symlink", ws_file.display());
                            continue;
                        }

                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::symlink;
                            println!("Creating symlink: {} -> {}", ws_file.display(), target.display());
                            symlink(&target, &ws_file)?;
                        }
                        #[cfg(windows)]
                        {
                            use std::os::windows::fs::symlink_dir;
                            println!("Creating symlink: {} -> {}", ws_file.display(), target.display());
                            symlink_dir(&target, &ws_file)?;
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

fn determine_strategy(
    rel_path: &str,
    sharing_rules: &HashMap<String, SharingStrategy>,
    default: SharingStrategy,
) -> SharingStrategy {
    // Normalize path and try to match against sharing rule keys.
    // We match if any path component equals a key (so keys like "addons" match "Interface/AddOns"),
    // or if the path starts with the key (for root-level keys like "Screenshots").
    let normalized_path = rel_path.to_lowercase();
    let components: Vec<&str> = normalized_path.split('/').collect();

    for (key, strategy) in sharing_rules {
        let normalized_key = key.to_lowercase();

        if normalized_path == normalized_key
            || normalized_path.starts_with(&format!("{normalized_key}/"))
            || components.iter().any(|c| *c == normalized_key)
        {
            return strategy.clone();
        }
    }

    default
}

fn create_shared_link(
    rel_path: &str,
    workspace_file: &Path,
    global_shared_dir: &Path,
    per_base_shared_dir: &Path,
    strategy: SharingStrategy,
) -> Result<()> {
    if workspace_file.exists() {
        return Ok(());
    }

    let target = match strategy {
        SharingStrategy::Global => global_shared_dir.join(rel_path),
        SharingStrategy::Base => per_base_shared_dir.join(rel_path),
        SharingStrategy::Workspace => {
            // For workspace-specific, just create the directory in place
            if !workspace_file.exists() {
                std::fs::create_dir_all(workspace_file)?;
            }
            #[cfg(test)]
            println!("  -> created workspace-specific directory");
            return Ok(());
        }
    };

    // Ensure target directory exists (create it if it doesn't)
    if !target.exists() {
        std::fs::create_dir_all(&target)?;
        #[cfg(test)]
        println!("  -> created target directory: {}", target.display());
    }

    // Create symlink
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        #[cfg(test)]
        println!(
            "  -> creating symlink: {} -> {}",
            workspace_file.display(),
            target.display()
        );
        symlink(&target, workspace_file)
            .with_context(|| format!("Failed to create symlink for {}", rel_path))?;
    }

    #[cfg(windows)]
    {
        use std::os::windows::fs::{symlink_dir, symlink_file};
        if target.is_dir() {
            symlink_dir(&target, workspace_file)?;
        } else {
            symlink_file(&target, workspace_file)?;
        }
    }

    Ok(())
}

/// Load workspace config
pub fn load_workspace_config(workspace_path: &Path) -> Result<WorkspaceConfig> {
    let config_path = workspace_path.join("workspace.toml");
    let content = std::fs::read_to_string(config_path)?;
    let config: WorkspaceConfig = toml::from_str(&content)?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::{Profile, write_manifest, scan_and_build_manifest};
    use std::fs;
    use tempfile::TempDir;

    /// Helper function to print directory tree for debugging
    #[allow(dead_code)]
    pub fn print_dir_tree(path: &Path) {
        println!("\nDirectory tree under {}:", path.display());
        let output = std::process::Command::new("tree")
            .arg("-a") // include hidden files
            .arg("-L")
            .arg("4") // limit depth to 4 levels
            .arg(path)
            .output()
            .expect("failed to execute tree");
        println!("{}", String::from_utf8_lossy(&output.stdout));
    }

    /// Helper function to create a mock WoW base directory for testing
    fn create_mock_base(base_dir: &Path, profile: &Profile) -> Result<()> {
        // Create required directories
        fs::create_dir_all(base_dir.join("Data"))?;
        fs::create_dir_all(base_dir.join("Screenshots"))?;
        fs::create_dir_all(base_dir.join("WTF"))?;
        fs::create_dir_all(base_dir.join("Interface/AddOns"))?;
        fs::create_dir_all(base_dir.join("Cache"))?;

        // Create required files
        fs::write(base_dir.join("Wow.exe"), b"mock executable")?;
        fs::write(base_dir.join("Data/common.MPQ"), b"mock data file")?;
        fs::write(base_dir.join("Data/patch.MPQ"), b"mock patch file")?;
        fs::write(base_dir.join("Data/lichking.MPQ"), b"mock expansion data")?;

        // Create some user data files
        fs::write(
            base_dir.join("Screenshots/WoWScrnShot_001.jpg"),
            b"mock screenshot",
        )?;

        // Additional user config and addons
        fs::write(base_dir.join("WTF/Config.wtf"), b"mock config")?;
        fs::create_dir_all(base_dir.join("Interface/AddOns/SomeAddon"))?;
        fs::write(
            base_dir.join("Interface/AddOns/SomeAddon/SomeAddon.toc"),
            b"mock addon",
        )?;

        // Create manifest
        let manifest = crate::base::scan_and_build_manifest(base_dir, profile)?;
        write_manifest(&manifest, base_dir)?;

        Ok(())
    }

    fn create_mock_base_112(base_dir: &Path, profile: &Profile) -> Result<()> {
        // Create required directories for 1.12 layout
        fs::create_dir_all(base_dir.join("Data"))?;
        fs::create_dir_all(base_dir.join("Screenshots"))?;
        fs::create_dir_all(base_dir.join("WTF/Account"))?;
        fs::create_dir_all(base_dir.join("Interface/AddOns"))?;
        fs::create_dir_all(base_dir.join("Logs"))?;
        fs::create_dir_all(base_dir.join("WDB"))?;

        // Create required files
        fs::write(base_dir.join("WoW.exe"), b"mock executable")?;
        fs::write(base_dir.join("realmlist.wtf"), b"mock realmlist")?;
        fs::write(base_dir.join("Data/base.MPQ"), b"mock data file")?;
        fs::write(base_dir.join("Data/dbc.MPQ"), b"mock data file")?;
        fs::write(base_dir.join("Data/interface.MPQ"), b"mock data file")?;
        fs::write(base_dir.join("Data/patch.MPQ"), b"mock patch file")?;
        fs::write(base_dir.join("Data/patch-2.MPQ"), b"mock patch file")?;

        // Additional user config and addons
        fs::write(base_dir.join("WTF/Config.wtf"), b"mock config")?;
        fs::create_dir_all(base_dir.join("Interface/AddOns/SomeAddon"))?;
        fs::write(
            base_dir.join("Interface/AddOns/SomeAddon/SomeAddon.toc"),
            b"mock addon",
        )?;

        let manifest = crate::base::scan_and_build_manifest(base_dir, profile)?;
        write_manifest(&manifest, base_dir)?;

        Ok(())
    }

    #[test]
    fn test_fix_recreates_missing_shared_dirs() -> Result<()> {
        use tempfile::TempDir;

        let tmp = TempDir::new()?;
        let base_dir = tmp.path().join("base");
        fs::create_dir_all(&base_dir)?;

        let profile = Profile::chromie_335a();
        create_mock_base(&base_dir, &profile)?;
        let manifest = scan_and_build_manifest(&base_dir, &profile)?;
        write_manifest(&manifest, &base_dir)?;

        // Create workspace root and workspace
        let ws_root = tmp.path().join("workspaces");
        fs::create_dir_all(&ws_root)?;
        let config = create_workspace("ws1", &base_dir, &ws_root, default_sharing_rules())?;

        // Remove the target of a global shared dir (Screenshots)
        let global_shared = ws_root.join(".shared").join("global").join("Screenshots");
        if global_shared.exists() {
            fs::remove_dir_all(&global_shared)?;
        }
        assert!(!global_shared.exists());

        // Run fix
        fix_workspace(&config.workspace_path)?;

        // Target should be recreated
        assert!(global_shared.exists());
        Ok(())
    }

    #[test]
    fn test_fix_warns_on_replaced_symlink_and_preserves_data() -> Result<()> {
        use tempfile::TempDir;

        let tmp = TempDir::new()?;
        let base_dir = tmp.path().join("base2");
        fs::create_dir_all(&base_dir)?;

        let profile = Profile::chromie_335a();
        create_mock_base(&base_dir, &profile)?;
        let manifest = scan_and_build_manifest(&base_dir, &profile)?;
        write_manifest(&manifest, &base_dir)?;

        // Create workspace
        let ws_root = tmp.path().join("workspaces2");
        fs::create_dir_all(&ws_root)?;
        let config = create_workspace("ws2", &base_dir, &ws_root, default_sharing_rules())?;

        // Replace the Screenshots symlink inside workspace with a real directory containing user data
        let ws_screenshots = config.workspace_path.join("Screenshots");
        if ws_screenshots.exists() {
            fs::remove_file(&ws_screenshots).ok();
            fs::remove_dir_all(&ws_screenshots).ok();
        }
        fs::create_dir_all(&ws_screenshots)?;
        fs::write(ws_screenshots.join("user.jpg"), b"user data")?;

        // Run fix
        fix_workspace(&config.workspace_path)?;

        // Ensure we didn't remove the user's file and we didn't replace the directory with a symlink
        assert!(ws_screenshots.exists());
        assert!(ws_screenshots.join("user.jpg").exists());
        assert!(ws_screenshots.read_link().is_err());

        Ok(())
    }


    #[test]
    fn test_workspace_creation_basic() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_dir = temp_dir.path().join("base");
        let workspace_root = temp_dir.path().join("workspaces");

        // Create mock base
        fs::create_dir(&base_dir)?;
        let profile = Profile::chromie_335a();
        create_mock_base(&base_dir, &profile)?;

        // Create workspace
        let sharing_rules = default_sharing_rules();
        let workspace_config = create_workspace(
            "test_workspace",
            &base_dir,
            &workspace_root,
            sharing_rules.clone(),
        )?;

        // Verify workspace was created
        assert!(workspace_config.workspace_path.exists());
        assert_eq!(workspace_config.name, "test_workspace");
        assert_eq!(workspace_config.base_name, profile.name);
        assert_eq!(workspace_config.sharing_rules, sharing_rules);

        // Verify workspace config file exists
        let config_path = workspace_config.workspace_path.join("workspace.toml");
        assert!(config_path.exists());

        // Verify directory structure
        assert!(workspace_config.workspace_path.join("Data").exists());
        assert!(workspace_config.workspace_path.join("Screenshots").exists());
        assert!(workspace_config.workspace_path.join("WTF").exists());

        Ok(())
    }

    #[test]
    fn test_workspace_creation_112() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_dir = temp_dir.path().join("base112");
        let workspace_root = temp_dir.path().join("workspaces112");

        // Create mock base
        fs::create_dir(&base_dir)?;
        let profile = Profile::vanilla_112();
        create_mock_base_112(&base_dir, &profile)?;

        // Create workspace
        let sharing_rules = default_sharing_rules();
        let workspace_config = create_workspace(
            "test_workspace_112",
            &base_dir,
            &workspace_root,
            sharing_rules.clone(),
        )?;

        // Verify workspace was created
        assert!(workspace_config.workspace_path.exists());
        assert_eq!(workspace_config.base_name, profile.name);

        // Verify directory structure
        assert!(workspace_config.workspace_path.join("Data").exists());
        assert!(workspace_config.workspace_path.join("WTF").exists());
        assert!(workspace_config.workspace_path.join("Interface").exists());
        assert!(workspace_config.workspace_path.join("WDB").exists());

        Ok(())
    }

    #[test]
    fn test_multiple_workspace_creation() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_dir = temp_dir.path().join("base");
        let workspace_root = temp_dir.path().join("workspaces");

        // Create mock base
        fs::create_dir(&base_dir)?;
        let profile = Profile::chromie_335a();
        create_mock_base(&base_dir, &profile)?;

        let sharing_rules = default_sharing_rules();

        // Create multiple workspaces
        let workspace1 = create_workspace(
            "workspace1",
            &base_dir,
            &workspace_root,
            sharing_rules.clone(),
        )?;
        let workspace2 = create_workspace(
            "workspace2",
            &base_dir,
            &workspace_root,
            sharing_rules.clone(),
        )?;
        let workspace3 = create_workspace(
            "workspace3",
            &base_dir,
            &workspace_root,
            sharing_rules.clone(),
        )?;

        // Verify all workspaces exist
        assert!(workspace1.workspace_path.exists());
        assert!(workspace2.workspace_path.exists());
        assert!(workspace3.workspace_path.exists());

        // Verify they're separate directories
        assert_ne!(workspace1.workspace_path, workspace2.workspace_path);
        assert_ne!(workspace2.workspace_path, workspace3.workspace_path);
        assert_ne!(workspace1.workspace_path, workspace3.workspace_path);

        Ok(())
    }

    #[test]
    fn test_duplicate_workspace_fails() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_dir = temp_dir.path().join("base");
        let workspace_root = temp_dir.path().join("workspaces");

        // Create mock base
        fs::create_dir(&base_dir)?;
        let profile = Profile::chromie_335a();
        create_mock_base(&base_dir, &profile)?;

        let sharing_rules = default_sharing_rules();

        // Create first workspace
        create_workspace(
            "test_workspace",
            &base_dir,
            &workspace_root,
            sharing_rules.clone(),
        )?;

        // Attempt to create duplicate workspace
        let result = create_workspace("test_workspace", &base_dir, &workspace_root, sharing_rules);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Workspace already exists")
        );

        Ok(())
    }

    #[test]
    fn test_global_screenshots_sharing() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_dir = temp_dir.path().join("base");
        let workspace_root = temp_dir.path().join("workspaces");

        // Create mock base
        fs::create_dir(&base_dir)?;
        let profile = Profile::chromie_335a();
        create_mock_base(&base_dir, &profile)?;

        let sharing_rules = default_sharing_rules();

        // Create two workspaces
        let workspace1 = create_workspace(
            "workspace1",
            &base_dir,
            &workspace_root,
            sharing_rules.clone(),
        )?;
        let workspace2 = create_workspace(
            "workspace2",
            &base_dir,
            &workspace_root,
            sharing_rules.clone(),
        )?;

        // Get screenshot paths
        let screenshot1 = workspace1.workspace_path.join("Screenshots");
        let screenshot2 = workspace2.workspace_path.join("Screenshots");

        // Both should exist
        assert!(screenshot1.exists(), "Workspace1 Screenshots doesn't exist");
        assert!(screenshot2.exists(), "Workspace2 Screenshots doesn't exist");

        // Both should be symlinks (on Unix)
        #[cfg(unix)]
        {
            assert!(
                screenshot1.read_link().is_ok(),
                "Workspace1 Screenshots is not a symlink"
            );
            assert!(
                screenshot2.read_link().is_ok(),
                "Workspace2 Screenshots is not a symlink"
            );
        }

        // Get the global shared directory
        let global_screenshots = workspace_root.join(".shared/global/Screenshots");

        // Add a screenshot in workspace1
        fs::write(
            global_screenshots.join("test_screenshot.jpg"),
            b"test screenshot data",
        )?;

        // Verify it's accessible from both workspaces
        let screenshot1_file = screenshot1.join("test_screenshot.jpg");
        let screenshot2_file = screenshot2.join("test_screenshot.jpg");

        assert!(
            screenshot1_file.exists(),
            "Screenshot not accessible from workspace1"
        );
        assert!(
            screenshot2_file.exists(),
            "Screenshot not accessible from workspace2"
        );

        // Read content to verify it's the same file
        let content1 = fs::read(&screenshot1_file)?;
        let content2 = fs::read(&screenshot2_file)?;
        assert_eq!(
            content1, content2,
            "Screenshot content differs between workspaces"
        );
        assert_eq!(content1, b"test screenshot data");

        Ok(())
    }

    #[test]
    fn test_addons_key_shares_addons_and_icons_workspace_local() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_dir = temp_dir.path().join("base");
        let workspace_root = temp_dir.path().join("workspaces");

        // Create mock base
        fs::create_dir(&base_dir)?;
        let profile = Profile::chromie_335a();
        create_mock_base(&base_dir, &profile)?;

        // Add an Interface/icons directory (should remain workspace-local)
        fs::create_dir_all(base_dir.join("Interface/icons"))?;
        fs::write(base_dir.join("Interface/icons/icon.tga"), b"icon data")?;

        // Re-scan the base to include the newly added icons directory in the manifest
        let manifest = crate::base::scan_and_build_manifest(&base_dir, &profile)?;
        crate::base::write_manifest(&manifest, &base_dir)?;

        // Use default sharing rules (includes "addons" => Base)
        let sharing_rules = default_sharing_rules();

        // Create two workspaces
        let workspace1 = create_workspace(
            "workspace1",
            &base_dir,
            &workspace_root,
            sharing_rules.clone(),
        )?;
        let workspace2 = create_workspace(
            "workspace2",
            &base_dir,
            &workspace_root,
            sharing_rules.clone(),
        )?;

        // AddOns should be shared via the 'addons' key
        let addons1 = workspace1.workspace_path.join("Interface/AddOns");
        let addons2 = workspace2.workspace_path.join("Interface/AddOns");

        assert!(addons1.exists(), "Workspace1 AddOns doesn't exist");
        assert!(addons2.exists(), "Workspace2 AddOns doesn't exist");

        #[cfg(unix)]
        {
            let link1 = addons1.read_link()?;
            let link2 = addons2.read_link()?;
            assert_eq!(link1, link2, "AddOns are not sharing the same base directory");
            assert!(link1.to_string_lossy().contains(".shared"), "AddOns not in shared directory");
            assert!(link1
                .to_string_lossy()
                .contains(&workspace1.base_name.to_lowercase()),
                "AddOns not in base directory");
        }

        // Icons should be workspace-local directories (not symlinked)
        let icons1 = workspace1.workspace_path.join("Interface/icons");
        let icons2 = workspace2.workspace_path.join("Interface/icons");
        assert!(icons1.exists(), "Workspace1 icons doesn't exist");
        assert!(icons2.exists(), "Workspace2 icons doesn't exist");

        #[cfg(unix)]
        {
            assert!(icons1.read_link().is_err(), "icons should be real directories");
            assert!(icons2.read_link().is_err(), "icons should be real directories");
        }

        Ok(())
    }

    #[test]
    fn test_interface_addons_sharing() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_dir = temp_dir.path().join("base");
        let workspace_root = temp_dir.path().join("workspaces");

        // Create mock base
        fs::create_dir(&base_dir)?;
        let profile = Profile::chromie_335a();
        create_mock_base(&base_dir, &profile)?;

        // Add an Interface/icons directory (should remain workspace-local)
        fs::create_dir_all(base_dir.join("Interface/icons"))?;
        fs::write(base_dir.join("Interface/icons/icon.tga"), b"icon data")?;

        // Re-scan the base to include the newly added icons directory in the manifest
        let manifest = crate::base::scan_and_build_manifest(&base_dir, &profile)?;
        crate::base::write_manifest(&manifest, &base_dir)?;

        // Create sharing rules simulating: --share interface/addons=base
        let mut sharing_rules = default_sharing_rules();
        sharing_rules.insert("interface/addons".to_string(), SharingStrategy::Base);

        // Create two workspaces
        let workspace1 = create_workspace(
            "workspace1",
            &base_dir,
            &workspace_root,
            sharing_rules.clone(),
        )?;
        let workspace2 = create_workspace(
            "workspace2",
            &base_dir,
            &workspace_root,
            sharing_rules.clone(),
        )?;

        // AddOns should be shared via the 'interface/addons' key
        let addons1 = workspace1.workspace_path.join("Interface/AddOns");
        let addons2 = workspace2.workspace_path.join("Interface/AddOns");

        assert!(addons1.exists(), "Workspace1 AddOns doesn't exist");
        assert!(addons2.exists(), "Workspace2 AddOns doesn't exist");

        #[cfg(unix)]
        {
            let link1 = addons1.read_link()?;
            let link2 = addons2.read_link()?;
            assert_eq!(link1, link2, "AddOns are not sharing the same base directory");
            assert!(link1.to_string_lossy().contains(".shared"), "AddOns not in shared directory");
            assert!(link1
                .to_string_lossy()
                .contains(&workspace1.base_name.to_lowercase()),
                "AddOns not in base directory");
        }

        // Icons should be workspace-local directories (not symlinked)
        let icons1 = workspace1.workspace_path.join("Interface/icons");
        let icons2 = workspace2.workspace_path.join("Interface/icons");
        assert!(icons1.exists(), "Workspace1 icons doesn't exist");
        assert!(icons2.exists(), "Workspace2 icons doesn't exist");

        #[cfg(unix)]
        {
            assert!(icons1.read_link().is_err(), "icons should be real directories");
            assert!(icons2.read_link().is_err(), "icons should be real directories");
        }

        // Create a file in workspace1 icons and verify it is not visible in workspace2
        fs::write(icons1.join("local_icon.tga"), b"local icon")?;
        assert!(icons1.join("local_icon.tga").exists());
        assert!(!icons2.join("local_icon.tga").exists(), "icons change leaked across workspaces");

        Ok(())
    }

    #[test]
    fn test_per_base_addons_sharing() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_dir = temp_dir.path().join("base");
        let workspace_root = temp_dir.path().join("workspaces");

        // Create mock base
        fs::create_dir(&base_dir)?;
        let profile = Profile::chromie_335a();
        create_mock_base(&base_dir, &profile)?;

        let mut sharing_rules = default_sharing_rules();
        sharing_rules.insert("Interface/AddOns".to_string(), SharingStrategy::Base);

        // Create two workspaces with the same base
        let workspace1 = create_workspace(
            "workspace1",
            &base_dir,
            &workspace_root,
            sharing_rules.clone(),
        )?;
        let workspace2 = create_workspace(
            "workspace2",
            &base_dir,
            &workspace_root,
            sharing_rules.clone(),
        )?;

        // Get addon paths
        let addons1 = workspace1.workspace_path.join("Interface/AddOns");
        let addons2 = workspace2.workspace_path.join("Interface/AddOns");

        // Both should exist
        assert!(addons1.exists(), "Workspace1 AddOns doesn't exist");
        assert!(addons2.exists(), "Workspace2 AddOns doesn't exist");

        // Both should be symlinks pointing to base shared directory
        #[cfg(unix)]
        {
            let link1 = addons1.read_link()?;
            let link2 = addons2.read_link()?;

            // Both should point to the same base location
            assert_eq!(
                link1, link2,
                "AddOns are not sharing the same base directory"
            );

            // Verify it's in the base shared directory
            assert!(
                link1.to_string_lossy().contains(".shared"),
                "AddOns not in shared directory"
            );
            assert!(
                link1
                    .to_string_lossy()
                    .contains(&workspace1.base_name.to_lowercase()),
                "AddOns not in base directory"
            );
        }

        Ok(())
    }

    #[test]
    fn test_workspace_specific_wtf() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_dir = temp_dir.path().join("base");
        let workspace_root = temp_dir.path().join("workspaces");

        // Create mock base
        fs::create_dir(&base_dir)?;
        let profile = Profile::chromie_335a();
        create_mock_base(&base_dir, &profile)?;

        let sharing_rules = default_sharing_rules();

        // Create two workspaces
        let workspace1 = create_workspace(
            "workspace1",
            &base_dir,
            &workspace_root,
            sharing_rules.clone(),
        )?;
        let workspace2 = create_workspace(
            "workspace2",
            &base_dir,
            &workspace_root,
            sharing_rules.clone(),
        )?;

        // Get WTF paths
        let wtf1 = workspace1.workspace_path.join("WTF");
        let wtf2 = workspace2.workspace_path.join("WTF");

        // Both should exist
        assert!(wtf1.exists(), "Workspace1 WTF doesn't exist");
        assert!(wtf2.exists(), "Workspace2 WTF doesn't exist");

        // WTF should be workspace-specific (not symlinks to shared location)
        // Create files in each workspace's WTF directory
        fs::write(wtf1.join("workspace1_config.wtf"), b"workspace1 config")?;
        fs::write(wtf2.join("workspace2_config.wtf"), b"workspace2 config")?;

        // Verify files are separate
        assert!(wtf1.join("workspace1_config.wtf").exists());
        assert!(!wtf1.join("workspace2_config.wtf").exists());

        assert!(wtf2.join("workspace2_config.wtf").exists());
        assert!(!wtf2.join("workspace1_config.wtf").exists());

        Ok(())
    }

    #[test]
    fn test_base_data_files_linked() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_dir = temp_dir.path().join("base");
        let workspace_root = temp_dir.path().join("workspaces");

        // Create mock base
        fs::create_dir(&base_dir)?;
        let profile = Profile::chromie_335a();
        create_mock_base(&base_dir, &profile)?;

        let sharing_rules = default_sharing_rules();

        // Create workspace
        let workspace =
            create_workspace("test_workspace", &base_dir, &workspace_root, sharing_rules)?;

        // Verify base data files are linked (not copied)
        let base_exe = base_dir.join("Wow.exe");
        let workspace_exe = workspace.workspace_path.join("Wow.exe");

        assert!(workspace_exe.exists(), "Wow.exe not in workspace");

        // Check if they're the same inode (hard link) or symlink
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;

            // Try to get metadata for both files
            let base_metadata = fs::metadata(&base_exe)?;
            let workspace_metadata = fs::metadata(&workspace_exe)?;

            // Check if hard linked (same inode) or if workspace file is a symlink
            let is_hardlinked = base_metadata.ino() == workspace_metadata.ino();
            let is_symlinked = workspace_exe.read_link().is_ok();

            assert!(
                is_hardlinked || is_symlinked,
                "Executable should be hard-linked or symlinked, not copied"
            );
        }

        Ok(())
    }

    #[test]
    fn test_shared_directory_structure() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_dir = temp_dir.path().join("base");
        let workspace_root = temp_dir.path().join("workspaces");

        // Create mock base
        fs::create_dir(&base_dir)?;
        let profile = Profile::chromie_335a();
        create_mock_base(&base_dir, &profile)?;

        let sharing_rules = default_sharing_rules();

        // Create workspace
        create_workspace("test_workspace", &base_dir, &workspace_root, sharing_rules)?;

        // Verify shared directory structure exists
        let shared_dir = workspace_root.join(".shared");
        assert!(shared_dir.exists(), "Shared directory doesn't exist");

        let global_dir = shared_dir.join("global");
        assert!(global_dir.exists(), "Global shared directory doesn't exist");

        let per_base_dir = shared_dir.join(&profile.name);
        assert!(
            per_base_dir.exists(),
            "Base shared directory doesn't exist"
        );

        Ok(())
    }

    #[test]
    fn test_load_workspace_config() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_dir = temp_dir.path().join("base");
        let workspace_root = temp_dir.path().join("workspaces");

        // Create mock base
        fs::create_dir(&base_dir)?;
        let profile = Profile::chromie_335a();
        create_mock_base(&base_dir, &profile)?;

        let sharing_rules = default_sharing_rules();

        // Create workspace
        let created_config =
            create_workspace("test_workspace", &base_dir, &workspace_root, sharing_rules)?;

        // Load workspace config
        let loaded_config = load_workspace_config(&created_config.workspace_path)?;

        // Verify loaded config matches created config
        assert_eq!(loaded_config.name, created_config.name);
        assert_eq!(loaded_config.base_name, created_config.base_name);
        assert_eq!(loaded_config.base_path, created_config.base_path);
        assert_eq!(loaded_config.sharing_rules, created_config.sharing_rules);

        Ok(())
    }

    #[test]
    fn test_screenshot_global_accessibility_with_subdirs() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_dir = temp_dir.path().join("base");
        let workspace_root = temp_dir.path().join("workspaces");

        // Create mock base
        fs::create_dir(&base_dir)?;
        let profile = Profile::chromie_335a();
        create_mock_base(&base_dir, &profile)?;

        let sharing_rules = default_sharing_rules();

        // Create three workspaces
        let workspace1 = create_workspace(
            "workspace1",
            &base_dir,
            &workspace_root,
            sharing_rules.clone(),
        )?;
        let workspace2 = create_workspace(
            "workspace2",
            &base_dir,
            &workspace_root,
            sharing_rules.clone(),
        )?;
        let workspace3 = create_workspace(
            "workspace3",
            &base_dir,
            &workspace_root,
            sharing_rules.clone(),
        )?;

        // Create a subdirectory with files in the global shared screenshots
        let global_screenshots = workspace_root.join(".shared/global/Screenshots");
        let subdir = global_screenshots.join("2024-12");
        fs::create_dir_all(&subdir)?;
        fs::write(subdir.join("screenshot1.jpg"), b"screenshot 1")?;
        fs::write(subdir.join("screenshot2.jpg"), b"screenshot 2")?;
        fs::write(
            global_screenshots.join("root_screenshot.jpg"),
            b"root screenshot",
        )?;

        // Verify all workspaces can access all screenshots
        for (i, workspace) in [&workspace1, &workspace2, &workspace3].iter().enumerate() {
            let ws_screenshots = workspace.workspace_path.join("Screenshots");

            assert!(
                ws_screenshots.join("2024-12/screenshot1.jpg").exists(),
                "Workspace {} cannot access screenshot1",
                i + 1
            );
            assert!(
                ws_screenshots.join("2024-12/screenshot2.jpg").exists(),
                "Workspace {} cannot access screenshot2",
                i + 1
            );
            assert!(
                ws_screenshots.join("root_screenshot.jpg").exists(),
                "Workspace {} cannot access root_screenshot",
                i + 1
            );

            // Verify content
            let content = fs::read(ws_screenshots.join("2024-12/screenshot1.jpg"))?;
            assert_eq!(content, b"screenshot 1");
        }

        Ok(())
    }

    #[test]
    fn test_default_sharing_rules() {
        let rules = default_sharing_rules();

        // Verify expected sharing strategies
        assert_eq!(rules.get("screenshots"), Some(&SharingStrategy::Global));
        assert_eq!(rules.get("addons"), Some(&SharingStrategy::Base));
        assert_eq!(rules.get("wtf"), Some(&SharingStrategy::Workspace));
    }

    /// Helper function to calculate actual disk usage using du command
    fn get_disk_usage(path: &Path) -> Result<u64> {
        let output = std::process::Command::new("du")
            .arg("-sb") // -s for summary, -b for bytes
            .arg(path)
            .output()
            .context("Failed to run du command")?;

        if !output.status.success() {
            anyhow::bail!("du command failed");
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let size_str = output_str
            .split_whitespace()
            .next()
            .context("Failed to parse du output")?;
        let size = size_str
            .parse::<u64>()
            .context("Failed to parse size as u64")?;

        Ok(size)
    }

    /// Helper function to calculate directory size by summing file sizes (not accounting for hard links)
    fn get_apparent_size(path: &Path) -> Result<u64> {
        let output = std::process::Command::new("du")
            .arg("-sb")
            .arg("--apparent-size") // Show apparent size (logical size) not disk usage
            .arg(path)
            .output()
            .context("Failed to run du command")?;

        if !output.status.success() {
            anyhow::bail!("du command failed");
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let size_str = output_str
            .split_whitespace()
            .next()
            .context("Failed to parse du output")?;
        let size = size_str
            .parse::<u64>()
            .context("Failed to parse size as u64")?;

        Ok(size)
    }

    #[test]
    fn test_disk_space_efficiency_multiple_workspaces() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_dir = temp_dir.path().join("base");
        let workspace_root = temp_dir.path().join("workspaces");

        // Create mock base with larger files to better simulate real WoW installation
        fs::create_dir(&base_dir)?;
        let profile = Profile::chromie_335a();

        // Create directories
        fs::create_dir_all(base_dir.join("Data"))?;
        fs::create_dir_all(base_dir.join("Screenshots"))?;
        fs::create_dir_all(base_dir.join("WTF"))?;
        fs::create_dir_all(base_dir.join("Interface/AddOns"))?;
        fs::create_dir_all(base_dir.join("Cache"))?;

        // Create larger mock files to better simulate real WoW data
        // Real WoW common.MPQ is ~800MB, lichking.MPQ is ~3GB
        // We'll use 10MB files for testing (good balance between realism and test speed)
        let large_data = vec![0u8; 10_000_000]; // 10MB per file
        fs::write(base_dir.join("Wow.exe"), &large_data)?;
        fs::write(base_dir.join("Data/common.MPQ"), &large_data)?;
        fs::write(base_dir.join("Data/patch.MPQ"), &large_data)?;
        fs::write(base_dir.join("Data/lichking.MPQ"), &large_data)?;

        // Smaller user data files
        fs::write(
            base_dir.join("Screenshots/WoWScrnShot_001.jpg"),
            b"mock screenshot",
        )?;
        fs::write(base_dir.join("WTF/Config.wtf"), b"mock config")?;
        fs::create_dir_all(base_dir.join("Interface/AddOns/SomeAddon"))?;
        fs::write(
            base_dir.join("Interface/AddOns/SomeAddon/SomeAddon.toc"),
            b"mock addon",
        )?;

        // Create manifest
        let manifest = crate::base::scan_and_build_manifest(&base_dir, &profile)?;
        write_manifest(&manifest, &base_dir)?;

        println!("\n=== Base directory structure ===");
        print_dir_tree(&base_dir);

        // Get base directory size
        let base_size = get_disk_usage(&base_dir)?;
        println!(
            "\nBase directory actual disk usage: {} bytes ({:.2} KB)",
            base_size,
            base_size as f64 / 1024.0
        );

        let sharing_rules = default_sharing_rules();

        // Create 10 workspaces
        let num_workspaces = 10;
        let mut workspaces = Vec::new();

        for i in 1..=num_workspaces {
            let workspace = create_workspace(
                &format!("workspace{}", i),
                &base_dir,
                &workspace_root,
                sharing_rules.clone(),
            )?;
            workspaces.push(workspace);
        }

        println!("\n=== Workspace root structure (10 workspaces created) ===");
        print_dir_tree(&workspace_root);

        // Get total disk usage of all workspaces
        let total_workspace_usage = get_disk_usage(&workspace_root)?;
        println!(
            "\nTotal disk usage for {} workspaces: {} bytes ({:.2} KB)",
            num_workspaces,
            total_workspace_usage,
            total_workspace_usage as f64 / 1024.0
        );

        // Get apparent size (what it would be if we copied everything)
        let apparent_size = get_apparent_size(&workspace_root)?;
        println!(
            "Apparent size (if everything was copied): {} bytes ({:.2} KB)",
            apparent_size,
            apparent_size as f64 / 1024.0
        );

        // Calculate what the size would be if we naively copied everything
        // Each workspace would have a full copy of base data
        let naive_copy_size = base_size * num_workspaces;
        println!(
            "Naive copy size ({} × base): {} bytes ({:.2} KB)",
            num_workspaces,
            naive_copy_size,
            naive_copy_size as f64 / 1024.0
        );

        // Calculate efficiency
        let space_saved = naive_copy_size.saturating_sub(total_workspace_usage);
        let efficiency_percent = if naive_copy_size > 0 {
            (space_saved as f64 / naive_copy_size as f64) * 100.0
        } else {
            0.0
        };

        println!(
            "\nSpace saved: {} bytes ({:.2} KB)",
            space_saved,
            space_saved as f64 / 1024.0
        );
        println!("Efficiency: {:.1}% space saved", efficiency_percent);

        // The actual disk usage should be significantly less than naive copying
        // We expect:
        // - Base data files (MPQ, exe) are hard-linked, so they count once
        // - Shared directories (Screenshots, etc.) count once
        // - Only workspace-specific files (WTF configs, workspace.toml) are duplicated

        // With hard linking, the total usage should be roughly:
        // base_size + (num_workspaces * workspace_specific_overhead)
        // where workspace_specific_overhead is small (just WTF files + config)

        // Assert that we're using significantly less space than naive copying
        // With larger files, hard linking becomes more efficient
        // We should save at least 50% of space with hard linking
        // (Real WoW installations with GB-sized files would see >95% efficiency)
        assert!(
            efficiency_percent > 50.0,
            "Expected >50% space efficiency, got {:.1}%. \
             Actual usage: {} bytes, Naive copy: {} bytes. \
             Hard linking may not be working correctly.",
            efficiency_percent,
            total_workspace_usage,
            naive_copy_size
        );

        // The total usage should be much less than the naive copy size
        // With 10 workspaces, if hard linking works, we should use significantly less
        // than 10x the base size
        let usage_multiplier = total_workspace_usage as f64 / base_size as f64;
        assert!(
            usage_multiplier < (num_workspaces as f64 / 2.0),
            "Total workspace usage ({} bytes, {:.1}x base) should be less than {}x base size. \
             With {} workspaces, hard linking should keep usage well below naive duplication.",
            total_workspace_usage,
            usage_multiplier,
            num_workspaces as f64 / 2.0,
            num_workspaces
        );

        // Verify that we're using significantly less than naive copying
        // The ratio should be much better than 1:1
        let usage_ratio = total_workspace_usage as f64 / naive_copy_size as f64;
        assert!(
            usage_ratio < 0.5,
            "Usage ratio ({:.2}) should be less than 0.5. \
             We should use less than half the space of naive copying.",
            usage_ratio
        );

        // Verify that each workspace has the expected structure
        for workspace in &workspaces {
            assert!(workspace.workspace_path.join("Data").exists());
            assert!(workspace.workspace_path.join("Screenshots").exists());
            assert!(workspace.workspace_path.join("WTF").exists());
            assert!(workspace.workspace_path.join("Wow.exe").exists());
            assert!(workspace.workspace_path.join("workspace.toml").exists());
        }

        println!("\n✓ Disk space efficiency test passed!");
        println!(
            "  {} workspaces use only {:.1}x the space of 1 base (instead of {}x)",
            num_workspaces, usage_multiplier, num_workspaces
        );
        println!(
            "  Space saved compared to naive copying: {:.1}%",
            efficiency_percent
        );

        Ok(())
    }

    #[test]
    fn test_hard_link_verification() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_dir = temp_dir.path().join("base");
        let workspace_root = temp_dir.path().join("workspaces");

        // Create mock base
        fs::create_dir(&base_dir)?;
        let profile = Profile::chromie_335a();
        create_mock_base(&base_dir, &profile)?;

        let sharing_rules = default_sharing_rules();

        // Create a workspace
        let workspace =
            create_workspace("test_workspace", &base_dir, &workspace_root, sharing_rules)?;

        // Verify hard links for base data files
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;

            let test_files = vec![
                ("Wow.exe", true), // Should be hard-linked
                ("Data/common.MPQ", true),
                ("Data/lichking.MPQ", true),
                ("Data/patch.MPQ", false), // MutableData - might be copied
            ];

            for (rel_path, should_be_hardlinked) in test_files {
                let base_file = base_dir.join(rel_path);
                let workspace_file = workspace.workspace_path.join(rel_path);

                if !base_file.exists() || !workspace_file.exists() {
                    continue;
                }

                let base_metadata = fs::metadata(&base_file)?;
                let workspace_metadata = fs::metadata(&workspace_file)?;

                let base_inode = base_metadata.ino();
                let workspace_inode = workspace_metadata.ino();

                if should_be_hardlinked {
                    // For files that should be hard-linked, check if they share the same inode
                    // Or if workspace file is a symlink
                    let is_hardlinked = base_inode == workspace_inode;
                    let is_symlinked = workspace_file.read_link().is_ok();

                    assert!(
                        is_hardlinked || is_symlinked,
                        "{} should be hard-linked or symlinked (base inode: {}, workspace inode: {})",
                        rel_path,
                        base_inode,
                        workspace_inode
                    );

                    println!(
                        "✓ {} is {} (inode: {})",
                        rel_path,
                        if is_hardlinked {
                            "hard-linked"
                        } else {
                            "symlinked"
                        },
                        workspace_inode
                    );
                }
            }
        }

        // Verify symlinks for shared directories
        #[cfg(unix)]
        {
            let screenshots = workspace.workspace_path.join("Screenshots");
            assert!(
                screenshots.read_link().is_ok(),
                "Screenshots should be a symlink"
            );
            println!("✓ Screenshots is a symlink: {:?}", screenshots.read_link()?);
        }

        Ok(())
    }
}
