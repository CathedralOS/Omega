use crate::tests::*;

use super::fixture::{caller_machine, staged_selected};

#[test]
fn exact_target_legal_and_selected_call_chain_survives_on_both_isas() {
    let caller = caller_machine();
    let callee = MachineId::new(SCALAR_CALL_UNIT_CALLEE_BASE + 1).unwrap();
    let left = ValueId::new(SCALAR_CALL_UNIT_LEFT).unwrap();
    let right = ValueId::new(SCALAR_CALL_UNIT_RIGHT).unwrap();
    let first_result = ValueId::new(SCALAR_CALL_UNIT_FIRST_RESULT).unwrap();
    let second_result = ValueId::new(SCALAR_CALL_UNIT_SECOND_RESULT).unwrap();
    let third_result = ValueId::new(SCALAR_CALL_UNIT_THIRD_RESULT).unwrap();
    let call_operations = [
        OperationId::new(SCALAR_CALL_UNIT_FIRST_CALL).unwrap(),
        OperationId::new(SCALAR_CALL_UNIT_SECOND_CALL).unwrap(),
        OperationId::new(SCALAR_CALL_UNIT_THIRD_CALL).unwrap(),
    ];

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_selected(target);
        let target_function = staged
            .optimized_target()
            .target_operations()
            .functions
            .iter()
            .find(|function| function.machine == caller)
            .unwrap();
        let TargetOperation::UnitBody(target_body) = &target_function.operation else {
            panic!("attached caller must retain its Unit body")
        };
        let [
            TargetUnitOperation::IntegerConstant {
                psi_operation: left_operation,
                result: target_left,
                ..
            },
            TargetUnitOperation::IntegerConstant {
                psi_operation: right_operation,
                result: target_right,
                ..
            },
            TargetUnitOperation::ScalarCall {
                psi_operation: first_operation,
                callee: first_callee,
                call_plan: first_plan,
                result_home: first_home,
                arguments: first_arguments,
                ..
            },
            TargetUnitOperation::ScalarCall {
                psi_operation: second_operation,
                callee: second_callee,
                call_plan: second_plan,
                result_home: second_home,
                arguments: second_arguments,
                ..
            },
            TargetUnitOperation::ScalarCall {
                psi_operation: third_operation,
                callee: third_callee,
                call_plan: third_plan,
                result_home: third_home,
                arguments: third_arguments,
                ..
            },
            TargetUnitOperation::Return { psi_edge, .. },
        ] = target_body.operations.as_slice()
        else {
            panic!("caller target body must be the exact three-call chain")
        };
        assert_eq!(
            *left_operation,
            OperationId::new(SCALAR_CALL_UNIT_LEFT_OPERATION).unwrap()
        );
        assert_eq!(
            *right_operation,
            OperationId::new(SCALAR_CALL_UNIT_RIGHT_OPERATION).unwrap()
        );
        assert_eq!((*target_left, *target_right), (left, right));
        assert_eq!(
            [*first_operation, *second_operation, *third_operation],
            call_operations
        );
        assert_eq!([*first_callee, *second_callee, *third_callee], [callee; 3]);
        assert_eq!(first_plan, second_plan);
        assert_eq!(second_plan, third_plan);
        assert_eq!(first_plan.parameters.len(), 2);
        assert_eq!(
            *psi_edge,
            EdgeId::new(SCALAR_CALL_UNIT_RETURN_EDGE).unwrap()
        );
        assert_eq!(
            (
                first_home.source_value,
                second_home.source_value,
                third_home.source_value
            ),
            (first_result, second_result, third_result)
        );
        assert_eq!(
            first_arguments
                .iter()
                .map(|argument| argument.source.source_value())
                .collect::<Vec<_>>(),
            [left, right]
        );
        assert_eq!(
            second_arguments
                .iter()
                .map(|argument| argument.source.source_value())
                .collect::<Vec<_>>(),
            [left, right]
        );
        assert_eq!(
            third_arguments
                .iter()
                .map(|argument| argument.source.source_value())
                .collect::<Vec<_>>(),
            [first_result, second_result]
        );
        assert!(
            first_arguments
                .iter()
                .chain(second_arguments)
                .all(|argument| matches!(
                    argument.source,
                    TargetUnitScalarArgumentSource::IntegerImmediate { .. }
                ))
        );
        assert_eq!(
            third_arguments[0].source,
            TargetUnitScalarArgumentSource::Home(*first_home)
        );
        assert_eq!(
            third_arguments[1].source,
            TargetUnitScalarArgumentSource::Home(*second_home)
        );

        let legal = staged.legalized().plan();
        let legal_caller = legal
            .scalar_functions
            .iter()
            .find(|function| function.machine == caller)
            .unwrap();
        assert_eq!(legal_caller.machine, caller);
        let calls = legal_caller.blocks[0]
            .instructions
            .iter()
            .filter_map(|instruction| match &instruction.kind {
                legalized_operations::LegalizedScalarInstructionKind::Call(call) => {
                    Some((instruction, call))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(legal_caller.blocks[0].instructions.len(), 5);
        assert_eq!(calls.len(), 3);
        assert_eq!(
            calls
                .iter()
                .map(|(instruction, _)| instruction.operation)
                .collect::<Vec<_>>(),
            call_operations
        );
        assert!(calls.iter().all(|(_, call)| call.callee == callee));
        assert_eq!(calls[0].1.arguments[0].source, left);
        assert_eq!(calls[0].1.arguments[1].source, right);
        assert_eq!(calls[1].1.arguments[0].source, left);
        assert_eq!(calls[1].1.arguments[1].source, right);
        assert_eq!(calls[2].1.arguments[0].source, first_result);
        assert_eq!(calls[2].1.arguments[1].source, second_result);
        let legalized_operations::LegalizedScalarTerminator::Return(returned) =
            &legal_caller.blocks[0].terminator
        else {
            panic!("the scalar call fixture must retain its Unit return");
        };
        assert_eq!(
            returned.edge,
            EdgeId::new(SCALAR_CALL_UNIT_RETURN_EDGE).unwrap()
        );
        assert_eq!(returned.fuel.len(), 1);

        let selected = staged.selected().plan();
        let selected_caller = selected
            .functions
            .iter()
            .find(|function| function.machine == caller)
            .unwrap();
        assert_eq!(selected_caller.virtual_registers.len(), 14);
        assert_eq!(selected_caller.blocks.len(), 1);
        let block = &selected_caller.blocks[0];
        assert_eq!(block.instructions.len(), 14);
        assert_eq!(
            block
                .instructions
                .iter()
                .map(|instruction| instruction.kind)
                .collect::<Vec<_>>(),
            [
                SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(7)
                },
                SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(9)
                },
                SelectedInstructionKind::CopyI64,
                SelectedInstructionKind::CopyI64,
                SelectedInstructionKind::CallI64 { callee },
                SelectedInstructionKind::CopyI64,
                SelectedInstructionKind::CopyI64,
                SelectedInstructionKind::CopyI64,
                SelectedInstructionKind::CallI64 { callee },
                SelectedInstructionKind::CopyI64,
                SelectedInstructionKind::CopyI64,
                SelectedInstructionKind::CopyI64,
                SelectedInstructionKind::CallI64 { callee },
                SelectedInstructionKind::CopyI64,
            ]
        );
        let call_constraint = staged
            .register_environment()
            .constraint(staged.register_environment().selected_keys().call_i64[2])
            .unwrap();
        for (instruction_index, operation, values, registers) in [
            (
                4,
                call_operations[0],
                [left, right, first_result],
                [2, 3, 4],
            ),
            (
                8,
                call_operations[1],
                [left, right, second_result],
                [6, 7, 8],
            ),
            (
                12,
                call_operations[2],
                [first_result, second_result, third_result],
                [10, 11, 12],
            ),
        ] {
            let instruction = &block.instructions[instruction_index];
            assert_eq!(
                instruction.kind,
                SelectedInstructionKind::CallI64 { callee }
            );
            assert_eq!(instruction.constraint, call_constraint.key);
            assert_eq!(instruction.implicit_uses, call_constraint.implicit_uses);
            assert_eq!(instruction.implicit_defs, call_constraint.implicit_defs);
            assert_eq!(instruction.clobbers, call_constraint.clobbers);
            assert_eq!(instruction.provenance.operations, [operation]);
            assert_eq!(instruction.provenance.values, values);
            assert_eq!(instruction.provenance.fuel.len(), 1);
            assert_eq!(instruction.operands.len(), 3);
            for ((operand, constraint), register) in instruction
                .operands
                .iter()
                .zip(&call_constraint.operands)
                .zip(registers)
            {
                assert_eq!(operand.operand, constraint.operand);
                assert_eq!(operand.access, constraint.access);
                assert_eq!(operand.class, constraint.class);
                assert_eq!(operand.fixed_view, constraint.fixed_view);
                assert_eq!(operand.virtual_register, VirtualRegisterId(register));
            }
        }
        let SelectedTerminator::Return {
            instruction,
            psi_return_edge,
        } = &block.terminator
        else {
            panic!("call chain must end with ReturnUnit")
        };
        assert_eq!(instruction.kind, SelectedInstructionKind::ReturnUnit);
        assert_eq!(
            *psi_return_edge,
            EdgeId::new(SCALAR_CALL_UNIT_RETURN_EDGE).unwrap()
        );
    }
}
