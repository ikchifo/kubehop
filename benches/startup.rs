//! Criterion benchmarks for startup-critical paths.
//!
//! Covers the hot path from kubeconfig load through context listing,
//! fuzzy scoring, and context switching. Run with:
//!
//! ```sh
//! cargo bench
//! ```
//!
//! HTML reports are generated in `target/criterion/`.

use std::fmt::Write as _;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use khop::context::list::list_contexts;
use khop::context::switch::switch_context;
use khop::kubeconfig::KubeConfigView;
use khop::picker::{PickerItem, score_items};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

/// Pre-built test data at a given scale.
struct Scale {
    label: &'static str,
    yaml: String,
    view: KubeConfigView,
    items: Vec<PickerItem>,
}

impl Scale {
    fn new(label: &'static str, n: usize) -> Self {
        let yaml = generate_kubeconfig(n);
        let view = KubeConfigView::from_reader(yaml.as_bytes()).unwrap();
        let items = to_picker_items(&view);
        Self {
            label,
            yaml,
            view,
            items,
        }
    }

    fn from_fixture(label: &'static str, path: &str) -> Self {
        let yaml = fs::read_to_string(fixture(path)).unwrap();
        let view = KubeConfigView::from_reader(yaml.as_bytes()).unwrap();
        let items = to_picker_items(&view);
        Self {
            label,
            yaml,
            view,
            items,
        }
    }
}

/// Generate a synthetic kubeconfig YAML string with `n` contexts.
fn generate_kubeconfig(n: usize) -> String {
    let mut yaml =
        String::from("apiVersion: v1\nkind: Config\ncurrent-context: ctx-0\ncontexts:\n");
    for i in 0..n {
        let _ = write!(
            yaml,
            "  - name: ctx-{i}\n    context:\n      cluster: cluster-{i}\n"
        );
    }
    yaml.push_str("clusters:\n");
    for i in 0..n {
        let _ = write!(
            yaml,
            "  - name: cluster-{i}\n    cluster:\n      server: https://k8s-{i}.example.com\n"
        );
    }
    yaml
}

fn write_temp(content: &str) -> tempfile::NamedTempFile {
    let mut f = tempfile::NamedTempFile::new().expect("create temp file");
    f.write_all(content.as_bytes())
        .expect("write temp kubeconfig");
    f.flush().expect("flush");
    f
}

fn to_picker_items(view: &KubeConfigView) -> Vec<PickerItem> {
    list_contexts(view)
        .unwrap()
        .into_iter()
        .map(PickerItem::from)
        .collect()
}

// ---------------------------------------------------------------------------
// Benchmark groups
// ---------------------------------------------------------------------------

fn bench_parse(c: &mut Criterion) {
    let scales = [
        Scale::from_fixture("3 contexts", "simple.yaml"),
        Scale::new("50 contexts", 50),
        Scale::new("150 contexts", 150),
        Scale::new("250 contexts", 250),
    ];

    let mut group = c.benchmark_group("parse");
    for s in &scales {
        group.bench_function(s.label, |b| {
            b.iter(|| KubeConfigView::from_reader(s.yaml.as_bytes()).unwrap());
        });
    }
    group.finish();
}

fn bench_list_and_sort(c: &mut Criterion) {
    let scales = [
        Scale::from_fixture("3 contexts", "simple.yaml"),
        Scale::new("50 contexts", 50),
        Scale::new("150 contexts", 150),
        Scale::new("250 contexts", 250),
    ];

    let mut group = c.benchmark_group("list_and_sort");
    for s in &scales {
        group.bench_function(s.label, |b| {
            b.iter(|| list_contexts(&s.view).unwrap());
        });
    }
    group.finish();
}

fn bench_score(c: &mut Criterion) {
    let scales = [
        Scale::new("50 items", 50),
        Scale::new("150 items", 150),
        Scale::new("250 items", 250),
    ];

    let mut group = c.benchmark_group("score");

    group.bench_function("empty query (50 items)", |b| {
        b.iter(|| score_items(&scales[0].items, ""));
    });

    for s in &scales {
        group.bench_function(format!("short query ({})", s.label), |b| {
            b.iter(|| score_items(&s.items, "ctx"));
        });
    }

    group.bench_function("narrow query (250 items)", |b| {
        b.iter(|| score_items(&scales[2].items, "ctx-42"));
    });

    group.finish();
}

fn bench_switch(c: &mut Criterion) {
    let s = Scale::new("10 contexts", 10);

    let mut group = c.benchmark_group("switch");
    group.bench_function("switch context (10 contexts)", |b| {
        b.iter_batched(
            || write_temp(&s.yaml),
            |f| switch_context(f.path(), "ctx-5").unwrap(),
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

fn bench_full_pipeline(c: &mut Criterion) {
    let scales = [
        Scale::from_fixture("3", "simple.yaml"),
        Scale::new("50", 50),
        Scale::new("150", 150),
        Scale::new("250", 250),
    ];

    let mut group = c.benchmark_group("full_pipeline");
    for s in &scales {
        group.bench_function(format!("{}: parse + list + score", s.label), |b| {
            b.iter(|| {
                let view = KubeConfigView::from_reader(s.yaml.as_bytes()).unwrap();
                let items = to_picker_items(&view);
                score_items(&items, "ctx")
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_parse,
    bench_list_and_sort,
    bench_score,
    bench_switch,
    bench_full_pipeline,
);
criterion_main!(benches);
