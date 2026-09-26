//! Crash-declaring structural-result calls carry their continuation roster.

use super::super::fixtures::call_structural::{
    fixture, mixed_result_fixture, reference_fixture, sum_reference_fixture,
};
use crate::{
    legalize_target_operations, select_instructions, selection_constraints,
    validate_legalized_operations, validate_selected_instructions,
};
use target_operations::TargetUnitOperation;

#[test]
fn crash_declaring_structural_calls_select_and_replay_their_continuation_roster() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let (source, target, unit) = fixture(
            native,
            vec![terminal_psi::CrashRouteBucket {
                cause: terminal_psi::CrashCause::Abort,
                alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
            }],
        );
        let legal = legalize_target_operations(&target, &source, &unit).unwrap();
        validate_legalized_operations(&target, &source, &unit, legal.plan().clone()).unwrap();
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = selection_constraints(&legal, &environment);
        let selected = select_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        let call = selected
            .plan()
            .functions
            .iter()
            .flat_map(|function| &function.calls)
            .next()
            .expect("structural call");
        assert_eq!(
            call.call.crash_continuations,
            vec![terminal_psi::CrashRouteBucket {
                cause: terminal_psi::CrashCause::Abort,
                alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
            }]
        );
        validate_selected_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
        // The roster is correspondence custody: a forged or dropped
        // continuation rejects at every replay level.
        let mut widened = target.clone();
        let TargetUnitOperation::Call {
            crash_continuations,
            ..
        } = widened.functions[0]
            .graph
            .blocks
            .iter_mut()
            .flat_map(|block| block.operations.iter_mut())
            .find(|operation| matches!(operation, TargetUnitOperation::Call { .. }))
            .expect("call")
        else {
            unreachable!("call");
        };
        crash_continuations.clear();
        assert!(legalize_target_operations(&widened, &source, &unit).is_err());
        let mut changed = legal.plan().clone();
        let changed_call = changed
            .scalar_functions
            .iter_mut()
            .flat_map(|function| function.blocks.iter_mut())
            .flat_map(|block| block.instructions.iter_mut())
            .find_map(|instruction| {
                if let legalized_operations::LegalizedScalarInstructionKind::Call(call) =
                    &mut instruction.kind
                {
                    Some(call)
                } else {
                    None
                }
            })
            .expect("call instruction");
        changed_call.crash_continuations.clear();
        assert!(validate_legalized_operations(&target, &source, &unit, changed).is_err());
        let mut forged = selected.plan().clone();
        forged.functions[0].calls[0]
            .call
            .crash_continuations
            .clear();
        assert!(
            validate_selected_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
                forged
            )
            .is_err()
        );
        // A caller-side roster mismatch against the callee's verified crash
        // contract fails unit-custody before any instruction projects.
        let mut contract_changed = unit.clone();
        contract_changed.functions[1]
            .verified_contract
            .as_mut()
            .expect("contract")
            .crash_routes
            .clear();
        contract_changed.identity =
            optimization_unit::recompute_psi_optimization_unit_identity(&contract_changed);
        assert!(legalize_target_operations(&target, &source, &contract_changed).is_err());
    }
}

#[test]
fn crash_declaring_reference_calls_select_and_replay_their_continuation_roster() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let (source, target, unit) = reference_fixture(
            native,
            vec![terminal_psi::CrashRouteBucket {
                cause: terminal_psi::CrashCause::Abort,
                alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
            }],
        );
        let legal = legalize_target_operations(&target, &source, &unit).unwrap();
        validate_legalized_operations(&target, &source, &unit, legal.plan().clone()).unwrap();
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = selection_constraints(&legal, &environment);
        let selected = select_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        let call = selected
            .plan()
            .functions
            .iter()
            .flat_map(|function| &function.calls)
            .next()
            .expect("structural call");
        assert_eq!(
            call.call.crash_continuations,
            vec![terminal_psi::CrashRouteBucket {
                cause: terminal_psi::CrashCause::Abort,
                alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
            }]
        );
        validate_selected_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
        // The roster is correspondence custody: a forged or dropped
        // continuation rejects at every replay level.
        let mut widened = target.clone();
        let TargetUnitOperation::Call {
            crash_continuations,
            ..
        } = widened.functions[0]
            .graph
            .blocks
            .iter_mut()
            .flat_map(|block| block.operations.iter_mut())
            .find(|operation| matches!(operation, TargetUnitOperation::Call { .. }))
            .expect("call")
        else {
            unreachable!("call");
        };
        crash_continuations.clear();
        assert!(legalize_target_operations(&widened, &source, &unit).is_err());
        let mut changed = legal.plan().clone();
        let changed_call = changed
            .scalar_functions
            .iter_mut()
            .flat_map(|function| function.blocks.iter_mut())
            .flat_map(|block| block.instructions.iter_mut())
            .find_map(|instruction| {
                if let legalized_operations::LegalizedScalarInstructionKind::Call(call) =
                    &mut instruction.kind
                {
                    Some(call)
                } else {
                    None
                }
            })
            .expect("call instruction");
        changed_call.crash_continuations.clear();
        assert!(validate_legalized_operations(&target, &source, &unit, changed).is_err());
        let mut forged = selected.plan().clone();
        forged.functions[0].calls[0]
            .call
            .crash_continuations
            .clear();
        assert!(
            validate_selected_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
                forged
            )
            .is_err()
        );
        // A caller-side roster mismatch against the callee's verified crash
        // contract fails unit-custody before any instruction projects.
        let mut contract_changed = unit.clone();
        contract_changed.functions[1]
            .verified_contract
            .as_mut()
            .expect("contract")
            .crash_routes
            .clear();
        contract_changed.identity =
            optimization_unit::recompute_psi_optimization_unit_identity(&contract_changed);
        assert!(legalize_target_operations(&target, &source, &contract_changed).is_err());
        // A forged roster on the call itself fails the same contract pairing.
        let mut roster_changed = unit.clone();
        roster_changed.functions[1]
            .verified_contract
            .as_mut()
            .expect("contract")
            .crash_routes
            .push(terminal_psi::CrashRouteBucket {
                cause: terminal_psi::CrashCause::Trap,
                alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
            });
        roster_changed.identity =
            optimization_unit::recompute_psi_optimization_unit_identity(&roster_changed);
        assert!(legalize_target_operations(&target, &source, &roster_changed).is_err());
    }
}

#[test]
fn crash_declaring_sum_reference_calls_select_and_replay_their_continuation_roster() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let (source, target, unit) = sum_reference_fixture(
            native,
            vec![terminal_psi::CrashRouteBucket {
                cause: terminal_psi::CrashCause::Abort,
                alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
            }],
        );
        let legal = legalize_target_operations(&target, &source, &unit).unwrap();
        validate_legalized_operations(&target, &source, &unit, legal.plan().clone()).unwrap();
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = selection_constraints(&legal, &environment);
        let selected = select_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        let call = selected
            .plan()
            .functions
            .iter()
            .flat_map(|function| &function.calls)
            .next()
            .expect("structural call");
        assert_eq!(
            call.call.crash_continuations,
            vec![terminal_psi::CrashRouteBucket {
                cause: terminal_psi::CrashCause::Abort,
                alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
            }]
        );
        validate_selected_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
        // The roster is correspondence custody: a forged or dropped
        // continuation rejects at every replay level.
        let mut widened = target.clone();
        let TargetUnitOperation::Call {
            crash_continuations,
            ..
        } = widened.functions[0]
            .graph
            .blocks
            .iter_mut()
            .flat_map(|block| block.operations.iter_mut())
            .find(|operation| matches!(operation, TargetUnitOperation::Call { .. }))
            .expect("call")
        else {
            unreachable!("call");
        };
        crash_continuations.clear();
        assert!(legalize_target_operations(&widened, &source, &unit).is_err());
        let mut changed = legal.plan().clone();
        let changed_call = changed
            .scalar_functions
            .iter_mut()
            .flat_map(|function| function.blocks.iter_mut())
            .flat_map(|block| block.instructions.iter_mut())
            .find_map(|instruction| {
                if let legalized_operations::LegalizedScalarInstructionKind::Call(call) =
                    &mut instruction.kind
                {
                    Some(call)
                } else {
                    None
                }
            })
            .expect("call instruction");
        changed_call.crash_continuations.clear();
        assert!(validate_legalized_operations(&target, &source, &unit, changed).is_err());
        let mut forged = selected.plan().clone();
        forged.functions[0].calls[0]
            .call
            .crash_continuations
            .clear();
        assert!(
            validate_selected_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
                forged
            )
            .is_err()
        );
        // A caller-side roster mismatch against the callee's verified crash
        // contract fails unit-custody before any instruction projects.
        let mut contract_changed = unit.clone();
        contract_changed.functions[1]
            .verified_contract
            .as_mut()
            .expect("contract")
            .crash_routes
            .clear();
        contract_changed.identity =
            optimization_unit::recompute_psi_optimization_unit_identity(&contract_changed);
        assert!(legalize_target_operations(&target, &source, &contract_changed).is_err());
        let mut roster_changed = unit.clone();
        roster_changed.functions[1]
            .verified_contract
            .as_mut()
            .expect("contract")
            .crash_routes
            .push(terminal_psi::CrashRouteBucket {
                cause: terminal_psi::CrashCause::Trap,
                alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
            });
        roster_changed.identity =
            optimization_unit::recompute_psi_optimization_unit_identity(&roster_changed);
        assert!(legalize_target_operations(&target, &source, &roster_changed).is_err());
    }
}

/// A Mixed carrier returned through a structural call reuses the conventional
/// sum result home; each layer replays that exact transport.
#[test]
fn mixed_structural_results_select_and_replay_their_result_home() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let (source, target, unit) = mixed_result_fixture(native);
        let legal = legalize_target_operations(&target, &source, &unit).unwrap();
        validate_legalized_operations(&target, &source, &unit, legal.plan().clone()).unwrap();
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = selection_constraints(&legal, &environment);
        let selected = select_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        let call = selected
            .plan()
            .functions
            .iter()
            .flat_map(|function| &function.calls)
            .next()
            .expect("structural call");
        let result = call.call.structural_result.as_ref().expect("mixed result");
        assert_eq!(
            result.structural_type,
            semantic_vocabulary::StructuralTypeId::new(1).unwrap()
        );
        assert_eq!(
            call.call
                .call_plan
                .result
                .as_ref()
                .expect("result")
                .shape
                .byte_size,
            12
        );
        let TargetUnitOperation::Call {
            result:
                target_operations::TargetCallResult::Structural {
                    result_home: Some(home),
                    ..
                },
            ..
        } = target.functions[0]
            .graph
            .blocks
            .iter()
            .flat_map(|block| block.operations.iter())
            .find(|operation| matches!(operation, TargetUnitOperation::Call { .. }))
            .expect("call")
        else {
            unreachable!("call");
        };
        assert!(matches!(
            home.layout,
            target_operations::TargetStructuralHomeLayout::Sum(_)
        ));
        validate_selected_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
        // Dropping or substituting the agreed result home rejects at the
        // legalization replay; selection output is checked verbatim.
        let mut no_home = target.clone();
        let TargetUnitOperation::Call {
            result: target_operations::TargetCallResult::Structural { result_home, .. },
            ..
        } = no_home.functions[0]
            .graph
            .blocks
            .iter_mut()
            .flat_map(|block| block.operations.iter_mut())
            .find(|operation| matches!(operation, TargetUnitOperation::Call { .. }))
            .expect("call")
        else {
            unreachable!("call");
        };
        *result_home = None;
        assert!(legalize_target_operations(&no_home, &source, &unit).is_err());
        let mut wrong_type = target.clone();
        let TargetUnitOperation::Call {
            result: target_operations::TargetCallResult::Structural { result, .. },
            ..
        } = wrong_type.functions[0]
            .graph
            .blocks
            .iter_mut()
            .flat_map(|block| block.operations.iter_mut())
            .find(|operation| matches!(operation, TargetUnitOperation::Call { .. }))
            .expect("call")
        else {
            unreachable!("call");
        };
        result.multiplicity = terminal_psi::StructuralMultiplicity::Unrestricted;
        assert!(legalize_target_operations(&wrong_type, &source, &unit).is_err());
    }
}
