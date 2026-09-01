//! Serialized Terminal fixture for public optimized target-roster custody.

use crate::tests::*;

pub(crate) fn projected_structural_call_return_artifact() -> (Vec<u8>, Vec<u8>) {
    let (semantic, proof) = unit_return_artifact();
    let mut module = psi_terminal_codec::decode_module(&semantic).unwrap();
    let callee = MachineId::new(3_801).unwrap();
    let root = StructuralTypeId::new(3_802).unwrap();
    let leaf = StructuralTypeId::new(3_803).unwrap();
    let domain = StructuralDomainId::new(3_804).unwrap();
    let caller_input = PlaceId::new(3_805).unwrap();
    let caller_result = PlaceId::new(3_806).unwrap();
    let call_result = PlaceId::new(3_807).unwrap();
    let callee_input = PlaceId::new(3_808).unwrap();
    let callee_result = PlaceId::new(3_809).unwrap();
    let call = OperationId::new(3_810).unwrap();
    let caller_claim = psi_core::ClaimId::new(1).unwrap();
    let callee_claim = psi_core::ClaimId::new(1).unwrap();
    let row = psi_terminal::StructuralPathQualification {
        path: vec![psi_terminal::StructuralPathSegment::Field("payload".into())],
        domain,
    };
    let parameter = |place| StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type: root,
        multiplicity: StructuralMultiplicity::Linear,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: vec![row.clone()],
    };
    let result = |place| psi_terminal::StructuralResultDeclaration {
        place,
        structural_type: root,
        multiplicity: StructuralMultiplicity::Linear,
        qualifications: Vec::new(),
        projected_qualifications: vec![row.clone()],
    };
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: root,
            identity: "ProjectedRoot".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: StructuralFieldId::new(3_813).unwrap(),
                    identity: "payload".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(leaf),
                }],
            },
        },
        StructuralTypeDeclaration {
            id: leaf,
            identity: "ProjectedLeaf".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: StructuralFieldId::new(3_814).unwrap(),
                    identity: "value".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    )),
                }],
            },
        },
    ];
    module.structural_domains = vec![StructuralDomainDeclaration {
        id: domain,
        semantic_domain: DomainSemanticId::new(3_804).unwrap(),
        identity: "ProjectedReady".into(),
        carrier: leaf,
        content_projection: None,
    }];
    let caller_machine = &mut module.machines[0];
    caller_machine.structural_parameters = vec![parameter(caller_input)];
    caller_machine.result = TerminalMachineResult::Structural(result(caller_result));
    caller_machine.structural_places = vec![
        place(
            caller_input,
            StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        ),
        place(caller_result, StructuralPlaceKind::Result),
        place(
            call_result,
            StructuralPlaceKind::OperationResult {
                producer: call,
                structural_type: root,
            },
        ),
    ];
    caller_machine.entry_claims = vec![psi_terminal::EntryClaim {
        claim: caller_claim,
        input: caller_input,
        path: Vec::new(),
    }];
    caller_machine.blocks[0].operations = vec![Operation {
        id: call,
        result: OperationResult::Structural(psi_terminal::StructuralOperationResult {
            place: call_result,
            structural_type: root,
            multiplicity: StructuralMultiplicity::Linear,
            qualifications: Vec::new(),
            projected_qualifications: vec![row.clone()],
            claims: vec![psi_terminal::StructuralResultClaimBinding {
                claim: caller_claim,
                path: Vec::new(),
            }],
        }),
        kind: OperationKind::CallStructural {
            callee,
            structural_arguments: vec![psi_terminal::StructuralArgument {
                place: caller_input,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            }],
            claim_transfers: vec![psi_terminal::ClaimTransfer {
                claim: caller_claim,
                argument_index: 0,
            }],
            returned_claim_transfers: vec![psi_terminal::StructuralResultClaimTransfer {
                callee_claim,
                caller_claim,
            }],
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
            selected_evidence: Vec::new(),
        },
    }];
    caller_machine.blocks[0].terminator = Terminator::ReturnStructural {
        edge: EdgeId::new(3_815).unwrap(),
        source: call_result,
        returned_claims: vec![caller_claim],
        trivial_affine_discards: Vec::new(),
    };

    let mut callee_machine = caller_machine.clone();
    callee_machine.id = callee;
    callee_machine.structural_parameters = vec![parameter(callee_input)];
    callee_machine.result = TerminalMachineResult::Structural(result(callee_result));
    callee_machine.structural_places = vec![
        place(
            callee_input,
            StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        ),
        place(callee_result, StructuralPlaceKind::Result),
    ];
    callee_machine.entry_claims = vec![psi_terminal::EntryClaim {
        claim: callee_claim,
        input: callee_input,
        path: Vec::new(),
    }];
    callee_machine.entry = BlockId::new(3_816).unwrap();
    callee_machine.blocks = vec![Block {
        id: callee_machine.entry,
        parameters: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::ReturnStructural {
            edge: EdgeId::new(3_817).unwrap(),
            source: callee_input,
            returned_claims: vec![callee_claim],
            trivial_affine_discards: Vec::new(),
        },
    }];
    callee_machine.contract.id = ContractId::new(3_818).unwrap();
    module.machines.push(callee_machine);
    (psi_terminal_codec::encode_module(&module).unwrap(), proof)
}

fn place(id: PlaceId, kind: StructuralPlaceKind) -> StructuralPlaceDeclaration {
    StructuralPlaceDeclaration { id, kind }
}
