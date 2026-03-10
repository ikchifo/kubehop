// Rust guideline compliant 2026-02-21
//! Integration tests for context mutation operations (rename, delete, unset).

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use khop::context::error::ContextError;
use khop::context::mutate::{delete_context, rename_context, unset_context};
use khop::kubeconfig::KubeConfigView;
use tempfile::NamedTempFile;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn temp_copy(fixture_name: &str) -> NamedTempFile {
    let content = fs::read_to_string(fixture(fixture_name)).expect("read fixture");
    let mut f = NamedTempFile::new().expect("create temp file");
    f.write_all(content.as_bytes())
        .expect("write temp kubeconfig");
    f.flush().expect("flush temp file");
    f
}

fn write_temp(content: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().expect("create temp file");
    f.write_all(content.as_bytes())
        .expect("write temp kubeconfig");
    f.flush().expect("flush temp file");
    f
}

/// Reloads the written file as a raw `serde_yaml::Value` so tests can
/// inspect all fields, including those outside `KubeConfigView`.
fn reload_raw(f: &NamedTempFile) -> serde_yaml::Value {
    let raw = fs::read_to_string(f.path()).expect("read back temp file");
    serde_yaml::from_str(&raw).expect("parse rewritten YAML")
}

fn context_names_from_value(doc: &serde_yaml::Value) -> Vec<String> {
    doc.get("contexts")
        .and_then(serde_yaml::Value::as_sequence)
        .map(|seq| {
            seq.iter()
                .filter_map(|e| e.get("name").and_then(serde_yaml::Value::as_str))
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Rename
// ---------------------------------------------------------------------------

#[test]
fn rename_context_in_file() {
    let f = temp_copy("simple.yaml");
    let result = rename_context(f.path(), "staging", "qa").unwrap();

    assert_eq!(result.old_name, "staging");
    assert_eq!(result.new_name, "qa");

    let names = context_names_from_value(&reload_raw(&f));
    assert!(names.contains(&"qa".to_owned()));
    assert!(!names.contains(&"staging".to_owned()));
}

#[test]
fn rename_updates_current_context_when_target_is_active() {
    let f = temp_copy("simple.yaml");

    let result = rename_context(f.path(), "dev", "development").unwrap();
    assert_eq!(result.old_name, "dev");
    assert_eq!(result.new_name, "development");

    let doc = reload_raw(&f);
    assert_eq!(
        doc.get("current-context")
            .and_then(serde_yaml::Value::as_str),
        Some("development"),
    );
}

#[test]
fn rename_leaves_current_context_unchanged_for_non_active() {
    let f = temp_copy("simple.yaml");
    let _ = rename_context(f.path(), "staging", "qa").unwrap();

    let doc = reload_raw(&f);
    assert_eq!(
        doc.get("current-context")
            .and_then(serde_yaml::Value::as_str),
        Some("dev"),
        "current-context must stay unchanged when a non-active context is renamed",
    );
}

#[test]
fn rename_nonexistent_context_returns_not_found() {
    let f = temp_copy("simple.yaml");
    let err = rename_context(f.path(), "nonexistent", "new-name").unwrap_err();
    assert!(
        matches!(err, ContextError::NotFound(ref name) if name == "nonexistent"),
        "expected NotFound(\"nonexistent\"), got {err:?}",
    );
}

#[test]
fn rename_preserves_clusters_and_users() {
    let f = temp_copy("simple.yaml");
    let _ = rename_context(f.path(), "staging", "qa").unwrap();

    let doc = reload_raw(&f);

    let clusters = doc
        .get("clusters")
        .and_then(serde_yaml::Value::as_sequence);
    assert!(
        clusters.is_some(),
        "clusters field must survive rename round-trip",
    );
    assert_eq!(clusters.unwrap().len(), 3);

    let users = doc.get("users").and_then(serde_yaml::Value::as_sequence);
    assert!(
        users.is_some(),
        "users field must survive rename round-trip",
    );
    assert_eq!(users.unwrap().len(), 1);
}

#[test]
fn rename_preserves_api_version_and_kind() {
    let f = temp_copy("simple.yaml");
    let _ = rename_context(f.path(), "production", "prod-v2").unwrap();

    let doc = reload_raw(&f);
    assert_eq!(
        doc.get("apiVersion").and_then(serde_yaml::Value::as_str),
        Some("v1"),
    );
    assert_eq!(
        doc.get("kind").and_then(serde_yaml::Value::as_str),
        Some("Config"),
    );
}

// ---------------------------------------------------------------------------
// Delete
// ---------------------------------------------------------------------------

#[test]
fn delete_removes_context_from_file() {
    let f = temp_copy("simple.yaml");
    let result = delete_context(f.path(), "staging").unwrap();

    assert_eq!(result.deleted, "staging");
    assert!(!result.was_current);

    let names = context_names_from_value(&reload_raw(&f));
    assert!(!names.contains(&"staging".to_owned()));
    assert_eq!(names.len(), 2, "only the targeted context should be removed");
}

#[test]
fn delete_current_context_removes_and_unsets() {
    let f = temp_copy("simple.yaml");
    let result = delete_context(f.path(), "dev").unwrap();

    assert_eq!(result.deleted, "dev");
    assert!(result.was_current);

    let doc = reload_raw(&f);
    assert!(
        doc.get("current-context").is_none(),
        "current-context must be removed when the active context is deleted",
    );

    let names = context_names_from_value(&doc);
    assert!(!names.contains(&"dev".to_owned()));
}

#[test]
fn delete_nonexistent_context_returns_not_found() {
    let f = temp_copy("simple.yaml");
    let err = delete_context(f.path(), "ghost").unwrap_err();
    assert!(
        matches!(err, ContextError::NotFound(ref name) if name == "ghost"),
        "expected NotFound(\"ghost\"), got {err:?}",
    );
}

#[test]
fn delete_preserves_remaining_contexts_and_other_fields() {
    let f = temp_copy("simple.yaml");
    let _ = delete_context(f.path(), "staging").unwrap();

    let doc = reload_raw(&f);

    let names = context_names_from_value(&doc);
    assert!(names.contains(&"dev".to_owned()));
    assert!(names.contains(&"production".to_owned()));

    let clusters = doc
        .get("clusters")
        .and_then(serde_yaml::Value::as_sequence);
    assert!(
        clusters.is_some(),
        "clusters field must survive delete round-trip",
    );

    let users = doc.get("users").and_then(serde_yaml::Value::as_sequence);
    assert!(
        users.is_some(),
        "users field must survive delete round-trip",
    );
}

#[test]
fn delete_non_current_leaves_current_context_intact() {
    let f = temp_copy("simple.yaml");
    let _ = delete_context(f.path(), "production").unwrap();

    let doc = reload_raw(&f);
    assert_eq!(
        doc.get("current-context")
            .and_then(serde_yaml::Value::as_str),
        Some("dev"),
        "current-context must stay unchanged when deleting a non-active context",
    );
}

// ---------------------------------------------------------------------------
// Unset
// ---------------------------------------------------------------------------

#[test]
fn unset_clears_current_context_field() {
    let f = temp_copy("simple.yaml");
    let result = unset_context(f.path()).unwrap();

    assert_eq!(result.previous.as_deref(), Some("dev"));

    let doc = reload_raw(&f);
    assert!(
        doc.get("current-context").is_none(),
        "current-context must be removed after unset",
    );
}

#[test]
fn unset_preserves_all_contexts() {
    let f = temp_copy("simple.yaml");
    let _ = unset_context(f.path()).unwrap();

    let view = KubeConfigView::load(f.path()).expect("reload after unset");
    assert_eq!(
        view.context_names(),
        vec!["dev", "staging", "production"],
        "all context entries must survive unset",
    );
}

#[test]
fn unset_preserves_clusters_and_users() {
    let f = temp_copy("simple.yaml");
    let _ = unset_context(f.path()).unwrap();

    let doc = reload_raw(&f);

    let clusters = doc
        .get("clusters")
        .and_then(serde_yaml::Value::as_sequence);
    assert!(
        clusters.is_some(),
        "clusters field must survive unset round-trip",
    );
    assert_eq!(clusters.unwrap().len(), 3);

    let users = doc.get("users").and_then(serde_yaml::Value::as_sequence);
    assert!(
        users.is_some(),
        "users field must survive unset round-trip",
    );
}

#[test]
fn unset_when_no_current_context_is_noop() {
    let f = temp_copy("no_current.yaml");
    let result = unset_context(f.path()).unwrap();

    assert_eq!(result.previous, None);

    let view = KubeConfigView::load(f.path()).expect("reload after noop unset");
    assert_eq!(
        view.context_names(),
        vec!["alpha", "beta"],
        "contexts must be unchanged after noop unset",
    );
}

#[test]
fn unset_on_inline_yaml_without_current_context() {
    let content = "\
apiVersion: v1
kind: Config
contexts:
  - name: only-ctx
    context:
      cluster: only-cluster
";
    let f = write_temp(content);
    let result = unset_context(f.path()).unwrap();

    assert_eq!(result.previous, None);
}
