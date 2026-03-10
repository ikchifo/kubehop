// Rust guideline compliant 2026-02-21
//! Integration tests for context listing and natural sort order.

use std::io::Write;
use std::path::PathBuf;

use khop::context::list::{list_contexts, ContextListItem};
use khop::kubeconfig::KubeConfigView;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

// -- Listing from a simple fixture ------------------------------------------

#[test]
fn test_list_contexts_returns_all_contexts_from_fixture() {
    let view = KubeConfigView::load(fixture("simple.yaml")).unwrap();

    let items = list_contexts(&view).unwrap();

    let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names.len(), 3);
    assert!(names.contains(&"dev"));
    assert!(names.contains(&"staging"));
    assert!(names.contains(&"production"));
}

#[test]
fn test_list_contexts_simple_fixture_sorted_alphabetically() {
    let view = KubeConfigView::load(fixture("simple.yaml")).unwrap();

    let items = list_contexts(&view).unwrap();

    let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, vec!["dev", "production", "staging"]);
}

// -- Natural sort order -----------------------------------------------------

#[test]
fn test_list_contexts_natural_sort_numeric_suffixes() {
    let yaml = br"
apiVersion: v1
kind: Config
contexts:
  - name: cluster-10
    context:
      cluster: c10
  - name: cluster-2
    context:
      cluster: c2
  - name: cluster-1
    context:
      cluster: c1
  - name: cluster-20
    context:
      cluster: c20
  - name: cluster-3
    context:
      cluster: c3
";
    let view = KubeConfigView::from_reader(&yaml[..]).unwrap();

    let items = list_contexts(&view).unwrap();

    let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["cluster-1", "cluster-2", "cluster-3", "cluster-10", "cluster-20"]
    );
}

#[test]
fn test_list_contexts_natural_sort_mixed_prefixes() {
    let yaml = br"
apiVersion: v1
kind: Config
contexts:
  - name: prod
    context: { cluster: c }
  - name: dev-3
    context: { cluster: c }
  - name: dev-1
    context: { cluster: c }
  - name: staging
    context: { cluster: c }
  - name: dev-10
    context: { cluster: c }
";
    let view = KubeConfigView::from_reader(&yaml[..]).unwrap();

    let items = list_contexts(&view).unwrap();

    let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, vec!["dev-1", "dev-3", "dev-10", "prod", "staging"]);
}

#[test]
fn test_list_contexts_natural_sort_leading_zeros() {
    let yaml = br"
apiVersion: v1
kind: Config
contexts:
  - name: v002
    context: { cluster: c }
  - name: v02
    context: { cluster: c }
  - name: v2
    context: { cluster: c }
";
    let view = KubeConfigView::from_reader(&yaml[..]).unwrap();

    let items = list_contexts(&view).unwrap();

    let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
    // Same numeric value; shorter digit run sorts first.
    assert_eq!(names, vec!["v2", "v02", "v002"]);
}

#[test]
fn test_list_contexts_natural_sort_pure_numeric_names() {
    let yaml = br"
apiVersion: v1
kind: Config
contexts:
  - name: '100'
    context: { cluster: c }
  - name: '3'
    context: { cluster: c }
  - name: '20'
    context: { cluster: c }
  - name: '1'
    context: { cluster: c }
";
    let view = KubeConfigView::from_reader(&yaml[..]).unwrap();

    let items = list_contexts(&view).unwrap();

    let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, vec!["1", "3", "20", "100"]);
}

// -- Current context marking ------------------------------------------------

#[test]
fn test_list_contexts_marks_current_context() {
    let view = KubeConfigView::load(fixture("simple.yaml")).unwrap();

    let items = list_contexts(&view).unwrap();

    let current: Vec<&ContextListItem> = items.iter().filter(|i| i.is_current).collect();
    assert_eq!(current.len(), 1);
    assert_eq!(current[0].name, "dev");
}

#[test]
fn test_list_contexts_no_current_context_all_unmarked() {
    let view = KubeConfigView::load(fixture("no_current.yaml")).unwrap();

    let items = list_contexts(&view).unwrap();

    assert!(
        items.iter().all(|i| !i.is_current),
        "all items should have is_current == false when no current-context is set"
    );
}

#[test]
fn test_list_contexts_nonexistent_current_context_all_unmarked() {
    let yaml = br"
apiVersion: v1
kind: Config
current-context: does-not-exist
contexts:
  - name: alpha
    context: { cluster: c }
  - name: beta
    context: { cluster: c }
";
    let view = KubeConfigView::from_reader(&yaml[..]).unwrap();

    let items = list_contexts(&view).unwrap();

    assert!(
        items.iter().all(|i| !i.is_current),
        "no item should be marked current when current-context references a missing name"
    );
}

#[test]
fn test_list_contexts_single_context_is_current() {
    let yaml = br"
apiVersion: v1
kind: Config
current-context: only
contexts:
  - name: only
    context: { cluster: c }
";
    let view = KubeConfigView::from_reader(&yaml[..]).unwrap();

    let items = list_contexts(&view).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "only");
    assert!(items[0].is_current);
}

// -- Empty / missing contexts -----------------------------------------------

#[test]
fn test_list_contexts_empty_fixture_returns_error() {
    let view = KubeConfigView::load(fixture("empty.yaml")).unwrap();

    let result = list_contexts(&view);

    assert!(result.is_err(), "empty contexts should produce an error");
}

#[test]
fn test_list_contexts_empty_returns_no_contexts_variant() {
    let view = KubeConfigView::load(fixture("empty.yaml")).unwrap();

    let err = list_contexts(&view).unwrap_err();

    assert!(
        matches!(err, khop::context::ContextError::NoContexts),
        "expected NoContexts variant, got {err:?}"
    );
}

#[test]
fn test_list_contexts_missing_contexts_field_returns_error() {
    let yaml = br"
apiVersion: v1
kind: Config
current-context: ghost
";
    let view = KubeConfigView::from_reader(&yaml[..]).unwrap();

    let err = list_contexts(&view).unwrap_err();

    assert!(
        matches!(err, khop::context::ContextError::NoContexts),
        "expected NoContexts when contexts key is absent, got {err:?}"
    );
}

// -- Multi-file merged listing ----------------------------------------------

#[test]
fn test_list_contexts_merged_deduplicates_and_sorts() {
    let paths = vec![fixture("multi_a.yaml"), fixture("multi_b.yaml")];
    let view = KubeConfigView::load_merged(&paths).unwrap();

    let items = list_contexts(&view).unwrap();

    let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, vec!["ctx-a", "ctx-b", "shared"]);
}

#[test]
fn test_list_contexts_merged_marks_current_from_first_file() {
    let paths = vec![fixture("multi_a.yaml"), fixture("multi_b.yaml")];
    let view = KubeConfigView::load_merged(&paths).unwrap();

    let items = list_contexts(&view).unwrap();

    let current: Vec<&ContextListItem> = items.iter().filter(|i| i.is_current).collect();
    assert_eq!(current.len(), 1);
    assert_eq!(
        current[0].name, "ctx-a",
        "current-context should come from the first file"
    );
}

#[test]
fn test_list_contexts_merged_empty_paths_returns_no_contexts() {
    let view = KubeConfigView::load_merged(&[]).unwrap();

    let err = list_contexts(&view).unwrap_err();

    assert!(
        matches!(err, khop::context::ContextError::NoContexts),
        "empty merge should yield NoContexts, got {err:?}"
    );
}

#[test]
fn test_list_contexts_merged_tempfiles_natural_sorted() {
    let dir = tempfile::tempdir().unwrap();

    let file_a = dir.path().join("a.yaml");
    std::fs::write(
        &file_a,
        b"apiVersion: v1\nkind: Config\ncurrent-context: node-2\ncontexts:\n  \
          - name: node-2\n    context: { cluster: c }\n  \
          - name: node-10\n    context: { cluster: c }\n",
    )
    .unwrap();

    let file_b = dir.path().join("b.yaml");
    std::fs::write(
        &file_b,
        b"apiVersion: v1\nkind: Config\ncontexts:\n  \
          - name: node-1\n    context: { cluster: c }\n  \
          - name: node-3\n    context: { cluster: c }\n",
    )
    .unwrap();

    let paths = vec![file_a, file_b];
    let view = KubeConfigView::load_merged(&paths).unwrap();

    let items = list_contexts(&view).unwrap();

    let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, vec!["node-1", "node-2", "node-3", "node-10"]);

    let current: Vec<&ContextListItem> = items.iter().filter(|i| i.is_current).collect();
    assert_eq!(current.len(), 1);
    assert_eq!(current[0].name, "node-2");
}

#[test]
fn test_list_contexts_merged_tempfiles_dedup_first_wins() {
    let dir = tempfile::tempdir().unwrap();

    let file_a = dir.path().join("a.yaml");
    let mut fa = std::fs::File::create(&file_a).unwrap();
    writeln!(fa, "apiVersion: v1").unwrap();
    writeln!(fa, "kind: Config").unwrap();
    writeln!(fa, "current-context: overlap").unwrap();
    writeln!(fa, "contexts:").unwrap();
    writeln!(fa, "  - name: overlap").unwrap();
    writeln!(fa, "    context:").unwrap();
    writeln!(fa, "      cluster: from-a").unwrap();
    writeln!(fa, "  - name: unique-a").unwrap();
    writeln!(fa, "    context:").unwrap();
    writeln!(fa, "      cluster: from-a").unwrap();
    drop(fa);

    let file_b = dir.path().join("b.yaml");
    let mut fb = std::fs::File::create(&file_b).unwrap();
    writeln!(fb, "apiVersion: v1").unwrap();
    writeln!(fb, "kind: Config").unwrap();
    writeln!(fb, "contexts:").unwrap();
    writeln!(fb, "  - name: overlap").unwrap();
    writeln!(fb, "    context:").unwrap();
    writeln!(fb, "      cluster: from-b").unwrap();
    writeln!(fb, "  - name: unique-b").unwrap();
    writeln!(fb, "    context:").unwrap();
    writeln!(fb, "      cluster: from-b").unwrap();
    drop(fb);

    let paths = vec![file_a, file_b];
    let view = KubeConfigView::load_merged(&paths).unwrap();

    let items = list_contexts(&view).unwrap();

    // "overlap" appears once (from file a), both unique contexts present
    let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, vec!["overlap", "unique-a", "unique-b"]);
    assert_eq!(items.len(), 3);
}

#[test]
fn test_list_contexts_merged_three_files_with_numeric_names() {
    let dir = tempfile::tempdir().unwrap();

    let write_yaml = |name: &str, contexts: &[&str], current: Option<&str>| -> PathBuf {
        let path = dir.path().join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "apiVersion: v1").unwrap();
        writeln!(f, "kind: Config").unwrap();
        if let Some(c) = current {
            writeln!(f, "current-context: {c}").unwrap();
        }
        writeln!(f, "contexts:").unwrap();
        for ctx in contexts {
            writeln!(f, "  - name: {ctx}").unwrap();
            writeln!(f, "    context: {{ cluster: c }}").unwrap();
        }
        drop(f);
        path
    };

    let f1 = write_yaml("f1.yaml", &["env-20", "env-3"], Some("env-3"));
    let f2 = write_yaml("f2.yaml", &["env-1", "env-100"], None);
    let f3 = write_yaml("f3.yaml", &["env-10", "env-3"], None); // env-3 overlaps

    let paths = vec![f1, f2, f3];
    let view = KubeConfigView::load_merged(&paths).unwrap();

    let items = list_contexts(&view).unwrap();

    let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["env-1", "env-3", "env-10", "env-20", "env-100"]
    );

    let current: Vec<&ContextListItem> = items.iter().filter(|i| i.is_current).collect();
    assert_eq!(current.len(), 1);
    assert_eq!(current[0].name, "env-3");
}
