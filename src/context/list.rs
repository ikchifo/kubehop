//! Context listing with natural sort order.

use crate::kubeconfig::KubeConfigView;

/// A single entry in the context list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextListItem {
    /// The context name as it appears in the kubeconfig.
    pub name: String,
    /// Whether this context is the active (`current-context`) one.
    pub is_current: bool,
}

/// Build a sorted context list from a kubeconfig view.
///
/// Contexts are sorted using natural sort order (numbers within
/// strings compare numerically, so `ctx-2` sorts before `ctx-10`).
/// The current context is marked with `is_current: true`.
///
/// # Errors
///
/// Returns [`ContextError::NoContexts`](super::ContextError::NoContexts) if
/// the kubeconfig has no contexts.
pub fn list_contexts(view: &KubeConfigView) -> Result<Vec<ContextListItem>, super::ContextError> {
    if view.contexts.is_empty() {
        return Err(super::ContextError::NoContexts);
    }

    let current = view.current_context();

    let mut items: Vec<ContextListItem> = view
        .contexts
        .iter()
        .map(|entry| ContextListItem {
            is_current: current == Some(entry.name.as_str()),
            name: entry.name.clone(),
        })
        .collect();

    items.sort_by(|a, b| natural_cmp(&a.name, &b.name));

    Ok(items)
}

/// Segment of a string split into text and numeric parts for natural ordering.
#[derive(Debug, PartialEq, Eq)]
enum Segment<'a> {
    Text(&'a str),
    Numeric(u64, usize), // (value, original digit count for tie-breaking)
}

/// Split a string into alternating text and numeric segments.
fn segments(s: &str) -> Vec<Segment<'_>> {
    let mut result = Vec::new();
    let mut rest = s;

    while !rest.is_empty() {
        if rest.as_bytes()[0].is_ascii_digit() {
            let end = rest
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(rest.len());
            let digits = &rest[..end];
            let value = digits.parse::<u64>().unwrap_or(u64::MAX);
            result.push(Segment::Numeric(value, digits.len()));
            rest = &rest[end..];
        } else {
            let end = rest
                .find(|c: char| c.is_ascii_digit())
                .unwrap_or(rest.len());
            result.push(Segment::Text(&rest[..end]));
            rest = &rest[end..];
        }
    }

    result
}

/// Compare two strings in natural sort order.
///
/// Runs of digits are compared numerically (so `"2" < "10"`), while
/// non-digit runs are compared lexicographically. When numeric values
/// are equal, shorter digit runs sort first (e.g., `"02"` before `"002"`).
fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let segs_a = segments(a);
    let segs_b = segments(b);

    for (sa, sb) in segs_a.iter().zip(segs_b.iter()) {
        let ord = match (sa, sb) {
            (Segment::Numeric(va, la), Segment::Numeric(vb, lb)) => {
                va.cmp(vb).then_with(|| la.cmp(lb))
            }
            (Segment::Text(ta), Segment::Text(tb)) => ta.cmp(tb),
            // Digits sort before text when segment types differ.
            (Segment::Numeric(..), Segment::Text(..)) => std::cmp::Ordering::Less,
            (Segment::Text(..), Segment::Numeric(..)) => std::cmp::Ordering::Greater,
        };

        if ord != std::cmp::Ordering::Equal {
            return ord;
        }
    }

    segs_a.len().cmp(&segs_b.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kubeconfig::ContextEntry;

    fn make_view(names: &[&str], current: Option<&str>) -> KubeConfigView {
        KubeConfigView {
            current_context: current.map(String::from),
            contexts: names
                .iter()
                .map(|n| ContextEntry {
                    name: (*n).to_string(),
                    context: None,
                })
                .collect(),
        }
    }

    #[test]
    fn empty_contexts_returns_error() {
        let view = make_view(&[], None);
        let err = list_contexts(&view).unwrap_err();
        assert!(
            matches!(err, super::super::ContextError::NoContexts),
            "expected NoContexts, got {err:?}"
        );
    }

    #[test]
    fn contexts_sorted_naturally() {
        let view = make_view(&["ctx-10", "ctx-1", "ctx-2", "ctx-20", "ctx-3"], None);
        let items = list_contexts(&view).unwrap();
        let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, vec!["ctx-1", "ctx-2", "ctx-3", "ctx-10", "ctx-20"]);
    }

    #[test]
    fn current_context_marked() {
        let view = make_view(&["alpha", "beta", "gamma"], Some("beta"));
        let items = list_contexts(&view).unwrap();

        let current_items: Vec<&ContextListItem> = items.iter().filter(|i| i.is_current).collect();
        assert_eq!(current_items.len(), 1);
        assert_eq!(current_items[0].name, "beta");
    }

    #[test]
    fn no_current_context_all_false() {
        let view = make_view(&["alpha", "beta"], None);
        let items = list_contexts(&view).unwrap();
        assert!(items.iter().all(|i| !i.is_current));
    }

    #[test]
    fn current_context_not_in_list_all_false() {
        let view = make_view(&["alpha", "beta"], Some("nonexistent"));
        let items = list_contexts(&view).unwrap();
        assert!(items.iter().all(|i| !i.is_current));
    }

    #[test]
    fn natural_sort_mixed_alpha_numeric() {
        let view = make_view(&["prod", "dev-1", "dev-10", "dev-2", "staging"], None);
        let items = list_contexts(&view).unwrap();
        let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, vec!["dev-1", "dev-2", "dev-10", "prod", "staging"]);
    }

    #[test]
    fn natural_cmp_basic_cases() {
        use std::cmp::Ordering;

        assert_eq!(natural_cmp("a", "b"), Ordering::Less);
        assert_eq!(natural_cmp("b", "a"), Ordering::Greater);
        assert_eq!(natural_cmp("a", "a"), Ordering::Equal);
        assert_eq!(natural_cmp("a2", "a10"), Ordering::Less);
        assert_eq!(natural_cmp("a10", "a2"), Ordering::Greater);
        assert_eq!(natural_cmp("file1", "file1"), Ordering::Equal);
    }

    #[test]
    fn natural_cmp_leading_zeros() {
        use std::cmp::Ordering;

        // Same numeric value, shorter digit run sorts first.
        assert_eq!(natural_cmp("v02", "v002"), Ordering::Less);
    }

    #[test]
    fn single_context() {
        let view = make_view(&["only-one"], Some("only-one"));
        let items = list_contexts(&view).unwrap();
        assert_eq!(items.len(), 1);
        assert!(items[0].is_current);
        assert_eq!(items[0].name, "only-one");
    }
}
