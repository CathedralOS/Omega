//! Current-claim replay rejection for duplicate transfer, stale crash, and invalid returns.

use crate::tests::{
    affine_claim_join_unit, affine_claim_transfer_unit, id, refresh_function_derivatives,
    refresh_node_derivatives, structural_result_call_unit,
};
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use omega_abstract_operations::{AbstractFunctionResult, AbstractOperation};
use psi_core::{ClaimId, OperationId};

#[test]
fn current_claim_replay_rejects_double_transfer_stale_crash_and_invalid_returns() {
    let claim = id(1, ClaimId::new);
    validate_psi_optimization_unit(&affine_claim_join_unit(true))
        .expect("equal current claim settlement on both arms joins exactly");
    assert!(matches!(
        validate_psi_optimization_unit(&affine_claim_join_unit(false)),
        Err(OptimizationUnitValidationError::CurrentClaimJoinMismatch { .. })
    ));

    let baseline = affine_claim_transfer_unit();
    validate_psi_optimization_unit(&baseline).expect("one affine claim transfer is live");

    let mut double_transfer = baseline.clone();
    let mut repeated = double_transfer.functions[0].blocks[0].nodes[0].clone();
    let AbstractOperation::CallUnit { psi_operation, .. } = &mut repeated.operation else {
        unreachable!("fixture starts with a Unit call")
    };
    *psi_operation = id(341, OperationId::new);
    double_transfer.functions[0].blocks[0]
        .nodes
        .insert(1, repeated);
    refresh_function_derivatives(&mut double_transfer, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&double_transfer),
        Err(OptimizationUnitValidationError::CurrentClaimNotLive {
            node: 1,
            claim: actual,
            ..
        }) if actual == claim
    ));

    let mut stale_crash = baseline;
    let return_node = stale_crash.functions[0].blocks[0].nodes.len() - 1;
    let psi_edge = match stale_crash.functions[0].blocks[0].nodes[return_node].operation {
        AbstractOperation::ReturnUnit { psi_edge, .. } => psi_edge,
        _ => unreachable!("fixture returns Unit"),
    };
    stale_crash.functions[0].blocks[0].nodes[return_node].operation = AbstractOperation::Crash {
        psi_edge,
        cause: psi_terminal::CrashCause::Trap,
        site_guard: Vec::new(),
        frontier_lower_bound: vec![claim],
    };
    refresh_node_derivatives(&mut stale_crash, 0, 0, return_node);
    assert!(matches!(
        validate_psi_optimization_unit(&stale_crash),
        Err(OptimizationUnitValidationError::CurrentCrashClaimFrontierMismatch { .. })
    ));

    let baseline = structural_result_call_unit();
    let mut missing_return = baseline.clone();
    let return_node = missing_return.functions[0].blocks[0].nodes.len() - 1;
    let AbstractOperation::ReturnStructural {
        returned_claims, ..
    } = &mut missing_return.functions[0].blocks[0].nodes[return_node].operation
    else {
        unreachable!("fixture returns the structural call result")
    };
    returned_claims.clear();
    refresh_node_derivatives(&mut missing_return, 0, 0, return_node);
    assert!(matches!(
        validate_psi_optimization_unit(&missing_return),
        Err(OptimizationUnitValidationError::CurrentStructuralReturnClaimSetMismatch { .. })
    ));

    let mut linear_unit_return = baseline;
    let result_place = linear_unit_return.functions[0]
        .result
        .structural()
        .expect("fixture has a structural result")
        .place;
    linear_unit_return.functions[0].result = AbstractFunctionResult::Unit;
    linear_unit_return.functions[0]
        .structural_places
        .retain(|place| place.id != result_place);
    linear_unit_return.functions[0]
        .declared_places
        .remove(&result_place);
    let return_node = linear_unit_return.functions[0].blocks[0].nodes.len() - 1;
    let psi_edge = match linear_unit_return.functions[0].blocks[0].nodes[return_node].operation {
        AbstractOperation::ReturnStructural { psi_edge, .. } => psi_edge,
        _ => unreachable!("fixture returns structurally"),
    };
    linear_unit_return.functions[0].blocks[0].nodes[return_node].operation =
        AbstractOperation::ReturnUnit {
            psi_edge,
            cleanup_actions: Vec::new(),
        };
    refresh_node_derivatives(&mut linear_unit_return, 0, 0, return_node);
    assert!(matches!(
        validate_psi_optimization_unit(&linear_unit_return),
        Err(OptimizationUnitValidationError::CurrentLinearClaimAtReturn {
            claim: actual,
            ..
        }) if actual == claim
    ));
}
