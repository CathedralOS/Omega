//! Owned-place replay rejection for duplicate moves, unequal joins, and invalid residuals.

use crate::tests::{
    affine_place_join_unit, affine_place_transfer_unit, boolean_structural_field_unit, id,
    partial_affine_place_unit, refresh_function_derivatives, refresh_identity,
    refresh_node_derivatives,
};
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use omega_abstract_operations::AbstractOperation;
use omega_optimization_unit::PsiOptimizationUnit;
use psi_core::{OperationId, PlaceId};

#[test]
fn current_owned_place_replay_rejects_double_moves_unequal_joins_and_bad_residuals() {
    let baseline = affine_place_transfer_unit();
    validate_psi_optimization_unit(&baseline)
        .expect("one claim-free affine whole-root transfer is exact");

    let mut double_move = baseline;
    let mut repeated = double_move.functions[0].blocks[0].nodes[0].clone();
    let AbstractOperation::CallUnit { psi_operation, .. } = &mut repeated.operation else {
        unreachable!("fixture begins with a Unit call")
    };
    *psi_operation = id(4_862, OperationId::new);
    double_move.functions[0].blocks[0].nodes.insert(1, repeated);
    refresh_function_derivatives(&mut double_move, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&double_move),
        Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive { node: 1, .. })
    ));

    validate_psi_optimization_unit(&affine_place_join_unit(true))
        .expect("equal whole-root settlement on both arms joins exactly");
    assert!(matches!(
        validate_psi_optimization_unit(&affine_place_join_unit(false)),
        Err(OptimizationUnitValidationError::CurrentOwnedPlaceJoinMismatch { .. })
    ));

    let baseline = partial_affine_place_unit();
    validate_psi_optimization_unit(&baseline)
        .expect("one projected move and its exact residual cleanup validate");

    let mut overlap = baseline.clone();
    let mut repeated = overlap.functions[0].blocks[0].nodes[0].clone();
    let AbstractOperation::CallUnit { psi_operation, .. } = &mut repeated.operation else {
        unreachable!("fixture begins with a projected Unit call")
    };
    *psi_operation = id(4_863, OperationId::new);
    overlap.functions[0].blocks[0].nodes.insert(1, repeated);
    refresh_function_derivatives(&mut overlap, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&overlap),
        Err(OptimizationUnitValidationError::CurrentProjectedMoveOverlap { node: 1, .. })
    ));

    let mutate_residual =
        |unit: &mut PsiOptimizationUnit,
         mutate: &dyn Fn(&mut psi_terminal::StructuralAffineDiscard)| {
            let return_node = unit.functions[0].blocks[0].nodes.len() - 1;
            let AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } = &mut unit.functions[0].blocks[0].nodes[return_node].operation
            else {
                unreachable!("fixture returns Unit")
            };
            let [psi_terminal::TerminalAffineCleanupAction::DiscardResidual(residual)] =
                cleanup_actions.as_mut_slice()
            else {
                unreachable!("fixture has one residual cleanup")
            };
            mutate(residual);
            refresh_node_derivatives(unit, 0, 0, return_node);
        };

    let mut wrong_path = baseline.clone();
    mutate_residual(&mut wrong_path, &|residual| {
        residual.path = vec![psi_terminal::StructuralPathSegment::Field("right".into())];
    });
    assert!(matches!(
        validate_psi_optimization_unit(&wrong_path),
        Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
    ));

    let mut wrong_type = baseline.clone();
    let pair_type = wrong_type.functions[0].structural_parameters[0].structural_type;
    mutate_residual(&mut wrong_type, &|residual| {
        residual.structural_type = pair_type;
    });
    assert!(matches!(
        validate_psi_optimization_unit(&wrong_type),
        Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
    ));

    let mut missing = baseline;
    let return_node = missing.functions[0].blocks[0].nodes.len() - 1;
    let AbstractOperation::ReturnUnit {
        cleanup_actions, ..
    } = &mut missing.functions[0].blocks[0].nodes[return_node].operation
    else {
        unreachable!("fixture returns Unit")
    };
    cleanup_actions.clear();
    refresh_node_derivatives(&mut missing, 0, 0, return_node);
    assert!(matches!(
        validate_psi_optimization_unit(&missing),
        Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
    ));

    let nominal = boolean_structural_field_unit();
    let mut missing_target = nominal.clone();
    missing_target.functions.pop();
    refresh_identity(&mut missing_target);
    assert!(matches!(
        validate_psi_optimization_unit(&missing_target),
        Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
    ));

    let mut wrong_attachment = nominal.clone();
    wrong_attachment.functions[1].attachment = None;
    refresh_identity(&mut wrong_attachment);
    assert!(matches!(
        validate_psi_optimization_unit(&wrong_attachment),
        Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
    ));

    let mut unnormalized = nominal;
    let AbstractOperation::Return {
        cleanup_actions, ..
    } = &mut unnormalized.functions[0].blocks[0].nodes[1].operation
    else {
        unreachable!("nominal fixture returns a scalar")
    };
    let [psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup)] =
        cleanup_actions.as_mut_slice()
    else {
        unreachable!("nominal fixture has one cleanup")
    };
    cleanup.cleanup_receiver = Some(id(4_864, PlaceId::new));
    refresh_node_derivatives(&mut unnormalized, 0, 0, 1);
    assert!(matches!(
        validate_psi_optimization_unit(&unnormalized),
        Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
    ));
}
