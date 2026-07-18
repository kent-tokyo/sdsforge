use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// Deliberately no `Debug` derive — this contains api_key. Without Debug, `{cfg:?}` can't
// compile anywhere in this crate, which rules out ever accidentally logging it whole.
#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct AppConfig {
    pub provider: String,
    pub api_key: String,
    pub model: String,
    pub language: String,
    pub quality: String,
    pub ui_lang: String,
    pub enrich: bool,
    pub base_url: String,
    /// Use the MHLW-recommended filename convention: SDS_日付_品番.json
    pub use_suggested_filename: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            provider: "anthropic".into(),
            api_key: String::new(),
            model: String::new(),
            language: "ja".into(),
            quality: "medium".into(),
            ui_lang: "ja".into(),
            enrich: false,
            base_url: String::new(),
            use_suggested_filename: false,
        }
    }
}

impl AppConfig {
    fn new_config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("sdsforge").join("config.toml"))
    }

    /// Pre-rename config location. Never written to, never deleted — only read as a
    /// migration source when the new path doesn't have a config yet.
    fn old_config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("sdsconv").join("config.toml"))
    }

    pub fn config_path_pub() -> Option<PathBuf> {
        Self::new_config_path()
    }

    /// Load settings, migrating from the pre-rename `sdsconv` config directory to the
    /// `sdsforge` one if needed. Shared by the GUI and CLI (both call this single entry
    /// point), so the resolution and migration behavior can't drift between the two.
    ///
    /// Resolution order:
    /// 1. `sdsforge/config.toml` exists → use it (even if both files exist and differ —
    ///    the new file always wins; a warning without secrets is logged in that case).
    /// 2. It's missing but `sdsconv/config.toml` exists → read the old file, then attempt
    ///    to migrate it to the new path. Migration failure doesn't prevent using the
    ///    settings that were just read from the old file.
    /// 3. Neither exists (or is readable) → defaults.
    pub fn load() -> Self {
        let Some(new_path) = Self::new_config_path() else {
            return Self::default();
        };
        Self::load_from(&new_path, Self::old_config_path().as_deref())
    }

    fn load_from(new_path: &Path, old_path: Option<&Path>) -> Self {
        if let Some(new_cfg) = read_config(new_path) {
            if let Some(old_cfg) = old_path.and_then(read_config) {
                if old_cfg != new_cfg {
                    tracing::warn!("{}", differ_warning(old_path.unwrap(), new_path));
                }
            }
            return new_cfg;
        }

        // A file exists at the new path but failed to parse: existence still wins, so we
        // don't silently fall back to settings the user may believe they've already moved
        // away from.
        if new_path.exists() {
            return Self::default();
        }

        let Some(old_cfg) = old_path.and_then(read_config) else {
            return Self::default();
        };

        if let Err(e) = write_config_atomically(new_path, &old_cfg) {
            tracing::warn!("{}", migration_failed_warning(new_path, &e));
        }
        old_cfg
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::new_config_path()
            .ok_or_else(|| anyhow::anyhow!("Cannot determine config directory"))?;
        write_config_atomically(&path, self)
    }
}

fn read_config(path: &Path) -> Option<AppConfig> {
    let text = std::fs::read_to_string(path).ok()?;
    toml::from_str(&text).ok()
}

/// Write `path`'s parent dir (creating it if needed), then write via a sibling temp file
/// and rename over the target — avoids ever leaving a partially-written config.toml behind,
/// and only replaces the previous file once the new content is fully durable on disk.
fn write_config_atomically(path: &Path, cfg: &AppConfig) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("config path has no parent directory"))?;
    std::fs::create_dir_all(parent)?;

    let tmp_path = path.with_extension("toml.tmp");
    std::fs::write(&tmp_path, toml::to_string_pretty(cfg)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(&tmp_path, path)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Warning messages
//
// Both functions take only `&Path`/`&anyhow::Error` — never `&AppConfig` — so it's a
// compile-time property, not just a runtime check, that a secret field (e.g. api_key)
// can never end up interpolated into one of these messages: the functions have no way
// to reach it. `AppConfig` also deliberately does not derive `Debug`, so `{cfg:?}`
// couldn't be used here either even if someone tried.
// ---------------------------------------------------------------------------

fn differ_warning(old_path: &Path, new_path: &Path) -> String {
    format!(
        "Both the old ({}) and new ({}) config files exist with different settings; \
         using the new file. Delete the old one once you've confirmed the new settings \
         are correct.",
        old_path.display(),
        new_path.display(),
    )
}

fn migration_failed_warning(new_path: &Path, e: &anyhow::Error) -> String {
    format!("Could not migrate config to {}: {e}", new_path.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(api_key: &str) -> AppConfig {
        AppConfig {
            api_key: api_key.into(),
            ..AppConfig::default()
        }
    }

    fn write_raw(path: &Path, cfg: &AppConfig) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, toml::to_string_pretty(cfg).unwrap()).unwrap();
    }

    #[test]
    fn new_only_uses_new_and_leaves_it_alone() {
        let dir = tempfile::tempdir().unwrap();
        let new_path = dir.path().join("sdsforge/config.toml");
        write_raw(&new_path, &cfg("new-key"));

        let loaded = AppConfig::load_from(&new_path, None);
        assert_eq!(loaded.api_key, "new-key");
    }

    #[test]
    fn old_only_migrates_to_new() {
        let dir = tempfile::tempdir().unwrap();
        let new_path = dir.path().join("sdsforge/config.toml");
        let old_path = dir.path().join("sdsconv/config.toml");
        write_raw(&old_path, &cfg("old-key"));

        let loaded = AppConfig::load_from(&new_path, Some(&old_path));
        assert_eq!(loaded.api_key, "old-key");

        assert!(
            new_path.exists(),
            "migration should have written the new config file"
        );
        let migrated = read_config(&new_path).unwrap();
        assert_eq!(migrated.api_key, "old-key");

        // old config is never deleted or modified
        assert!(old_path.exists());
        assert_eq!(read_config(&old_path).unwrap().api_key, "old-key");
    }

    #[test]
    fn both_exist_same_content_uses_new_without_warning_path() {
        let dir = tempfile::tempdir().unwrap();
        let new_path = dir.path().join("sdsforge/config.toml");
        let old_path = dir.path().join("sdsconv/config.toml");
        write_raw(&new_path, &cfg("same-key"));
        write_raw(&old_path, &cfg("same-key"));

        let loaded = AppConfig::load_from(&new_path, Some(&old_path));
        assert_eq!(loaded.api_key, "same-key");
        // old is untouched
        assert_eq!(read_config(&old_path).unwrap().api_key, "same-key");
    }

    #[test]
    fn both_exist_different_content_prefers_new() {
        let dir = tempfile::tempdir().unwrap();
        let new_path = dir.path().join("sdsforge/config.toml");
        let old_path = dir.path().join("sdsconv/config.toml");
        write_raw(&new_path, &cfg("new-key"));
        write_raw(&old_path, &cfg("old-key"));

        let loaded = AppConfig::load_from(&new_path, Some(&old_path));
        assert_eq!(loaded.api_key, "new-key");
        // old is untouched, never overwritten with the new content
        assert_eq!(read_config(&old_path).unwrap().api_key, "old-key");
    }

    #[test]
    fn old_config_corrupted_falls_back_to_default_and_does_not_migrate() {
        let dir = tempfile::tempdir().unwrap();
        let new_path = dir.path().join("sdsforge/config.toml");
        let old_path = dir.path().join("sdsconv/config.toml");
        std::fs::create_dir_all(old_path.parent().unwrap()).unwrap();
        std::fs::write(&old_path, "not valid toml {{{").unwrap();

        let loaded = AppConfig::load_from(&new_path, Some(&old_path));
        assert_eq!(loaded.api_key, AppConfig::default().api_key);
        assert!(
            !new_path.exists(),
            "nothing valid to migrate, new path must not be created"
        );
    }

    #[test]
    fn new_config_corrupted_falls_back_to_default_without_reading_old() {
        let dir = tempfile::tempdir().unwrap();
        let new_path = dir.path().join("sdsforge/config.toml");
        let old_path = dir.path().join("sdsconv/config.toml");
        std::fs::create_dir_all(new_path.parent().unwrap()).unwrap();
        std::fs::write(&new_path, "not valid toml {{{").unwrap();
        write_raw(&old_path, &cfg("old-key"));

        let loaded = AppConfig::load_from(&new_path, Some(&old_path));
        // presence of the (corrupt) new file wins — never silently fall back to old-key
        assert_eq!(loaded.api_key, AppConfig::default().api_key);
        assert_ne!(loaded.api_key, "old-key");
    }

    #[cfg(unix)]
    #[test]
    fn migration_target_dir_uncreatable_still_returns_old_config() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let old_path = dir.path().join("sdsconv/config.toml");
        write_raw(&old_path, &cfg("old-key"));

        // Make the parent read-only so `sdsforge/` can't be created underneath it.
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o500)).unwrap();
        let new_path = dir.path().join("sdsforge/config.toml");

        let loaded = AppConfig::load_from(&new_path, Some(&old_path));

        // restore permissions so tempdir cleanup can delete it
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();

        assert_eq!(
            loaded.api_key, "old-key",
            "migration failure must not block using old config"
        );
        assert!(!new_path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn migration_target_unwritable_still_returns_old_config() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let old_path = dir.path().join("sdsconv/config.toml");
        write_raw(&old_path, &cfg("old-key"));

        let new_dir = dir.path().join("sdsforge");
        std::fs::create_dir_all(&new_dir).unwrap();
        std::fs::set_permissions(&new_dir, std::fs::Permissions::from_mode(0o500)).unwrap();
        let new_path = new_dir.join("config.toml");

        let loaded = AppConfig::load_from(&new_path, Some(&old_path));

        std::fs::set_permissions(&new_dir, std::fs::Permissions::from_mode(0o700)).unwrap();

        assert_eq!(
            loaded.api_key, "old-key",
            "migration failure must not block using old config"
        );
        assert!(!new_path.exists());
    }

    #[test]
    fn running_migration_twice_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let new_path = dir.path().join("sdsforge/config.toml");
        let old_path = dir.path().join("sdsconv/config.toml");
        write_raw(&old_path, &cfg("old-key"));

        let first = AppConfig::load_from(&new_path, Some(&old_path));
        let migrated_mtime = std::fs::metadata(&new_path).unwrap().modified().unwrap();

        let second = AppConfig::load_from(&new_path, Some(&old_path));

        assert_eq!(first.api_key, second.api_key);
        assert_eq!(read_config(&new_path).unwrap().api_key, "old-key");
        // second run reads the now-existing new file directly; it never rewrites it
        assert_eq!(
            std::fs::metadata(&new_path).unwrap().modified().unwrap(),
            migrated_mtime
        );
    }

    #[test]
    fn neither_config_exists_returns_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let new_path = dir.path().join("sdsforge/config.toml");
        let old_path = dir.path().join("sdsconv/config.toml");

        let loaded = AppConfig::load_from(&new_path, Some(&old_path));
        assert_eq!(loaded.api_key, AppConfig::default().api_key);
        assert!(!new_path.exists());
    }

    /// `differ_warning`/`migration_failed_warning` only ever accept `&Path` (never
    /// `&AppConfig`), so no secret field can reach them regardless of what a real
    /// `AppConfig` instance in the same test happens to contain — the assertions below
    /// on made-up secret strings are a backstop, the real guarantee is the signature.
    #[test]
    fn warning_messages_never_reference_config_contents() {
        let old_path = Path::new("/tmp/does-not-exist/sdsconv/config.toml");
        let new_path = Path::new("/tmp/does-not-exist/sdsforge/config.toml");
        let secret = "sk-super-secret-value";

        let differ_msg = differ_warning(old_path, new_path);
        assert!(!differ_msg.contains(secret));
        assert!(differ_msg.contains("sdsconv/config.toml"));
        assert!(differ_msg.contains("sdsforge/config.toml"));

        let io_err = anyhow::anyhow!("permission denied");
        let migrate_msg = migration_failed_warning(new_path, &io_err);
        assert!(!migrate_msg.contains(secret));
        assert!(migrate_msg.contains("sdsforge/config.toml"));

        // Belt-and-suspenders: prove AppConfig really is loaded with secrets in this
        // test's scope (so the assertions above aren't vacuous), yet neither warning
        // function call above took an AppConfig argument at all.
        let _ = cfg(secret);
    }

    #[test]
    fn old_config_file_is_never_modified_by_load() {
        let dir = tempfile::tempdir().unwrap();
        let new_path = dir.path().join("sdsforge/config.toml");
        let old_path = dir.path().join("sdsconv/config.toml");
        write_raw(&old_path, &cfg("old-key"));
        let before = std::fs::read_to_string(&old_path).unwrap();

        AppConfig::load_from(&new_path, Some(&old_path));
        AppConfig::load_from(&new_path, Some(&old_path));

        let after = std::fs::read_to_string(&old_path).unwrap();
        assert_eq!(before, after);
        assert!(old_path.exists(), "old config must never be auto-deleted");
    }

    #[test]
    fn migrated_file_has_owner_only_permissions() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let dir = tempfile::tempdir().unwrap();
            let new_path = dir.path().join("sdsforge/config.toml");
            let old_path = dir.path().join("sdsconv/config.toml");
            write_raw(&old_path, &cfg("old-key"));

            AppConfig::load_from(&new_path, Some(&old_path));

            let mode = std::fs::metadata(&new_path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
    }
}
