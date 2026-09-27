//! Selection retains repeated Unit calls and rejects forged result/reference transport.
use crate::{
    legalize_target_operations, select_instructions, selection_constraints,
    validate_selected_instructions,
};

#[test]
fn borrowed_mixed_arguments_select_and_replay_the_reconstructed_row() {
    use semantic_vocabulary::{MachineId, OperationId, StructuralTypeId};
    use target_operations::{TargetStructuralArgumentSource, TargetUnitOperation};
    use terminal_psi::StructuralAccess;
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let (abstracted, targeted, unit) =
            crate::tests::fixtures::shared_unit_calls::mixed_borrowed_fixture(native);
        let legalized = legalize_target_operations(&targeted, &abstracted, &unit).unwrap();
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = selection_constraints(&legalized, &environment);
        let selected = select_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        // The mixed referent rides the call as one borrowed pointer operand.
        let call = &selected.plan().functions[0].calls[0];
        assert_eq!(call.call.callee, MachineId::new(2).unwrap());
        let instruction = selected.plan().functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find(|instruction| instruction.id == call.instruction)
            .unwrap();
        assert_eq!(instruction.operands.len(), 3);
        validate_selected_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
        // The reconstruction replays the argument's custody and geometry
        // verbatim: a forged row, source, or access always declines.
        for mutation in 0..6 {
            let mut changed = targeted.clone();
            let wrong_destination = changed.functions[0].graph.call_plan.parameters[0].clone();
            let TargetUnitOperation::Call { arguments, .. } =
                &mut changed.functions[0].graph.blocks[0].operations[0]
            else {
                unreachable!("call argument")
            };
            match mutation {
                0 => arguments[0].shape.byte_size = 16,
                1 => arguments[0].source_byte_offset = 4,
                2 => arguments[0].access = StructuralAccess::MutableBorrow,
                3 => arguments[0].root_structural_type = StructuralTypeId::new(2).unwrap(),
                4 => {
                    arguments[0].source = TargetStructuralArgumentSource::StructuralHome {
                        psi_operation: OperationId::new(10).unwrap(),
                    }
                }
                _ => arguments[0].destination = wrong_destination,
            }
            assert!(
                legalize_target_operations(&changed, &abstracted, &unit).is_err(),
                "mutation {mutation} on {native:?}"
            );
        }
    }
}

#[test]
fn owned_mixed_arguments_select_and_replay_the_reconstructed_row() {
    use semantic_vocabulary::{MachineId, OperationId, StructuralTypeId};
    use target_operations::{TargetStructuralArgumentSource, TargetUnitOperation};
    use terminal_psi::StructuralAccess;
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let (abstracted, targeted, unit) =
            crate::tests::fixtures::shared_unit_calls::mixed_owned_fixture(native);
        let legalized = legalize_target_operations(&targeted, &abstracted, &unit).unwrap();
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = selection_constraints(&legalized, &environment);
        let selected = select_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        // The owned mixed carrier rides the call as inline argument fragments
        // (the fragment count follows the native ABI).
        let call = &selected.plan().functions[0].calls[0];
        assert_eq!(call.call.callee, MachineId::new(2).unwrap());
        let TargetUnitOperation::Call { arguments, .. } =
            &targeted.functions[0].graph.blocks[0].operations[0]
        else {
            unreachable!("call argument")
        };
        assert_eq!(arguments[0].access, StructuralAccess::Owned);
        assert_eq!(arguments[0].shape.byte_size, 12);
        assert!(
            selected.plan().functions[0]
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .find(|instruction| instruction.id == call.instruction)
                .unwrap()
                .operands
                .len()
                > 2
        );
        validate_selected_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
        // The reconstruction replays the argument's custody and geometry
        // verbatim: a forged row, source, or access always declines.
        for mutation in 0..6 {
            let mut changed = targeted.clone();
            let wrong_destination = changed.functions[0].graph.call_plan.parameters[0].clone();
            let TargetUnitOperation::Call { arguments, .. } =
                &mut changed.functions[0].graph.blocks[0].operations[0]
            else {
                unreachable!("call argument")
            };
            match mutation {
                0 => arguments[0].shape.byte_size = 16,
                1 => arguments[0].source_byte_offset = 4,
                2 => arguments[0].access = StructuralAccess::SharedBorrow,
                3 => arguments[0].root_structural_type = StructuralTypeId::new(2).unwrap(),
                4 => {
                    arguments[0].source = TargetStructuralArgumentSource::StructuralHome {
                        psi_operation: OperationId::new(10).unwrap(),
                    }
                }
                _ => arguments[0].destination = wrong_destination,
            }
            assert!(
                legalize_target_operations(&changed, &abstracted, &unit).is_err(),
                "mutation {mutation} on {native:?}"
            );
        }
    }
}
