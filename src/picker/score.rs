//! Thin wrapper around `nucleo-matcher` for scoring picker items.

use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

/// A scored item with its original index, score, and matched character positions.
#[derive(Debug, Clone)]
pub struct ScoredItem {
    /// Index into the original items slice.
    pub index: usize,
    /// Match score (higher is better). Zero when the query is empty.
    pub score: u32,
    /// Character indices within the item name that matched the query.
    pub indices: Vec<u32>,
}

/// Score all items against the given query.
///
/// When `query` is empty, returns every item in its original order with a
/// zero score and no match indices. Otherwise returns only matching items,
/// sorted by score descending.
#[must_use]
pub fn score_items(items: &[super::PickerItem], query: &str) -> Vec<ScoredItem> {
    let mut matcher = Matcher::new(Config::DEFAULT);
    score_items_with_matcher(items, query, &mut matcher)
}

/// Score items reusing an existing matcher to avoid per-call allocation.
pub(crate) fn score_items_with_matcher(
    items: &[super::PickerItem],
    query: &str,
    matcher: &mut Matcher,
) -> Vec<ScoredItem> {
    if query.is_empty() {
        return items
            .iter()
            .enumerate()
            .map(|(i, _)| ScoredItem {
                index: i,
                score: 0,
                indices: Vec::new(),
            })
            .collect();
    }

    let pattern = Pattern::new(
        query,
        CaseMatching::Ignore,
        Normalization::Smart,
        AtomKind::Fuzzy,
    );

    let mut buf = Vec::new();
    let mut results: Vec<ScoredItem> = items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            let haystack = Utf32Str::new(&item.name, &mut buf);
            let mut indices = Vec::new();
            let score = pattern.indices(haystack, matcher, &mut indices)?;
            indices.sort_unstable();
            indices.dedup();
            Some(ScoredItem {
                index: i,
                score,
                indices,
            })
        })
        .collect();

    results.sort_by(|a, b| b.score.cmp(&a.score));
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::picker::PickerItem;

    fn make_items(names: &[&str]) -> Vec<PickerItem> {
        names
            .iter()
            .map(|n| PickerItem {
                name: (*n).to_string(),
                is_current: false,
            })
            .collect()
    }

    #[test]
    fn empty_query_returns_all_in_order() {
        let items = make_items(&["alpha", "beta", "gamma"]);
        let scored = score_items(&items, "");

        assert_eq!(scored.len(), 3);
        for (i, s) in scored.iter().enumerate() {
            assert_eq!(s.index, i);
            assert_eq!(s.score, 0);
            assert!(s.indices.is_empty());
        }
    }

    #[test]
    fn matching_query_returns_subset() {
        let items = make_items(&["production", "staging", "dev-prod-eu"]);
        let scored = score_items(&items, "prod");

        assert!(!scored.is_empty());
        for s in &scored {
            let name = &items[s.index].name;
            assert!(name.contains("prod"), "expected {name:?} to match 'prod'");
        }
    }

    #[test]
    fn results_sorted_by_score_descending() {
        let items = make_items(&["abc", "aXbXc", "axyzbc"]);
        let scored = score_items(&items, "abc");

        for pair in scored.windows(2) {
            assert!(
                pair[0].score >= pair[1].score,
                "scores should be descending: {} >= {}",
                pair[0].score,
                pair[1].score
            );
        }
    }

    #[test]
    fn non_matching_query_returns_empty() {
        let items = make_items(&["alpha", "beta", "gamma"]);
        let scored = score_items(&items, "zzz");
        assert!(scored.is_empty());
    }

    #[test]
    fn case_insensitive_matching() {
        let items = make_items(&["Production", "staging"]);
        let scored = score_items(&items, "production");

        assert_eq!(scored.len(), 1);
        assert_eq!(items[scored[0].index].name, "Production");
    }
}
