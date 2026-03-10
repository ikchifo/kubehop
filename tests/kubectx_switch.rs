// Rust guideline compliant 2026-02-21
//! Integration tests for context switching and state file persistence.

#[allow(dead_code)]
mod common;

use std::fs;

use common::{fixture, write_temp};
use khop::context::error::ContextError;
use khop::context::state::StateFile;
use khop::context::switch::switch_context;
use khop::kubeconfig::{KubeConfigView, KubeconfigError};

const FULL_KUBECONFIG: &str = "\
apiVersion: v1
kind: Config
current-context: dev
contexts:
  - name: dev
    context:
      cluster: dev-cluster
      namespace: default
  - name: staging
    context:
      cluster: staging-cluster
      namespace: staging
  - name: production
    context:
      cluster: prod-cluster
clusters:
  - name: dev-cluster
    cluster:
      server: https://dev.example.com
  - name: staging-cluster
    cluster:
      server: https://staging.example.com
  - name: prod-cluster
    cluster:
      server: https://prod.example.com
users:
  - name: dev-user
    user:
      token: fake-dev-token
  - name: staging-user
    user:
      token: fake-staging-token
";

// --------------------------------------------------------------------------
// 1. Switching to an existing context updates the file
// --------------------------------------------------------------------------

#[test]
fn switch_updates_current_context_in_file() {
    let f = write_temp(FULL_KUBECONFIG);
    switch_context(f.path(), "staging").unwrap();

    let view = KubeConfigView::load(f.path()).unwrap();
    assert_eq!(view.current_context(), Some("staging"));
}

#[test]
fn switch_updates_file_on_disk_immediately() {
    let f = write_temp(FULL_KUBECONFIG);
    switch_context(f.path(), "production").unwrap();

    let raw = fs::read_to_string(f.path()).unwrap();
    let doc: serde_yaml::Value = serde_yaml::from_str(&raw).unwrap();
    assert_eq!(
        doc.get("current-context").and_then(serde_yaml::Value::as_str),
        Some("production")
    );
}

#[test]
fn consecutive_switches_each_update_file() {
    let f = write_temp(FULL_KUBECONFIG);

    switch_context(f.path(), "staging").unwrap();
    assert_eq!(
        KubeConfigView::load(f.path()).unwrap().current_context(),
        Some("staging")
    );

    switch_context(f.path(), "production").unwrap();
    assert_eq!(
        KubeConfigView::load(f.path()).unwrap().current_context(),
        Some("production")
    );

    switch_context(f.path(), "dev").unwrap();
    assert_eq!(
        KubeConfigView::load(f.path()).unwrap().current_context(),
        Some("dev")
    );
}

// --------------------------------------------------------------------------
// 2. Switch returns previous context correctly
// --------------------------------------------------------------------------

#[test]
fn switch_returns_previous_context() {
    let f = write_temp(FULL_KUBECONFIG);
    let result = switch_context(f.path(), "staging").unwrap();

    assert_eq!(result.previous.as_deref(), Some("dev"));
    assert_eq!(result.current, "staging");
}

#[test]
fn switch_chains_track_previous_correctly() {
    let f = write_temp(FULL_KUBECONFIG);

    let r1 = switch_context(f.path(), "staging").unwrap();
    assert_eq!(r1.previous.as_deref(), Some("dev"));
    assert_eq!(r1.current, "staging");

    let r2 = switch_context(f.path(), "production").unwrap();
    assert_eq!(r2.previous.as_deref(), Some("staging"));
    assert_eq!(r2.current, "production");

    let r3 = switch_context(f.path(), "dev").unwrap();
    assert_eq!(r3.previous.as_deref(), Some("production"));
    assert_eq!(r3.current, "dev");
}

#[test]
fn switch_to_same_context_returns_self_as_previous() {
    let f = write_temp(FULL_KUBECONFIG);
    let result = switch_context(f.path(), "dev").unwrap();

    assert_eq!(result.previous.as_deref(), Some("dev"));
    assert_eq!(result.current, "dev");
}

// --------------------------------------------------------------------------
// 3. Switching to non-existent context returns NotFound error
// --------------------------------------------------------------------------

#[test]
fn switch_to_nonexistent_context_returns_not_found() {
    let f = write_temp(FULL_KUBECONFIG);
    let err = switch_context(f.path(), "nonexistent").unwrap_err();

    assert!(
        matches!(err, ContextError::NotFound(ref name) if name == "nonexistent"),
        "expected NotFound(\"nonexistent\"), got: {err:?}"
    );
}

#[test]
fn switch_to_empty_string_returns_not_found() {
    let f = write_temp(FULL_KUBECONFIG);
    let err = switch_context(f.path(), "").unwrap_err();

    assert!(
        matches!(err, ContextError::NotFound(ref name) if name.is_empty()),
        "expected NotFound(\"\"), got: {err:?}"
    );
}

#[test]
fn not_found_does_not_mutate_file() {
    let f = write_temp(FULL_KUBECONFIG);
    let before = fs::read_to_string(f.path()).unwrap();

    let _ = switch_context(f.path(), "nonexistent");

    let after = fs::read_to_string(f.path()).unwrap();
    assert_eq!(before, after, "file must not change on NotFound error");
}

#[test]
fn switch_on_missing_file_returns_kubeconfig_error() {
    let err = switch_context("/tmp/khop-test-nonexistent-99999.yaml", "dev").unwrap_err();
    assert!(
        matches!(err, ContextError::Kubeconfig(KubeconfigError::Read(_))),
        "expected Kubeconfig(Read(_)), got: {err:?}"
    );
}

// --------------------------------------------------------------------------
// 4. State file is written/read correctly through switch operations
// --------------------------------------------------------------------------

#[test]
fn state_file_saves_previous_after_switch() {
    let f = write_temp(FULL_KUBECONFIG);
    let state_dir = tempfile::tempdir().unwrap();
    let state = StateFile::new(state_dir.path());

    let result = switch_context(f.path(), "staging").unwrap();

    if let Some(prev) = &result.previous {
        state.save(prev).unwrap();
    }

    assert_eq!(state.load().unwrap().as_deref(), Some("dev"));
}

#[test]
fn state_file_round_trips_through_multiple_switches() {
    let f = write_temp(FULL_KUBECONFIG);
    let state_dir = tempfile::tempdir().unwrap();
    let state = StateFile::new(state_dir.path());

    // First switch: dev -> staging
    let r1 = switch_context(f.path(), "staging").unwrap();
    if let Some(prev) = &r1.previous {
        state.save(prev).unwrap();
    }
    assert_eq!(state.load().unwrap().as_deref(), Some("dev"));

    // Second switch: staging -> production
    let r2 = switch_context(f.path(), "production").unwrap();
    if let Some(prev) = &r2.previous {
        state.save(prev).unwrap();
    }
    assert_eq!(state.load().unwrap().as_deref(), Some("staging"));

    // Third switch: production -> dev (toggling back)
    let r3 = switch_context(f.path(), "dev").unwrap();
    if let Some(prev) = &r3.previous {
        state.save(prev).unwrap();
    }
    assert_eq!(state.load().unwrap().as_deref(), Some("production"));
}

#[test]
fn state_file_is_not_written_when_no_previous_context() {
    let content = "\
apiVersion: v1
kind: Config
contexts:
  - name: alpha
    context:
      cluster: alpha-cluster
";
    let f = write_temp(content);
    let state_dir = tempfile::tempdir().unwrap();
    let state = StateFile::new(state_dir.path());

    let result = switch_context(f.path(), "alpha").unwrap();
    assert!(result.previous.is_none());

    // State file should remain absent when there is nothing to save.
    assert_eq!(state.load().unwrap(), None);
}

#[test]
fn state_file_toggle_pattern() {
    let f = write_temp(FULL_KUBECONFIG);
    let state_dir = tempfile::tempdir().unwrap();
    let state = StateFile::new(state_dir.path());

    // Switch dev -> staging, save "dev" as previous.
    let r1 = switch_context(f.path(), "staging").unwrap();
    if let Some(prev) = &r1.previous {
        state.save(prev).unwrap();
    }

    // Simulate toggle: load previous from state, switch to it.
    let toggle_target = state.load().unwrap().expect("state should have a value");
    let r2 = switch_context(f.path(), &toggle_target).unwrap();
    assert_eq!(r2.current, "dev");
    assert_eq!(r2.previous.as_deref(), Some("staging"));

    // Save the new previous.
    if let Some(prev) = &r2.previous {
        state.save(prev).unwrap();
    }
    assert_eq!(state.load().unwrap().as_deref(), Some("staging"));
}

// --------------------------------------------------------------------------
// 5. File round-trip preserves other fields (clusters, users)
// --------------------------------------------------------------------------

#[test]
fn round_trip_preserves_clusters() {
    let f = write_temp(FULL_KUBECONFIG);
    switch_context(f.path(), "production").unwrap();

    let raw = fs::read_to_string(f.path()).unwrap();
    let doc: serde_yaml::Value = serde_yaml::from_str(&raw).unwrap();

    let clusters = doc
        .get("clusters")
        .and_then(serde_yaml::Value::as_sequence)
        .expect("clusters must survive round-trip");
    assert_eq!(clusters.len(), 3);

    let names: Vec<&str> = clusters
        .iter()
        .filter_map(|c| c.get("name").and_then(serde_yaml::Value::as_str))
        .collect();
    assert!(names.contains(&"dev-cluster"));
    assert!(names.contains(&"staging-cluster"));
    assert!(names.contains(&"prod-cluster"));
}

#[test]
fn round_trip_preserves_users() {
    let f = write_temp(FULL_KUBECONFIG);
    switch_context(f.path(), "staging").unwrap();

    let raw = fs::read_to_string(f.path()).unwrap();
    let doc: serde_yaml::Value = serde_yaml::from_str(&raw).unwrap();

    let users = doc
        .get("users")
        .and_then(serde_yaml::Value::as_sequence)
        .expect("users must survive round-trip");
    assert_eq!(users.len(), 2);

    let names: Vec<&str> = users
        .iter()
        .filter_map(|u| u.get("name").and_then(serde_yaml::Value::as_str))
        .collect();
    assert!(names.contains(&"dev-user"));
    assert!(names.contains(&"staging-user"));
}

#[test]
fn round_trip_preserves_all_contexts() {
    let f = write_temp(FULL_KUBECONFIG);
    switch_context(f.path(), "staging").unwrap();

    let view = KubeConfigView::load(f.path()).unwrap();
    assert_eq!(view.context_names(), vec!["dev", "staging", "production"]);
}

#[test]
fn round_trip_preserves_api_version_and_kind() {
    let f = write_temp(FULL_KUBECONFIG);
    switch_context(f.path(), "production").unwrap();

    let raw = fs::read_to_string(f.path()).unwrap();
    let doc: serde_yaml::Value = serde_yaml::from_str(&raw).unwrap();

    assert_eq!(
        doc.get("apiVersion").and_then(serde_yaml::Value::as_str),
        Some("v1")
    );
    assert_eq!(
        doc.get("kind").and_then(serde_yaml::Value::as_str),
        Some("Config")
    );
}

#[test]
fn round_trip_preserves_user_tokens() {
    let f = write_temp(FULL_KUBECONFIG);
    switch_context(f.path(), "production").unwrap();

    let raw = fs::read_to_string(f.path()).unwrap();
    let doc: serde_yaml::Value = serde_yaml::from_str(&raw).unwrap();

    let users = doc
        .get("users")
        .and_then(serde_yaml::Value::as_sequence)
        .unwrap();
    let dev_user = users
        .iter()
        .find(|u| u.get("name").and_then(serde_yaml::Value::as_str) == Some("dev-user"))
        .expect("dev-user must survive round-trip");
    let token = dev_user
        .get("user")
        .and_then(|u| u.get("token"))
        .and_then(serde_yaml::Value::as_str);
    assert_eq!(token, Some("fake-dev-token"));
}

#[test]
fn multiple_switches_preserve_all_fields() {
    let f = write_temp(FULL_KUBECONFIG);

    switch_context(f.path(), "staging").unwrap();
    switch_context(f.path(), "production").unwrap();
    switch_context(f.path(), "dev").unwrap();
    switch_context(f.path(), "staging").unwrap();

    let raw = fs::read_to_string(f.path()).unwrap();
    let doc: serde_yaml::Value = serde_yaml::from_str(&raw).unwrap();

    assert!(doc.get("clusters").and_then(serde_yaml::Value::as_sequence).is_some());
    assert!(doc.get("users").and_then(serde_yaml::Value::as_sequence).is_some());
    assert!(doc.get("contexts").and_then(serde_yaml::Value::as_sequence).is_some());
    assert_eq!(
        doc.get("current-context").and_then(serde_yaml::Value::as_str),
        Some("staging")
    );
}

// --------------------------------------------------------------------------
// 6. Switching when no current context is set
// --------------------------------------------------------------------------

#[test]
fn switch_with_no_current_context_returns_none_previous() {
    let content = "\
apiVersion: v1
kind: Config
contexts:
  - name: alpha
    context:
      cluster: alpha-cluster
  - name: beta
    context:
      cluster: beta-cluster
";
    let f = write_temp(content);
    let result = switch_context(f.path(), "beta").unwrap();

    assert_eq!(result.previous, None);
    assert_eq!(result.current, "beta");
}

#[test]
fn switch_with_no_current_context_sets_context_in_file() {
    let content = "\
apiVersion: v1
kind: Config
contexts:
  - name: alpha
    context:
      cluster: alpha-cluster
";
    let f = write_temp(content);
    switch_context(f.path(), "alpha").unwrap();

    let view = KubeConfigView::load(f.path()).unwrap();
    assert_eq!(view.current_context(), Some("alpha"));
}

#[test]
fn switch_from_no_current_context_fixture() {
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join("config");
    fs::copy(fixture("no_current.yaml"), &dest).unwrap();

    let result = switch_context(&dest, "alpha").unwrap();
    assert_eq!(result.previous, None);
    assert_eq!(result.current, "alpha");

    let view = KubeConfigView::load(&dest).unwrap();
    assert_eq!(view.current_context(), Some("alpha"));
}

#[test]
fn switch_from_no_current_context_then_switch_again() {
    let content = "\
apiVersion: v1
kind: Config
contexts:
  - name: alpha
    context:
      cluster: alpha-cluster
  - name: beta
    context:
      cluster: beta-cluster
";
    let f = write_temp(content);

    let r1 = switch_context(f.path(), "alpha").unwrap();
    assert_eq!(r1.previous, None);

    let r2 = switch_context(f.path(), "beta").unwrap();
    assert_eq!(r2.previous.as_deref(), Some("alpha"));
    assert_eq!(r2.current, "beta");
}
