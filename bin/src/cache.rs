//! Results kept between runs, so unchanged files and scripts aren't checked
//! again.
//!
//! Two JSON files per project under `$STATIX_CACHE_DIR`,
//! `$XDG_CACHE_HOME/statix` or `~/.cache/statix`:
//! - `<project>.files.json`: per `.nix` file its content hash, the script
//!   files it refers to (with their hashes) and whether it had no findings;
//! - `<project>.scripts.json`: checker findings per script (keyed by a hash of
//!   its text), only loaded when something needs checking.
//!
//! A file that had no findings and whose content and referenced scripts are
//! unchanged is skipped. Everything is dropped when the statix binary, the
//! enabled lints, the configuration or the checker versions change.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{LintMap, config::ConfFile};

const FORMAT: u32 = 2;
/// Entries not used for this many days are dropped.
const MAX_AGE_DAYS: u32 = 30;

#[derive(Serialize, Deserialize)]
struct Saved<T> {
    format: u32,
    epoch: String,
    data: T,
}

type Files = HashMap<PathBuf, FileEntry>;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FileEntry {
    hash: String,
    size: u64,
    mtime_ns: u128,
    /// Script files it refers to, with their content hash.
    refs: Vec<(PathBuf, String)>,
    /// No findings at all the last time it was checked.
    clean: bool,
    last_used: u32,
}

pub struct Cache {
    files_path: PathBuf,
    scripts_path: PathBuf,
    epoch: String,
    files: Files,
    files_changed: bool,
    scripts_loaded: bool,
}

fn today() -> u32 {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    u32::try_from(secs / 86_400).unwrap_or(u32::MAX)
}

fn stat(path: &Path) -> Option<(u64, u128)> {
    let meta = fs::metadata(path).ok()?;
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos();
    Some((meta.len(), mtime))
}

fn cache_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("STATIX_CACHE_DIR") {
        return Some(PathBuf::from(dir));
    }
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    Some(base.join("statix"))
}

/// What the saved results depend on besides file contents.
fn epoch(lints: &LintMap, conf: &ConfFile) -> String {
    let binary = std::env::current_exe()
        .ok()
        .and_then(|exe| fs::read(exe).ok())
        .map(|bytes| lib::content_hash(&bytes))
        .unwrap_or_default();
    let mut names: Vec<&str> = lints.values().flatten().map(|l| l.name()).collect();
    names.sort_unstable();
    names.dedup();
    let conf = toml::to_string(conf).unwrap_or_default();
    lib::content_hash(
        format!(
            "{binary}\n{}\n{conf}\n{}",
            names.join(","),
            lib::tool_versions()
        )
        .as_bytes(),
    )
}

/// Saved data, if it was saved in this format and epoch.
fn read<T: DeserializeOwned>(path: &Path, epoch: &str) -> Option<T> {
    let saved: Saved<T> = serde_json::from_slice(&fs::read(path).ok()?).ok()?;
    (saved.format == FORMAT && saved.epoch == epoch).then_some(saved.data)
}

/// Write via a temporary file and rename, so readers never see a partial file.
fn write<T: Serialize>(path: &Path, epoch: &str, data: T) {
    let Some(dir) = path.parent() else { return };
    if fs::create_dir_all(dir).is_err() {
        return;
    }
    let saved = Saved {
        format: FORMAT,
        epoch: epoch.to_string(),
        data,
    };
    let Ok(json) = serde_json::to_vec(&saved) else {
        return;
    };
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    if fs::write(&tmp, json).is_ok() && fs::rename(&tmp, path).is_err() {
        let _ = fs::remove_file(&tmp);
    }
}

impl Cache {
    /// The project's cache (for the current directory), or `None` when
    /// caching is off (`--no-cache`, `STATIX_NO_CACHE`) or impossible.
    #[must_use]
    pub fn open(lints: &LintMap, conf: &ConfFile, enabled: bool) -> Option<Self> {
        if !enabled || std::env::var_os("STATIX_NO_CACHE").is_some() {
            return None;
        }
        let project = fs::canonicalize(".").ok()?;
        let name = lib::content_hash(project.to_string_lossy().as_bytes());
        let dir = cache_dir()?;
        let epoch = epoch(lints, conf);
        let files_path = dir.join(format!("{}.files.json", &name[..16]));
        let files = read(&files_path, &epoch).unwrap_or_default();
        Some(Self {
            scripts_path: dir.join(format!("{}.scripts.json", &name[..16])),
            files_path,
            epoch,
            files,
            files_changed: false,
            scripts_loaded: false,
        })
    }

    /// Load the saved checker results, before checking anything.
    pub fn load_scripts(&mut self) {
        if !self.scripts_loaded {
            self.scripts_loaded = true;
            if let Some(scripts) = read(&self.scripts_path, &self.epoch) {
                lib::import_script_cache(scripts);
            }
        }
    }

    /// Whether `path` with `contents` had no findings last time, and neither it
    /// nor the scripts it refers to changed since.
    #[must_use]
    pub fn is_clean(&self, path: &Path, contents: &str) -> bool {
        let Some(entry) = self.files.get(path) else {
            return false;
        };
        entry.clean
            && entry.hash == lib::content_hash(contents.as_bytes())
            && entry.refs.iter().all(|(script, hash)| {
                fs::read(script).is_ok_and(|bytes| lib::content_hash(&bytes) == *hash)
            })
    }

    /// Script files `path` refers to, if it hasn't changed since they were
    /// recorded (judged by size and modification time, without reading it).
    #[must_use]
    pub fn refs_if_unchanged(&self, path: &Path) -> Option<Vec<PathBuf>> {
        let entry = self.files.get(path)?;
        let (size, mtime_ns) = stat(path)?;
        (entry.size == size && entry.mtime_ns == mtime_ns)
            .then(|| entry.refs.iter().map(|(p, _)| p.clone()).collect())
    }

    /// Remember what checking `path` found.
    pub fn record(&mut self, path: &Path, contents: &str, refs: Vec<PathBuf>, clean: bool) {
        let Some((size, mtime_ns)) = stat(path) else {
            return;
        };
        let refs = refs
            .into_iter()
            .filter_map(|r| {
                let hash = lib::content_hash(&fs::read(&r).ok()?);
                Some((r, hash))
            })
            .collect();
        self.files.insert(
            path.to_path_buf(),
            FileEntry {
                hash: lib::content_hash(contents.as_bytes()),
                size,
                mtime_ns,
                refs,
                clean,
                last_used: today(),
            },
        );
        self.files_changed = true;
    }

    /// Mark `path` as used, so it isn't pruned.
    pub fn touch(&mut self, path: &Path) {
        let today = today();
        if let Some(entry) = self.files.get_mut(path)
            && entry.last_used != today
        {
            entry.last_used = today;
            self.files_changed = true;
        }
    }

    /// Save what changed, merged with what other statix processes saved
    /// meanwhile.
    pub fn save(self) {
        if self.files_changed {
            let mut files: Files = read(&self.files_path, &self.epoch).unwrap_or_default();
            files.extend(self.files);
            let oldest = today().saturating_sub(MAX_AGE_DAYS);
            files.retain(|path, entry| entry.last_used >= oldest && path.exists());
            write(&self.files_path, &self.epoch, files);
        }
        if lib::script_cache_changed() {
            if let Some(scripts) = read(&self.scripts_path, &self.epoch) {
                lib::import_script_cache(scripts);
            }
            write(&self.scripts_path, &self.epoch, lib::export_script_cache());
        }
    }
}
