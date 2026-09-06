use crate::tests::*;

use super::fixture::staged_integer_less_or_equal_conditional;

#[test]
fn runtime_u64_parameter_less_or_equal_selects_reversed_compare_on_both_isas() {
    let comparison_operation = OperationId::new(19_411).unwrap();
    let left = ValueId::new(19_405).unwrap();
    let right = ValueId::new(19_406).unwrap();
    let condition = ValueId::new(19_407).unwrap();
    let true_edge = EdgeId::new(19_414).unwrap();
    let false_edge = EdgeId::new(19_415).unwrap();

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_integer_less_or_equal_conditional(target);
        assert!(matches!(
            &staged.optimized_target().target_operations().functions[0].operation,
            TargetOperation::ReturnIntegerExpressionConditionalControl {
                condition_source,
                condition: target_operations::TargetBooleanExpression::IntegerLessOrEqual {
                    psi_operation,
                    scalar_type,
                    left: target_left,
                    right: target_right,
                },
                scalar_type: result_type,
                ..
            } if *condition_source == condition
                && *psi_operation == comparison_operation
                && *scalar_type == IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
                && *result_type == IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
                && matches!(
                    target_left.as_ref(),
                    TargetIntegerExpression::Parameter { source_value, parameter_index: 0, .. }
                        if *source_value == left
                )
                && matches!(
                    target_right.as_ref(),
                    TargetIntegerExpression::Parameter { source_value, parameter_index: 1, .. }
                        if *source_value == right
                )
        ));
        validate_legalized_operations(
            staged.optimized_target().target_operations(),
            staged.optimized_target().optimized().plan(),
            staged.optimized_target().optimized().unit(),
            staged.legalized().plan().clone(),
        )
        .unwrap();
        validate_raw_selection(&staged, staged.selected().plan().clone()).unwrap();

        let legalized = staged.legalized().plan().functions[0].conditional();
        assert_eq!(
            legalized.recipe,
            LegalizationRecipe::ReturnU64IntegerLessOrEqualParametersConditionalV1
        );
        let legalized_operations::LegalizedCondition::IntegerLessOrEqualParametersV1 {
            operation,
            left: legalized_left,
            right: legalized_right,
            fuel,
            ..
        } = &legalized.condition
        else {
            panic!("legalization must retain authored inclusive ordering")
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
            panic!("entry must contain exactly one reversed ordered compare")
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

        let SelectedTerminator::ConditionalBranchU64LessThan {
            instruction,
            when_less,
            when_not_less,
        } = &entry.terminator
        else {
            panic!("inclusive ordering must branch when authored right is less than left")
        };
        assert_eq!(
            instruction.kind,
            SelectedInstructionKind::ConditionalBranchU64LessThan
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
fn inclusive_runtime_predicate_covers_order_and_u64_boundaries_on_both_isas() {
    let cases = [
        (7_u64, 9_u64, true),
        (9, 9, true),
        (9, 7, false),
        (0, 0, true),
        (0, u64::MAX, true),
        (u64::MAX, 0, false),
        (u64::MAX, u64::MAX, true),
    ];

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_integer_less_or_equal_conditional(target);
        let SelectedTerminator::ConditionalBranchU64LessThan {
            when_less,
            when_not_less,
            ..
        } = &staged.selected().plan().functions[0].blocks[0].terminator
        else {
            panic!("fixture must use the reversed strict-less-than predicate")
        };
        for (left, right, expected_true) in cases {
            let reversed_compare_is_less = right < left;
            let selected_edge = if reversed_compare_is_less {
                when_less.psi_edge
            } else {
                when_not_less.psi_edge
            };
            assert_eq!(
                selected_edge == EdgeId::new(19_414).unwrap(),
                expected_true,
                "{left} <= {right} on {target:?}"
            );
        }
    }
}
