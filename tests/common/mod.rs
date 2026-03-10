// Rust guideline compliant 2026-02-21
//! Shared test helpers for integration tests.

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use khop::picker::PickerItem;
use tempfile::NamedTempFile;

/// Resolves a fixture file path under `tests/fixtures/{name}`.
pub fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

/// Writes `content` to a new temporary file and returns it.
pub fn write_temp(content: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().expect("create temp file");
    f.write_all(content.as_bytes())
        .expect("write temp kubeconfig");
    f.flush().expect("flush temp file");
    f
}

/// Reads a fixture by name and writes its content to a temporary file.
pub fn temp_copy(fixture_name: &str) -> NamedTempFile {
    let content = fs::read_to_string(fixture(fixture_name)).expect("read fixture");
    write_temp(&content)
}

/// Reads a `NamedTempFile` back as a raw `serde_yaml::Value`.
pub fn reload_raw(f: &NamedTempFile) -> serde_yaml::Value {
    let raw = fs::read_to_string(f.path()).expect("read back temp file");
    serde_yaml::from_str(&raw).expect("parse rewritten YAML")
}

/// Creates `PickerItem` values from a list of names, optionally marking
/// one as the current context.
pub fn make_picker_items(names: &[&str], current: Option<&str>) -> Vec<PickerItem> {
    names
        .iter()
        .map(|n| PickerItem {
            name: (*n).to_string(),
            is_current: current == Some(*n),
        })
        .collect()
}
