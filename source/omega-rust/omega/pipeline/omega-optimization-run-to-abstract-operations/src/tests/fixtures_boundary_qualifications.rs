//! Whole-root boundary qualification consumption fixture.

use super::*;

pub(super) fn boundary_qualification_verified() -> VerifiedPsiOptimizationUnit {
    let machine = MachineId::new(1_901).unwrap();
    let block = BlockId::new(1_902).unwrap();
    let boundary = psi_core::BoundaryMachineId::new(1_903).unwrap();
    let structural_type = psi_core::StructuralTypeId::new(1_904).unwrap();
    let required_domain = psi_core::StructuralDomainId::new(1_905).unwrap();
    let unrelated_domain = psi_core::StructuralDomainId::new(1_906).unwrap();
    let caller_place = psi_core::PlaceId::new(1_907).unwrap();
    let boundary_place = psi_core::PlaceId::new(1_908).unwrap();
    let operation = OperationId::new(1_909).unwrap();
    let parameter = |place, qualifications| psi_terminal::StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: psi_terminal::StructuralMultiplicity::Unrestricted,
        access: psi_terminal::StructuralAccess::SharedBorrow,
        qualifications,
        projected_qualifications: Vec::new(),
    };
    let mut module = module_with_blocks(
        machine,
        block,
        TerminalMachineResult::Unit,
        vec![Block {
            id: block,
            parameters: Vec::new(),
            operations: vec![Operation {
                id: operation,
                result: OperationResult::Unit,
                kind: OperationKind::BoundaryCall {
                    boundary,
                    arguments: Vec::new(),
                    structural_arguments: vec![psi_terminal::StructuralArgument {
                        place: caller_place,
                        path: Vec::new(),
                        access: psi_terminal::StructuralAccess::SharedBorrow,
                    }],
                    completion_receipts: Vec::new(),
                },
            }],
            terminator: Terminator::ReturnUnit {
                edge: EdgeId::new(1_910).unwrap(),
                trivial_affine_discards: Vec::new(),
            },
        }],
    );
    module.structural_types = vec![psi_terminal::StructuralTypeDeclaration {
        id: structural_type,
        identity: "test::QualifiedBoundaryCarrier".into(),
        shape: psi_terminal::StructuralTypeShape::Record { fields: Vec::new() },
    }];
    module.structural_domains = vec![
        psi_terminal::StructuralDomainDeclaration {
            id: required_domain,
            semantic_domain: psi_core::DomainSemanticId::new(1_911).unwrap(),
            identity: "test::RequiredBoundaryQualification".into(),
            carrier: structural_type,
            content_projection: None,
        },
        psi_terminal::StructuralDomainDeclaration {
            id: unrelated_domain,
            semantic_domain: psi_core::DomainSemanticId::new(1_912).unwrap(),
            identity: "test::UnrelatedQualification".into(),
            carrier: structural_type,
            content_projection: None,
        },
    ];
    module.boundary_machines = vec![psi_terminal::BoundaryMachineDeclaration {
        id: boundary,
        identity: "test::consume_qualification".into(),
        attachment: None,
        scalar_parameters: Vec::new(),
        structural_parameters: vec![parameter(boundary_place, Vec::new())],
        result: None,
        requires: vec![psi_terminal::StructuralDomainRequirement {
            argument_index: 0,
            domain: required_domain,
        }],
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
    }];
    let caller = &mut module.machines[0];
    caller.structural_parameters = vec![parameter(caller_place, vec![required_domain])];
    caller.structural_places = vec![psi_terminal::StructuralPlaceDeclaration {
        id: caller_place,
        kind: psi_core::StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }];

    verified(module, ProofBundle::default())
}

pub(super) fn partial_path_boundary_qualification_verified() -> VerifiedPsiOptimizationUnit {
    let machine = MachineId::new(1_921).unwrap();
    let block = BlockId::new(1_922).unwrap();
    let boundary = psi_core::BoundaryMachineId::new(1_923).unwrap();
    let leaf_type = psi_core::StructuralTypeId::new(1_924).unwrap();
    let root_type = psi_core::StructuralTypeId::new(1_925).unwrap();
    let required_domain = psi_core::StructuralDomainId::new(1_926).unwrap();
    let unrelated_domain = psi_core::StructuralDomainId::new(1_927).unwrap();
    let caller_place = psi_core::PlaceId::new(1_928).unwrap();
    let boundary_place = psi_core::PlaceId::new(1_929).unwrap();
    let operation = OperationId::new(1_930).unwrap();
    let left_path = vec![psi_terminal::StructuralPathSegment::Field("left".into())];
    let mut module = module_with_blocks(
        machine,
        block,
        TerminalMachineResult::Unit,
        vec![Block {
            id: block,
            parameters: Vec::new(),
            operations: vec![Operation {
                id: operation,
                result: OperationResult::Unit,
                kind: OperationKind::BoundaryCall {
                    boundary,
                    arguments: Vec::new(),
                    structural_arguments: vec![psi_terminal::StructuralArgument {
                        place: caller_place,
                        path: left_path.clone(),
                        access: psi_terminal::StructuralAccess::SharedBorrow,
                    }],
                    completion_receipts: Vec::new(),
                },
            }],
            terminator: Terminator::ReturnUnit {
                edge: EdgeId::new(1_931).unwrap(),
                trivial_affine_discards: vec![caller_place],
            },
        }],
    );
    module.structural_types = vec![
        psi_terminal::StructuralTypeDeclaration {
            id: leaf_type,
            identity: "test::QualifiedBoundaryLeaf".into(),
            shape: psi_terminal::StructuralTypeShape::Record { fields: Vec::new() },
        },
        psi_terminal::StructuralTypeDeclaration {
            id: root_type,
            identity: "test::QualifiedBoundaryRoot".into(),
            shape: psi_terminal::StructuralTypeShape::Record {
                fields: ["left", "right"]
                    .into_iter()
                    .enumerate()
                    .map(
                        |(index, identity)| psi_terminal::StructuralFieldDeclaration {
                            id: psi_core::StructuralFieldId::new(1_932 + index as u64).unwrap(),
                            identity: identity.into(),
                            relevance: psi_terminal::BindingRelevance::Relevant,
                            field_type: psi_terminal::StructuralFieldType::Structural(leaf_type),
                        },
                    )
                    .collect(),
            },
        },
    ];
    module.structural_domains = vec![
        psi_terminal::StructuralDomainDeclaration {
            id: required_domain,
            semantic_domain: psi_core::DomainSemanticId::new(1_926).unwrap(),
            identity: "test::RequiredProjectedBoundaryQualification".into(),
            carrier: leaf_type,
            content_projection: None,
        },
        psi_terminal::StructuralDomainDeclaration {
            id: unrelated_domain,
            semantic_domain: psi_core::DomainSemanticId::new(1_927).unwrap(),
            identity: "test::UnrelatedProjectedBoundaryQualification".into(),
            carrier: leaf_type,
            content_projection: None,
        },
    ];
    module.boundary_machines = vec![psi_terminal::BoundaryMachineDeclaration {
        id: boundary,
        identity: "test::consume_projected_qualification".into(),
        attachment: None,
        scalar_parameters: Vec::new(),
        structural_parameters: vec![psi_terminal::StructuralParameterDeclaration {
            place: boundary_place,
            position: 0,
            is_self: false,
            structural_type: leaf_type,
            multiplicity: psi_terminal::StructuralMultiplicity::Affine,
            access: psi_terminal::StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }],
        result: None,
        requires: vec![psi_terminal::StructuralDomainRequirement {
            argument_index: 0,
            domain: required_domain,
        }],
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
    }];
    let caller = &mut module.machines[0];
    caller.structural_parameters = vec![psi_terminal::StructuralParameterDeclaration {
        place: caller_place,
        position: 0,
        is_self: false,
        structural_type: root_type,
        multiplicity: psi_terminal::StructuralMultiplicity::Affine,
        access: psi_terminal::StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: vec![psi_terminal::StructuralPathQualification {
            path: left_path,
            domain: required_domain,
        }],
    }];
    caller.structural_places = vec![psi_terminal::StructuralPlaceDeclaration {
        id: caller_place,
        kind: psi_core::StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }];

    verified(module, ProofBundle::default())
}

pub(super) fn projected_structural_result_verified() -> VerifiedPsiOptimizationUnit {
    let caller = MachineId::new(1_940).unwrap();
    let callee = MachineId::new(1_941).unwrap();
    let caller_block = BlockId::new(1_942).unwrap();
    let callee_block = BlockId::new(1_943).unwrap();
    let root = psi_core::StructuralTypeId::new(1_944).unwrap();
    let leaf = psi_core::StructuralTypeId::new(1_945).unwrap();
    let domain = psi_core::StructuralDomainId::new(1_946).unwrap();
    let caller_input = psi_core::PlaceId::new(1_947).unwrap();
    let caller_result = psi_core::PlaceId::new(1_948).unwrap();
    let call_result = psi_core::PlaceId::new(1_949).unwrap();
    let callee_input = psi_core::PlaceId::new(1_950).unwrap();
    let callee_result = psi_core::PlaceId::new(1_951).unwrap();
    let call = OperationId::new(1_952).unwrap();
    let claim = psi_core::ClaimId::new(1).unwrap();
    let row = psi_terminal::StructuralPathQualification {
        path: vec![psi_terminal::StructuralPathSegment::Field("payload".into())],
        domain,
    };
    let parameter = |place| psi_terminal::StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type: root,
        multiplicity: psi_terminal::StructuralMultiplicity::Linear,
        access: psi_terminal::StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: vec![row.clone()],
    };
    let result = |place| psi_terminal::StructuralResultDeclaration {
        place,
        structural_type: root,
        multiplicity: psi_terminal::StructuralMultiplicity::Linear,
        qualifications: Vec::new(),
        projected_qualifications: vec![row.clone()],
    };
    let mut module = module_with_blocks(
        caller,
        caller_block,
        TerminalMachineResult::Structural(result(caller_result)),
        vec![Block {
            id: caller_block,
            parameters: Vec::new(),
            operations: vec![Operation {
                id: call,
                result: OperationResult::Structural(psi_terminal::StructuralOperationResult {
                    place: call_result,
                    structural_type: root,
                    multiplicity: psi_terminal::StructuralMultiplicity::Linear,
                    qualifications: Vec::new(),
                    projected_qualifications: vec![row.clone()],
                    claims: vec![psi_terminal::StructuralResultClaimBinding {
                        claim,
                        path: Vec::new(),
                    }],
                }),
                kind: OperationKind::CallStructural {
                    callee,
                    structural_arguments: vec![psi_terminal::StructuralArgument {
                        place: caller_input,
                        path: Vec::new(),
                        access: psi_terminal::StructuralAccess::Owned,
                    }],
                    claim_transfers: vec![psi_terminal::ClaimTransfer {
                        claim,
                        argument_index: 0,
                    }],
                    returned_claim_transfers: vec![psi_terminal::StructuralResultClaimTransfer {
                        callee_claim: claim,
                        caller_claim: claim,
                    }],
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                    selected_evidence: Vec::new(),
                },
            }],
            terminator: Terminator::ReturnStructural {
                edge: EdgeId::new(1_953).unwrap(),
                source: call_result,
                returned_claims: vec![claim],
                trivial_affine_discards: Vec::new(),
            },
        }],
    );
    module.structural_types = vec![
        psi_terminal::StructuralTypeDeclaration {
            id: root,
            identity: "test::ProjectedResultRoot".into(),
            shape: psi_terminal::StructuralTypeShape::Record {
                fields: vec![psi_terminal::StructuralFieldDeclaration {
                    id: psi_core::StructuralFieldId::new(1_954).unwrap(),
                    identity: "payload".into(),
                    relevance: psi_terminal::BindingRelevance::Relevant,
                    field_type: psi_terminal::StructuralFieldType::Structural(leaf),
                }],
            },
        },
        psi_terminal::StructuralTypeDeclaration {
            id: leaf,
            identity: "test::ProjectedResultLeaf".into(),
            shape: psi_terminal::StructuralTypeShape::Record {
                fields: vec![psi_terminal::StructuralFieldDeclaration {
                    id: psi_core::StructuralFieldId::new(1_956).unwrap(),
                    identity: "value".into(),
                    relevance: psi_terminal::BindingRelevance::Relevant,
                    field_type: psi_terminal::StructuralFieldType::Scalar(
                        psi_core::ScalarType::Integer(
                            psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 64)
                                .unwrap(),
                        ),
                    ),
                }],
            },
        },
    ];
    module.structural_domains = vec![psi_terminal::StructuralDomainDeclaration {
        id: domain,
        semantic_domain: psi_core::DomainSemanticId::new(1_946).unwrap(),
        identity: "test::ProjectedResultReady".into(),
        carrier: leaf,
        content_projection: None,
    }];
    let caller_machine = &mut module.machines[0];
    caller_machine.structural_parameters = vec![parameter(caller_input)];
    caller_machine.structural_places = vec![
        psi_terminal::StructuralPlaceDeclaration {
            id: caller_input,
            kind: psi_core::StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        psi_terminal::StructuralPlaceDeclaration {
            id: caller_result,
            kind: psi_core::StructuralPlaceKind::Result,
        },
        psi_terminal::StructuralPlaceDeclaration {
            id: call_result,
            kind: psi_core::StructuralPlaceKind::OperationResult {
                producer: call,
                structural_type: root,
            },
        },
    ];
    caller_machine.entry_claims = vec![psi_terminal::EntryClaim {
        claim,
        input: caller_input,
        path: Vec::new(),
    }];

    let mut callee_machine = caller_machine.clone();
    callee_machine.id = callee;
    callee_machine.structural_parameters = vec![parameter(callee_input)];
    callee_machine.result = TerminalMachineResult::Structural(result(callee_result));
    callee_machine.structural_places = vec![
        psi_terminal::StructuralPlaceDeclaration {
            id: callee_input,
            kind: psi_core::StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        psi_terminal::StructuralPlaceDeclaration {
            id: callee_result,
            kind: psi_core::StructuralPlaceKind::Result,
        },
    ];
    callee_machine.entry_claims = vec![psi_terminal::EntryClaim {
        claim,
        input: callee_input,
        path: Vec::new(),
    }];
    callee_machine.entry = callee_block;
    callee_machine.blocks = vec![Block {
        id: callee_block,
        parameters: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::ReturnStructural {
            edge: EdgeId::new(1_955).unwrap(),
            source: callee_input,
            returned_claims: vec![claim],
            trivial_affine_discards: Vec::new(),
        },
    }];
    callee_machine.contract.id = ContractId::new(1_941).unwrap();
    module.machines.push(callee_machine);

    verified(module, ProofBundle::default())
}
