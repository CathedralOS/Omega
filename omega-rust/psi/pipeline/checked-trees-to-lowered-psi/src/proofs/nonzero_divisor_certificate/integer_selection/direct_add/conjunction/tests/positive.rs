use semantic_vocabulary::IntegerSign;

use super::super::model::SearchBudget;
use super::fixture::{fork_join, outer_fork_join, remainder_leaf_joins, shared_join};

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

/// dice_roller shape: `((r1 + r2) + r3) + r4` over rolls defined as
/// `remainder(dividend, 6) + 1`. Remainder-defined operands terminate the chain
/// through their bounded total image, and three nested computed joins compose
/// the partial sums — more than the previous single-join envelope.
#[test]
fn remainder_defined_leaves_terminate_nested_computed_join_chains() {
    for (sign, bits, lower) in [
        (IntegerSign::Unsigned, 8, false),
        (IntegerSign::Unsigned, 8, true),
        (IntegerSign::Unsigned, 64, false),
        (IntegerSign::Signed, 8, false),
        (IntegerSign::Signed, 8, true),
        (IntegerSign::Signed, 32, false),
        (IntegerSign::Signed, 32, true),
    ] {
        let fixture = remainder_leaf_joins(sign, bits, lower);
        let outcome = fixture.prove(SearchBudget::default());
        assert!(
            outcome.proof.is_some(),
            "{sign:?}{bits} lower={lower}: exhausted={} usage={:?}",
            outcome.exhausted,
            outcome.usage
        );
        assert!(!outcome.exhausted);
        assert_eq!(outcome.usage.computed_joins, 3);
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
