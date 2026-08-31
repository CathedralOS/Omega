//! Scalar internal and boundary call signature corruption rejection.

use crate::tests::{
    refresh_identity, refresh_node_derivatives, scalar_boundary_call_unit, scalar_call_unit,
};
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use omega_abstract_operations::AbstractOperation;

#[test]
fn rejects_self_consistent_call_signature_corruption() {
    let mut call = scalar_call_unit();
    let (psi_operation, result) = match &call.functions[0].blocks[0].nodes[0].operation {
        AbstractOperation::IntegerConstant {
            psi_operation,
            result,
            ..
        } => (*psi_operation, *result),
        _ => panic!("caller begins with an integer constant"),
    };
    call.functions[0].blocks[0].nodes[0].operation = AbstractOperation::BooleanConstant {
        psi_operation,
        result,
        value: true,
    };
    refresh_node_derivatives(&mut call, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&call),
        Err(OptimizationUnitValidationError::ScalarOperationContractMismatch { node: 1, .. })
    ));

    let mut boundary = scalar_boundary_call_unit();
    let (psi_operation, result) = match &boundary.functions[0].blocks[0].nodes[0].operation {
        AbstractOperation::IntegerConstant {
            psi_operation,
            result,
            ..
        } => (*psi_operation, *result),
        _ => panic!("boundary caller begins with an integer constant"),
    };
    boundary.functions[0].blocks[0].nodes[0].operation = AbstractOperation::BooleanConstant {
        psi_operation,
        result,
        value: true,
    };
    refresh_node_derivatives(&mut boundary, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&boundary),
        Err(OptimizationUnitValidationError::ScalarOperationContractMismatch { node: 1, .. })
    ));

    let mut duplicate_boundary = scalar_boundary_call_unit();
    duplicate_boundary
        .boundary_machines
        .push(duplicate_boundary.boundary_machines[0].clone());
    refresh_identity(&mut duplicate_boundary);
    assert!(matches!(
        validate_psi_optimization_unit(&duplicate_boundary),
        Err(OptimizationUnitValidationError::DuplicateBoundaryMachine(_))
    ));
}
