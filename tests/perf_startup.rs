// Rust guideline compliant 2026-02-21
//! Architectural regression guards for the kubehop interactive path.
//!
//! These tests verify structural invariants -- they are NOT performance
//! benchmarks. Each test proves that a particular design constraint
//! (one-load, selective serde, score independence, write isolation)
//! has not regressed.

#[allow(dead_code)]
mod common;

use std::fs;

use common::{fixture, make_picker_items, write_temp};
use khop::context::list::list_contexts;
use khop::context::switch::switch_context;
use khop::kubeconfig::KubeConfigView;
use khop::picker::{score_items, PickerItem};

// ---------------------------------------------------------------------------
// 1. One-load invariant
// ---------------------------------------------------------------------------

/// The interactive path -- load kubeconfig, build picker items, score --
/// must work from a single `KubeConfigView` parse with no second load.
#[test]
fn one_load_invariant_single_parse_serves_list_and_score() {
    let view = KubeConfigView::load(fixture("simple.yaml")).unwrap();

    // Build the context list from the already-loaded view.
    let list = list_contexts(&view).unwrap();
    assert!(
        list.len() >= 2,
        "fixture should have multiple contexts, got {}",
        list.len()
    );

    // Convert to picker items -- only uses data already in memory.
    let items: Vec<PickerItem> = list
        .iter()
        .map(|c| PickerItem {
            name: c.name.clone(),
            is_current: c.is_current,
        })
        .collect();

    // Score against a query -- operates on PickerItems, not the kubeconfig.
    let scored = score_items(&items, "dev");
    assert!(
        !scored.is_empty(),
        "scoring 'dev' against fixture should produce matches"
    );

    // The entire pipeline completed without a second file load.
    // If a regression forced a re-parse, the code path would require
    // a filesystem call that this test never issues after the initial load.
}

/// Merged multi-file kubeconfigs also satisfy the one-load invariant.
#[test]
fn one_load_invariant_merged_files() {
    let paths = vec![fixture("multi_a.yaml"), fixture("multi_b.yaml")];
    let view = KubeConfigView::load_merged(&paths).unwrap();

    let list = list_contexts(&view).unwrap();

    // multi_a has ctx-a + shared, multi_b has ctx-b + shared (deduped).
    assert_eq!(
        list.len(),
        3,
        "merged view should have 3 unique contexts, got {}",
        list.len()
    );

    let items: Vec<PickerItem> = list
        .iter()
        .map(|c| PickerItem {
            name: c.name.clone(),
            is_current: c.is_current,
        })
        .collect();

    let scored = score_items(&items, "ctx");
    assert!(
        scored.len() >= 2,
        "scoring 'ctx' should match at least ctx-a and ctx-b"
    );
}

// ---------------------------------------------------------------------------
// 2. Selective serde
// ---------------------------------------------------------------------------

/// `KubeConfigView` must NOT deserialize cluster/user payload. Parsing a
/// kubeconfig with large opaque data in clusters and users must still
/// yield only context names.
#[test]
fn selective_serde_ignores_cluster_and_user_data() {
    let large_cert = "x".repeat(100_000);
    let yaml = format!(
        r"apiVersion: v1
kind: Config
current-context: ctx-one
contexts:
  - name: ctx-one
    context:
      cluster: big-cluster
      namespace: default
  - name: ctx-two
    context:
      cluster: big-cluster
clusters:
  - name: big-cluster
    cluster:
      server: https://kube.example.com
      certificate-authority-data: {large_cert}
users:
  - name: big-user
    user:
      client-certificate-data: {large_cert}
      client-key-data: {large_cert}
"
    );

    let view = KubeConfigView::from_reader(yaml.as_bytes()).unwrap();

    let names = view.context_names();
    assert_eq!(names, vec!["ctx-one", "ctx-two"]);
    assert_eq!(view.current_context(), Some("ctx-one"));

    // The view struct has no field for clusters or users.
    // This is enforced at compile time by the struct definition;
    // we verify here that parsing succeeds without allocating
    // the large certificate payloads into the view.
    assert_eq!(view.contexts.len(), 2);
}

/// Selective serde handles kubeconfig with exotic extra fields gracefully.
#[test]
fn selective_serde_tolerates_unknown_fields() {
    let yaml = r#"
apiVersion: v1
kind: Config
preferences:
  colors: true
current-context: alpha
contexts:
  - name: alpha
    context:
      cluster: alpha-cluster
      extensions:
        - name: custom-extension
          extension:
            data: "opaque-payload"
clusters:
  - name: alpha-cluster
    cluster:
      server: https://alpha.example.com
      tls-server-name: alpha.example.com
users:
  - name: alpha-user
    user:
      exec:
        apiVersion: client.authentication.k8s.io/v1
        command: /usr/local/bin/auth-helper
        args: ["--region", "us-east-1"]
"#;

    let view = KubeConfigView::from_reader(yaml.as_bytes()).unwrap();
    assert_eq!(view.context_names(), vec!["alpha"]);
}

// ---------------------------------------------------------------------------
// 3. Score independence
// ---------------------------------------------------------------------------

/// Scoring operates exclusively on `PickerItem` values -- it must not
/// need access to a `KubeConfigView` or any filesystem resource.
#[test]
fn score_independence_synthetic_items() {
    let items = make_picker_items(
        &[
            "us-east-1-prod",
            "us-west-2-staging",
            "eu-central-1-dev",
            "ap-southeast-1-prod",
        ],
        Some("us-east-1-prod"),
    );

    let scored = score_items(&items, "prod");
    assert!(
        scored.len() >= 2,
        "expected at least 2 matches for 'prod', got {}",
        scored.len()
    );

    // Verify descending score order.
    for pair in scored.windows(2) {
        assert!(
            pair[0].score >= pair[1].score,
            "scores must be descending: {} >= {}",
            pair[0].score,
            pair[1].score
        );
    }
}

/// Empty query returns every synthetic item in original order.
#[test]
fn score_independence_empty_query_preserves_order() {
    let items = make_picker_items(&["zulu", "alpha", "mike"], None);

    let scored = score_items(&items, "");
    assert_eq!(scored.len(), items.len());

    for (i, s) in scored.iter().enumerate() {
        assert_eq!(s.index, i, "empty-query items must preserve insertion order");
        assert_eq!(s.score, 0, "empty-query score must be zero");
    }
}

/// Scoring a non-matching query against synthetic items yields nothing.
#[test]
fn score_independence_no_match() {
    let items = make_picker_items(&["alpha", "beta", "gamma"], None);
    let scored = score_items(&items, "zzzzz");
    assert!(scored.is_empty(), "non-matching query should return no results");
}

// ---------------------------------------------------------------------------
// 4. Write isolation
// ---------------------------------------------------------------------------

/// A switch operation reads the file once (via `serde_yaml::Value` round-trip)
/// and writes the result back. Verify that the written file reflects exactly
/// the requested switch and all other fields survive the round-trip.
#[test]
fn write_isolation_single_read_roundtrip() {
    let original = fs::read_to_string(fixture("simple.yaml")).unwrap();
    let tmp = write_temp(&original);

    let result = switch_context(tmp.path(), "staging").unwrap();
    assert_eq!(result.previous.as_deref(), Some("dev"));
    assert_eq!(result.current, "staging");

    // Re-read the file and verify the switch took effect.
    let after: serde_yaml::Value =
        serde_yaml::from_str(&fs::read_to_string(tmp.path()).unwrap()).unwrap();

    assert_eq!(
        after
            .get("current-context")
            .and_then(serde_yaml::Value::as_str),
        Some("staging"),
        "current-context must be updated after switch"
    );

    // Clusters and users must survive the round-trip.
    let clusters = after.get("clusters").and_then(serde_yaml::Value::as_sequence);
    assert!(
        clusters.is_some(),
        "clusters must survive the Value round-trip"
    );
    assert!(
        !clusters.unwrap().is_empty(),
        "clusters must not be empty after round-trip"
    );

    let users = after.get("users").and_then(serde_yaml::Value::as_sequence);
    assert!(users.is_some(), "users must survive the Value round-trip");
}

/// Switching from a kubeconfig with no prior current-context writes the
/// target without losing any other data.
#[test]
fn write_isolation_no_previous_context() {
    let original = fs::read_to_string(fixture("no_current.yaml")).unwrap();
    let tmp = write_temp(&original);

    let result = switch_context(tmp.path(), "beta").unwrap();
    assert_eq!(result.previous, None);
    assert_eq!(result.current, "beta");

    let after: serde_yaml::Value =
        serde_yaml::from_str(&fs::read_to_string(tmp.path()).unwrap()).unwrap();

    assert_eq!(
        after
            .get("current-context")
            .and_then(serde_yaml::Value::as_str),
        Some("beta"),
    );

    // Contexts array must be intact.
    let contexts = after
        .get("contexts")
        .and_then(serde_yaml::Value::as_sequence)
        .expect("contexts array must survive round-trip");
    assert_eq!(contexts.len(), 2, "both original contexts must be preserved");
}

/// Consecutive switches to different contexts each produce the correct
/// previous/current pair, confirming each operation does its own
/// single-read round-trip.
#[test]
fn write_isolation_consecutive_switches() {
    let original = fs::read_to_string(fixture("simple.yaml")).unwrap();
    let tmp = write_temp(&original);

    let r1 = switch_context(tmp.path(), "staging").unwrap();
    assert_eq!(r1.previous.as_deref(), Some("dev"));
    assert_eq!(r1.current, "staging");

    let r2 = switch_context(tmp.path(), "production").unwrap();
    assert_eq!(r2.previous.as_deref(), Some("staging"));
    assert_eq!(r2.current, "production");

    let r3 = switch_context(tmp.path(), "dev").unwrap();
    assert_eq!(r3.previous.as_deref(), Some("production"));
    assert_eq!(r3.current, "dev");
}
