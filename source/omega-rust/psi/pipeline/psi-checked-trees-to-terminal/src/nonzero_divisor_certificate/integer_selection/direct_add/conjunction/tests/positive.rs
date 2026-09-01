use psi_core::IntegerSign;

use super::super::model::SearchBudget;
use super::fixture::{fork_join, outer_fork_join, shared_join};

#[test]
fn fork_join_is_produced_and_independently_admitted_for_fixed_integer_families() {
    for (sign, bits, lower, commute) in [
        (IntegerSign::Unsigned, 8, false, false),
        (IntegerSign::Unsigned, 64, true, true),
        (IntegerSign::Signed, 8, true, false),
        (IntegerSign::Signed, 64, false, true),
    ] {
        let fixture = fork_join(sign, bits, lower, commute);
        let outcome = fixture.prove(SearchBudget::default());
        assert!(!outcome.exhausted);
        assert_eq!(outcome.usage.definition_visits, 3);
        assert_eq!(outcome.usage.peak_depth, 2);
        fixture.admit(&outcome);
    }
}

#[test]
fn one_internal_computed_join_is_kernel_admitted_in_both_outer_orders() {
    for (sign, bits, lower, commute) in [
        (IntegerSign::Unsigned, 8, false, false),
        (IntegerSign::Unsigned, 64, true, true),
        (IntegerSign::Signed, 8, true, false),
        (IntegerSign::Signed, 64, false, true),
    ] {
        let fixture = outer_fork_join(sign, bits, lower, commute);
        let outcome = fixture.prove(SearchBudget::default());
        assert!(!outcome.exhausted);
        assert_eq!(outcome.usage.definition_visits, 4);
        assert_eq!(outcome.usage.peak_depth, 3);
        assert_eq!(outcome.usage.computed_joins, 1);
        fixture.admit(&outcome);
    }
}

#[test]
fn shared_affine_chain_endpoint_is_memoized_and_deterministic() {
    let fixture = shared_join(IntegerSign::Unsigned, 16, false);
    let first = fixture.prove(SearchBudget::default());
    let second = fixture.prove(SearchBudget::default());
    assert_eq!(first.proof, second.proof);
    assert_eq!(first.usage, second.usage);
    assert_eq!(first.usage.definition_visits, 2);
    assert_eq!(first.usage.peak_depth, 2);
    assert_eq!(first.usage.memo_hits, 1);
    fixture.admit(&first);
}
