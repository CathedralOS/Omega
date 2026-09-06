//! Result roots retain producer identity and the exact partial ownership frontier.

use super::fixtures::*;
use abstract_operations::AbstractOperation;
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::{OperationId, PlaceId, StructuralPlaceKind};
use terminal_psi::{StructuralAccess, TerminalAffineCleanupAction};

use crate::tests::{id, refresh_function_derivatives};
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};

fn mixed_result() -> PsiOptimizationUnit {
    call_result_unit(
        vec![
            record(1, &[]),
            array(2, 1, 3),
            array(3, 2, 2),
            record(4, &[("left", 1), ("grid", 3), ("tail", 1)]),
        ],
        4,
        &[
            (vec![field("grid"), index(1)], 2),
            (vec![field("grid"), index(0), index(1)], 1),
        ],
        &[
            (vec![field("tail")], 1),
            (vec![field("grid"), index(0), index(2)], 1),
            (vec![field("grid"), index(0), index(0)], 1),
            (vec![field("left")], 1),
        ],
    )
}

fn empty_result() -> PsiOptimizationUnit {
    call_result_unit(
        vec![record(1, &[]), array(2, 1, 3), array(3, 2, 2)],
        3,
        &[
            (vec![index(1)], 2),
            (vec![index(0), index(2)], 1),
            (vec![index(0), index(0)], 1),
            (vec![index(0), index(1)], 1),
        ],
        &[],
    )
}

#[test]
fn call_result_partial_affine_retains_exact_residual_complement() {
    validate_psi_optimization_unit(&call_result_unit(
        vec![record(1, &[]), record(2, &[("left", 1), ("right", 1)])],
        2,
        &[(vec![field("right")], 1)],
        &[(vec![field("left")], 1)],
    ))
    .expect("an ordinary identity producer transfers one owner into the partial result root");
}

#[test]
fn call_result_partial_affine_requires_the_exact_claim_free_identity_body() {
    let baseline = mixed_result();
    validate_psi_optimization_unit(&baseline).unwrap();
    for mutation in 0..3 {
        let mut changed = baseline.clone();
        let producer_index = changed.functions.len() - 1;
        match mutation {
            0 => changed.functions[producer_index].verified_contract = None,
            1 => {
                let signature_result = changed.functions[producer_index]
                    .result
                    .structural()
                    .unwrap()
                    .place;
                let AbstractOperation::ReturnStructural { source, .. } =
                    &mut changed.functions[producer_index].blocks[0].nodes[0].operation
                else {
                    unreachable!()
                };
                // A declared output is not an established input to return.
                *source = signature_result;
            }
            2 => {
                let AbstractOperation::CallStructural { result, .. } =
                    &mut changed.functions[0].blocks[0].nodes[0].operation
                else {
                    unreachable!()
                };
                result
                    .claims
                    .push(terminal_psi::StructuralResultClaimBinding {
                        claim: id(90_020, semantic_vocabulary::ClaimId::new),
                        path: Vec::new(),
                    });
            }
            _ => unreachable!(),
        }
        refresh_function_derivatives(&mut changed, producer_index);
        refresh_function_derivatives(&mut changed, 0);
        let result = validate_psi_optimization_unit(&changed);
        assert!(
            matches!(
                result,
                Err(
                    OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. }
                )
            ),
            "identity producer mutation {mutation}: {result:?}"
        );
    }
}

#[test]
fn call_result_partial_affine_preserves_mixed_subtrees_and_empty_complements() {
    validate_psi_optimization_unit(&mixed_result()).expect("authored row and leaf moves");
    validate_psi_optimization_unit(&call_result_unit(
        vec![record(1, &[]), array(2, 1, 3), array(3, 2, 2)],
        3,
        &[(vec![index(1), index(1)], 1)],
        &[
            (vec![index(1), index(2)], 1),
            (vec![index(1), index(0)], 1),
            (vec![index(0)], 2),
        ],
    ))
    .expect("the untouched row remains one maximal residual");
    validate_psi_optimization_unit(&empty_result())
        .expect("fully transferred result is not discarded again");
}

#[test]
fn call_result_partial_affine_rejects_residual_custody_drift() {
    let baseline = mixed_result();
    validate_psi_optimization_unit(&baseline).unwrap();
    for mutation in 0..7 {
        let mut changed = baseline.clone();
        let input = changed.functions[0].structural_parameters[0].place;
        let actions = cleanup_actions(&mut changed);
        let TerminalAffineCleanupAction::DiscardResidual(first) = &actions[0] else {
            unreachable!()
        };
        let root = first.place;
        match mutation {
            0 => {
                actions.pop();
            }
            1 => actions.reverse(),
            2 => actions.push(actions[0].clone()),
            3 => {
                let TerminalAffineCleanupAction::DiscardResidual(first) = &mut actions[0] else {
                    unreachable!()
                };
                first.place = input;
            }
            4 => {
                let TerminalAffineCleanupAction::DiscardResidual(first) = &mut actions[0] else {
                    unreachable!()
                };
                first.path = vec![field("grid"), index(1)];
            }
            5 => actions.insert(0, TerminalAffineCleanupAction::DiscardRoot(root)),
            6 => actions.clear(),
            _ => unreachable!(),
        }
        refresh_function_derivatives(&mut changed, 0);
        let result = validate_psi_optimization_unit(&changed);
        assert!(
            matches!(
                result,
                Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
            ),
            "residual mutation {mutation}: {result:?}"
        );
    }
}

#[test]
fn call_result_partial_affine_rejects_producer_and_metadata_substitution() {
    let baseline = mixed_result();
    validate_psi_optimization_unit(&baseline).unwrap();
    for mutation in 0..6 {
        let mut changed = baseline.clone();
        let AbstractOperation::CallStructural { result, .. } =
            &changed.functions[0].blocks[0].nodes[0].operation
        else {
            unreachable!()
        };
        let root = result.place;
        match mutation {
            0 => {
                let declaration = changed.functions[0]
                    .structural_places
                    .iter_mut()
                    .find(|place| place.id == root)
                    .unwrap();
                let StructuralPlaceKind::OperationResult { producer, .. } = &mut declaration.kind
                else {
                    unreachable!()
                };
                *producer = id(90_001, OperationId::new);
            }
            1 => {
                let declaration = changed.functions[0]
                    .structural_places
                    .iter_mut()
                    .find(|place| place.id == root)
                    .unwrap();
                let StructuralPlaceKind::OperationResult {
                    structural_type, ..
                } = &mut declaration.kind
                else {
                    unreachable!()
                };
                *structural_type = changed.structural_types[0].id;
            }
            2 => {
                let AbstractOperation::CallStructural { result, .. } =
                    &mut changed.functions[0].blocks[0].nodes[0].operation
                else {
                    unreachable!()
                };
                result.structural_type = changed.structural_types[0].id;
            }
            3 => {
                changed.functions[0].blocks[0].nodes.swap(0, 1);
            }
            4 => {
                changed.functions[0].blocks[0].nodes.remove(0);
            }
            5 => {
                let input = changed.functions[0].structural_parameters[0].place;
                let AbstractOperation::CallUnit {
                    structural_arguments,
                    ..
                } = &mut changed.functions[0].blocks[0].nodes[1].operation
                else {
                    unreachable!()
                };
                // Same type and path, but the original owner was consumed.
                structural_arguments[0].place = input;
            }
            _ => unreachable!(),
        }
        refresh_function_derivatives(&mut changed, 0);
        let result = validate_psi_optimization_unit(&changed);
        assert!(result.is_err(), "producer mutation {mutation}: {result:?}");
    }
}

#[test]
fn call_result_partial_affine_rejects_overlapping_moves() {
    for moves in [
        vec![(vec![index(0)], 2), (vec![index(0), index(1)], 1)],
        vec![(vec![index(0), index(1)], 1), (vec![index(0)], 2)],
        vec![(vec![index(0), index(1)], 1), (vec![index(0), index(1)], 1)],
    ] {
        let result = validate_psi_optimization_unit(&call_result_unit(
            vec![record(1, &[]), array(2, 1, 3), array(3, 2, 2)],
            3,
            &moves,
            &[],
        ));
        assert!(
            matches!(
                result,
                Err(OptimizationUnitValidationError::CurrentProjectedMoveOverlap { .. })
            ),
            "{result:?}"
        );
    }
    let result = validate_psi_optimization_unit(&call_result_unit(
        vec![record(1, &[]), array(2, 1, 3), array(3, 2, 2)],
        3,
        &[(vec![index(0), index(1)], 1), (vec![], 3)],
        &[],
    ));
    assert!(
        matches!(
            result,
            Err(OptimizationUnitValidationError::CurrentWholePlacePartiallyMoved { .. })
        ),
        "{result:?}"
    );
}

#[test]
fn call_result_empty_complement_rejects_duplicate_cleanup() {
    let baseline = empty_result();
    validate_psi_optimization_unit(&baseline).unwrap();
    for residual in [false, true] {
        let mut changed = baseline.clone();
        let AbstractOperation::CallStructural { result, .. } =
            &changed.functions[0].blocks[0].nodes[0].operation
        else {
            unreachable!()
        };
        let root = result.place;
        let token_type = changed.structural_types[0].id;
        cleanup_actions(&mut changed).push(if residual {
            TerminalAffineCleanupAction::DiscardResidual(terminal_psi::StructuralAffineDiscard {
                place: root,
                path: vec![index(0), index(0)],
                structural_type: token_type,
            })
        } else {
            TerminalAffineCleanupAction::DiscardRoot(root)
        });
        refresh_function_derivatives(&mut changed, 0);
        let result = validate_psi_optimization_unit(&changed);
        assert!(
            matches!(
                result,
                Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
            ),
            "{result:?}"
        );
    }
}

#[test]
fn call_result_empty_complement_cannot_hide_a_second_live_result() {
    let mut changed = empty_result();
    validate_psi_optimization_unit(&changed).unwrap();
    let input = id(90_010, PlaceId::new);
    let root = id(90_011, PlaceId::new);
    let producer = id(90_012, OperationId::new);
    let mut parameter = changed.functions[0].structural_parameters[0].clone();
    parameter.place = input;
    parameter.position = 1;
    let structural_type = parameter.structural_type;
    changed.functions[0].structural_parameters.push(parameter);
    changed.functions[0].declared_places.extend([input, root]);
    changed.functions[0].structural_places.extend([
        terminal_psi::StructuralPlaceDeclaration {
            id: input,
            kind: StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        },
        terminal_psi::StructuralPlaceDeclaration {
            id: root,
            kind: StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            },
        },
    ]);
    let mut second = changed.functions[0].blocks[0].nodes[0].clone();
    let AbstractOperation::CallStructural {
        psi_operation,
        result,
        structural_arguments,
        ..
    } = &mut second.operation
    else {
        unreachable!()
    };
    *psi_operation = producer;
    result.place = root;
    structural_arguments[0].place = input;
    structural_arguments[0].access = StructuralAccess::Owned;
    let before_return = changed.functions[0].blocks[0].nodes.len() - 1;
    changed.functions[0].blocks[0]
        .nodes
        .insert(before_return, second);
    refresh_function_derivatives(&mut changed, 0);
    let result = validate_psi_optimization_unit(&changed);
    assert!(
        matches!(
            result,
            Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
        ),
        "a second still-owned result has no disposal: {result:?}"
    );
}
