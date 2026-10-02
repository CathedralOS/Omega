//! Plain aggregate disposal is no-code work, not absent source work.
//! Mandatory graph/physical replay checks the live frontier. Publication also
//! retains the exact target edge and its ordered disposal on branches and returns.

use abstract_operations::{AbstractOperation, AbstractSuccessor};
use target_operations::{TargetControlSuccessor, TargetControlTerminator, TargetFunction};
use terminal_psi::TerminalAffineCleanupAction;

pub(super) fn retained(source: &AbstractOperation, target: &TargetFunction) -> bool {
    let mut candidates =
        target
            .graph
            .blocks
            .iter()
            .filter(|block| match (source, &block.terminator) {
                (
                    AbstractOperation::Crash { psi_edge, .. },
                    TargetControlTerminator::Crash {
                        psi_edge: actual, ..
                    },
                ) => psi_edge == actual,
                (
                    AbstractOperation::Return { psi_edge, .. }
                    | AbstractOperation::ReturnUnit { psi_edge, .. },
                    TargetControlTerminator::ReturnScalar { psi_edge: edge, .. }
                    | TargetControlTerminator::Return { psi_edge: edge, .. },
                ) => edge == psi_edge,
                (
                    AbstractOperation::Jump { psi_edge, .. },
                    TargetControlTerminator::Jump { successor },
                ) => successor.psi_edge == *psi_edge,
                (
                    AbstractOperation::Conditional {
                        when_true,
                        when_false,
                        ..
                    },
                    TargetControlTerminator::Conditional {
                        when_true: actual_true,
                        when_false: actual_false,
                        ..
                    },
                ) => [actual_true.psi_edge, actual_false.psi_edge]
                    .iter()
                    .any(|edge| *edge == when_true.psi_edge || *edge == when_false.psi_edge),
                _ => false,
            });
    let Some(candidate) = candidates.next() else {
        return false;
    };
    if candidates.next().is_some() {
        return false;
    }
    match (source, &candidate.terminator) {
        (
            AbstractOperation::Crash {
                psi_edge,
                cause,
                site_guard,
                frontier_lower_bound,
            },
            TargetControlTerminator::Crash {
                psi_edge: actual_edge,
                cause: actual_cause,
                site_guard: actual_guard,
                frontier_lower_bound: actual_frontier,
            },
        ) => {
            psi_edge == actual_edge
                && cause == actual_cause
                && site_guard == actual_guard
                && frontier_lower_bound == actual_frontier
        }
        (
            AbstractOperation::Return {
                value,
                cleanup_actions,
                ..
            },
            TargetControlTerminator::ReturnScalar {
                source_value,
                cleanup_actions: actual,
                ..
            },
        ) => {
            value == source_value
                && cleanup_actions == actual
                && return_cleanup_calls_match(
                    cleanup_actions,
                    &candidate.operations,
                    match source {
                        AbstractOperation::Return { psi_edge, .. }
                        | AbstractOperation::ReturnUnit { psi_edge, .. } => *psi_edge,
                        _ => unreachable!(),
                    },
                )
        }
        (
            AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            },
            TargetControlTerminator::Return {
                cleanup_actions: actual,
                ..
            },
        ) => {
            cleanup_actions == actual
                && return_cleanup_calls_match(
                    cleanup_actions,
                    &candidate.operations,
                    match source {
                        AbstractOperation::Return { psi_edge, .. }
                        | AbstractOperation::ReturnUnit { psi_edge, .. } => *psi_edge,
                        _ => unreachable!(),
                    },
                )
        }
        (
            AbstractOperation::Jump {
                psi_edge,
                target,
                bindings,
                structural_bindings,
                trivial_affine_discards,
                residual_affine_discards,
            },
            TargetControlTerminator::Jump { successor },
        ) => {
            successor.psi_edge == *psi_edge
                && successor.target == *target
                && successor.bindings == *bindings
                && successor.structural_bindings == *structural_bindings
                && residual_affine_discards.is_empty()
                && cleanup_matches(&successor.cleanup_actions, trivial_affine_discards)
        }
        (
            AbstractOperation::Conditional {
                condition,
                when_true,
                when_false,
            },
            TargetControlTerminator::Conditional {
                condition_source,
                when_true: target_true,
                when_false: target_false,
                ..
            },
        ) => {
            condition_source == condition
                && successor_matches(when_true, target_true)
                && successor_matches(when_false, target_false)
        }
        _ => false,
    }
}

fn successor_matches(source: &AbstractSuccessor, target: &TargetControlSuccessor) -> bool {
    source.psi_edge == target.psi_edge
        && source.target == target.target
        && source.bindings == target.bindings
        && source.structural_bindings == target.structural_bindings
        && cleanup_matches(&target.cleanup_actions, &source.trivial_affine_discards)
}

pub(super) fn cleanup_matches(
    actions: &[TerminalAffineCleanupAction],
    places: &[semantic_vocabulary::PlaceId],
) -> bool {
    actions.len() == places.len()
        && actions.iter().zip(places).all(|(action, place)| matches!(action, TerminalAffineCleanupAction::DiscardRoot(source) if source == place))
}

/// Earlier graph admission owns the live frontier and callee ABI/body. This
/// image join retains every executable action's edge, ordinal and original root.
fn return_cleanup_calls_match(
    actions: &[TerminalAffineCleanupAction],
    operations: &[target_operations::TargetUnitOperation],
    edge: semantic_vocabulary::EdgeId,
) -> bool {
    let mut calls = operations.iter().filter_map(|operation| match operation {
        target_operations::TargetUnitOperation::CleanupCall {
            owner,
            callee,
            arguments,
            call_plan,
        } => Some((owner, callee, arguments, call_plan)),
        _ => None,
    });
    for (ordinal, action) in actions.iter().enumerate() {
        let nominal = match action {
            TerminalAffineCleanupAction::DiscardRoot(_) => continue,
            TerminalAffineCleanupAction::InvokeNominal(nominal) => nominal,
            TerminalAffineCleanupAction::DiscardResidual(_) => return false,
        };
        let Ok(action_ordinal) = u32::try_from(ordinal) else {
            return false;
        };
        let Some((owner, callee, arguments, call_plan)) = calls.next() else {
            return false;
        };
        if *owner
            != (target_operations::CallSiteOwner::CleanupAction {
                edge,
                action_ordinal,
            })
            || *callee != nominal.cleanup_machine
            || !nominal.requirement_obligations.is_empty()
            || call_plan.result.is_some()
            || arguments.len() != usize::from(nominal.cleanup_receiver.is_some())
            || arguments.len() != call_plan.parameters.len()
            || arguments
                .iter()
                .zip(&call_plan.parameters)
                .any(|(argument, placement)| {
                    argument.place != nominal.place
                        || argument.root_structural_type != nominal.structural_type
                        || argument.structural_type != nominal.structural_type
                        || !argument.path.is_empty()
                        || !argument.indices.is_empty()
                        || argument.source_byte_offset != 0
                        || !matches!(
                            argument.access,
                            terminal_psi::StructuralAccess::MutableBorrow
                                | terminal_psi::StructuralAccess::WriteOnlyBorrow
                        )
                        || argument.shape.class
                            != calling_conventions::ValueClass::BorrowedReference
                        || argument.shape != placement.shape
                        || argument.destination != *placement
                })
        {
            return false;
        }
    }
    calls.next().is_none()
}

#[cfg(test)]
mod cleanup_tests {
    use super::return_cleanup_calls_match;
    use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
    use semantic_vocabulary::{EdgeId, MachineId, OperationId, PlaceId, StructuralTypeId};
    use target_operations::{CallSiteOwner, TargetStructuralArgument, TargetUnitOperation};
    use terminal_psi::{NominalAffineCleanup, StructuralAccess, TerminalAffineCleanupAction};

    #[test]
    fn nominal_image_return_rejoins_every_action_and_original_receiver() {
        let edge = EdgeId::new(1).unwrap();
        let structural_type = StructuralTypeId::new(1).unwrap();
        let machine = MachineId::new(9).unwrap();
        let shape = ValueShape::borrowed_reference(4, 4);
        let plan = evaluate_call_plan(
            CallingPolicy::native_for_target(target::NativeTarget::linux_x64()),
            &CallSignature {
                parameters: vec![shape],
                result: None,
            },
        )
        .unwrap();
        let mut actions = Vec::new();
        let mut calls = Vec::new();
        for (ordinal, place) in [(0, PlaceId::new(1).unwrap()), (2, PlaceId::new(2).unwrap())] {
            actions.push(TerminalAffineCleanupAction::InvokeNominal(
                NominalAffineCleanup {
                    place,
                    structural_type,
                    cleanup_machine: machine,
                    cleanup_receiver: Some(PlaceId::new(3).unwrap()),
                    requirement_obligations: Vec::new(),
                },
            ));
            if ordinal == 0 {
                actions.push(TerminalAffineCleanupAction::DiscardRoot(
                    PlaceId::new(4).unwrap(),
                ));
            }
            calls.push(TargetUnitOperation::CleanupCall {
                owner: CallSiteOwner::CleanupAction {
                    edge,
                    action_ordinal: ordinal,
                },
                callee: machine,
                call_plan: plan.clone(),
                arguments: vec![TargetStructuralArgument {
                    place,
                    access: StructuralAccess::MutableBorrow,
                    path: Vec::new(),
                    root_structural_type: structural_type,
                    structural_type,
                    shape,
                    source_byte_offset: 0,
                    fixed_array_length: None,
                    element_stride: None,
                    indices: Vec::new(),
                    source: plan.parameters[0].clone().into(),
                    destination: plan.parameters[0].clone(),
                }],
            });
        }
        // A raw image join only: earlier source and graph replay own frontier
        // liveness, original storage residence, hook contract and callee body.
        assert!(return_cleanup_calls_match(&actions, &calls, edge));
        for mutation in 0..6 {
            let mut hostile = calls.clone();
            match mutation {
                0 => {
                    hostile.pop();
                }
                1 => {
                    hostile.push(hostile[0].clone());
                }
                2 => {
                    hostile.swap(0, 1);
                }
                _ => {
                    let TargetUnitOperation::CleanupCall {
                        owner,
                        callee,
                        arguments,
                        ..
                    } = &mut hostile[1]
                    else {
                        unreachable!()
                    };
                    match mutation {
                        3 => *owner = CallSiteOwner::Operation(OperationId::new(1).unwrap()),
                        4 => *callee = MachineId::new(10).unwrap(),
                        _ => arguments[0].place = PlaceId::new(1).unwrap(),
                    }
                }
            }
            assert!(!return_cleanup_calls_match(&actions, &hostile, edge));
        }
    }
}
