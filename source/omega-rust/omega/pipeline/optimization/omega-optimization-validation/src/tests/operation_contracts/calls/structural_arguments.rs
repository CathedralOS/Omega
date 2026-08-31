//! Structural-call argument arity and structural-access corruption rejection.

use crate::tests::{id, refresh_identity, refresh_node_derivatives, structural_call_unit};
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use omega_abstract_operations::AbstractOperation;
use psi_core::BoundaryMachineId;

#[test]
fn rejects_structural_call_argument_arity_and_access_corruption() {
    let baseline = structural_call_unit();
    validate_psi_optimization_unit(&baseline)
        .expect("matching structural argument access should validate");

    let mut access = baseline.clone();
    let AbstractOperation::CallUnit {
        structural_arguments,
        ..
    } = &mut access.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with a structural Unit call")
    };
    structural_arguments[0].access = psi_terminal::StructuralAccess::SharedBorrow;
    refresh_node_derivatives(&mut access, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&access),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));

    let mut arity = baseline;
    let AbstractOperation::CallUnit {
        structural_arguments,
        ..
    } = &mut arity.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with a structural Unit call")
    };
    structural_arguments.clear();
    refresh_node_derivatives(&mut arity, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&arity),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));

    let mut boundary = structural_call_unit();
    let boundary_id = id(341, BoundaryMachineId::new);
    boundary
        .boundary_machines
        .push(psi_terminal::BoundaryMachineDeclaration {
            id: boundary_id,
            identity: "validation::structural-boundary".into(),
            attachment: None,
            scalar_parameters: Vec::new(),
            structural_parameters: vec![boundary.functions[1].structural_parameters[0].clone()],
            result: None,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        });
    let (psi_operation, structural_arguments) =
        match &boundary.functions[0].blocks[0].nodes[0].operation {
            AbstractOperation::CallUnit {
                psi_operation,
                structural_arguments,
                ..
            } => (*psi_operation, structural_arguments.clone()),
            _ => panic!("fixture begins with a structural Unit call"),
        };
    boundary.functions[0].blocks[0].nodes[0].operation = AbstractOperation::BoundaryCall {
        psi_operation,
        result: None,
        boundary: boundary_id,
        arguments: Vec::new(),
        structural_arguments,
        completion_claim_sources: Vec::new(),
        completion_receipts: Vec::new(),
    };
    refresh_node_derivatives(&mut boundary, 0, 0, 0);
    validate_psi_optimization_unit(&boundary)
        .expect("matching boundary structural access should validate");

    boundary.boundary_machines[0].structural_parameters[0].access =
        psi_terminal::StructuralAccess::SharedBorrow;
    refresh_identity(&mut boundary);
    assert!(matches!(
        validate_psi_optimization_unit(&boundary),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));
}
