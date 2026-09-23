//! Selection retains repeated Unit calls and rejects forged result/reference transport.
use crate::tests::fixtures::shared_unit_calls::fixture;
use crate::{
    legalize_target_operations, select_instructions, selection_constraints,
    validate_selected_instructions,
};
use abstract_operations::AbstractOperation;
use legalized_operations::LegalizedScalarArgument;
use selected_instructions::{SelectedInstructionKind, VirtualRegisterOrigin};
use semantic_vocabulary::FuelScheduleIdentity;

#[test]
fn repeated_shared_unit_calls_select_on_each_native_register_abi() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let (abstracted, targeted, unit) = fixture(native);
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
        assert_eq!(selected.plan().functions[0].calls.len(), 3);
        assert_eq!(selected.plan().functions[1].calls.len(), 1);
        for function in &selected.plan().functions {
            assert!(function.outgoing_arguments.is_empty());
            for call in &function.calls {
                assert!(call.call.result_placement.is_none());
                let instruction = function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .find(|instruction| instruction.id == call.instruction)
                    .unwrap();
                assert!(matches!(
                    instruction.kind,
                    SelectedInstructionKind::CallUnit { .. }
                ));
                assert_eq!(instruction.operands.len(), 3);
            }
        }
        for mutation in 0..7 {
            let mut changed = selected.plan().clone();
            let function = &mut changed.functions[0];
            let instruction_id = function.calls[0].instruction;
            match mutation {
                0 => function.calls.swap(0, 1),
                1 => {
                    let instruction = function.blocks[0]
                        .instructions
                        .iter_mut()
                        .find(|instruction| instruction.id == instruction_id)
                        .unwrap();
                    instruction.operands.push(instruction.operands[0]);
                }
                2 => {
                    let LegalizedScalarArgument::Structural { target, .. } =
                        function.calls[0].call.arguments.last_mut().unwrap()
                    else {
                        panic!("view argument");
                    };
                    target.source_byte_offset = 8;
                }
                3 => {
                    let LegalizedScalarArgument::Scalar { placement, .. } =
                        &mut function.calls[0].call.arguments[1]
                    else {
                        panic!("Boolean");
                    };
                    placement.shape = calling_conventions::ValueShape::integer(8, 8);
                }
                4 => {
                    let register = function
                        .virtual_registers
                        .iter_mut()
                        .find(|register| {
                            matches!(
                                register.origin,
                                VirtualRegisterOrigin::StructuralParameter { .. }
                            )
                        })
                        .unwrap();
                    let VirtualRegisterOrigin::StructuralParameter { place, .. } =
                        &mut register.origin
                    else {
                        panic!("pointer");
                    };
                    *place = semantic_vocabulary::PlaceId::new(99).unwrap();
                }
                5 => {
                    function.calls[0].call.callee = semantic_vocabulary::MachineId::new(3).unwrap()
                }
                _ => function.calls[0].effect.output += 1,
            }
            assert!(
                validate_selected_instructions(
                    &legalized,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                    changed
                )
                .is_err(),
                "mutation {mutation} on {native:?}"
            );
        }
    }
}

#[test]
fn crash_declaring_shared_unit_calls_select_and_replay_their_roster() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let (mut abstracted, _, _) = fixture(native);
        for function in &mut abstracted.functions {
            for operation in &mut function.operations {
                if let AbstractOperation::CallUnit {
                    crash_continuations,
                    ..
                } = operation
                {
                    *crash_continuations = vec![terminal_psi::CrashRouteBucket {
                        cause: terminal_psi::CrashCause::Abort,
                        alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
                    }];
                }
            }
        }
        let targeted = abstract_operations_to_target_operations::lower_to_target_operations(
            &abstracted,
            abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
        )
        .unwrap();
        let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
            &abstracted,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
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
        for function in &selected.plan().functions {
            for call in &function.calls {
                assert_eq!(
                    call.call.crash_continuations,
                    vec![terminal_psi::CrashRouteBucket {
                        cause: terminal_psi::CrashCause::Abort,
                        alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
                    }]
                );
            }
        }
        validate_selected_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
        let mut changed = selected.plan().clone();
        changed.functions[0].calls[0]
            .call
            .crash_continuations
            .clear();
        assert!(
            validate_selected_instructions(
                &legalized,
                &constraints,
                environment.physical(),
                environment.constraints(),
                changed
            )
            .is_err(),
            "forged roster on {native:?}"
        );
    }
}
