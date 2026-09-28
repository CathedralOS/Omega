//! Algebraic identity convergence and deterministic overlap precedence.
use super::super::super::super::{OptimizationRunError, run_unit};
use super::super::super::{
    Optimization, OptimizationSelections, WrappingNeutralOperation, budget, built_in_psi_registry,
    wrapping_neutral_identity_unit,
};
use crate::rules::tests::fixtures::global_value_numbering::{
    BitwiseNeutralOperation, SaturatingNeutralOperation, bitwise_neutral_identity_unit,
    saturating_neutral_identity_unit,
    wrapping_neutral_identity_unit_with_value_and_identity_types_and_liveness,
};

#[test]
fn total_scalar_identity_rules_are_runtime_disabled_deterministic_and_budgeted() {
    let cases = [
        wrapping_neutral_identity_unit(
            WrappingNeutralOperation::Add,
            semantic_vocabulary::IntegerValue::Unsigned(0),
            false,
            false,
        ),
        wrapping_neutral_identity_unit_with_value_and_identity_types_and_liveness(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 17)
                .unwrap(),
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
                .unwrap(),
            WrappingNeutralOperation::ShiftLeft,
            semantic_vocabulary::IntegerValue::Unsigned(0),
            false,
            false,
            true,
        ),
        wrapping_neutral_identity_unit(
            WrappingNeutralOperation::Multiply,
            semantic_vocabulary::IntegerValue::Unsigned(0),
            false,
            false,
        ),
        saturating_neutral_identity_unit(
            SaturatingNeutralOperation::Add,
            semantic_vocabulary::IntegerValue::Unsigned(0),
            false,
            false,
        ),
        saturating_neutral_identity_unit(
            SaturatingNeutralOperation::Multiply,
            semantic_vocabulary::IntegerValue::Unsigned(0),
            false,
            false,
        ),
        bitwise_neutral_identity_unit(
            BitwiseNeutralOperation::Or,
            semantic_vocabulary::IntegerValue::Unsigned(0),
            false,
            false,
        ),
        bitwise_neutral_identity_unit(
            BitwiseNeutralOperation::And,
            semantic_vocabulary::IntegerValue::Unsigned(0),
            false,
            false,
        ),
    ];
    let disabled = built_in_psi_registry(&OptimizationSelections::default()).unwrap();
    let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();

    for unit in cases {
        let (unchanged, commits, usage, decisions, manifest, ledger) =
            run_unit(unit.clone(), &disabled, budget(8)).unwrap();
        assert_eq!(unchanged, unit);
        assert!(commits.is_empty());
        assert_eq!(usage.iterations, 1);
        assert_eq!(usage.rule_evaluations, 0);
        assert!(decisions.records.is_empty());
        assert!(manifest.is_none());
        assert!(ledger.records().is_empty());

        let first = run_unit(unit.clone(), &registry, budget(8)).unwrap();
        let second = run_unit(unit.clone(), &registry, budget(8)).unwrap();
        assert_eq!(first.0, second.0);
        assert_eq!(first.1, second.1);
        assert_eq!(first.2, second.2);
        assert_eq!(first.3, second.3);
        assert_eq!(first.4, second.4);
        assert_eq!(first.5, second.5);

        let first_error = run_unit(unit.clone(), &registry, budget(1)).unwrap_err();
        let second_error = run_unit(unit, &registry, budget(1)).unwrap_err();
        assert_eq!(first_error, second_error);
        assert_eq!(
            first_error,
            OptimizationRunError::WorkBudgetExhausted("iterations")
        );
    }
}
