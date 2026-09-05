use crate::tests::*;

use super::fixture::staged_integer_not_equal_conditional;

#[test]
fn runtime_u64_parameter_inequality_selects_exact_three_block_graph_on_both_isas() {
    let equal_operation = OperationId::new(19_611).unwrap();
    let not_operation = OperationId::new(19_620).unwrap();
    let left = ValueId::new(19_605).unwrap();
    let right = ValueId::new(19_606).unwrap();
    let equal = ValueId::new(19_607).unwrap();
    let not_equal = ValueId::new(19_619).unwrap();
    let true_edge = EdgeId::new(19_614).unwrap();
    let false_edge = EdgeId::new(19_615).unwrap();

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_integer_not_equal_conditional(target);
        assert!(matches!(
            &staged.optimized_target().target_operations().functions[0].operation,
            TargetOperation::ReturnIntegerExpressionConditionalControl {
                condition_source,
                condition: target_operations::TargetBooleanExpression::Not {
                    psi_operation,
                    operand,
                },
                scalar_type: result_type,
                ..
            } if *condition_source == not_equal
                && *psi_operation == not_operation
                && *result_type == IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
                && matches!(
                    operand.as_ref(),
                    target_operations::TargetBooleanExpression::IntegerEqual {
                        psi_operation,
                        scalar_type,
                        left: target_left,
                        right: target_right,
                    } if *psi_operation == equal_operation
                        && *scalar_type == IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
                        && matches!(
                            target_left.as_ref(),
                            TargetIntegerExpression::Parameter {
                                source_value,
                                parameter_index: 0,
                                ..
                            } if *source_value == left
                        )
                        && matches!(
                            target_right.as_ref(),
                            TargetIntegerExpression::Parameter {
                                source_value,
                                parameter_index: 1,
                                ..
                            } if *source_value == right
                        )
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

        let legalized = &staged.legalized().plan().functions[0];
        assert_eq!(
            legalized.recipe,
            LegalizationRecipe::ReturnU64IntegerNotEqualParametersConditionalV1
        );
        let legalized_operations::LegalizedCondition::IntegerNotEqualParametersV1 {
            equality_operation: legalized_equal_operation,
            equality_result,
            equality_fuel,
            boolean_not_operation: legalized_not_operation,
            boolean_not_result,
            boolean_not_fuel,
            left: legalized_left,
            right: legalized_right,
            ..
        } = &legalized.condition
        else {
            panic!("legalization must retain equality followed by BooleanNot")
        };
        assert_eq!(*legalized_equal_operation, equal_operation);
        assert_eq!(*equality_result, equal);
        assert_eq!(*legalized_not_operation, not_operation);
        assert_eq!(*boolean_not_result, not_equal);
        assert_eq!(
            equality_fuel[0].site,
            PsiProvenance::Operation(equal_operation)
        );
        assert_eq!(
            boolean_not_fuel[0].site,
            PsiProvenance::Operation(not_operation)
        );
        assert_eq!(legalized_left.source_value, left);
        assert_eq!(legalized_left.parameter_index, 0);
        assert_eq!(legalized_right.source_value, right);
        assert_eq!(legalized_right.parameter_index, 1);

        let function = &staged.selected().plan().functions[0];
        assert_eq!(function.blocks.len(), 3);
        assert_eq!(function.virtual_registers.len(), 4);
        assert_eq!(staged.selected().receipt().instruction_count(), 6);
        let entry = &function.blocks[0];
        let [compare] = entry.instructions.as_slice() else {
            panic!("entry must contain exactly one equality compare")
        };
        assert_eq!(compare.kind, SelectedInstructionKind::CompareI64);
        assert_eq!(
            compare
                .operands
                .iter()
                .map(|operand| operand.virtual_register)
                .collect::<Vec<_>>(),
            [VirtualRegisterId(0), VirtualRegisterId(1)]
        );
        assert_eq!(compare.provenance.operations, [equal_operation]);
        assert_eq!(compare.provenance.values, [left, right, equal]);
        assert_eq!(
            compare.provenance.fuel[0].site,
            PsiProvenance::Operation(equal_operation)
        );

        let SelectedTerminator::ConditionalBranch {
            instruction,
            when_nonzero,
            when_zero,
        } = &entry.terminator
        else {
            panic!("inequality must branch on the nonzero comparison predicate")
        };
        assert_eq!(
            instruction.kind,
            SelectedInstructionKind::ConditionalBranchNonZero
        );
        assert_eq!(instruction.provenance.operations, [not_operation]);
        assert_eq!(instruction.provenance.values, [equal, not_equal]);
        assert_eq!(
            instruction.provenance.fuel[0].site,
            PsiProvenance::Operation(not_operation)
        );
        assert_eq!(when_nonzero.psi_edge, true_edge);
        assert_eq!(
            when_nonzero.block,
            selected_instructions::SelectedBlockId(1)
        );
        assert_eq!(when_zero.psi_edge, false_edge);
        assert_eq!(when_zero.block, selected_instructions::SelectedBlockId(2));
        assert_eq!(when_nonzero.fuel[0].site, PsiProvenance::Edge(true_edge));
        assert_eq!(when_zero.fuel[0].site, PsiProvenance::Edge(false_edge));
    }
}

#[test]
fn inequality_semantics_cover_less_equal_greater_and_u64_boundaries_on_both_isas() {
    let cases = [
        (7_u64, 9_u64, true),
        (9, 9, false),
        (9, 7, true),
        (0, 0, false),
        (0, u64::MAX, true),
        (u64::MAX, 0, true),
        (u64::MAX, u64::MAX, false),
    ];

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_integer_not_equal_conditional(target);
        let SelectedTerminator::ConditionalBranch {
            when_nonzero,
            when_zero,
            ..
        } = &staged.selected().plan().functions[0].blocks[0].terminator
        else {
            panic!("fixture must branch on the nonzero comparison predicate")
        };
        for &(left, right, expected_true) in &cases {
            let selected_edge = if left != right {
                when_nonzero.psi_edge
            } else {
                when_zero.psi_edge
            };
            assert_eq!(
                selected_edge == EdgeId::new(19_614).unwrap(),
                expected_true,
                "{left} != {right} on {target:?}"
            );
        }
    }
}
