// Rust guideline compliant 2026-02-21
//! Integration tests for picker scoring and fuzzy matching.

use khop::picker::{PickerItem, ScoredItem, score_items};

fn make_items(names: &[&str]) -> Vec<PickerItem> {
    names
        .iter()
        .map(|n| PickerItem {
            name: (*n).to_string(),
            is_current: false,
        })
        .collect()
}

fn names_from_scored<'a>(items: &'a [PickerItem], scored: &[ScoredItem]) -> Vec<&'a str> {
    scored.iter().map(|s| items[s.index].name.as_str()).collect()
}

// -- Empty query ----------------------------------------------------------

#[test]
fn test_empty_query_returns_all_items_in_original_order() {
    let items = make_items(&["production", "staging", "dev"]);

    let scored = score_items(&items, "");

    assert_eq!(scored.len(), items.len());
    for (i, s) in scored.iter().enumerate() {
        assert_eq!(s.index, i, "original order must be preserved");
        assert_eq!(s.score, 0, "empty query must produce zero score");
        assert!(s.indices.is_empty(), "empty query must produce no match indices");
    }
}

// -- Exact match ----------------------------------------------------------

#[test]
fn test_exact_match_scores_highest() {
    let items = make_items(&["staging", "production", "prod-eu"]);

    let scored = score_items(&items, "production");

    assert!(!scored.is_empty());
    let top = &scored[0];
    assert_eq!(
        items[top.index].name, "production",
        "exact full-string match should rank first"
    );
    for s in &scored[1..] {
        assert!(
            top.score >= s.score,
            "exact match score ({}) must be >= other scores ({})",
            top.score,
            s.score
        );
    }
}

// -- Prefix match ---------------------------------------------------------

#[test]
fn test_prefix_match_scores_well() {
    let items = make_items(&["staging-eu", "dev-staging", "staging"]);

    let scored = score_items(&items, "stag");

    assert!(!scored.is_empty());
    let top_name = &items[scored[0].index].name;
    assert!(
        top_name.starts_with("stag"),
        "a prefix match should rank near the top, got {top_name:?}"
    );
}

// -- Fuzzy matching -------------------------------------------------------

#[test]
fn test_fuzzy_matching_finds_non_contiguous_characters() {
    let items = make_items(&["production", "staging", "development"]);

    let scored = score_items(&items, "prd");

    let matched_names = names_from_scored(&items, &scored);
    assert!(
        matched_names.contains(&"production"),
        "fuzzy query 'prd' should match 'production', got {matched_names:?}"
    );
}

#[test]
fn test_fuzzy_matching_ranks_tighter_matches_higher() {
    let items = make_items(&["abc", "a---b---c", "aXbXc"]);

    let scored = score_items(&items, "abc");

    assert!(scored.len() >= 2);
    assert_eq!(
        items[scored[0].index].name, "abc",
        "contiguous match should rank first"
    );
}

// -- Non-matching query ---------------------------------------------------

#[test]
fn test_non_matching_query_returns_empty() {
    let items = make_items(&["production", "staging", "dev"]);

    let scored = score_items(&items, "zzzzz");

    assert!(scored.is_empty(), "no items should match 'zzzzz'");
}

#[test]
fn test_non_matching_on_empty_items_returns_empty() {
    let items: Vec<PickerItem> = Vec::new();

    let scored = score_items(&items, "anything");

    assert!(scored.is_empty());
}

// -- Case insensitive matching --------------------------------------------

#[test]
fn test_case_insensitive_lowercase_query_matches_uppercase_item() {
    let items = make_items(&["Production", "Staging"]);

    let scored = score_items(&items, "production");

    assert_eq!(scored.len(), 1);
    assert_eq!(items[scored[0].index].name, "Production");
}

#[test]
fn test_case_insensitive_uppercase_query_matches_lowercase_item() {
    let items = make_items(&["production", "staging"]);

    let scored = score_items(&items, "PROD");

    let matched_names = names_from_scored(&items, &scored);
    assert!(
        matched_names.contains(&"production"),
        "uppercase query 'PROD' should match 'production', got {matched_names:?}"
    );
}

#[test]
fn test_case_insensitive_mixed_case() {
    let items = make_items(&["Dev-Cluster-EU", "dev-cluster-us", "staging"]);

    let scored = score_items(&items, "dev-cluster");

    assert_eq!(scored.len(), 2);
    let matched_names = names_from_scored(&items, &scored);
    assert!(matched_names.contains(&"Dev-Cluster-EU"));
    assert!(matched_names.contains(&"dev-cluster-us"));
}

// -- Original indices -----------------------------------------------------

#[test]
fn test_scored_items_contain_correct_original_indices() {
    let items = make_items(&["alpha", "beta", "gamma", "delta"]);

    let scored = score_items(&items, "a");

    for s in &scored {
        let name = &items[s.index].name;
        assert!(
            name.contains('a'),
            "item at index {} ({name:?}) should contain 'a'",
            s.index
        );
    }

    // Verify the index range is valid.
    for s in &scored {
        assert!(
            s.index < items.len(),
            "index {} must be < items.len() ({})",
            s.index,
            items.len()
        );
    }
}

#[test]
fn test_scored_items_indices_are_unique() {
    let items = make_items(&["aaa", "aab", "abc", "bcd"]);

    let scored = score_items(&items, "a");

    let mut seen = std::collections::HashSet::new();
    for s in &scored {
        assert!(
            seen.insert(s.index),
            "duplicate index {} in scored results",
            s.index
        );
    }
}

// -- Match indices for highlighting ---------------------------------------

#[test]
fn test_match_indices_populated_for_exact_query() {
    let items = make_items(&["production"]);

    let scored = score_items(&items, "prod");

    assert_eq!(scored.len(), 1);
    let s = &scored[0];
    assert!(
        !s.indices.is_empty(),
        "match indices must be populated for a matching query"
    );
    // "prod" should match the first 4 characters.
    assert!(
        s.indices.contains(&0) && s.indices.contains(&1)
            && s.indices.contains(&2) && s.indices.contains(&3),
        "expected indices [0,1,2,3] for prefix 'prod', got {:?}",
        s.indices
    );
}

#[test]
fn test_match_indices_are_sorted_and_deduplicated() {
    let items = make_items(&["production", "dev-prod-eu"]);

    let scored = score_items(&items, "prod");

    for s in &scored {
        for w in s.indices.windows(2) {
            assert!(
                w[0] < w[1],
                "indices must be sorted and unique: {:?}",
                s.indices
            );
        }
    }
}

#[test]
fn test_match_indices_empty_for_empty_query() {
    let items = make_items(&["production"]);

    let scored = score_items(&items, "");

    assert_eq!(scored.len(), 1);
    assert!(
        scored[0].indices.is_empty(),
        "empty query must produce no match indices"
    );
}

#[test]
fn test_match_indices_within_item_name_bounds() {
    let items = make_items(&["dev", "staging", "production"]);

    let scored = score_items(&items, "t");

    for s in &scored {
        #[allow(clippy::cast_possible_truncation)]
        let name_len = items[s.index].name.len() as u32;
        for &idx in &s.indices {
            assert!(
                idx < name_len,
                "match index {idx} out of bounds for {:?} (len {name_len})",
                items[s.index].name
            );
        }
    }
}
