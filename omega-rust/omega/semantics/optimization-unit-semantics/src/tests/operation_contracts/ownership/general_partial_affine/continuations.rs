//! Continuation cleanup is replayed before the successor, including result roots.

use super::fixtures::*;
use abstract_operations::AbstractOperation;
use optimization_unit::{OptimizationBlock, PsiOptimizationUnit};
use semantic_vocabulary::{BlockId, EdgeId, OperationId, PlaceId, StructuralTypeId};
use terminal_psi::{StructuralAffineDiscard, TerminalAffineCleanupAction};

use crate::tests::{id, refresh_function_derivatives};
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};

fn continuation(mut unit: PsiOptimizationUnit) -> PsiOptimizationUnit {
    let target = id(70_001, BlockId::new);
    let mut returned = unit.functions[0].blocks[0].nodes.pop().unwrap();
    let AbstractOperation::ReturnUnit {
        cleanup_actions, ..
    } = &mut returned.operation
    else {
        unreachable!()
    };
    let residual_affine_discards = std::mem::take(cleanup_actions)
        .into_iter()
        .map(|action| {
            let TerminalAffineCleanupAction::DiscardResidual(discard) = action else {
                unreachable!()
            };
            discard
        })
        .collect();
    let mut jump = returned.clone();
    jump.operation = AbstractOperation::Jump {
        structural_bindings: Vec::new(),
        psi_edge: id(70_002, EdgeId::new),
        target,
        bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards,
    };
    unit.functions[0].blocks[0].nodes.push(jump);
    unit.functions[0].blocks.push(OptimizationBlock {
        structural_parameters: Vec::new(),
        id: target,
        parameters: Vec::new(),
        nodes: vec![returned],
    });
    refresh_function_derivatives(&mut unit, 0);
    unit
}

fn result_continuation(boundary: bool, empty: bool) -> PsiOptimizationUnit {
    let builder = if boundary {
        boundary_result_unit
    } else {
        call_result_unit
    };
    let moves = if empty {
        vec![
            (vec![field("right")], 1),
            (vec![field("middle")], 1),
            (vec![field("left")], 1),
        ]
    } else {
        vec![(vec![field("middle")], 1)]
    };
    let residuals = if empty {
        Vec::new()
    } else {
        vec![(vec![field("right")], 1), (vec![field("left")], 1)]
    };
    continuation(builder(
        vec![
            record(1, &[]),
            record(2, &[("left", 1), ("middle", 1), ("right", 1)]),
        ],
        2,
        &moves,
        &residuals,
    ))
}

#[test]
fn partial_result_continuation_replays_ordinary_and_boundary_roots() {
    for boundary in [false, true] {
        for empty in [false, true] {
            validate_psi_optimization_unit(&result_continuation(boundary, empty))
                .unwrap_or_else(|error| panic!("boundary={boundary} empty={empty}: {error:?}"));
        }
    }
}

#[test]
fn partial_continuation_preserves_nested_reverse_residual_order() {
    validate_psi_optimization_unit(&continuation(unit(
        vec![
            record(1, &[]),
            array(2, 1, 3),
            record(3, &[("left", 1), ("grid", 2), ("tail", 1)]),
        ],
        3,
        &[(vec![field("grid"), index(1)], 1)],
        &[
            (vec![field("tail")], 1),
            (vec![field("grid"), index(2)], 1),
            (vec![field("grid"), index(0)], 1),
            (vec![field("left")], 1),
        ],
    )))
    .expect("nested maximal residuals remain ordered before successor entry");
}

#[test]
fn partial_result_continuation_rejects_missing_reordered_or_forged_cleanup() {
    for boundary in [false, true] {
        for mutation in 0..7 {
            let mut changed = result_continuation(boundary, false);
            let AbstractOperation::Jump {
                trivial_affine_discards,
                residual_affine_discards,
                ..
            } = &mut changed.functions[0].blocks[0]
                .nodes
                .last_mut()
                .unwrap()
                .operation
            else {
                unreachable!()
            };
            match mutation {
                0 => residual_affine_discards.clear(),
                1 => {
                    residual_affine_discards.pop();
                }
                2 => residual_affine_discards.reverse(),
                3 => residual_affine_discards[0].structural_type = id(2, StructuralTypeId::new),
                4 => residual_affine_discards[0].place = id(8_001, PlaceId::new),
                5 => trivial_affine_discards.push(id(1_000, PlaceId::new)),
                6 => residual_affine_discards.push(StructuralAffineDiscard {
                    place: id(1_000, PlaceId::new),
                    path: vec![field("middle")],
                    structural_type: id(1, StructuralTypeId::new),
                }),
                _ => unreachable!(),
            }
            refresh_function_derivatives(&mut changed, 0);
            assert!(
                matches!(
                    validate_psi_optimization_unit(&changed),
                    Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
                ),
                "boundary={boundary} mutation={mutation}"
            );
        }
    }
}

#[test]
fn partial_continuation_rejects_cleanup_delayed_until_successor_return() {
    reject_successor_cleanup(false);
}

#[test]
fn partial_continuation_rejects_cleanup_repeated_at_successor_return() {
    reject_successor_cleanup(true);
}

fn reject_successor_cleanup(retain_jump_cleanup: bool) {
    for boundary in [false, true] {
        let mut changed = result_continuation(boundary, false);
        validate_psi_optimization_unit(&changed).expect("the original continuation validates");
        let AbstractOperation::Jump {
            residual_affine_discards,
            ..
        } = &mut changed.functions[0].blocks[0]
            .nodes
            .last_mut()
            .unwrap()
            .operation
        else {
            unreachable!()
        };
        let residuals = if retain_jump_cleanup {
            residual_affine_discards.clone()
        } else {
            std::mem::take(residual_affine_discards)
        };
        let AbstractOperation::ReturnUnit {
            cleanup_actions, ..
        } = &mut changed.functions[0].blocks[1].nodes[0].operation
        else {
            unreachable!()
        };
        *cleanup_actions = residuals
            .into_iter()
            .map(TerminalAffineCleanupAction::DiscardResidual)
            .collect();
        refresh_function_derivatives(&mut changed, 0);
        let expected_block = changed.functions[0].blocks[usize::from(retain_jump_cleanup)].id;
        assert_eq!(
            validate_psi_optimization_unit(&changed),
            Err(OptimizationUnitValidationError::CurrentCleanupMismatch {
                machine: changed.functions[0].machine,
                block: expected_block,
            }),
            "boundary={boundary} retain_jump_cleanup={retain_jump_cleanup}",
        );
    }
}

fn append_caller_parameter(
    unit: &mut PsiOptimizationUnit,
    parameter: terminal_psi::StructuralParameterDeclaration,
) {
    unit.functions[0].declared_places.insert(parameter.place);
    unit.functions[0]
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: parameter.place,
            kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                position: parameter.position,
                is_self: parameter.is_self,
            },
        });
    unit.functions[0].structural_parameters.push(parameter);
}

#[test]
fn partial_continuation_retains_unrelated_owner_until_successor_cleanup() {
    let mut unit = result_continuation(true, false);
    let unrelated = id(70_003, PlaceId::new);
    let mut parameter = unit.functions[1].structural_parameters[0].clone();
    parameter.place = unrelated;
    append_caller_parameter(&mut unit, parameter);
    let AbstractOperation::ReturnUnit {
        cleanup_actions, ..
    } = &mut unit.functions[0].blocks[1].nodes[0].operation
    else {
        unreachable!()
    };
    cleanup_actions.push(TerminalAffineCleanupAction::DiscardRoot(unrelated));
    refresh_function_derivatives(&mut unit, 0);
    validate_psi_optimization_unit(&unit)
        .expect("the successor still owns the unrelated parameter");
}

#[test]
fn empty_partial_continuation_rejects_interleaved_whole_cleanup() {
    let mut unit = result_continuation(true, true);
    let unrelated = id(70_003, PlaceId::new);
    let mut parameter = unit.functions[1].structural_parameters[0].clone();
    parameter.place = unrelated;
    append_caller_parameter(&mut unit, parameter);
    let AbstractOperation::Jump {
        trivial_affine_discards,
        ..
    } = &mut unit.functions[0].blocks[0]
        .nodes
        .last_mut()
        .unwrap()
        .operation
    else {
        unreachable!()
    };
    trivial_affine_discards.push(unrelated);
    refresh_function_derivatives(&mut unit, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&unit),
        Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
    ));
}

#[test]
fn empty_partial_continuation_rejects_two_dying_roots() {
    let mut unit = continuation(unit(
        vec![record(1, &[]), array(2, 1, 1)],
        2,
        &[(vec![index(0)], 1)],
        &[],
    ));
    let unrelated = id(70_003, PlaceId::new);
    let mut parameter = unit.functions[0].structural_parameters[0].clone();
    parameter.place = unrelated;
    parameter.position = 1;
    append_caller_parameter(&mut unit, parameter);
    let mut second = unit.functions[0].blocks[0].nodes[0].clone();
    let AbstractOperation::CallUnit {
        psi_operation,
        structural_arguments,
        ..
    } = &mut second.operation
    else {
        unreachable!()
    };
    *psi_operation = id(70_004, OperationId::new);
    structural_arguments[0].place = unrelated;
    unit.functions[0].blocks[0].nodes.insert(1, second);
    refresh_function_derivatives(&mut unit, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&unit),
        Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
    ));
}

#[test]
fn partial_continuation_rejoins_exact_retained_edge_snapshots() {
    use optimization_unit::{
        OwnershipFrontierFact, OwnershipFrontierOwnedPlace, OwnershipFrontierPartialCustody,
        OwnershipFrontierSite, OwnershipFrontierSnapshot,
    };
    use terminal_psi::StructuralMultiplicity;

    for mutation in 0..4 {
        let mut unit = result_continuation(false, false);
        let root = id(1_000, PlaceId::new);
        let mut entry = OwnershipFrontierSnapshot {
            claims: Vec::new(),
            owned_places: vec![OwnershipFrontierOwnedPlace {
                place: root,
                multiplicity: StructuralMultiplicity::Affine,
            }],
            partial_custody: vec![OwnershipFrontierPartialCustody {
                place: root,
                moved_paths: vec![vec![field("middle")]],
            }],
        };
        let mut exit = OwnershipFrontierSnapshot {
            claims: Vec::new(),
            owned_places: Vec::new(),
            partial_custody: Vec::new(),
        };
        match mutation {
            0 => {}
            1 => exit.owned_places = entry.owned_places.clone(),
            2 => exit.partial_custody = entry.partial_custody.clone(),
            3 => entry.partial_custody[0].moved_paths = vec![vec![field("right")]],
            _ => unreachable!(),
        }
        let machine = unit.functions[0].machine;
        let edge = id(70_002, EdgeId::new);
        unit.ownership_frontier_facts = vec![
            OwnershipFrontierFact::new(
                unit.psi,
                machine,
                OwnershipFrontierSite::EdgeEntry(edge),
                entry,
            ),
            OwnershipFrontierFact::new(
                unit.psi,
                machine,
                OwnershipFrontierSite::EdgeExit(edge),
                exit,
            ),
        ];
        crate::tests::refresh_identity(&mut unit);
        let result = validate_psi_optimization_unit(&unit);
        if mutation == 0 {
            result.expect("retained entry and exit reconstruct the exact same partial cleanup");
        } else {
            assert!(
                matches!(
                    result,
                    Err(
                        OptimizationUnitValidationError::StructuralEdgeAffineDiscardsMismatch { .. }
                    )
                ),
                "mutation={mutation}: {result:?}"
            );
        }
    }
}
