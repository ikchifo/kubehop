//! Context listing with natural sort order.

use crate::kubeconfig::KubeConfigView;

/// A single entry in the context list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextListItem {
    /// The context name as it appears in the kubeconfig.
    pub name: String,
    /// Whether this context is the active (`current-context`) one.
    pub is_current: bool,
    /// The default namespace, if set.
    pub namespace: Option<String>,
    /// The target cluster, if set.
    pub cluster: Option<String>,
    /// The user credential, if set.
    pub user: Option<String>,
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
        .map(|entry| {
            let fields = entry.context.as_ref();
            ContextListItem {
                is_current: current == Some(entry.name.as_str()),
                name: entry.name.clone(),
                namespace: fields.and_then(|f| f.namespace.clone()),
                cluster: fields.and_then(|f| f.cluster.clone()),
                user: fields.and_then(|f| f.user.clone()),
            }
        })
        .collect();

    items.sort_by(|a, b| natural_cmp(&a.name, &b.name));

    Ok(items)
}

/// Compare two strings in natural sort order without allocation.
///
/// Runs of digits are compared numerically (so `"2" < "10"`), while
/// non-digit bytes are compared lexicographically. When numeric values
/// are equal, shorter digit runs sort first (e.g., `"02"` before `"002"`).
fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    let mut a = a.as_bytes();
    let mut b = b.as_bytes();

    loop {
        match (a.first(), b.first()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(&ac), Some(&bc)) => {
                let a_digit = ac.is_ascii_digit();
                let b_digit = bc.is_ascii_digit();

                if a_digit && b_digit {
                    let (av, a_len, a_rest) = consume_number(a);
                    let (bv, b_len, b_rest) = consume_number(b);
                    let ord = av.cmp(&bv).then_with(|| a_len.cmp(&b_len));
                    if ord != Ordering::Equal {
                        return ord;
                    }
                    a = a_rest;
                    b = b_rest;
                } else if a_digit != b_digit {
                    return if a_digit {
                        Ordering::Less
                    } else {
                        Ordering::Greater
                    };
                } else {
                    let ord = ac.cmp(&bc);
                    if ord != Ordering::Equal {
                        return ord;
                    }
                    a = &a[1..];
                    b = &b[1..];
                }
            }
        }
    }
}

/// Consume a run of ASCII digits, returning (numeric value, digit count, remaining bytes).
fn consume_number(bytes: &[u8]) -> (u64, usize, &[u8]) {
    let mut val: u64 = 0;
    let mut len = 0;
    for &b in bytes {
        if b.is_ascii_digit() {
            val = val.saturating_mul(10).saturating_add(u64::from(b - b'0'));
            len += 1;
        } else {
            break;
        }
    }
    (val, len, &bytes[len..])
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
