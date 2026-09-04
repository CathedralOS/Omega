use crate::tests::*;

use super::fixture::staged_integer_equal_conditional;

#[test]
fn runtime_u64_parameter_equality_selects_exact_three_block_graph_on_both_isas() {
    let equal_operation = OperationId::new(19_011).unwrap();
    let left = ValueId::new(19_005).unwrap();
    let right = ValueId::new(19_006).unwrap();
    let condition = ValueId::new(19_007).unwrap();
    let true_edge = EdgeId::new(19_014).unwrap();
    let false_edge = EdgeId::new(19_015).unwrap();

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_integer_equal_conditional(target);
        assert!(matches!(
            &staged.optimized_target().target_operations().functions[0].operation,
            TargetOperation::ReturnIntegerExpressionConditionalControl {
                condition_source,
                condition: omega_target_operations::TargetBooleanExpression::IntegerEqual {
                    psi_operation,
                    scalar_type,
                    left: target_left,
                    right: target_right,
                },
                scalar_type: result_type,
                ..
            } if *condition_source == condition
                && *psi_operation == equal_operation
                && *scalar_type == IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
                && *result_type == IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
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
            LegalizationRecipe::ReturnU64IntegerEqualParametersConditionalV1
        );
        let omega_legalized_operations::LegalizedCondition::IntegerEqualParametersV1 {
            operation,
            fuel,
            left: legalized_left,
            right: legalized_right,
            ..
        } = &legalized.condition
        else {
            panic!("legalization must retain the integer-equality condition")
        };
        assert_eq!(*operation, equal_operation);
        assert_eq!(fuel.len(), 1);
        assert_eq!(fuel[0].site, PsiProvenance::Operation(equal_operation));
        assert_eq!(legalized_left.source_value, left);
        assert_eq!(legalized_left.parameter_index, 0);
        assert_eq!(legalized_right.source_value, right);
        assert_eq!(legalized_right.parameter_index, 1);

        let function = &staged.selected().plan().functions[0];
        assert_eq!(function.blocks.len(), 3);
        assert_eq!(function.virtual_registers.len(), 4);
        assert_eq!(staged.selected().receipt().instruction_count(), 6);
        assert!(matches!(
            function.virtual_registers[0].origin,
            VirtualRegisterOrigin::EntryParameter {
                source_value,
                parameter_index: 0,
            } if source_value == left
        ));
        assert!(matches!(
            function.virtual_registers[1].origin,
            VirtualRegisterOrigin::EntryParameter {
                source_value,
                parameter_index: 1,
            } if source_value == right
        ));
        assert!(function.virtual_registers[0].entry_fixed_view.is_some());
        assert!(function.virtual_registers[1].entry_fixed_view.is_some());

        let entry = &function.blocks[0];
        assert_eq!(entry.instructions.len(), 1);
        let compare = &entry.instructions[0];
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
        assert_eq!(compare.provenance.values, [left, right, condition]);
        assert_eq!(compare.provenance.fuel.len(), 1);
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
            panic!("entry must branch on the equality condition")
        };
        assert_eq!(
            instruction.kind,
            SelectedInstructionKind::ConditionalBranchNonZero
        );
        assert_eq!(instruction.provenance.values, [condition]);
        assert!(instruction.provenance.operations.is_empty());
        assert!(instruction.provenance.fuel.is_empty());
        assert_eq!(when_zero.psi_edge, true_edge);
        assert_eq!(
            when_zero.block,
            omega_selected_instructions::SelectedBlockId(1)
        );
        assert_eq!(when_nonzero.psi_edge, false_edge);
        assert_eq!(
            when_nonzero.block,
            omega_selected_instructions::SelectedBlockId(2)
        );
        assert_eq!(when_zero.fuel.len(), 1);
        assert_eq!(when_zero.fuel[0].site, PsiProvenance::Edge(true_edge));
        assert_eq!(when_nonzero.fuel.len(), 1);
        assert_eq!(when_nonzero.fuel[0].site, PsiProvenance::Edge(false_edge));

        for block in &function.blocks[1..] {
            assert!(matches!(
                block.instructions[0].kind,
                SelectedInstructionKind::MaterializeI64 { .. }
            ));
            assert_eq!(block.instructions[0].provenance.fuel.len(), 1);
            let SelectedTerminator::Return { instruction, .. } = &block.terminator else {
                panic!("leaf must return")
            };
            assert_eq!(instruction.kind, SelectedInstructionKind::ReturnI64);
            assert_eq!(instruction.provenance.fuel.len(), 1);
        }
    }
}
