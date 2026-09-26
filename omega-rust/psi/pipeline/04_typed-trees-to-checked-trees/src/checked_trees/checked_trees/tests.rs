use crate::checked_trees::{CheckFacts, CheckedTrees};

#[test]
fn checked_tree_constructor_keeps_typed_tree_and_fact_roots_explicit() {
    let typed = symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees::default();
    let facts = CheckFacts::default();

    let checked = CheckedTrees::with_roots(typed.clone(), facts.clone());

    assert_eq!(checked.typed, typed);
    assert_eq!(checked.facts, facts);
}
