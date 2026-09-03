//! Installed-provider structural call with affine claim-completion custody.

use super::structural_call::{qualified_fixture_unit, structural_call_fixture};
use omega_abstract_operations::{AbstractOperation, AbstractOperationPlan, CompletionClaimSource};
use omega_optimization_unit::PsiOptimizationUnit;
use omega_target_operations::TargetOperationPlan;
use psi_core::{BoundaryMachineId, ClaimId, FuelScheduleIdentity, OperationId, PlaceId};
use psi_terminal::{
    BoundaryMachineDeclaration, CompletionReceipt, EntryClaim, ProviderCandidateConformance,
    ProviderParameterRefinement, ProviderSignatureParameter, ProviderUnitRefinement,
    ProviderUnitSignature, StructuralArgument, StructuralMultiplicity,
    StructuralParameterDeclaration,
};

pub(in crate::tests) fn installed_provider_legalization_fixture() -> (
    AbstractOperationPlan,
    TargetOperationPlan,
    PsiOptimizationUnit,
) {
    let (mut abstract_plan, mut target, _) = structural_call_fixture();
    let boundary = BoundaryMachineId::new(1).unwrap();
    let callee = abstract_plan.functions[1].machine;
    let operation = OperationId::new(1).unwrap();
    let caller_parameters = abstract_plan.functions[0].structural_parameters.clone();
    let structural_type = caller_parameters[0].structural_type;
    for function in &mut abstract_plan.functions {
        for parameter in &mut function.structural_parameters {
            parameter.multiplicity = StructuralMultiplicity::Affine;
        }
    }
    let caller_claims = caller_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| EntryClaim {
            claim: ClaimId::new(index as u64 + 1).unwrap(),
            input: parameter.place,
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let callee_claims = abstract_plan.functions[1]
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| EntryClaim {
            claim: ClaimId::new(index as u64 + 1).unwrap(),
            input: parameter.place,
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let arguments = caller_parameters
        .iter()
        .map(|parameter| StructuralArgument {
            place: parameter.place,
            access: parameter.access,
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let completion_sources = caller_claims
        .iter()
        .cloned()
        .map(|entry| CompletionClaimSource {
            claim: entry.claim,
            entry: Some(entry),
            content: None,
        })
        .collect::<Vec<_>>();
    let receipts = caller_claims
        .iter()
        .enumerate()
        .map(|(index, claim)| CompletionReceipt {
            claim: claim.claim,
            argument_index: index as u32,
        })
        .collect::<Vec<_>>();
    let provider = ProviderCandidateConformance {
        boundary,
        requirement_identity: "ProgramEntry::enter".into(),
        provider_identity: "UefiProgramProvider".into(),
        candidate_identity: "UefiProgramProvider::enter".into(),
        candidate: callee,
        signature: ProviderUnitSignature {
            parameters: caller_parameters
                .iter()
                .map(|parameter| ProviderSignatureParameter {
                    position: parameter.position,
                    is_self: parameter.is_self,
                    structural_type: parameter.structural_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    access: parameter.access,
                    qualifications: parameter.qualifications.clone(),
                    projected_qualifications: parameter.projected_qualifications.clone(),
                })
                .collect(),
        },
        refinement: ProviderUnitRefinement {
            positional_parameters: vec![
                ProviderParameterRefinement {
                    boundary_index: 0,
                    candidate_index: 0,
                },
                ProviderParameterRefinement {
                    boundary_index: 1,
                    candidate_index: 1,
                },
            ],
            required_domains: Vec::new(),
            realized_service_ceiling: Vec::new(),
        },
    };
    abstract_plan.boundary_machines = vec![BoundaryMachineDeclaration {
        id: boundary,
        identity: "ProgramEntry::enter".into(),
        attachment: None,
        scalar_parameters: Vec::new(),
        structural_parameters: caller_parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| StructuralParameterDeclaration {
                place: PlaceId::new(index as u64 + 5).unwrap(),
                position: parameter.position,
                is_self: parameter.is_self,
                structural_type,
                multiplicity: StructuralMultiplicity::Affine,
                access: parameter.access,
                qualifications: parameter.qualifications.clone(),
                projected_qualifications: parameter.projected_qualifications.clone(),
            })
            .collect(),
        result: psi_terminal::BoundaryMachineResult::Unit,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
    }];
    abstract_plan.provider_candidates = vec![provider.clone()];
    abstract_plan.functions[0].entry_claims = caller_claims.clone();
    abstract_plan.functions[0].operations[0] = AbstractOperation::BoundaryCall {
        psi_operation: operation,
        result: None,
        boundary,
        arguments: Vec::new(),
        structural_arguments: arguments.clone(),
        completion_claim_sources: completion_sources.clone(),
        completion_receipts: receipts.clone(),
    };
    abstract_plan.functions[1].entry_claims = callee_claims;
    for function in &mut target.functions {
        let omega_target_operations::TargetOperation::UnitBody(body) = &mut function.operation
        else {
            continue;
        };
        for parameter in &mut body.parameters {
            parameter.multiplicity = StructuralMultiplicity::Affine;
        }
    }
    let omega_target_operations::TargetOperation::UnitBody(provider_body) =
        &target.functions[1].operation
    else {
        panic!("provider Unit body");
    };
    let provider_call_plan = provider_body.call_plan.clone();
    let omega_target_operations::TargetOperation::UnitBody(caller_body) =
        &mut target.functions[0].operation
    else {
        panic!("caller Unit body");
    };
    let omega_target_operations::TargetUnitOperation::Call {
        arguments: target_arguments,
        ..
    } = caller_body.operations[0].clone()
    else {
        panic!("authored structural call fixture");
    };
    caller_body.operations[0] =
        omega_target_operations::TargetUnitOperation::InstalledProviderCall {
            psi_operation: operation,
            boundary,
            provider,
            call_plan: provider_call_plan,
            scalar_arguments: Vec::new(),
            source_arguments: arguments,
            arguments: target_arguments,
            claim_transfers: receipts
                .iter()
                .map(|receipt| psi_terminal::ClaimTransfer {
                    claim: receipt.claim,
                    argument_index: receipt.argument_index,
                })
                .collect(),
            completion_claim_sources: completion_sources,
            completion_receipts: receipts,
        };
    let unit = qualified_fixture_unit(
        omega_optimization_unit::reconstruct_psi_optimization_unit_seed(
            &abstract_plan,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .expect("installed provider optimization seed"),
        structural_type,
    );
    (abstract_plan, target, unit)
}
