//! Content-only boundary completion admission and correspondence corruption rejection.

use crate::tests::{
    content_entry_claim, id, install_content_owner, refresh_node_derivatives, structural_call_unit,
};
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use omega_abstract_operations::AbstractOperation;
use psi_core::{BoundaryMachineId, ClaimId};

#[test]
fn accepts_content_only_boundary_completion_and_rejects_correspondence_corruption() {
    let mut baseline = structural_call_unit();
    install_content_owner(&mut baseline);
    let claim = id(1, ClaimId::new);
    let caller_root = baseline.functions[0].structural_parameters[0].place;
    let content = content_entry_claim(claim, caller_root);
    baseline.functions[0]
        .content_entry_claims
        .push(content.clone());
    let boundary_id = id(346, BoundaryMachineId::new);
    baseline
        .boundary_machines
        .push(psi_terminal::BoundaryMachineDeclaration {
            id: boundary_id,
            identity: "validation::content-only-boundary".into(),
            attachment: None,
            scalar_parameters: Vec::new(),
            structural_parameters: vec![baseline.functions[1].structural_parameters[0].clone()],
            result: None,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        });
    let (psi_operation, structural_arguments) =
        match &baseline.functions[0].blocks[0].nodes[0].operation {
            AbstractOperation::CallUnit {
                psi_operation,
                structural_arguments,
                ..
            } => (*psi_operation, structural_arguments.clone()),
            _ => panic!("fixture begins with a structural Unit call"),
        };
    baseline.functions[0].blocks[0].nodes[0].operation = AbstractOperation::BoundaryCall {
        psi_operation,
        result: None,
        boundary: boundary_id,
        arguments: Vec::new(),
        structural_arguments,
        completion_claim_sources: vec![omega_abstract_operations::CompletionClaimSource {
            claim,
            entry: None,
            content: Some(content),
        }],
        completion_receipts: vec![psi_terminal::CompletionReceipt {
            claim,
            argument_index: 0,
        }],
    };
    refresh_node_derivatives(&mut baseline, 0, 0, 0);
    validate_psi_optimization_unit(&baseline)
        .expect("content-only claims participate in the live completion namespace");

    let mut narrowed = baseline.clone();
    let AbstractOperation::BoundaryCall {
        completion_claim_sources,
        completion_receipts,
        ..
    } = &mut narrowed.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture contains a boundary call")
    };
    completion_claim_sources.clear();
    completion_receipts.clear();
    refresh_node_derivatives(&mut narrowed, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&narrowed),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));

    let mut wrong_claim = baseline;
    let AbstractOperation::BoundaryCall {
        completion_receipts,
        ..
    } = &mut wrong_claim.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture contains a boundary call")
    };
    completion_receipts[0].claim = id(2, ClaimId::new);
    refresh_node_derivatives(&mut wrong_claim, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&wrong_claim),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));
}
