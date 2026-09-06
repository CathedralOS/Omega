//! Structural boundary results retain signature, effect, and partial ownership custody.

use abstract_operations::{AbstractBoundaryResult, AbstractOperation};
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::{OperationId, PlaceId, StructuralPlaceKind};
use terminal_psi::{BoundaryMachineResult, StructuralMultiplicity, TerminalAffineCleanupAction};

use super::fixtures::*;
use crate::tests::{id, refresh_function_derivatives, structural_domain};
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};

fn result_unit(empty: bool) -> PsiOptimizationUnit {
    let moves = if empty {
        vec![
            (vec![field("grid"), index(1)], 2),
            (vec![field("left")], 1),
            (vec![field("grid"), index(0), index(1)], 1),
            (vec![field("grid"), index(0), index(0)], 1),
        ]
    } else {
        vec![(vec![field("grid"), index(1), index(0)], 1)]
    };
    let residuals = if empty {
        vec![]
    } else {
        vec![
            (vec![field("grid"), index(1), index(1)], 1),
            (vec![field("grid"), index(0)], 2),
            (vec![field("left")], 1),
        ]
    };
    boundary_result_unit(
        vec![
            record(1, &[]),
            array(2, 1, 2),
            array(3, 2, 2),
            record(4, &[("left", 1), ("grid", 3)]),
        ],
        4,
        &moves,
        &residuals,
    )
}

fn result(unit: &mut PsiOptimizationUnit) -> &mut terminal_psi::StructuralOperationResult {
    let AbstractOperation::BoundaryCall {
        result: AbstractBoundaryResult::Structural(result),
        ..
    } = &mut unit.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with structural boundary production")
    };
    result
}

#[test]
fn boundary_result_partial_affine_retains_maximal_and_empty_complements() {
    for empty in [false, true] {
        let unit = result_unit(empty);
        assert!(unit.functions[0].structural_parameters.is_empty());
        assert_eq!(unit.root_service_reach.installation_dependencies.len(), 1);
        assert_eq!(
            unit.root_service_reach.installation_dependencies[0].upper_bound,
            unit.boundary_machines[0].published_service_ceiling
        );
        assert!(
            unit.functions
                .iter()
                .skip(1)
                .all(|function| function.published_service_ceiling.is_empty())
        );
        validate_psi_optimization_unit(&unit)
            .unwrap_or_else(|error| panic!("empty {empty}: {error:?}"));
    }
}

#[test]
fn boundary_result_partial_affine_rejects_signature_and_result_drift() {
    let baseline = result_unit(false);
    validate_psi_optimization_unit(&baseline).unwrap();
    for mutation in 0..9 {
        let mut changed = baseline.clone();
        let alternate = changed.structural_types[0].id;
        match mutation {
            0 => changed.boundary_machines[0].result = BoundaryMachineResult::Unit,
            1 => {
                let BoundaryMachineResult::Structural(signature) =
                    &mut changed.boundary_machines[0].result
                else {
                    unreachable!()
                };
                signature.structural_type = alternate;
            }
            2 => result(&mut changed).structural_type = alternate,
            3 => result(&mut changed).multiplicity = StructuralMultiplicity::Unrestricted,
            4 => {
                let BoundaryMachineResult::Structural(signature) =
                    &mut changed.boundary_machines[0].result
                else {
                    unreachable!()
                };
                signature.multiplicity = StructuralMultiplicity::Linear;
            }
            5 => {
                let root_type = result(&mut changed).structural_type;
                let domain = structural_domain(90_030, 90_030, root_type);
                result(&mut changed).qualifications.push(domain.id);
                changed.structural_domains = vec![domain].into();
            }
            6 => result(&mut changed)
                .claims
                .push(terminal_psi::StructuralResultClaimBinding {
                    claim: id(90_031, semantic_vocabulary::ClaimId::new),
                    path: Vec::new(),
                }),
            7 => changed.boundary_machines[0]
                .scalar_parameters
                .push(semantic_vocabulary::ScalarType::Boolean),
            8 => {
                let root_type = result(&mut changed).structural_type;
                let domain = structural_domain(90_032, 90_032, root_type);
                let BoundaryMachineResult::Structural(signature) =
                    &mut changed.boundary_machines[0].result
                else {
                    unreachable!()
                };
                signature.qualifications.push(domain.id);
                changed.structural_domains = vec![domain].into();
            }
            _ => unreachable!(),
        }
        refresh_function_derivatives(&mut changed, 0);
        let checked = validate_psi_optimization_unit(&changed);
        assert!(
            checked.is_err(),
            "boundary signature mutation {mutation}: {checked:?}"
        );
    }
}

#[test]
fn boundary_result_partial_affine_rejects_producer_and_service_custody_drift() {
    let baseline = result_unit(false);
    validate_psi_optimization_unit(&baseline).unwrap();
    for mutation in 0..6 {
        let mut changed = baseline.clone();
        let root = result(&mut changed).place;
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
                *producer = id(90_040, OperationId::new);
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
            2 => changed.functions[0].blocks[0].nodes.swap(0, 1),
            3 => {
                changed.functions[0].blocks[0].nodes.remove(0);
            }
            4 => changed.functions[0].published_service_ceiling.clear(),
            5 => {
                let AbstractOperation::BoundaryCall { boundary, .. } =
                    &mut changed.functions[0].blocks[0].nodes[0].operation
                else {
                    unreachable!()
                };
                *boundary = id(90_041, semantic_vocabulary::BoundaryMachineId::new);
            }
            _ => unreachable!(),
        }
        refresh_function_derivatives(&mut changed, 0);
        let checked = validate_psi_optimization_unit(&changed);
        assert!(
            checked.is_err(),
            "boundary producer mutation {mutation}: {checked:?}"
        );
    }
}

#[test]
fn boundary_result_partial_affine_rejects_residual_order_and_ownership_drift() {
    for empty in [false, true] {
        let baseline = result_unit(empty);
        validate_psi_optimization_unit(&baseline).unwrap();
        for mutation in 0..if empty { 2 } else { 6 } {
            let mut changed = baseline.clone();
            let root = result(&mut changed).place;
            let leaf_type = changed.structural_types[0].id;
            let actions = cleanup_actions(&mut changed);
            match mutation {
                0 => actions.push(TerminalAffineCleanupAction::DiscardRoot(root)),
                1 => actions.push(TerminalAffineCleanupAction::DiscardResidual(
                    terminal_psi::StructuralAffineDiscard {
                        place: root,
                        path: vec![field("grid"), index(1), index(0)],
                        structural_type: leaf_type,
                    },
                )),
                2 => actions.reverse(),
                3 => {
                    actions.pop();
                }
                4 => actions.push(actions[0].clone()),
                5 => {
                    let TerminalAffineCleanupAction::DiscardResidual(residual) = &mut actions[1]
                    else {
                        unreachable!()
                    };
                    residual.structural_type = leaf_type;
                }
                _ => unreachable!(),
            }
            refresh_function_derivatives(&mut changed, 0);
            let checked = validate_psi_optimization_unit(&changed);
            assert!(
                matches!(
                    checked,
                    Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
                ),
                "empty {empty}, residual mutation {mutation}: {checked:?}"
            );
        }
    }
}

#[test]
fn boundary_result_partial_affine_rejects_overlapping_transfers() {
    for moves in [
        vec![(vec![index(0)], 2), (vec![index(0), index(1)], 1)],
        vec![(vec![index(0), index(1)], 1), (vec![index(0)], 2)],
        vec![(vec![index(0), index(1)], 1), (vec![index(0), index(1)], 1)],
    ] {
        let checked = validate_psi_optimization_unit(&boundary_result_unit(
            vec![record(1, &[]), array(2, 1, 2), array(3, 2, 2)],
            3,
            &moves,
            &[],
        ));
        assert!(
            matches!(
                checked,
                Err(OptimizationUnitValidationError::CurrentProjectedMoveOverlap { .. })
            ),
            "{checked:?}"
        );
    }
}

#[test]
fn boundary_result_empty_complement_cannot_hide_another_live_result() {
    let mut changed = result_unit(true);
    validate_psi_optimization_unit(&changed).unwrap();
    let root = id(90_050, PlaceId::new);
    let producer = id(90_051, OperationId::new);
    let mut second = changed.functions[0].blocks[0].nodes[0].clone();
    let AbstractOperation::BoundaryCall {
        psi_operation,
        result: AbstractBoundaryResult::Structural(result),
        ..
    } = &mut second.operation
    else {
        unreachable!()
    };
    *psi_operation = producer;
    result.place = root;
    changed.functions[0].declared_places.insert(root);
    changed.functions[0]
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: root,
            kind: StructuralPlaceKind::OperationResult {
                producer,
                structural_type: result.structural_type,
            },
        });
    let before_return = changed.functions[0].blocks[0].nodes.len() - 1;
    changed.functions[0].blocks[0]
        .nodes
        .insert(before_return, second);
    refresh_function_derivatives(&mut changed, 0);
    let checked = validate_psi_optimization_unit(&changed);
    assert!(
        matches!(
            checked,
            Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
        ),
        "second undisposed factory result: {checked:?}"
    );
}

#[test]
fn boundary_result_return_cannot_implicitly_drop_an_untouched_result() {
    let checked = validate_psi_optimization_unit(&boundary_result_unit(
        vec![record(1, &[]), record(2, &[("left", 1), ("right", 1)])],
        2,
        &[],
        &[],
    ));
    assert!(
        matches!(
            checked,
            Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
        ),
        "an untouched boundary result is still owned: {checked:?}"
    );
}
