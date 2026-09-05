//! Whole-root boundary qualification consumption fixture.

use super::*;

pub(super) fn boundary_qualification_verified() -> VerifiedPsiOptimizationUnit {
    let machine = MachineId::new(1_901).unwrap();
    let block = BlockId::new(1_902).unwrap();
    let boundary = semantic_vocabulary::BoundaryMachineId::new(1_903).unwrap();
    let structural_type = semantic_vocabulary::StructuralTypeId::new(1_904).unwrap();
    let required_domain = semantic_vocabulary::StructuralDomainId::new(1_905).unwrap();
    let unrelated_domain = semantic_vocabulary::StructuralDomainId::new(1_906).unwrap();
    let caller_place = semantic_vocabulary::PlaceId::new(1_907).unwrap();
    let boundary_place = semantic_vocabulary::PlaceId::new(1_908).unwrap();
    let operation = OperationId::new(1_909).unwrap();
    let parameter = |place, qualifications| terminal_psi::StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        access: terminal_psi::StructuralAccess::SharedBorrow,
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
                    structural_arguments: vec![terminal_psi::StructuralArgument {
                        place: caller_place,
                        path: Vec::new(),
                        access: terminal_psi::StructuralAccess::SharedBorrow,
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
    module.structural_types = vec![terminal_psi::StructuralTypeDeclaration {
        id: structural_type,
        identity: "test::QualifiedBoundaryCarrier".into(),
        shape: terminal_psi::StructuralTypeShape::Record { fields: Vec::new() },
    }];
    module.structural_domains = vec![
        terminal_psi::StructuralDomainDeclaration {
            id: required_domain,
            semantic_domain: semantic_vocabulary::DomainSemanticId::new(1_911).unwrap(),
            identity: "test::RequiredBoundaryQualification".into(),
            carrier: structural_type,
            content_projection: None,
        },
        terminal_psi::StructuralDomainDeclaration {
            id: unrelated_domain,
            semantic_domain: semantic_vocabulary::DomainSemanticId::new(1_912).unwrap(),
            identity: "test::UnrelatedQualification".into(),
            carrier: structural_type,
            content_projection: None,
        },
    ];
    module.boundary_machines = vec![terminal_psi::BoundaryMachineDeclaration {
        id: boundary,
        identity: "test::consume_qualification".into(),
        attachment: None,
        scalar_parameters: Vec::new(),
        structural_parameters: vec![parameter(boundary_place, Vec::new())],
        result: terminal_psi::BoundaryMachineResult::Unit,
        requires: vec![terminal_psi::StructuralDomainRequirement {
            argument_index: 0,
            domain: required_domain,
        }],
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
    }];
    let caller = &mut module.machines[0];
    caller.structural_parameters = vec![parameter(caller_place, vec![required_domain])];
    caller.structural_places = vec![terminal_psi::StructuralPlaceDeclaration {
        id: caller_place,
        kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }];

    verified(module, ProofBundle::default())
}

pub(super) fn partial_path_boundary_qualification_verified() -> VerifiedPsiOptimizationUnit {
    let machine = MachineId::new(1_921).unwrap();
    let block = BlockId::new(1_922).unwrap();
    let boundary = semantic_vocabulary::BoundaryMachineId::new(1_923).unwrap();
    let leaf_type = semantic_vocabulary::StructuralTypeId::new(1_924).unwrap();
    let root_type = semantic_vocabulary::StructuralTypeId::new(1_925).unwrap();
    let required_domain = semantic_vocabulary::StructuralDomainId::new(1_926).unwrap();
    let unrelated_domain = semantic_vocabulary::StructuralDomainId::new(1_927).unwrap();
    let caller_place = semantic_vocabulary::PlaceId::new(1_928).unwrap();
    let boundary_place = semantic_vocabulary::PlaceId::new(1_929).unwrap();
    let operation = OperationId::new(1_930).unwrap();
    let left_path = vec![terminal_psi::StructuralPathSegment::Field("left".into())];
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
                    structural_arguments: vec![terminal_psi::StructuralArgument {
                        place: caller_place,
                        path: left_path.clone(),
                        access: terminal_psi::StructuralAccess::SharedBorrow,
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
        terminal_psi::StructuralTypeDeclaration {
            id: leaf_type,
            identity: "test::QualifiedBoundaryLeaf".into(),
            shape: terminal_psi::StructuralTypeShape::Record { fields: Vec::new() },
        },
        terminal_psi::StructuralTypeDeclaration {
            id: root_type,
            identity: "test::QualifiedBoundaryRoot".into(),
            shape: terminal_psi::StructuralTypeShape::Record {
                fields: ["left", "right"]
                    .into_iter()
                    .enumerate()
                    .map(
                        |(index, identity)| terminal_psi::StructuralFieldDeclaration {
                            id: semantic_vocabulary::StructuralFieldId::new(1_932 + index as u64)
                                .unwrap(),
                            identity: identity.into(),
                            relevance: terminal_psi::BindingRelevance::Relevant,
                            field_type: terminal_psi::StructuralFieldType::Structural(leaf_type),
                        },
                    )
                    .collect(),
            },
        },
    ];
    module.structural_domains = vec![
        terminal_psi::StructuralDomainDeclaration {
            id: required_domain,
            semantic_domain: semantic_vocabulary::DomainSemanticId::new(1_926).unwrap(),
            identity: "test::RequiredProjectedBoundaryQualification".into(),
            carrier: leaf_type,
            content_projection: None,
        },
        terminal_psi::StructuralDomainDeclaration {
            id: unrelated_domain,
            semantic_domain: semantic_vocabulary::DomainSemanticId::new(1_927).unwrap(),
            identity: "test::UnrelatedProjectedBoundaryQualification".into(),
            carrier: leaf_type,
            content_projection: None,
        },
    ];
    module.boundary_machines = vec![terminal_psi::BoundaryMachineDeclaration {
        id: boundary,
        identity: "test::consume_projected_qualification".into(),
        attachment: None,
        scalar_parameters: Vec::new(),
        structural_parameters: vec![terminal_psi::StructuralParameterDeclaration {
            place: boundary_place,
            position: 0,
            is_self: false,
            structural_type: leaf_type,
            multiplicity: terminal_psi::StructuralMultiplicity::Affine,
            access: terminal_psi::StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }],
        result: terminal_psi::BoundaryMachineResult::Unit,
        requires: vec![terminal_psi::StructuralDomainRequirement {
            argument_index: 0,
            domain: required_domain,
        }],
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
    }];
    let caller = &mut module.machines[0];
    caller.structural_parameters = vec![terminal_psi::StructuralParameterDeclaration {
        place: caller_place,
        position: 0,
        is_self: false,
        structural_type: root_type,
        multiplicity: terminal_psi::StructuralMultiplicity::Affine,
        access: terminal_psi::StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: vec![terminal_psi::StructuralPathQualification {
            path: left_path,
            domain: required_domain,
        }],
    }];
    caller.structural_places = vec![terminal_psi::StructuralPlaceDeclaration {
        id: caller_place,
        kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
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
    let root = semantic_vocabulary::StructuralTypeId::new(1_944).unwrap();
    let leaf = semantic_vocabulary::StructuralTypeId::new(1_945).unwrap();
    let domain = semantic_vocabulary::StructuralDomainId::new(1_946).unwrap();
    let caller_input = semantic_vocabulary::PlaceId::new(1_947).unwrap();
    let caller_result = semantic_vocabulary::PlaceId::new(1_948).unwrap();
    let call_result = semantic_vocabulary::PlaceId::new(1_949).unwrap();
    let callee_input = semantic_vocabulary::PlaceId::new(1_950).unwrap();
    let callee_result = semantic_vocabulary::PlaceId::new(1_951).unwrap();
    let call = OperationId::new(1_952).unwrap();
    let claim = semantic_vocabulary::ClaimId::new(1).unwrap();
    let row = terminal_psi::StructuralPathQualification {
        path: vec![terminal_psi::StructuralPathSegment::Field("payload".into())],
        domain,
    };
    let parameter = |place| terminal_psi::StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type: root,
        multiplicity: terminal_psi::StructuralMultiplicity::Linear,
        access: terminal_psi::StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: vec![row.clone()],
    };
    let result = |place| terminal_psi::StructuralResultDeclaration {
        place,
        structural_type: root,
        multiplicity: terminal_psi::StructuralMultiplicity::Linear,
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
                result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
                    place: call_result,
                    structural_type: root,
                    multiplicity: terminal_psi::StructuralMultiplicity::Linear,
                    qualifications: Vec::new(),
                    projected_qualifications: vec![row.clone()],
                    claims: vec![terminal_psi::StructuralResultClaimBinding {
                        claim,
                        path: Vec::new(),
                    }],
                }),
                kind: OperationKind::CallStructural {
                    callee,
                    structural_arguments: vec![terminal_psi::StructuralArgument {
                        place: caller_input,
                        path: Vec::new(),
                        access: terminal_psi::StructuralAccess::Owned,
                    }],
                    claim_transfers: vec![terminal_psi::ClaimTransfer {
                        claim,
                        argument_index: 0,
                    }],
                    returned_claim_transfers: vec![terminal_psi::StructuralResultClaimTransfer {
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
        terminal_psi::StructuralTypeDeclaration {
            id: root,
            identity: "test::ProjectedResultRoot".into(),
            shape: terminal_psi::StructuralTypeShape::Record {
                fields: vec![terminal_psi::StructuralFieldDeclaration {
                    id: semantic_vocabulary::StructuralFieldId::new(1_954).unwrap(),
                    identity: "payload".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: terminal_psi::StructuralFieldType::Structural(leaf),
                }],
            },
        },
        terminal_psi::StructuralTypeDeclaration {
            id: leaf,
            identity: "test::ProjectedResultLeaf".into(),
            shape: terminal_psi::StructuralTypeShape::Record {
                fields: vec![terminal_psi::StructuralFieldDeclaration {
                    id: semantic_vocabulary::StructuralFieldId::new(1_956).unwrap(),
                    identity: "value".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: terminal_psi::StructuralFieldType::Scalar(
                        semantic_vocabulary::ScalarType::Integer(
                            semantic_vocabulary::IntegerType::new(
                                semantic_vocabulary::IntegerSign::Unsigned,
                                64,
                            )
                            .unwrap(),
                        ),
                    ),
                }],
            },
        },
    ];
    module.structural_domains = vec![terminal_psi::StructuralDomainDeclaration {
        id: domain,
        semantic_domain: semantic_vocabulary::DomainSemanticId::new(1_946).unwrap(),
        identity: "test::ProjectedResultReady".into(),
        carrier: leaf,
        content_projection: None,
    }];
    let caller_machine = &mut module.machines[0];
    caller_machine.structural_parameters = vec![parameter(caller_input)];
    caller_machine.structural_places = vec![
        terminal_psi::StructuralPlaceDeclaration {
            id: caller_input,
            kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        terminal_psi::StructuralPlaceDeclaration {
            id: caller_result,
            kind: semantic_vocabulary::StructuralPlaceKind::Result,
        },
        terminal_psi::StructuralPlaceDeclaration {
            id: call_result,
            kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
                producer: call,
                structural_type: root,
            },
        },
    ];
    caller_machine.entry_claims = vec![terminal_psi::EntryClaim {
        claim,
        input: caller_input,
        path: Vec::new(),
    }];

    let mut callee_machine = caller_machine.clone();
    callee_machine.id = callee;
    callee_machine.structural_parameters = vec![parameter(callee_input)];
    callee_machine.result = TerminalMachineResult::Structural(result(callee_result));
    callee_machine.structural_places = vec![
        terminal_psi::StructuralPlaceDeclaration {
            id: callee_input,
            kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        terminal_psi::StructuralPlaceDeclaration {
            id: callee_result,
            kind: semantic_vocabulary::StructuralPlaceKind::Result,
        },
    ];
    callee_machine.entry_claims = vec![terminal_psi::EntryClaim {
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
