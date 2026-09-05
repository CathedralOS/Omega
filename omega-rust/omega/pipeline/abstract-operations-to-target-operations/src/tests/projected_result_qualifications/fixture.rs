//! Shared exact two-function projected structural call/return fixture.

use super::*;

pub(super) fn projected_structural_call_return_plan() -> AbstractOperationPlan {
    let caller = MachineId::new(900).unwrap();
    let callee = MachineId::new(901).unwrap();
    let root = StructuralTypeId::new(900).unwrap();
    let left = StructuralTypeId::new(901).unwrap();
    let caller_place = PlaceId::new(900).unwrap();
    let call_place = PlaceId::new(901).unwrap();
    let caller_result_place = PlaceId::new(902).unwrap();
    let callee_place = PlaceId::new(903).unwrap();
    let callee_result_place = PlaceId::new(904).unwrap();
    let caller_claim = semantic_vocabulary::ClaimId::new(900).unwrap();
    let callee_claim = semantic_vocabulary::ClaimId::new(901).unwrap();
    let call = OperationId::new(900).unwrap();
    let rows = vec![
        terminal_psi::StructuralPathQualification {
            path: vec![StructuralPathSegment::Field("left".into())],
            domain: semantic_vocabulary::StructuralDomainId::new(900).unwrap(),
        },
        terminal_psi::StructuralPathQualification {
            path: vec![StructuralPathSegment::Field("left".into())],
            domain: semantic_vocabulary::StructuralDomainId::new(901).unwrap(),
        },
    ];
    let parameter = |place| StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type: root,
        multiplicity: StructuralMultiplicity::Linear,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: rows.clone(),
    };
    let result = |place| terminal_psi::StructuralResultDeclaration {
        place,
        structural_type: root,
        multiplicity: StructuralMultiplicity::Linear,
        qualifications: Vec::new(),
        projected_qualifications: rows.clone(),
    };
    let leaf = |id, identity| StructuralTypeDeclaration {
        id,
        identity,
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: StructuralFieldId::new(id.get()).unwrap(),
                identity: "value".into(),
                relevance: terminal_psi::BindingRelevance::Relevant,
                field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                )),
            }],
        },
    };
    let block_entry = |block| AbstractBlockEntry {
        block,
        parameters: Vec::new(),
        operation_offset: 0,
    };
    AbstractOperationPlan {
        psi: identity(),
        entry: caller,
        structural_types: vec![
            StructuralTypeDeclaration {
                id: root,
                identity: "ProjectedPair".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![StructuralFieldDeclaration {
                        id: StructuralFieldId::new(910).unwrap(),
                        identity: "left".into(),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(left),
                    }],
                },
            },
            leaf(left, "ProjectedLeft".into()),
        ],
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![
            caller_function(
                caller,
                callee,
                caller_place,
                call_place,
                caller_claim,
                callee_claim,
                call,
                parameter(caller_place),
                result(caller_result_place),
                rows.clone(),
                block_entry(BlockId::new(900).unwrap()),
            ),
            callee_function(
                callee,
                callee_place,
                callee_claim,
                parameter(callee_place),
                result(callee_result_place),
                block_entry(BlockId::new(901).unwrap()),
            ),
        ],
    }
}

#[allow(clippy::too_many_arguments)]
fn caller_function(
    caller: MachineId,
    callee: MachineId,
    caller_place: PlaceId,
    call_place: PlaceId,
    caller_claim: semantic_vocabulary::ClaimId,
    callee_claim: semantic_vocabulary::ClaimId,
    call: OperationId,
    parameter: StructuralParameterDeclaration,
    result: terminal_psi::StructuralResultDeclaration,
    rows: Vec<terminal_psi::StructuralPathQualification>,
    block_entry: AbstractBlockEntry,
) -> AbstractFunction {
    AbstractFunction {
        machine: caller,
        attachment: None,
        entry: block_entry.block,
        parameters: Vec::new(),
        structural_parameters: vec![parameter],
        result: AbstractFunctionResult::Structural(result),
        entry_claims: vec![terminal_psi::EntryClaim {
            claim: caller_claim,
            input: caller_place,
            path: Vec::new(),
        }],
        published_service_ceiling: Vec::new(),
        block_entries: vec![block_entry],
        operations: vec![
            AbstractOperation::CallStructural {
                psi_operation: call,
                result: terminal_psi::StructuralOperationResult {
                    place: call_place,
                    structural_type: StructuralTypeId::new(900).unwrap(),
                    multiplicity: StructuralMultiplicity::Linear,
                    qualifications: Vec::new(),
                    projected_qualifications: rows,
                    claims: vec![terminal_psi::StructuralResultClaimBinding {
                        claim: caller_claim,
                        path: Vec::new(),
                    }],
                },
                callee,
                arguments: Vec::new(),
                structural_arguments: vec![StructuralArgument {
                    place: caller_place,
                    access: StructuralAccess::Owned,
                    path: Vec::new(),
                }],
                claim_transfers: vec![terminal_psi::ClaimTransfer {
                    claim: caller_claim,
                    argument_index: 0,
                }],
                returned_claim_transfers: vec![terminal_psi::StructuralResultClaimTransfer {
                    callee_claim,
                    caller_claim,
                }],
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
                selected_evidence: Vec::new(),
            },
            AbstractOperation::ReturnStructural {
                psi_edge: EdgeId::new(900).unwrap(),
                source: call_place,
                returned_claims: vec![caller_claim],
                trivial_affine_locals: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
        ],
    }
}

fn callee_function(
    callee: MachineId,
    place: PlaceId,
    claim: semantic_vocabulary::ClaimId,
    parameter: StructuralParameterDeclaration,
    result: terminal_psi::StructuralResultDeclaration,
    block_entry: AbstractBlockEntry,
) -> AbstractFunction {
    AbstractFunction {
        machine: callee,
        attachment: None,
        entry: block_entry.block,
        parameters: Vec::new(),
        structural_parameters: vec![parameter],
        result: AbstractFunctionResult::Structural(result),
        entry_claims: vec![terminal_psi::EntryClaim {
            claim,
            input: place,
            path: Vec::new(),
        }],
        published_service_ceiling: Vec::new(),
        block_entries: vec![block_entry],
        operations: vec![AbstractOperation::ReturnStructural {
            psi_edge: EdgeId::new(901).unwrap(),
            source: place,
            returned_claims: vec![claim],
            trivial_affine_locals: Vec::new(),
            trivial_affine_discards: Vec::new(),
        }],
    }
}
