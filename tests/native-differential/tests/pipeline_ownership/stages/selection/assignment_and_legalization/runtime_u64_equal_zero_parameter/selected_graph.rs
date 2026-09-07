use crate::tests::*;

use super::fixture::staged_equal_zero_parameter;

#[test]
fn u64_parameter_equal_zero_selects_exact_compare_zero_graph_on_both_isas() {
    let zero_operation = OperationId::new(20_019).unwrap();
    let equal_operation = OperationId::new(20_011).unwrap();
    let parameter = ValueId::new(20_005).unwrap();
    let zero = ValueId::new(20_006).unwrap();
    let condition = ValueId::new(20_007).unwrap();
    let true_edge = EdgeId::new(20_014).unwrap();
    let false_edge = EdgeId::new(20_015).unwrap();

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_equal_zero_parameter(target);
        assert_eq!(
            staged
                .optimized_target()
                .optimized()
                .selections()
                .as_slice(),
            [Optimization::CopyPropagation]
        );
        assert!(matches!(
            &staged.optimized_target().target_operations().functions[0].operation,
            TargetOperation::ReturnIntegerExpressionConditionalControl {
                condition_source,
                condition: target_operations::TargetBooleanExpression::IntegerEqual {
                    psi_operation,
                    scalar_type,
                    left,
                    right,
                },
                scalar_type: result_type,
                ..
            } if *condition_source == condition
                && *psi_operation == equal_operation
                && *scalar_type == IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
                && *result_type == IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
                && matches!(
                    left.as_ref(),
                    TargetIntegerExpression::Parameter {
                        source_value,
                        parameter_index: 0,
                        ..
                    } if *source_value == parameter
                )
                && matches!(
                    right.as_ref(),
                    TargetIntegerExpression::Immediate { source_value, value }
                        if *source_value == zero && *value == IntegerValue::Unsigned(0)
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

        assert_ordinary_graph_custody(&staged);

        let function = &staged.selected().plan().functions[0];
        assert_eq!(function.blocks.len(), 3);

        assert!(matches!(
            function.virtual_registers[0].origin,
            VirtualRegisterOrigin::EntryParameter {
                source_value,
                parameter_index: 0,
            } if source_value == parameter
        ));
        assert!(function.virtual_registers[0].entry_fixed_view.is_some());

        let entry = &function.blocks[0];
        let compare = function.blocks[0]
            .instructions
            .iter()
            .find(|row| row.kind == SelectedInstructionKind::CompareI64Zero)
            .expect("selected comparison");
        assert_eq!(compare.kind, SelectedInstructionKind::CompareI64Zero);
        assert_eq!(compare.operands.len(), 1);
        assert!(
            matches!(
                function.virtual_registers[compare.operands[0].virtual_register.0 as usize].origin,
                VirtualRegisterOrigin::InstructionResult { source_value, .. } if source_value == parameter
            ),
            "comparison reads the durable parameter snapshot"
        );
        assert_eq!(
            compare.provenance.operations,
            [zero_operation, equal_operation]
        );
        assert_eq!(compare.provenance.values, [parameter, zero, condition]);
        assert_eq!(compare.provenance.fuel.len(), 2);

        let SelectedTerminator::ConditionalBranch {
            instruction,
            when_nonzero,
            when_zero,
        } = &entry.terminator
        else {
            panic!("entry must branch on the compare-zero result")
        };
        assert_eq!(
            instruction.kind,
            SelectedInstructionKind::ConditionalBranchNonZero
        );
        assert_eq!(instruction.provenance.values, [condition]);
        assert_eq!(when_zero.psi_edge, true_edge);
        assert_eq!(when_zero.block, selected_instructions::SelectedBlockId(1));
        assert_eq!(when_nonzero.psi_edge, false_edge);
        assert_eq!(
            when_nonzero.block,
            selected_instructions::SelectedBlockId(2)
        );
    }
}
