//! Integration tests for recency-based picker sorting.

use khop::context::list::list_contexts;
use khop::kubeconfig::KubeConfigView;
use khop::picker::PickerItem;
use khop::picker::recency::{Domain, RecencyState, sort_by_recency};

#[test]
fn recency_sort_with_real_kubeconfig() {
    let yaml = r"
apiVersion: v1
kind: Config
current-context: alpha
contexts:
  - name: alpha
    context:
      cluster: alpha-cluster
  - name: beta
    context:
      cluster: beta-cluster
  - name: gamma
    context:
      cluster: gamma-cluster
";
    let view = KubeConfigView::from_reader(yaml.as_bytes()).unwrap();
    let list = list_contexts(&view).unwrap();
    let mut items: Vec<PickerItem> = list.into_iter().map(PickerItem::from).collect();

    assert_eq!(items[0].name, "alpha");
    assert_eq!(items[1].name, "beta");
    assert_eq!(items[2].name, "gamma");

    let dir = tempfile::tempdir().unwrap();
    let mut recency = RecencyState::load(dir.path());
    recency.insert(Domain::Context, "beta", 1000);
    recency.insert(Domain::Context, "gamma", 2000);
    recency.save().unwrap();

    let recency = RecencyState::load(dir.path());
    sort_by_recency(&mut items, &recency, Domain::Context);

    let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, vec!["gamma", "beta", "alpha"]);
}

#[test]
fn recency_state_persists_across_loads() {
    let dir = tempfile::tempdir().unwrap();

    let mut state = RecencyState::load(dir.path());
    state.record(Domain::Context, "prod");
    state.record(Domain::Namespace, "kube-system");
    state.save().unwrap();

    let reloaded = RecencyState::load(dir.path());
    assert!(reloaded.last_used(Domain::Context, "prod").is_some());
    assert!(
        reloaded
            .last_used(Domain::Namespace, "kube-system")
            .is_some()
    );
    assert!(reloaded.last_used(Domain::Context, "missing").is_none());
}
