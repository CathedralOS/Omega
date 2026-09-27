use super::builders::provider_module;
use super::ids::{boundary_id, claim_id, operation_id, place_id, structural_type_id};
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use terminal_psi::{
    BoundaryMachineDeclaration, CompletionReceipt, EntryClaim, Operation, OperationKind,
    OperationResult, ProviderParameterRefinement, ProviderSignatureParameter, StructuralAccess,
    StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape, TerminalModule,
};

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
        projected_qualifications: Vec::new(),
    };
    module.boundary_machines[0].parameter_order =
        vec![terminal_psi::BoundaryParameterKind::Structural];
    module.boundary_machines[0].structural_parameters =
        vec![structural_parameter(place_id(9), resource)];
    module.boundary_machines.push(BoundaryMachineDeclaration {
        scalar_requires: Vec::new(),
        fixed_service_reach: Vec::new(),
        id: boundary_id(2),
        identity: "Resource::settle".into(),
        attachment: None,
        parameter_order: vec![terminal_psi::BoundaryParameterKind::Structural],
        scalar_parameters: Vec::new(),
        structural_parameters: vec![structural_parameter(place_id(10), resource)],
        result: terminal_psi::BoundaryMachineResult::Unit,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
        crash_routes: Vec::new(),
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
            kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
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
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation_id(index as u64 + 1),
            result: OperationResult::Unit,
            kind: OperationKind::BoundaryCall {
                requirement_obligations: Vec::new(),
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
        projected_qualifications: Vec::new(),
    }
}
