use psi_core::IntegerSign;

use super::super::model::SearchBudget;
use super::fixture::{fork_join, shared_join};

#[test]
fn definition_budget_is_shared_across_forked_branches() {
    let fixture = fork_join(IntegerSign::Unsigned, 8, false, false);
    let exact = fixture.prove(SearchBudget {
        definition_visits: 3,
        depth: 2,
    });
    assert!(exact.proof.is_some());
    assert_eq!(exact.usage.definition_visits, 3);
    assert_eq!(exact.usage.peak_depth, 2);
    assert!(!exact.exhausted);

    let first_over = fixture.prove(SearchBudget {
        definition_visits: 2,
        depth: 2,
    });
    assert!(first_over.proof.is_none());
    assert_eq!(first_over.usage.definition_visits, 2);
    assert!(first_over.exhausted);
}

#[test]
fn depth_budget_refuses_the_first_nested_definition_atomically() {
    let fixture = fork_join(IntegerSign::Signed, 16, true, false);
    let outcome = fixture.prove(SearchBudget {
        definition_visits: 8,
        depth: 0,
    });
    assert!(outcome.proof.is_none());
    assert_eq!(outcome.usage.definition_visits, 1);
    assert_eq!(outcome.usage.peak_depth, 0);
    assert!(outcome.exhausted);

    let replay = fixture.prove(SearchBudget {
        definition_visits: 3,
        depth: 2,
    });
    assert!(
        replay.proof.is_some(),
        "exhaustion leaks no partial query state"
    );
    fixture.admit(&replay);
}

#[test]
fn memoized_branch_does_not_consume_a_second_definition_budget() {
    let fixture = shared_join(IntegerSign::Unsigned, 32, false);
    let outcome = fixture.prove(SearchBudget {
        definition_visits: 2,
        depth: 2,
    });
    assert!(outcome.proof.is_some());
    assert_eq!(outcome.usage.definition_visits, 2);
    assert_eq!(outcome.usage.memo_hits, 1);
    fixture.admit(&outcome);
}
