//! Exact selected graph and signed inclusive-order semantics.

use crate::tests::*;

use super::fixture::staged_signed_integer_less_or_equal_conditional;

#[test]
fn runtime_i64_parameter_less_or_equal_selects_reversed_signed_compare_on_both_isas() {
    let comparison_operation = OperationId::new(19_911).unwrap();
    let left = ValueId::new(19_905).unwrap();
    let right = ValueId::new(19_906).unwrap();
    let condition = ValueId::new(19_907).unwrap();
    let true_edge = EdgeId::new(19_914).unwrap();
    let false_edge = EdgeId::new(19_915).unwrap();

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_signed_integer_less_or_equal_conditional(target);
        validate_legalized_operations(
            staged.optimized_target().target_operations(),
            staged.optimized_target().optimized().plan(),
            staged.optimized_target().optimized().unit(),
            staged.legalized().plan().clone(),
        )
        .unwrap();
        validate_raw_selection(&staged, staged.selected().plan().clone()).unwrap();

        let legalized = &staged.legalized().plan().functions[0];
        assert_eq!(
            legalized.recipe,
            LegalizationRecipe::ReturnU64I64LessOrEqualParametersConditionalV1
        );
        let legalized_operations::LegalizedCondition::I64LessOrEqualParametersV1 {
            operation,
            left: legalized_left,
            right: legalized_right,
            fuel,
            ..
        } = &legalized.condition
        else {
            panic!("legalization must retain authored signed inclusive order")
        };
        assert_eq!(*operation, comparison_operation);
        assert_eq!(legalized_left.source_value, left);
        assert_eq!(legalized_left.parameter_index, 0);
        assert_eq!(legalized_right.source_value, right);
        assert_eq!(legalized_right.parameter_index, 1);
        assert_eq!(fuel[0].site, PsiProvenance::Operation(comparison_operation));

        let function = &staged.selected().plan().functions[0];
        assert_eq!(function.blocks.len(), 3);
        assert_eq!(function.virtual_registers.len(), 4);
        assert_eq!(staged.selected().receipt().instruction_count(), 6);
        let entry = &function.blocks[0];
        let [compare] = entry.instructions.as_slice() else {
            panic!("entry must contain exactly one reversed signed compare")
        };
        assert_eq!(compare.kind, SelectedInstructionKind::CompareI64);
        assert_eq!(
            compare
                .operands
                .iter()
                .map(|operand| operand.virtual_register)
                .collect::<Vec<_>>(),
            [VirtualRegisterId(1), VirtualRegisterId(0)]
        );
        assert_eq!(compare.provenance.operations, [comparison_operation]);

        let SelectedTerminator::ConditionalBranchI64LessThan {
            instruction,
            when_less,
            when_not_less,
        } = &entry.terminator
        else {
            panic!("inclusive signed ordering must branch when authored right is less than left")
        };
        assert_eq!(
            instruction.kind,
            SelectedInstructionKind::ConditionalBranchI64LessThan
        );
        assert_eq!(instruction.provenance.values, [condition]);
        assert_eq!(when_less.psi_edge, false_edge);
        assert_eq!(when_less.block, selected_instructions::SelectedBlockId(2));
        assert_eq!(when_not_less.psi_edge, true_edge);
        assert_eq!(
            when_not_less.block,
            selected_instructions::SelectedBlockId(1)
        );
        assert_eq!(when_less.fuel[0].site, PsiProvenance::Edge(false_edge));
        assert_eq!(when_not_less.fuel[0].site, PsiProvenance::Edge(true_edge));
    }
}

#[test]
fn inclusive_signed_predicate_covers_boundaries_and_disagrees_with_unsigned_order() {
    let values = [i64::MIN, -1, 0, 1, i64::MAX];
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_signed_integer_less_or_equal_conditional(target);
        let SelectedTerminator::ConditionalBranchI64LessThan {
            when_less,
            when_not_less,
            ..
        } = &staged.selected().plan().functions[0].blocks[0].terminator
        else {
            panic!("fixture must use the reversed signed-less-than predicate")
        };
        for left in values {
            for right in values {
                let selected_edge = if right < left {
                    when_less.psi_edge
                } else {
                    when_not_less.psi_edge
                };
                assert_eq!(
                    selected_edge == EdgeId::new(19_914).unwrap(),
                    left <= right,
                    "{left} <= {right} on {target:?}"
                );
            }
        }
    }

    assert!((i64::MIN as u64) > 0_u64);
    assert!((-1_i64 as u64) > 0_u64);
    assert!(0_u64 < (-1_i64 as u64));
}
