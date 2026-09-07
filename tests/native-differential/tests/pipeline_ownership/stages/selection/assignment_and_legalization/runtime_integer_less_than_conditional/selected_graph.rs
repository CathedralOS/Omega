use crate::tests::*;

use super::fixture::staged_integer_less_than_conditional;

#[test]
fn runtime_u64_parameter_less_than_selects_ordered_three_block_graph_on_both_isas() {
    let less_operation = OperationId::new(19_211).unwrap();
    let left = ValueId::new(19_205).unwrap();
    let right = ValueId::new(19_206).unwrap();
    let condition = ValueId::new(19_207).unwrap();
    let true_edge = EdgeId::new(19_214).unwrap();
    let false_edge = EdgeId::new(19_215).unwrap();

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_integer_less_than_conditional(target);
        assert!(matches!(
            &staged.optimized_target().target_operations().functions[0].operation,
            TargetOperation::ReturnIntegerExpressionConditionalControl {
                condition_source,
                condition: target_operations::TargetBooleanExpression::IntegerLessThan {
                    psi_operation,
                    scalar_type,
                    left: target_left,
                    right: target_right,
                },
                scalar_type: result_type,
                ..
            } if *condition_source == condition
                && *psi_operation == less_operation
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

        assert!(staged.legalized().plan().functions.is_empty());
        let legalized = &staged.legalized().plan().scalar_functions[0];
        let comparison = &legalized.blocks[0].instructions[0];
        assert_eq!(comparison.operation, less_operation);
        assert_eq!(comparison.result, condition);
        assert_eq!(
            comparison.kind,
            legalized_operations::LegalizedScalarInstructionKind::Compare {
                predicate: legalized_operations::LegalizedScalarComparison::LessThan,
                operand_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                left,
                right,
            }
        );
        assert_eq!(comparison.fuel.len(), 1);
        assert_eq!(
            comparison.fuel[0].site,
            PsiProvenance::Operation(less_operation)
        );
        assert_eq!(legalized.parameters[0].value, left);
        assert_eq!(legalized.parameters[1].value, right);

        let function = &staged.selected().plan().functions[0];
        assert_eq!(function.blocks.len(), 3);
        assert_eq!(function.virtual_registers.len(), 8);
        assert_eq!(staged.selected().receipt().instruction_count(), 10);
        let entry = &function.blocks[0];
        let [left_copy, right_copy, compare] = entry.instructions.as_slice() else {
            panic!("entry must copy both ABI parameters before comparing")
        };
        assert_eq!(left_copy.kind, SelectedInstructionKind::CopyI64);
        assert_eq!(right_copy.kind, SelectedInstructionKind::CopyI64);
        assert_eq!(left_copy.provenance.values, [left]);
        assert_eq!(right_copy.provenance.values, [right]);
        assert_eq!(left_copy.operands[0].virtual_register, VirtualRegisterId(0));
        assert_eq!(
            right_copy.operands[0].virtual_register,
            VirtualRegisterId(1)
        );
        assert_eq!(compare.kind, SelectedInstructionKind::CompareI64);
        assert_eq!(
            compare
                .operands
                .iter()
                .map(|operand| operand.virtual_register)
                .collect::<Vec<_>>(),
            [
                left_copy.operands[1].virtual_register,
                right_copy.operands[1].virtual_register
            ]
        );
        assert_eq!(compare.provenance.operations, [less_operation]);
        assert_eq!(compare.provenance.values, [left, right, condition]);

        let SelectedTerminator::ConditionalBranchU64LessThan {
            instruction,
            when_less,
            when_not_less,
        } = &entry.terminator
        else {
            panic!("entry must retain unsigned-less-than control")
        };
        assert_eq!(
            instruction.kind,
            SelectedInstructionKind::ConditionalBranchU64LessThan
        );
        assert_eq!(instruction.provenance.values, [condition]);
        assert_eq!(when_less.psi_edge, true_edge);
        assert_eq!(when_less.block, selected_instructions::SelectedBlockId(1));
        assert_eq!(when_not_less.psi_edge, false_edge);
        assert_eq!(
            when_not_less.block,
            selected_instructions::SelectedBlockId(2)
        );
        assert_eq!(when_less.fuel[0].site, PsiProvenance::Edge(true_edge));
        assert_eq!(when_not_less.fuel[0].site, PsiProvenance::Edge(false_edge));
    }
}
