//! Internal transfer and boundary completion contract corruption rejection.

use crate::tests::{
    affine_claim_transfer_unit, id, refresh_node_derivatives, structural_call_unit,
};
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use omega_abstract_operations::AbstractOperation;
use psi_core::{BoundaryMachineId, ClaimId};

#[test]
fn rejects_self_consistent_internal_claim_transfer_and_boundary_completion_corruption() {
    let internal = affine_claim_transfer_unit();
    let claim = id(1, ClaimId::new);
    validate_psi_optimization_unit(&internal)
        .expect("exact ordinary claim correspondence should validate");

    let mut missing_transfer = internal.clone();
    let AbstractOperation::CallUnit {
        claim_transfers, ..
    } = &mut missing_transfer.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with a structural Unit call")
    };
    claim_transfers.clear();
    refresh_node_derivatives(&mut missing_transfer, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&missing_transfer),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));

    let mut boundary = structural_call_unit();
    boundary.functions[0].structural_parameters[0].multiplicity =
        psi_terminal::StructuralMultiplicity::Affine;
    let entry = psi_terminal::EntryClaim {
        claim,
        input: boundary.functions[0].structural_parameters[0].place,
        path: Vec::new(),
    };
    boundary.functions[0]
        .entry_claim_declarations
        .push(entry.clone());
    boundary.functions[0].entry_claims.insert(claim);
    let boundary_id = id(345, BoundaryMachineId::new);
    let mut parameter = boundary.functions[1].structural_parameters[0].clone();
    parameter.multiplicity = psi_terminal::StructuralMultiplicity::Affine;
    boundary
        .boundary_machines
        .push(psi_terminal::BoundaryMachineDeclaration {
            id: boundary_id,
            identity: "validation::claim-completing-boundary".into(),
            attachment: None,
            scalar_parameters: Vec::new(),
            structural_parameters: vec![parameter],
            result: psi_terminal::BoundaryMachineResult::Unit,
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
        completion_claim_sources: vec![omega_abstract_operations::CompletionClaimSource {
            claim,
            entry: Some(entry),
            content: None,
        }],
        completion_receipts: vec![psi_terminal::CompletionReceipt {
            claim,
            argument_index: 0,
        }],
    };
    refresh_node_derivatives(&mut boundary, 0, 0, 0);
    validate_psi_optimization_unit(&boundary)
        .expect("exact boundary completion evidence should validate");

    let AbstractOperation::BoundaryCall {
        completion_claim_sources,
        completion_receipts,
        ..
    } = &mut boundary.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture now contains a boundary call")
    };
    completion_claim_sources.clear();
    completion_receipts.clear();
    refresh_node_derivatives(&mut boundary, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&boundary),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));
}
