//! Algebraic identity convergence and deterministic overlap precedence.

use super::super::super::*;
use crate::rules::tests::{
    BitwiseNeutralOperation, SaturatingNeutralOperation, bitwise_literal_pair_unit,
    bitwise_neutral_identity_unit, saturating_multiply_literal_pair_unit,
    saturating_neutral_identity_unit, wrapping_multiply_literal_pair_unit,
    wrapping_neutral_identity_unit_with_value_and_identity_types_and_liveness,
};
use crate::rules::{
    BitwiseAbsorbingLiteralIdentityRule, BitwiseNeutralLiteralIdentityRule,
    SaturatingMultiplyZeroAnnihilationRule, SaturatingNeutralArithmeticIdentityRule,
    WrappingMultiplyZeroAnnihilationRule, WrappingNeutralArithmeticIdentityRule,
    WrappingShiftZeroCountIdentityRule,
};

#[test]
fn named_global_value_numbering_reaches_a_wrapping_neutral_identity_fixed_point() {
    let unit = wrapping_neutral_identity_unit(
        WrappingNeutralOperation::Add,
        semantic_vocabulary::IntegerValue::Unsigned(0),
        false,
        false,
    );
    let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let (output, commits, usage, _, manifest, ledger) =
        run_unit(unit, &registry, budget(8)).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(usage.iterations, 2);
    assert_eq!(usage.rule_evaluations, 26);
    assert_eq!(usage.candidates, 1);
    assert_eq!(usage.validation_steps, 1);
    assert_eq!(manifest.unwrap().ordered_rules().len(), 16);
    assert_eq!(ledger.records().len(), 1);
    assert!(matches!(
        output.functions[0].blocks[0].nodes[1].operation,
        AbstractOperation::Return { value, .. }
            if value == semantic_vocabulary::ValueId::new(1_903).unwrap()
    ));

    let (second, second_commits, second_usage, _, _, second_ledger) =
        run_unit(output.clone(), &registry, budget(8)).unwrap();
    assert_eq!(second.identity, output.identity);
    assert!(second_commits.is_empty());
    assert_eq!(second_usage.iterations, 1);
    assert_eq!(second_usage.rule_evaluations, 16);
    assert!(second_ledger.records().is_empty());
}

#[test]
fn named_global_value_numbering_reaches_a_wrapping_shift_zero_fixed_point() {
    let unit = wrapping_neutral_identity_unit_with_value_and_identity_types_and_liveness(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 17)
            .unwrap(),
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
            .unwrap(),
        WrappingNeutralOperation::ShiftRight,
        semantic_vocabulary::IntegerValue::Unsigned(0),
        false,
        false,
        true,
    );
    let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let (output, commits, usage, _, manifest, ledger) =
        run_unit(unit, &registry, budget(8)).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(usage.iterations, 2);
    assert_eq!(usage.rule_evaluations, 27);
    assert_eq!(usage.candidates, 1);
    assert_eq!(usage.validation_steps, 1);
    let manifest = manifest.unwrap();
    assert_eq!(manifest.ordered_rules().len(), 16);
    assert_eq!(
        manifest.decisions()[0].rule(),
        WrappingShiftZeroCountIdentityRule::contract().identity()
    );
    assert_eq!(ledger.records().len(), 1);
    assert!(matches!(
        output.functions[0].blocks[0].nodes[1].operation,
        AbstractOperation::Return { value, .. }
            if value == semantic_vocabulary::ValueId::new(1_903).unwrap()
    ));

    let (second, second_commits, second_usage, _, _, second_ledger) =
        run_unit(output.clone(), &registry, budget(8)).unwrap();
    assert_eq!(second, output);
    assert!(second_commits.is_empty());
    assert_eq!(second_usage.iterations, 1);
    assert_eq!(second_usage.rule_evaluations, 16);
    assert!(second_ledger.records().is_empty());
}

#[test]
fn named_global_value_numbering_reaches_a_wrapping_multiply_zero_fixed_point() {
    let unit = wrapping_neutral_identity_unit(
        WrappingNeutralOperation::Multiply,
        semantic_vocabulary::IntegerValue::Unsigned(0),
        false,
        false,
    );
    let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let (output, commits, usage, _, manifest, ledger) =
        run_unit(unit, &registry, budget(8)).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(usage.iterations, 2);
    assert_eq!(usage.rule_evaluations, 28);
    assert_eq!(usage.candidates, 1);
    assert_eq!(usage.validation_steps, 1);
    let manifest = manifest.unwrap();
    assert_eq!(manifest.ordered_rules().len(), 16);
    assert_eq!(
        manifest.decisions()[0].rule(),
        WrappingMultiplyZeroAnnihilationRule::contract().identity()
    );
    assert_eq!(ledger.records().len(), 1);
    assert!(matches!(
        output.functions[0].blocks[0].nodes[1].operation,
        AbstractOperation::Return { value, .. }
            if value == semantic_vocabulary::ValueId::new(1_904).unwrap()
    ));

    let (second, second_commits, second_usage, _, _, second_ledger) =
        run_unit(output.clone(), &registry, budget(8)).unwrap();
    assert_eq!(second, output);
    assert!(second_commits.is_empty());
    assert_eq!(second_usage.iterations, 1);
    assert_eq!(second_usage.rule_evaluations, 16);
    assert!(second_ledger.records().is_empty());
}

#[test]
fn named_global_value_numbering_reaches_a_saturating_neutral_identity_fixed_point() {
    let unit = saturating_neutral_identity_unit(
        SaturatingNeutralOperation::Add,
        semantic_vocabulary::IntegerValue::Unsigned(0),
        false,
        false,
    );
    let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let (output, commits, usage, _, manifest, ledger) =
        run_unit(unit, &registry, budget(8)).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(usage.iterations, 2);
    assert_eq!(usage.rule_evaluations, 29);
    assert_eq!(usage.candidates, 1);
    assert_eq!(usage.validation_steps, 1);
    let manifest = manifest.unwrap();
    assert_eq!(manifest.ordered_rules().len(), 16);
    assert_eq!(
        manifest.decisions()[0].rule(),
        SaturatingNeutralArithmeticIdentityRule::contract().identity()
    );
    assert_eq!(ledger.records().len(), 1);
    assert!(matches!(
        output.functions[0].blocks[0].nodes[1].operation,
        AbstractOperation::Return { value, .. }
            if value == semantic_vocabulary::ValueId::new(1_903).unwrap()
    ));

    let (second, second_commits, second_usage, _, _, second_ledger) =
        run_unit(output.clone(), &registry, budget(8)).unwrap();
    assert_eq!(second.identity, output.identity);
    assert!(second_commits.is_empty());
    assert_eq!(second_usage.iterations, 1);
    assert_eq!(second_usage.rule_evaluations, 16);
    assert!(second_ledger.records().is_empty());
}

#[test]
fn named_global_value_numbering_reaches_a_saturating_multiply_zero_fixed_point() {
    let unit = saturating_neutral_identity_unit(
        SaturatingNeutralOperation::Multiply,
        semantic_vocabulary::IntegerValue::Unsigned(0),
        false,
        false,
    );
    let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let (output, commits, usage, _, manifest, ledger) =
        run_unit(unit, &registry, budget(8)).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(usage.iterations, 2);
    assert_eq!(usage.rule_evaluations, 30);
    assert_eq!(usage.candidates, 1);
    assert_eq!(usage.validation_steps, 1);
    let manifest = manifest.unwrap();
    assert_eq!(manifest.ordered_rules().len(), 16);
    assert_eq!(
        manifest.decisions()[0].rule(),
        SaturatingMultiplyZeroAnnihilationRule::contract().identity()
    );
    assert_eq!(ledger.records().len(), 1);
    assert!(matches!(
        output.functions[0].blocks[0].nodes[1].operation,
        AbstractOperation::Return { value, .. }
            if value == semantic_vocabulary::ValueId::new(1_904).unwrap()
    ));

    let (second, second_commits, second_usage, _, _, second_ledger) =
        run_unit(output.clone(), &registry, budget(8)).unwrap();
    assert_eq!(second.identity, output.identity);
    assert!(second_commits.is_empty());
    assert_eq!(second_usage.iterations, 1);
    assert_eq!(second_usage.rule_evaluations, 16);
    assert!(second_ledger.records().is_empty());
}

#[test]
fn named_global_value_numbering_reaches_a_bitwise_neutral_fixed_point() {
    let unit = bitwise_neutral_identity_unit(
        BitwiseNeutralOperation::Or,
        semantic_vocabulary::IntegerValue::Unsigned(0),
        false,
        false,
    );
    let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let (output, commits, usage, _, manifest, ledger) =
        run_unit(unit, &registry, budget(8)).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(usage.iterations, 2);
    assert_eq!(usage.rule_evaluations, 31);
    assert_eq!(usage.candidates, 1);
    assert_eq!(usage.validation_steps, 1);
    let manifest = manifest.unwrap();
    assert_eq!(manifest.ordered_rules().len(), 16);
    assert_eq!(
        manifest.decisions()[0].rule(),
        BitwiseNeutralLiteralIdentityRule::contract().identity()
    );
    assert_eq!(ledger.records().len(), 1);
    assert!(matches!(
        output.functions[0].blocks[0].nodes[1].operation,
        AbstractOperation::Return { value, .. }
            if value == semantic_vocabulary::ValueId::new(1_903).unwrap()
    ));

    let (second, second_commits, second_usage, _, _, second_ledger) =
        run_unit(output.clone(), &registry, budget(8)).unwrap();
    assert_eq!(second.identity, output.identity);
    assert!(second_commits.is_empty());
    assert_eq!(second_usage.iterations, 1);
    assert_eq!(second_usage.rule_evaluations, 16);
    assert!(second_ledger.records().is_empty());
}

#[test]
fn named_global_value_numbering_reaches_a_bitwise_absorbing_fixed_point() {
    let unit = bitwise_neutral_identity_unit(
        BitwiseNeutralOperation::And,
        semantic_vocabulary::IntegerValue::Unsigned(0),
        false,
        false,
    );
    let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let (output, commits, usage, _, manifest, ledger) =
        run_unit(unit, &registry, budget(8)).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(usage.iterations, 2);
    assert_eq!(usage.rule_evaluations, 32);
    assert_eq!(usage.candidates, 1);
    assert_eq!(usage.validation_steps, 1);
    let manifest = manifest.unwrap();
    assert_eq!(manifest.ordered_rules().len(), 16);
    assert_eq!(
        manifest.decisions()[0].rule(),
        BitwiseAbsorbingLiteralIdentityRule::contract().identity()
    );
    assert_eq!(ledger.records().len(), 1);
    assert!(matches!(
        output.functions[0].blocks[0].nodes[1].operation,
        AbstractOperation::Return { value, .. }
            if value == semantic_vocabulary::ValueId::new(1_904).unwrap()
    ));

    let (second, second_commits, second_usage, _, _, second_ledger) =
        run_unit(output.clone(), &registry, budget(8)).unwrap();
    assert_eq!(second.identity, output.identity);
    assert!(second_commits.is_empty());
    assert_eq!(second_usage.iterations, 1);
    assert_eq!(second_usage.rule_evaluations, 16);
    assert!(second_ledger.records().is_empty());
}

#[test]
fn bitwise_absorbing_overlap_uses_the_earlier_neutral_rule() {
    let all_ones = semantic_vocabulary::IntegerValue::Unsigned(u8::MAX.into());
    let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();

    for (operation, left, right, replacement) in [
        (
            BitwiseNeutralOperation::And,
            semantic_vocabulary::IntegerValue::Unsigned(0),
            all_ones,
            semantic_vocabulary::ValueId::new(1_953).unwrap(),
        ),
        (
            BitwiseNeutralOperation::And,
            all_ones,
            semantic_vocabulary::IntegerValue::Unsigned(0),
            semantic_vocabulary::ValueId::new(1_954).unwrap(),
        ),
        (
            BitwiseNeutralOperation::Or,
            semantic_vocabulary::IntegerValue::Unsigned(0),
            all_ones,
            semantic_vocabulary::ValueId::new(1_954).unwrap(),
        ),
        (
            BitwiseNeutralOperation::Or,
            all_ones,
            semantic_vocabulary::IntegerValue::Unsigned(0),
            semantic_vocabulary::ValueId::new(1_953).unwrap(),
        ),
    ] {
        let (output, commits, usage, _, manifest, ledger) = run_unit(
            bitwise_literal_pair_unit(operation, left, right),
            &registry,
            budget(8),
        )
        .unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(usage.rule_evaluations, 31);
        let manifest = manifest.unwrap();
        assert_eq!(manifest.ordered_rules().len(), 16);
        assert_eq!(
            manifest.decisions()[0].rule(),
            BitwiseNeutralLiteralIdentityRule::contract().identity()
        );
        assert_eq!(ledger.records().len(), 1);
        assert!(matches!(
            output.functions[0].blocks[0].nodes[2].operation,
            AbstractOperation::Return { value, .. } if value == replacement
        ));
    }
}

#[test]
fn saturating_multiply_zero_overlap_uses_the_earlier_neutral_rule() {
    let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();

    for (left, right, zero) in [
        (
            semantic_vocabulary::IntegerValue::Unsigned(0),
            semantic_vocabulary::IntegerValue::Unsigned(1),
            semantic_vocabulary::ValueId::new(1_953).unwrap(),
        ),
        (
            semantic_vocabulary::IntegerValue::Unsigned(1),
            semantic_vocabulary::IntegerValue::Unsigned(0),
            semantic_vocabulary::ValueId::new(1_954).unwrap(),
        ),
    ] {
        let (output, commits, usage, _, manifest, ledger) = run_unit(
            saturating_multiply_literal_pair_unit(left, right),
            &registry,
            budget(8),
        )
        .unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(usage.rule_evaluations, 29);
        let manifest = manifest.unwrap();
        assert_eq!(manifest.ordered_rules().len(), 16);
        assert_eq!(
            manifest.decisions()[0].rule(),
            SaturatingNeutralArithmeticIdentityRule::contract().identity()
        );
        assert_eq!(ledger.records().len(), 1);
        assert!(matches!(
            output.functions[0].blocks[0].nodes[2].operation,
            AbstractOperation::Return { value, .. } if value == zero
        ));
    }
}

#[test]
fn wrapping_multiply_zero_overlap_uses_the_earlier_neutral_rule() {
    let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();

    for (left, right, zero) in [
        (
            semantic_vocabulary::IntegerValue::Unsigned(0),
            semantic_vocabulary::IntegerValue::Unsigned(1),
            semantic_vocabulary::ValueId::new(1_953).unwrap(),
        ),
        (
            semantic_vocabulary::IntegerValue::Unsigned(1),
            semantic_vocabulary::IntegerValue::Unsigned(0),
            semantic_vocabulary::ValueId::new(1_954).unwrap(),
        ),
    ] {
        let (output, commits, usage, _, manifest, ledger) = run_unit(
            wrapping_multiply_literal_pair_unit(left, right),
            &registry,
            budget(8),
        )
        .unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(usage.rule_evaluations, 26);
        let manifest = manifest.unwrap();
        assert_eq!(manifest.ordered_rules().len(), 16);
        assert_eq!(
            manifest.decisions()[0].rule(),
            WrappingNeutralArithmeticIdentityRule::contract().identity()
        );
        assert_eq!(ledger.records().len(), 1);
        assert!(matches!(
            output.functions[0].blocks[0].nodes[2].operation,
            AbstractOperation::Return { value, .. } if value == zero
        ));
    }
}

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
