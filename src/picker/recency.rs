//! Recency tracking for picker items.
//!
//! Maintains per-domain (context / namespace) timestamps so the picker
//! can sort recently used items to the top. State is persisted as JSON
//! at `{cache_dir}/khop/recency.json`.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Domain that a recency entry belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Domain {
    /// Kubernetes context.
    Context,
    /// Kubernetes namespace.
    Namespace,
}

/// Persisted recency state for contexts and namespaces.
///
/// Tracks the last-used epoch seconds for each item name, keyed by
/// [`Domain`]. The state file lives at `{cache_dir}/khop/recency.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecencyState {
    #[serde(default)]
    contexts: HashMap<String, u64>,
    #[serde(default)]
    namespaces: HashMap<String, u64>,
    /// Path to the backing JSON file.
    #[serde(skip)]
    path: PathBuf,
}

impl Default for RecencyState {
    fn default() -> Self {
        Self {
            contexts: HashMap::new(),
            namespaces: HashMap::new(),
            path: PathBuf::new(),
        }
    }
}

impl RecencyState {
    /// Load recency state from `{cache_dir}/khop/recency.json`.
    ///
    /// Returns an empty default state if the file is missing, corrupt,
    /// or otherwise unreadable.
    #[must_use]
    pub fn load(cache_dir: impl AsRef<Path>) -> Self {
        let path = cache_dir.as_ref().join("khop").join("recency.json");
        let mut state = fs::read_to_string(&path)
            .ok()
            .and_then(|contents| serde_json::from_str::<Self>(&contents).ok())
            .unwrap_or_default();
        state.path = path;
        state
    }

    /// Record usage of `name` in `domain` with the current epoch seconds.
    pub fn record(&mut self, domain: Domain, name: &str) {
        let epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        self.map_mut(domain).insert(name.to_owned(), epoch);
    }

    /// Load state, record a single usage, and persist in one step.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if writing the state file fails.
    pub fn record_and_save(
        cache_dir: impl AsRef<Path>,
        domain: Domain,
        name: &str,
    ) -> std::io::Result<()> {
        let mut state = Self::load(cache_dir);
        state.record(domain, name);
        state.save()
    }

    /// Persist the current state to disk.
    ///
    /// Creates parent directories if needed.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if creating directories or writing fails.
    pub fn save(&self) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string(self).map_err(std::io::Error::other)?;
        fs::write(&self.path, json)
    }

    /// Look up the last-used epoch seconds for `name` in `domain`.
    #[must_use]
    pub fn last_used(&self, domain: Domain, name: &str) -> Option<u64> {
        self.map_ref(domain).get(name).copied()
    }

    /// Insert a specific timestamp for `name` in `domain`.
    pub fn insert(&mut self, domain: Domain, name: &str, epoch: u64) {
        self.map_mut(domain).insert(name.to_owned(), epoch);
    }

    /// Returns `true` if the given domain map is empty.
    #[cfg(test)]
    #[must_use]
    pub fn is_empty(&self, domain: Domain) -> bool {
        self.map_ref(domain).is_empty()
    }

    fn map_ref(&self, domain: Domain) -> &HashMap<String, u64> {
        match domain {
            Domain::Context => &self.contexts,
            Domain::Namespace => &self.namespaces,
        }
    }

    fn map_mut(&mut self, domain: Domain) -> &mut HashMap<String, u64> {
        match domain {
            Domain::Context => &mut self.contexts,
            Domain::Namespace => &mut self.namespaces,
        }
    }
}

/// Sort picker items by recency, most recent first.
///
/// Items with a recorded timestamp are placed before untracked items.
/// Among tracked items, the most recently used appears first. Untracked
/// items retain their relative order (the sort is stable).
pub fn sort_by_recency(items: &mut [super::PickerItem], state: &RecencyState, domain: Domain) {
    items.sort_by(|a, b| {
        let ts_a = state.last_used(domain, &a.name);
        let ts_b = state.last_used(domain, &b.name);
        match (ts_a, ts_b) {
            (Some(a), Some(b)) => b.cmp(&a),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });
}

/// Load recency state and sort picker items in one step.
pub fn load_and_sort(items: &mut [super::PickerItem], cache_dir: impl AsRef<Path>, domain: Domain) {
    let state = RecencyState::load(cache_dir);
    sort_by_recency(items, &state, domain);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::picker::PickerItem;

    fn make_items(names: &[&str]) -> Vec<PickerItem> {
        names
            .iter()
            .map(|n| PickerItem {
                name: (*n).to_string(),
                is_current: false,
                meta: None,
            })
            .collect()
    }

    #[test]
    fn load_returns_empty_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let state = RecencyState::load(dir.path());

        assert!(state.is_empty(Domain::Context));
        assert!(state.is_empty(Domain::Namespace));
    }

    #[test]
    fn record_and_load_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = RecencyState::load(dir.path());

        state.record(Domain::Context, "production");
        state.save().unwrap();

        let reloaded = RecencyState::load(dir.path());
        assert!(reloaded.last_used(Domain::Context, "production").is_some());
    }

    #[test]
    fn record_overwrites_previous_timestamp() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = RecencyState::load(dir.path());

        state.insert(Domain::Context, "dev", 100);
        state.record(Domain::Context, "dev");

        let ts = state.last_used(Domain::Context, "dev").unwrap();
        assert!(ts > 100);
    }

    #[test]
    fn context_and_namespace_are_independent() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = RecencyState::load(dir.path());

        state.record(Domain::Context, "shared-name");
        state.record(Domain::Namespace, "shared-name");
        state.save().unwrap();

        let reloaded = RecencyState::load(dir.path());
        assert!(reloaded.last_used(Domain::Context, "shared-name").is_some());
        assert!(
            reloaded
                .last_used(Domain::Namespace, "shared-name")
                .is_some()
        );
        assert!(reloaded.last_used(Domain::Context, "other").is_none());
    }

    #[test]
    fn load_handles_corrupt_file_gracefully() {
        let dir = tempfile::tempdir().unwrap();
        let khop_dir = dir.path().join("khop");
        fs::create_dir_all(&khop_dir).unwrap();
        fs::write(khop_dir.join("recency.json"), "not valid json {{{").unwrap();

        let state = RecencyState::load(dir.path());
        assert!(state.is_empty(Domain::Context));
        assert!(state.is_empty(Domain::Namespace));
    }

    #[test]
    fn save_creates_parent_directories() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("deep").join("nested");
        let mut state = RecencyState::load(&nested);

        state.record(Domain::Context, "test-ctx");
        state.save().unwrap();

        let reloaded = RecencyState::load(&nested);
        assert!(reloaded.last_used(Domain::Context, "test-ctx").is_some());
    }

    #[test]
    fn sort_puts_recent_items_first() {
        let mut state = RecencyState::load(tempfile::tempdir().unwrap().path());
        state.insert(Domain::Context, "b", 200);
        state.insert(Domain::Context, "c", 300);

        let mut items = make_items(&["a", "b", "c"]);
        sort_by_recency(&mut items, &state, Domain::Context);

        let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, vec!["c", "b", "a"]);
    }

    #[test]
    fn sort_preserves_order_for_untracked_items() {
        let mut state = RecencyState::load(tempfile::tempdir().unwrap().path());
        state.insert(Domain::Context, "tracked", 100);

        let mut items = make_items(&["z", "a", "m", "tracked"]);
        sort_by_recency(&mut items, &state, Domain::Context);

        let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, vec!["tracked", "z", "a", "m"]);
    }

    #[test]
    fn sort_with_empty_state_preserves_order() {
        let state = RecencyState::default();
        let mut items = make_items(&["c", "a", "b"]);
        let original: Vec<String> = items.iter().map(|i| i.name.clone()).collect();

        sort_by_recency(&mut items, &state, Domain::Context);

        let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
        let orig_refs: Vec<&str> = original.iter().map(String::as_str).collect();
        assert_eq!(names, orig_refs);
    }
}
