//! Independent rejection for signed inclusive-order and successor corruption.

use crate::tests::*;
use legalized_operations::LegalizedCondition;

use super::fixture::staged_signed_integer_less_or_equal_conditional;

#[test]
fn signed_inclusive_family_rejects_unsigned_strict_and_operand_substitution() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_signed_integer_less_or_equal_conditional(target);
        let original = staged.legalized().plan();
        let validate = |plan| {
            validate_legalized_operations(
                staged.optimized_target().target_operations(),
                staged.optimized_target().optimized().plan(),
                staged.optimized_target().optimized().unit(),
                plan,
            )
        };

        let mut swapped = original.clone();
        let LegalizedCondition::I64LessOrEqualParametersV1 { left, right, .. } =
            &mut swapped.functions[0].condition
        else {
            panic!("fixture must retain authored signed inclusive order")
        };
        std::mem::swap(left, right);
        assert_eq!(
            validate(swapped),
            Err(LegalizationError::NonCanonicalLegalizedPlan)
        );

        let LegalizedCondition::I64LessOrEqualParametersV1 {
            operation,
            result_definition_site,
            fuel,
            left,
            right,
        } = original.functions[0].condition.clone()
        else {
            unreachable!()
        };
        let mut strict = original.clone();
        strict.functions[0].condition = LegalizedCondition::I64LessThanParametersV1 {
            operation,
            result_definition_site,
            fuel: fuel.clone(),
            left: left.clone(),
            right: right.clone(),
        };
        assert_eq!(
            validate(strict),
            Err(LegalizationError::NonCanonicalLegalizedPlan)
        );

        let mut unsigned = original.clone();
        unsigned.functions[0].condition = LegalizedCondition::IntegerLessOrEqualParametersV1 {
            operation,
            result_definition_site,
            fuel,
            left,
            right,
        };
        assert_eq!(
            validate(unsigned),
            Err(LegalizationError::UnsupportedCondition { function: 0 })
        );
    }
}

#[test]
fn signed_predicate_compare_order_and_successor_corruption_fail_closed() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_signed_integer_less_or_equal_conditional(target);

        let mut operand_swap = staged.selected().plan().clone();
        operand_swap.functions[0].blocks[0].instructions[0]
            .operands
            .swap(0, 1);
        assert!(matches!(
            validate_raw_selection(&staged, operand_swap),
            Err(SelectedInstructionError::InstructionProjectionMismatch {
                function: 0,
                instruction: 0
            })
        ));

        let mut successor_swap = staged.selected().plan().clone();
        let SelectedTerminator::ConditionalBranchI64LessThan {
            when_less,
            when_not_less,
            ..
        } = &mut successor_swap.functions[0].blocks[0].terminator
        else {
            unreachable!()
        };
        std::mem::swap(when_less, when_not_less);
        assert!(matches!(
            validate_raw_selection(&staged, successor_swap),
            Err(SelectedInstructionError::SuccessorProjectionMismatch {
                function: 0,
                block: 0
            })
        ));
    }
}
