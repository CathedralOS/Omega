use super::builders::{artifact, provider_module, selected};
use super::ids::{boundary_id, claim_id, operation_id, place_id, structural_type_id};
use omega_psi_to_abstract_operations::{
    ProviderInstallationError, admit_provider_installation, lower_artifact_sections,
};
use psi_core::{PlaceId, StructuralTypeId};
use psi_proof_admission::AdmissionProfile;
use psi_terminal::{
    BoundaryMachineDeclaration, CompletionReceipt, EntryClaim, Operation, OperationKind,
    OperationResult, ProviderParameterRefinement, ProviderSignatureParameter, StructuralAccess,
    StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape, TerminalModule,
};

#[test]
fn omega_retains_and_replays_the_whole_root_structural_provider_call() {
    let module = structural_provider_module();
    let (semantic, proof) = artifact(&module);
    let profile = AdmissionProfile::default();
    let plan = lower_artifact_sections(&semantic, &proof, &profile).expect("verified lowering");
    let selected = selected("second-plan", "SecondProvider", "SecondProvider::emit");
    let installation = admit_provider_installation(&plan, &semantic, &proof, &profile, &selected)
        .expect("whole-root structural provider installation");
    let [call] = installation.installed_unit_calls() else {
        panic!("one installed structural provider call")
    };
    assert_eq!(call.provider(), &plan.provider_candidates[1]);
    assert!(call.structural_arguments()[0].path.is_empty());
    assert_eq!(
        call.structural_arguments()[0].access,
        StructuralAccess::Owned
    );
    assert_eq!(
        call.completion_receipts(),
        &[CompletionReceipt {
            claim: claim_id(1),
            argument_index: 0,
        }]
    );
    assert_eq!(call.completion_claim_sources()[0].claim, claim_id(1));

    let mut candidate_tamper = plan.clone();
    candidate_tamper.provider_candidates[1].candidate_identity = "SecondProvider::other".into();
    assert!(matches!(
        admit_provider_installation(&candidate_tamper, &semantic, &proof, &profile, &selected,),
        Err(ProviderInstallationError::PlanReplayMismatch)
    ));

    let mut access_tamper = plan.clone();
    let omega_abstract_operations::AbstractOperation::BoundaryCall {
        structural_arguments,
        ..
    } = &mut access_tamper.functions[0].operations[0]
    else {
        panic!("fixture starts with its provider boundary call")
    };
    structural_arguments[0].access = StructuralAccess::SharedBorrow;
    assert!(matches!(
        admit_provider_installation(&access_tamper, &semantic, &proof, &profile, &selected),
        Err(ProviderInstallationError::PlanReplayMismatch)
    ));
}

pub(super) fn structural_provider_module() -> TerminalModule {
    let mut module = provider_module();
    let resource = structural_type_id(3);
    module.structural_types.push(StructuralTypeDeclaration {
        id: resource,
        identity: "Resource".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let signature_parameter = ProviderSignatureParameter {
        position: 0,
        is_self: false,
        structural_type: resource,
        multiplicity: StructuralMultiplicity::Linear,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
    };
    module.boundary_machines[0].structural_parameters =
        vec![structural_parameter(place_id(9), resource)];
    module.boundary_machines.push(BoundaryMachineDeclaration {
        id: boundary_id(2),
        identity: "Resource::settle".into(),
        attachment: None,
        scalar_parameters: Vec::new(),
        structural_parameters: vec![structural_parameter(place_id(10), resource)],
        result: None,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
    });
    for row in &mut module.provider_candidates {
        row.signature.parameters = vec![signature_parameter.clone()];
        row.refinement.positional_parameters = vec![ProviderParameterRefinement {
            boundary_index: 0,
            candidate_index: 0,
        }];
    }
    for (index, machine) in module.machines.iter_mut().enumerate() {
        let place = place_id(index as u64 + 1);
        machine.structural_parameters = vec![structural_parameter(place, resource)];
        machine.structural_places = vec![StructuralPlaceDeclaration {
            id: place,
            kind: psi_core::StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        }];
        machine.entry_claims = vec![EntryClaim {
            claim: claim_id(1),
            input: place,
            path: Vec::new(),
        }];
        let boundary = if index == 0 {
            boundary_id(1)
        } else {
            boundary_id(2)
        };
        machine.blocks[0].operations = vec![Operation {
            id: operation_id(index as u64 + 1),
            result: OperationResult::Unit,
            kind: OperationKind::BoundaryCall {
                boundary,
                arguments: Vec::new(),
                structural_arguments: vec![StructuralArgument {
                    place,
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                }],
                completion_receipts: vec![CompletionReceipt {
                    claim: claim_id(1),
                    argument_index: 0,
                }],
                requirement_obligations: Vec::new(),
            },
        }];
    }
    module
}

fn structural_parameter(
    place: PlaceId,
    structural_type: StructuralTypeId,
) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Linear,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
    }
}
