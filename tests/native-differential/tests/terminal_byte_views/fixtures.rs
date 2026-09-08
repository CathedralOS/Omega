//! Authored Terminal fixtures and their independently reconstructed byte bounds proof.

use proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, ObligationClass, ProofNode, ProofRule,
    ProofSystemMarker,
};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, EvidenceIdentity, IntegerSign, IntegerType, IntegerValue,
    MachineId, ObligationId, OperationId, PlaceId, Proposition, ScalarTerm, ScalarType,
    StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_psi::{
    Block, ByteSequenceCarrier, MachineContract, Operation, OperationKind, OperationResult,
    StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge,
    TerminalMachine, TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
    VocabularyMarker,
};
use terminal_verifier::{ObligationEvidence, ProofBundle};

pub(super) fn byte_view_length_module() -> TerminalModule {
    let machine = MachineId::new(1).unwrap();
    let block = BlockId::new(2).unwrap();
    let source = PlaceId::new(3).unwrap();
    let structural_type = StructuralTypeId::new(4).unwrap();
    let length = ValueId::new(5).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine,
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "test::ImmutableBytes".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
        }],
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine,
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                place: source,
                position: 0,
                is_self: false,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::SharedBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                id: ValueId::new(6).unwrap(),
                scalar_type,
            }),
            structural_places: vec![StructuralPlaceDeclaration {
                id: source,
                kind: StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            }],
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block,
            blocks: vec![Block {
                id: block,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(7).unwrap(),
                    result: OperationResult::Scalar(ValueDeclaration {
                        id: length,
                        scalar_type,
                    }),
                    kind: OperationKind::ByteSequenceLength { source },
                }],
                terminator: Terminator::Return {
                    edge: EdgeId::new(8).unwrap(),
                    value: length,
                    cleanup_actions: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: ContractId::new(9).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

pub(super) fn byte_view_read_module() -> TerminalModule {
    let mut module = byte_view_length_module();
    let machine = &mut module.machines[0];
    let source = machine.structural_parameters[0].place;
    let length = ValueId::new(5).unwrap();
    let byte_index = ValueId::new(10).unwrap();
    let condition = ValueId::new(11).unwrap();
    let u64_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    machine.parameters.push(ValueDeclaration {
        id: byte_index,
        scalar_type: u64_type,
    });
    machine.blocks[0].operations.push(Operation {
        id: OperationId::new(11).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            id: condition,
            scalar_type: ScalarType::Boolean,
        }),
        kind: OperationKind::IntegerLessThan {
            left: byte_index,
            right: length,
        },
    });
    let successor = |edge, block| SuccessorEdge {
        edge: EdgeId::new(edge).unwrap(),
        target: BlockId::new(block).unwrap(),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    machine.blocks[0].terminator = Terminator::Conditional {
        condition,
        when_true: successor(17, 12),
        when_false: successor(18, 15),
    };
    machine.blocks.extend([
        Block {
            id: BlockId::new(12).unwrap(),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: vec![
                Operation {
                    id: OperationId::new(13).unwrap(),
                    result: OperationResult::Scalar(ValueDeclaration {
                        id: ValueId::new(13).unwrap(),
                        scalar_type: ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                        ),
                    }),
                    kind: OperationKind::ByteSequenceRead {
                        source,
                        index: byte_index,
                        length,
                        obligation: ObligationId::new(1).unwrap(),
                    },
                },
                Operation {
                    id: OperationId::new(14).unwrap(),
                    result: OperationResult::Scalar(ValueDeclaration {
                        id: ValueId::new(14).unwrap(),
                        scalar_type: u64_type,
                    }),
                    kind: OperationKind::IntegerWiden {
                        operand: ValueId::new(13).unwrap(),
                    },
                },
            ],
            terminator: Terminator::Return {
                edge: EdgeId::new(19).unwrap(),
                value: ValueId::new(14).unwrap(),
                cleanup_actions: Vec::new(),
            },
        },
        Block {
            id: BlockId::new(15).unwrap(),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: vec![Operation {
                id: OperationId::new(16).unwrap(),
                result: OperationResult::Scalar(ValueDeclaration {
                    id: ValueId::new(16).unwrap(),
                    scalar_type: u64_type,
                }),
                kind: OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(256),
                },
            }],
            terminator: Terminator::Return {
                edge: EdgeId::new(20).unwrap(),
                value: ValueId::new(16).unwrap(),
                cleanup_actions: Vec::new(),
            },
        },
    ]);
    module
}

pub(super) fn byte_view_length_helper_module() -> TerminalModule {
    byte_view_length_helper_chain(1)
}

pub(super) fn byte_view_length_helper_chain(depth: u64) -> TerminalModule {
    let mut module = byte_view_length_module();
    for level in 1..=depth {
        let identity_base = level * 10;
        let mut caller = module.machines[0].clone();
        caller.id = MachineId::new(identity_base).unwrap();
        caller.contract.id = ContractId::new(identity_base + 9).unwrap();
        caller.entry = BlockId::new(identity_base + 2).unwrap();
        caller.blocks[0].id = caller.entry;
        caller.structural_parameters[0].place = PlaceId::new(identity_base + 3).unwrap();
        caller.structural_places[0].id = PlaceId::new(identity_base + 3).unwrap();
        let TerminalMachineResult::Scalar(result) = &mut caller.result else {
            unreachable!()
        };
        result.id = ValueId::new(identity_base + 6).unwrap();
        caller.blocks[0].operations[0].id = OperationId::new(identity_base + 7).unwrap();
        let OperationResult::Scalar(result) = &mut caller.blocks[0].operations[0].result else {
            unreachable!()
        };
        result.id = ValueId::new(identity_base + 5).unwrap();
        caller.blocks[0].terminator = Terminator::Return {
            edge: EdgeId::new(identity_base + 8).unwrap(),
            value: ValueId::new(identity_base + 5).unwrap(),
            cleanup_actions: Vec::new(),
        };
        caller.blocks[0].operations[0].kind = OperationKind::CallStructuralScalar {
            callee: module.entry,
            arguments: Vec::new(),
            structural_arguments: vec![terminal_psi::StructuralArgument {
                place: caller.structural_parameters[0].place,
                path: Vec::new(),
                access: StructuralAccess::SharedBorrow,
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        };
        module.entry = caller.id;
        module.machines.push(caller);
    }
    module
}

pub(super) fn byte_view_read_proof(module: &TerminalModule) -> ProofBundle {
    let reconstructed = terminal_verifier::reconstruct_terminal_obligations(module).unwrap();
    let [site] = reconstructed.obligations() else {
        panic!("one reconstructed byte-read bounds obligation")
    };
    assert!(site.canonical_certificate);
    assert_eq!(site.obligation.class, ObligationClass::Derivable);
    let u64_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    assert_eq!(
        site.obligation.proposition,
        Proposition::LessThan(
            ScalarTerm::value(ValueId::new(10).unwrap(), u64_type),
            ScalarTerm::value(ValueId::new(5).unwrap(), u64_type),
        )
    );
    let citation = site
        .semantic_axioms
        .iter()
        .position(|fact| fact == &site.obligation.proposition)
        .expect("the true edge establishes this exact index-less-than-length premise");
    let proof = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation: site.obligation.id,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(1).unwrap(),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: site.obligation.proposition.clone(),
                    rule: ProofRule::SemanticAxiom { index: citation },
                },
            }),
        }],
        ..ProofBundle::default()
    };
    terminal_verifier::verify_module(module, &proof, &AdmissionProfile::default())
        .expect("the selected guard proves byte-read safety without admission");
    proof
}

pub(super) fn byte_view_read_call_module() -> TerminalModule {
    let mut module = byte_view_read_module();
    let mut caller = byte_view_length_module().machines.remove(0);
    let caller_id = MachineId::new(100).unwrap();
    let source = PlaceId::new(102).unwrap();
    let byte_index = ValueId::new(103).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    caller.id = caller_id;
    caller.entry = BlockId::new(101).unwrap();
    caller.parameters = vec![ValueDeclaration { id: byte_index, scalar_type }];
    caller.structural_parameters[0].place = source;
    caller.structural_places[0].id = source;
    caller.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: ValueId::new(104).unwrap(), scalar_type,
    });
    caller.contract.id = ContractId::new(110).unwrap();
    caller.blocks = vec![Block {
        id: caller.entry,
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        // The second invocation requires both original inputs to survive the first call.
        operations: [(105, 106), (107, 108)].into_iter().map(|(operation, result)| Operation {
            id: OperationId::new(operation).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration {
                id: ValueId::new(result).unwrap(), scalar_type,
            }),
            kind: OperationKind::CallStructuralScalar {
                callee: module.entry,
                arguments: vec![byte_index],
                structural_arguments: vec![terminal_psi::StructuralArgument {
                    place: source, path: Vec::new(), access: StructuralAccess::SharedBorrow,
                }],
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        }).collect(),
        terminator: Terminator::Return {
            edge: EdgeId::new(109).unwrap(),
            value: ValueId::new(108).unwrap(),
            cleanup_actions: Vec::new(),
        },
    }];
    module.entry = caller_id;
    module.machines.push(caller);
    module
}
