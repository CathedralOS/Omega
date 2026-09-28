use semantic_vocabulary::IntegerSign;

use super::super::SearchBudget;
use super::fixture::remainder_leaf_joins;

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
