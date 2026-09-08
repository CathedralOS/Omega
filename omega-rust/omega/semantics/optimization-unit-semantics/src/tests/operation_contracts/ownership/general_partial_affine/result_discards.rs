//! Abstract boundary results require exact, explicit whole-root cleanup.

use abstract_operations::{AbstractBoundaryResult, AbstractOperation};
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::{ClaimId, OperationId, PlaceId, StructuralPlaceKind};
use terminal_psi::{BoundaryMachineResult, StructuralMultiplicity, TerminalAffineCleanupAction};

use super::fixtures::*;
use crate::tests::{id, refresh_function_derivatives};
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};

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

fn result_unit() -> PsiOptimizationUnit {
    let mut unit = boundary_result_unit(
        vec![record(1, &[]), record(2, &[("left", 1), ("right", 1)])],
        2,
        &[],
        &[],
    );
    let root = result(&mut unit).place;
    *cleanup_actions(&mut unit) = vec![TerminalAffineCleanupAction::DiscardRoot(root)];
    refresh_function_derivatives(&mut unit, 0);
    unit
}

#[test]
fn boundary_result_whole_root_discard_is_explicit() {
    let baseline = result_unit();
    validate_psi_optimization_unit(&baseline).unwrap();
    let mut missing = baseline;
    cleanup_actions(&mut missing).clear();
    refresh_function_derivatives(&mut missing, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&missing),
        Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
    ));
}

#[test]
fn boundary_result_discards_preserve_reverse_producer_order() {
    let mut baseline = result_unit();
    let first = result(&mut baseline).place;
    let root = id(90_060, PlaceId::new);
    let producer = id(90_061, OperationId::new);
    let mut second = baseline.functions[0].blocks[0].nodes[0].clone();
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
    baseline.functions[0].declared_places.insert(root);
    baseline.functions[0]
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: root,
            kind: StructuralPlaceKind::OperationResult {
                producer,
                structural_type: result.structural_type,
            },
        });
    baseline.functions[0].blocks[0].nodes.insert(1, second);
    *cleanup_actions(&mut baseline) = vec![
        TerminalAffineCleanupAction::DiscardRoot(root),
        TerminalAffineCleanupAction::DiscardRoot(first),
    ];
    refresh_function_derivatives(&mut baseline, 0);
    validate_psi_optimization_unit(&baseline).unwrap();
    for mutation in 0..4 {
        let mut changed = baseline.clone();
        let actions = cleanup_actions(&mut changed);
        match mutation {
            0 => {
                actions.pop();
            }
            1 => actions.reverse(),
            2 => actions[0] = actions[1].clone(),
            3 => actions.push(actions[0].clone()),
            _ => unreachable!(),
        }
        refresh_function_derivatives(&mut changed, 0);
        let checked = validate_psi_optimization_unit(&changed);
        assert!(
            matches!(
                checked,
                Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
            ),
            "cleanup mutation {mutation}: {checked:?}"
        );
    }
}

#[test]
fn boundary_result_discard_rejects_linear_or_forged_claimed_results() {
    let baseline = result_unit();
    validate_psi_optimization_unit(&baseline).unwrap();
    for linear in [false, true] {
        let mut changed = baseline.clone();
        if linear {
            result(&mut changed).multiplicity = StructuralMultiplicity::Linear;
            let BoundaryMachineResult::Structural(signature) =
                &mut changed.boundary_machines[0].result
            else {
                unreachable!()
            };
            signature.multiplicity = StructuralMultiplicity::Linear;
        } else {
            result(&mut changed)
                .claims
                .push(terminal_psi::StructuralResultClaimBinding {
                    claim: id(90_062, ClaimId::new),
                    path: Vec::new(),
                });
        }
        refresh_function_derivatives(&mut changed, 0);
        let checked = validate_psi_optimization_unit(&changed);
        assert!(checked.is_err(), "linear {linear}: {checked:?}");
    }
}

#[test]
fn boundary_result_whole_root_discard_rejects_partial_custody() {
    let mut changed = boundary_result_unit(
        vec![record(1, &[]), record(2, &[("left", 1), ("right", 1)])],
        2,
        &[(vec![field("left")], 1)],
        &[(vec![field("right")], 1)],
    );
    validate_psi_optimization_unit(&changed).unwrap();
    let root = result(&mut changed).place;
    *cleanup_actions(&mut changed) = vec![TerminalAffineCleanupAction::DiscardRoot(root)];
    refresh_function_derivatives(&mut changed, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&changed),
        Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
    ));
}
