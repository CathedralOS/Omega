//! Independent rejection for signed inclusive-order and successor corruption.

use crate::tests::*;
use legalized_operations::{LegalizedScalarComparison, LegalizedScalarInstructionKind};

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
        let LegalizedScalarInstructionKind::Compare { left, right, .. } =
            &mut swapped.scalar_functions[0].blocks[0].instructions[0].kind
        else {
            panic!("fixture must retain authored signed inclusive order")
        };
        std::mem::swap(left, right);
        assert_eq!(
            validate(swapped),
            Err(LegalizationError::NonCanonicalLegalizedPlan)
        );

        let mut strict = original.clone();
        let LegalizedScalarInstructionKind::Compare { predicate, .. } =
            &mut strict.scalar_functions[0].blocks[0].instructions[0].kind
        else {
            unreachable!()
        };
        *predicate = LegalizedScalarComparison::LessThan;
        assert_eq!(
            validate(strict),
            Err(LegalizationError::NonCanonicalLegalizedPlan)
        );

        let mut unsigned = original.clone();
        let LegalizedScalarInstructionKind::Compare { operand_type, .. } =
            &mut unsigned.scalar_functions[0].blocks[0].instructions[0].kind
        else {
            unreachable!()
        };
        *operand_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        assert_eq!(
            validate(unsigned),
            Err(LegalizationError::NonCanonicalLegalizedPlan)
        );
    }
}

#[test]
fn signed_predicate_compare_order_and_successor_corruption_fail_closed() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_signed_integer_less_or_equal_conditional(target);

        let mut operand_swap = staged.selected().plan().clone();
        operand_swap.functions[0].blocks[0]
            .instructions
            .iter_mut()
            .find(|instruction| instruction.kind == SelectedInstructionKind::CompareI64)
            .unwrap()
            .operands
            .swap(0, 1);
        assert!(matches!(
            validate_raw_selection(&staged, operand_swap),
            Err(SelectedInstructionError::FunctionProjectionMismatch { function: 0 })
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
            Err(SelectedInstructionError::SourceCustodyMismatch)
        ));
    }
}
