//! Affine-quartet reconstruction of ordered moves and decreasing residuals.

use crate::tests::{partial_affine_quartet_unit, refresh_node_derivatives};
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use abstract_operations::AbstractOperation;

#[test]
fn affine_quartet_optimizer_reconstructs_two_moves_and_decreasing_residuals() {
    let baseline = partial_affine_quartet_unit();
    validate_psi_optimization_unit(&baseline)
        .expect("optimizer replay accepts the exact quartet partition");

    let mut increasing = baseline.clone();
    let return_node = increasing.functions[0].blocks[0].nodes.len() - 1;
    let AbstractOperation::ReturnUnit {
        cleanup_actions, ..
    } = &mut increasing.functions[0].blocks[0].nodes[return_node].operation
    else {
        unreachable!("quartet fixture returns Unit")
    };
    cleanup_actions.reverse();
    refresh_node_derivatives(&mut increasing, 0, 0, return_node);
    assert!(matches!(
        validate_psi_optimization_unit(&increasing),
        Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
    ));

    let mut duplicate = baseline;
    let duplicate_path = match &duplicate.functions[0].blocks[0].nodes[0].operation {
        AbstractOperation::CallUnit {
            structural_arguments,
            ..
        } => structural_arguments[0].path.clone(),
        _ => unreachable!("quartet fixture begins with a projected call"),
    };
    let AbstractOperation::CallUnit {
        structural_arguments,
        ..
    } = &mut duplicate.functions[0].blocks[0].nodes[1].operation
    else {
        unreachable!("quartet fixture has a second projected call")
    };
    structural_arguments[0].path = duplicate_path;
    refresh_node_derivatives(&mut duplicate, 0, 0, 1);
    assert!(matches!(
        validate_psi_optimization_unit(&duplicate),
        Err(OptimizationUnitValidationError::CurrentProjectedMoveOverlap { .. })
    ));
}
