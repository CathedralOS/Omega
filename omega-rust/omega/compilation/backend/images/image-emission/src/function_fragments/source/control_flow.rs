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
                && cleanup_matches_residuals(
                    &successor.cleanup_actions,
                    trivial_affine_discards,
                    residual_affine_discards,
                )
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
    cleanup_matches_residuals(actions, places, &[])
}

/// The source edge schedules roots before residual subtrees. Preserve that
/// complete roster; mandatory graph replay separately validates its custody.
fn cleanup_matches_residuals(
    actions: &[TerminalAffineCleanupAction],
    places: &[semantic_vocabulary::PlaceId],
    residuals: &[terminal_psi::StructuralAffineDiscard],
) -> bool {
    let Some((roots, retained_residuals)) = actions.split_at_checked(places.len()) else {
        return false;
    };
    roots.iter().zip(places).all(|(action, place)| {
        matches!(action, TerminalAffineCleanupAction::DiscardRoot(source) if source == place)
    }) && retained_residuals.len() == residuals.len()
        && retained_residuals.iter().zip(residuals).all(|(action, residual)| {
            matches!(action, TerminalAffineCleanupAction::DiscardResidual(source) if source == residual)
        })
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
            // Both disposals are no-code actions. Their positions still count
            // in the exact source ordinal of every following nominal call.
            TerminalAffineCleanupAction::DiscardRoot(_)
            | TerminalAffineCleanupAction::DiscardResidual(_) => continue,
            TerminalAffineCleanupAction::InvokeNominal(nominal) => nominal,
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
    use super::{retained, return_cleanup_calls_match};
    use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
    use semantic_vocabulary::{BlockId, EdgeId, MachineId, OperationId, PlaceId, StructuralTypeId};
    use target_operations::{CallSiteOwner, TargetStructuralArgument, TargetUnitOperation};
    use terminal_psi::{
        NominalAffineCleanup, StructuralAccess, StructuralAffineDiscard, StructuralPathSegment,
        TerminalAffineCleanupAction,
    };

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
        for (ordinal, place) in [(0, PlaceId::new(1).unwrap()), (3, PlaceId::new(2).unwrap())] {
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
                actions.push(TerminalAffineCleanupAction::DiscardResidual(residual()));
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
        for mutation in 0..7 {
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
                        5 => arguments[0].place = PlaceId::new(1).unwrap(),
                        _ => {
                            *owner = CallSiteOwner::CleanupAction {
                                edge,
                                action_ordinal: 2,
                            }
                        }
                    }
                }
            }
            assert!(!return_cleanup_calls_match(&actions, &hostile, edge));
        }
    }

    fn residual() -> StructuralAffineDiscard {
        StructuralAffineDiscard {
            place: PlaceId::new(5).unwrap(),
            path: vec![StructuralPathSegment::Field("left".into())],
            structural_type: StructuralTypeId::new(2).unwrap(),
        }
    }

    fn target(
        terminator: target_operations::TargetControlTerminator,
    ) -> target_operations::TargetFunction {
        target_operations::TargetFunction {
            machine: MachineId::new(1).unwrap(),
            attachment: None,
            scalar_abi: None,
            mixed_structural_scalar_abi: None,
            provenance: target_operations::TerminalPsiProvenance {
                operations: vec![],
                edges: vec![],
            },
            graph: target_operations::TargetControlGraph {
                structural_types: vec![].into(),
                call_plan: evaluate_call_plan(
                    CallingPolicy::native_for_target(target::NativeTarget::linux_x64()),
                    &CallSignature {
                        parameters: vec![],
                        result: None,
                    },
                )
                .unwrap(),
                scalar_parameters: vec![],
                parameters: vec![],
                dynamic_parameters: vec![],
                entry: BlockId::new(1).unwrap(),
                blocks: vec![target_operations::TargetControlBlock {
                    block: BlockId::new(1).unwrap(),
                    parameters: vec![],
                    structural_parameters: vec![],
                    operations: vec![],
                    terminator,
                }],
            },
        }
    }

    #[test]
    fn residual_image_cleanup_retains_exact_jump_and_return_rosters() {
        use abstract_operations::AbstractOperation;
        use target_operations::{TargetControlSuccessor, TargetControlTerminator};
        let edge = EdgeId::new(1).unwrap();
        let destination = BlockId::new(2).unwrap();
        let root = PlaceId::new(4).unwrap();
        let residual = residual();
        let actions = vec![
            TerminalAffineCleanupAction::DiscardRoot(root),
            TerminalAffineCleanupAction::DiscardResidual(residual.clone()),
        ];
        let jump = AbstractOperation::Jump {
            psi_edge: edge,
            target: destination,
            bindings: vec![],
            structural_bindings: vec![],
            trivial_affine_discards: vec![root],
            residual_affine_discards: vec![residual],
        };
        let return_unit = AbstractOperation::ReturnUnit {
            psi_edge: edge,
            cleanup_actions: actions.clone(),
        };
        let value = semantic_vocabulary::ValueId::new(1).unwrap();
        let return_scalar = AbstractOperation::Return {
            psi_edge: edge,
            result: value,
            value,
            scalar_type: semantic_vocabulary::ScalarType::Boolean,
            cleanup_actions: actions.clone(),
        };
        for mutation in 0..8 {
            let mut actual = actions.clone();
            match mutation {
                0 => {}
                1 => {
                    actual.pop();
                }
                2 => actual.push(actual[1].clone()),
                3 => actual.swap(0, 1),
                4 => actual[1] = TerminalAffineCleanupAction::DiscardRoot(PlaceId::new(5).unwrap()),
                _ => {
                    let TerminalAffineCleanupAction::DiscardResidual(discard) = &mut actual[1]
                    else {
                        unreachable!()
                    };
                    match mutation {
                        5 => discard.place = PlaceId::new(6).unwrap(),
                        6 => discard.path = vec![StructuralPathSegment::Field("right".into())],
                        _ => discard.structural_type = StructuralTypeId::new(3).unwrap(),
                    }
                }
            }
            let jump_target = target(TargetControlTerminator::Jump {
                successor: TargetControlSuccessor {
                    psi_edge: edge,
                    target: destination,
                    bindings: vec![],
                    structural_bindings: vec![],
                    cleanup_actions: actual.clone(),
                },
            });
            assert_eq!(
                retained(&jump, &jump_target),
                mutation == 0,
                "jump mutation {mutation}"
            );
            let unit_target = target(TargetControlTerminator::Return {
                psi_edge: edge,
                cleanup_actions: actual.clone(),
            });
            assert_eq!(
                retained(&return_unit, &unit_target),
                mutation == 0,
                "Unit return mutation {mutation}"
            );
            let scalar_target = target(TargetControlTerminator::ReturnScalar {
                psi_edge: edge,
                source_value: value,
                expression: target_operations::TargetScalarExpression::Boolean(
                    target_operations::TargetBooleanExpression::Immediate {
                        source_value: value,
                        value: true,
                    },
                ),
                cleanup_actions: actual,
            });
            assert_eq!(
                retained(&return_scalar, &scalar_target),
                mutation == 0,
                "scalar return mutation {mutation}"
            );
        }
    }
}
