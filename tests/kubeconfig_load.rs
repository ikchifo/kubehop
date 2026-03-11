//! Integration tests for kubeconfig parsing, loading, and merging.

#[allow(dead_code)]
mod common;

use common::fixture;
use khop::kubeconfig::KubeConfigView;

#[test]
fn test_from_reader_parses_valid_yaml() {
    let yaml = br"
apiVersion: v1
kind: Config
current-context: dev
contexts:
  - name: dev
    context:
      cluster: dev-cluster
";

    let view = KubeConfigView::from_reader(&yaml[..]).expect("valid YAML should parse");
    assert_eq!(view.current_context(), Some("dev"));
    assert_eq!(view.context_names(), vec!["dev"]);
}

#[test]
fn test_from_reader_rejects_invalid_yaml() {
    let garbage = b"{{not: valid:: yaml]]]";
    let result = KubeConfigView::from_reader(&garbage[..]);
    assert!(result.is_err(), "malformed YAML must produce an error");
}

#[test]
fn test_load_reads_fixture_file() {
    let view = KubeConfigView::load(fixture("simple.yaml")).expect("fixture should load");
    assert_eq!(view.context_names(), vec!["dev", "staging", "production"]);
}

#[test]
fn test_context_names_preserves_file_order() {
    let yaml = br"
apiVersion: v1
kind: Config
contexts:
  - name: z-last
    context:
      cluster: c1
  - name: a-first
    context:
      cluster: c2
  - name: m-middle
    context:
      cluster: c3
";

    let view = KubeConfigView::from_reader(&yaml[..]).unwrap();
    assert_eq!(view.context_names(), vec!["z-last", "a-first", "m-middle"]);
}

#[test]
fn test_current_context_returns_active_context() {
    let view = KubeConfigView::load(fixture("simple.yaml")).unwrap();
    assert_eq!(view.current_context(), Some("dev"));
}

#[test]
fn test_current_context_returns_none_when_not_set() {
    let view = KubeConfigView::load(fixture("no_current.yaml")).unwrap();
    assert_eq!(view.current_context(), None);
}

#[test]
fn test_context_exists_returns_true_for_known_context() {
    let view = KubeConfigView::load(fixture("simple.yaml")).unwrap();
    assert!(view.context_exists("dev"));
    assert!(view.context_exists("staging"));
    assert!(view.context_exists("production"));
}

#[test]
fn test_context_exists_returns_false_for_unknown_context() {
    let view = KubeConfigView::load(fixture("simple.yaml")).unwrap();
    assert!(!view.context_exists("nonexistent"));
    assert!(!view.context_exists(""));
}

#[test]
fn test_empty_contexts_returns_empty_vec() {
    let view = KubeConfigView::load(fixture("empty.yaml")).unwrap();
    assert!(view.context_names().is_empty());
    assert!(!view.context_exists("anything"));
}

#[test]
fn test_load_merged_deduplicates_first_occurrence_wins() {
    let paths = vec![fixture("multi_a.yaml"), fixture("multi_b.yaml")];
    let view = KubeConfigView::load_merged(&paths).expect("merge should succeed");

    // ctx-a from file A, shared from file A (first wins), ctx-b from file B
    assert_eq!(view.context_names(), vec!["ctx-a", "shared", "ctx-b"]);
}

#[test]
fn test_load_merged_takes_current_context_from_first_file() {
    let paths = vec![fixture("multi_a.yaml"), fixture("multi_b.yaml")];
    let view = KubeConfigView::load_merged(&paths).unwrap();

    assert_eq!(
        view.current_context(),
        Some("ctx-a"),
        "current-context should come from the first file"
    );
}

#[test]
fn test_load_merged_falls_back_when_first_has_no_current() {
    let paths = vec![fixture("no_current.yaml"), fixture("multi_b.yaml")];
    let view = KubeConfigView::load_merged(&paths).unwrap();

    assert_eq!(
        view.current_context(),
        Some("ctx-b"),
        "current-context should come from the first file that defines one"
    );
}

#[test]
fn test_load_merged_empty_paths_returns_empty_view() {
    let view = KubeConfigView::load_merged(&[]).unwrap();
    assert!(view.context_names().is_empty());
    assert_eq!(view.current_context(), None);
}

#[test]
fn test_clusters_and_users_are_ignored_gracefully() {
    // The simple fixture has clusters and users sections. Parsing must
    // succeed even though KubeConfigView does not model those fields.
    let view = KubeConfigView::load(fixture("simple.yaml")).unwrap();
    assert_eq!(view.context_names().len(), 3);
    assert_eq!(view.current_context(), Some("dev"));
}
