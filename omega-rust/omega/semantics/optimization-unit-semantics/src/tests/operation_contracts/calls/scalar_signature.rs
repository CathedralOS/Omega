//! Scalar internal and boundary call signature corruption rejection.

use crate::tests::{
    id, refresh_identity, refresh_node_derivatives, scalar_boundary_call_unit, scalar_call_unit,
    structural_call_unit,
};
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use abstract_operations::AbstractOperation;
use optimization_unit::{ValueDefinition, ValueDefinitionSite};
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType, ValueId};

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

#[test]
fn unit_call_scalar_arguments_match_the_callee_signature() {
    let mut unit = structural_call_unit();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    let caller_value = id(341, ValueId::new);
    let callee_value = id(342, ValueId::new);
    unit.functions[0].parameters.push(ValueDefinition {
        value: caller_value,
        scalar_type,
        site: ValueDefinitionSite::FunctionParameter(0),
    });
    unit.functions[1].parameters.push(ValueDefinition {
        value: callee_value,
        scalar_type,
        site: ValueDefinitionSite::FunctionParameter(0),
    });
    let AbstractOperation::CallUnit { arguments, .. } =
        &mut unit.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with a Unit call")
    };
    arguments.push(caller_value);
    refresh_node_derivatives(&mut unit, 0, 0, 0);
    validate_psi_optimization_unit(&unit)
        .expect("Unit call scalar argument matches its callee parameter");

    let AbstractOperation::CallUnit { arguments, .. } =
        &mut unit.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    arguments.clear();
    refresh_node_derivatives(&mut unit, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&unit),
        Err(OptimizationUnitValidationError::ScalarOperationContractMismatch { node: 0, .. })
    ));
}
