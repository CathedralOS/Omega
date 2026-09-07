use proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, IntegerAffineWitness, ProofNode,
    ProofRule, ProofSystemMarker,
};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, EvidenceIdentity, IntegerSign, IntegerType, IntegerValue,
    MachineId, ObligationId, OperationId, PlaceId, Proposition, ScalarTerm, ScalarType,
    StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_psi::{
    Block, MachineContract, Operation, OperationKind, OperationResult, StructuralAccess,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge, TerminalMachine,
    TerminalMachineResult, TerminalModule, TerminalRankedGuard, TerminalRankedScc,
    TerminalRankedSccEdge, TerminalRankedSuccessorArgument, Terminator, ValueDeclaration,
    VocabularyMarker,
};
use terminal_verifier::{
    ModuleError, ObligationEvidence, ProofBundle, VerificationError,
    reconstruct_interpretable_operation_obligations, validate_module,
    validate_module_for_interpretation, validate_module_for_optimization,
    validate_module_representation, verify_module, verify_module_for_fixed_fuel,
    verify_module_for_interpretation, verify_module_for_native_ranked_countdown,
    verify_module_for_optimization,
};

fn id<T>(raw: u64, constructor: impl FnOnce(u64) -> Option<T>) -> T {
    constructor(raw).expect("nonzero fixture identity")
}

fn ranked_countdown() -> TerminalModule {
    ranked_countdown_with_width(32)
}

fn ranked_countdown_with_width(bits: u16) -> TerminalModule {
    let machine = id(1, MachineId::new);
    let preheader = id(1, BlockId::new);
    let header = id(2, BlockId::new);
    let decrement = id(3, BlockId::new);
    let done = id(4, BlockId::new);
    let initial = id(1, ValueId::new);
    let rank = id(2, ValueId::new);
    let zero = id(3, ValueId::new);
    let condition = id(4, ValueId::new);
    let one = id(5, ValueId::new);
    let next = id(6, ValueId::new);
    let integer = IntegerType::new(IntegerSign::Unsigned, bits).unwrap();
    let scalar = ScalarType::Integer(integer);
    let preheader_edge = id(1, EdgeId::new);
    let guard_edge = id(2, EdgeId::new);
    let exit_edge = id(3, EdgeId::new);
    let backedge = id(4, EdgeId::new);
    let return_edge = id(5, EdgeId::new);

    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine,
        structural_types: Vec::new(),
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
            parameters: vec![ValueDeclaration {
                id: initial,
                scalar_type: scalar,
            }],
            structural_parameters: Vec::new(),
            ranked_scc: Some(TerminalRankedScc {
                header,
                rank_parameter: rank,
                rank_type: integer,
                lower_bound: IntegerValue::Unsigned(0),
                upper_bound: integer.maximum_value(),
                covered_cyclic_edges: vec![TerminalRankedSccEdge {
                    edge: backedge,
                    source: decrement,
                    target: header,
                    guard: TerminalRankedGuard::UnsignedParameterPositive {
                        block: header,
                        edge: guard_edge,
                        condition,
                        parameter: rank,
                    },
                    successor_argument:
                        TerminalRankedSuccessorArgument::UnsignedParameterMinusOne {
                            argument_index: 0,
                            argument: next,
                            source_parameter: rank,
                            target_parameter: rank,
                        },
                }],
            }),
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: preheader,
            blocks: vec![
                Block {
                    id: preheader,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Jump {
                        edge: preheader_edge,
                        target: header,
                        arguments: vec![initial],
                        residual_affine_discards: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
                Block {
                    id: header,
                    parameters: vec![ValueDeclaration {
                        id: rank,
                        scalar_type: scalar,
                    }],
                    operations: vec![
                        Operation {
                            id: id(1, OperationId::new),
                            result: OperationResult::Scalar(ValueDeclaration {
                                id: zero,
                                scalar_type: scalar,
                            }),
                            kind: OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(0),
                            },
                        },
                        Operation {
                            id: id(2, OperationId::new),
                            result: OperationResult::Scalar(ValueDeclaration {
                                id: condition,
                                scalar_type: ScalarType::Boolean,
                            }),
                            kind: OperationKind::IntegerLessThan {
                                left: zero,
                                right: rank,
                            },
                        },
                    ],
                    terminator: Terminator::Conditional {
                        condition,
                        when_true: SuccessorEdge {
                            edge: guard_edge,
                            target: decrement,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            edge: exit_edge,
                            target: done,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                },
                Block {
                    id: decrement,
                    parameters: Vec::new(),
                    operations: vec![
                        Operation {
                            id: id(3, OperationId::new),
                            result: OperationResult::Scalar(ValueDeclaration {
                                id: one,
                                scalar_type: scalar,
                            }),
                            kind: OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(1),
                            },
                        },
                        Operation {
                            id: id(4, OperationId::new),
                            result: OperationResult::Scalar(ValueDeclaration {
                                id: next,
                                scalar_type: scalar,
                            }),
                            kind: OperationKind::ExactIntegerSubtract {
                                left: rank,
                                right: one,
                                obligation: id(1, ObligationId::new),
                            },
                        },
                    ],
                    terminator: Terminator::Jump {
                        edge: backedge,
                        target: header,
                        arguments: vec![next],
                        residual_affine_discards: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
                Block {
                    id: done,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: return_edge,
                        trivial_affine_discards: Vec::new(),
                    },
                },
            ],
            contract: MachineContract {
                id: id(1, ContractId::new),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

fn ranked_countdown_proof(module: &TerminalModule) -> ProofBundle {
    let interpretable = validate_module_for_interpretation(module)
        .expect("ranked countdown fixture is interpreter-valid");
    let obligations = reconstruct_interpretable_operation_obligations(interpretable)
        .expect("ranked countdown proof question reconstructs");
    let [reconstructed] = obligations.as_slice() else {
        panic!("ranked countdown has exactly one proof obligation")
    };
    let scalar_type = module.machines[0].parameters[0].scalar_type;
    let ScalarType::Integer(integer_type) = scalar_type else {
        unreachable!("ranked countdown parameter is an integer")
    };
    let rank = ScalarTerm::value(id(2, ValueId::new), scalar_type);
    let one = ScalarTerm::value(id(5, ValueId::new), scalar_type);
    let literal_one = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(1))
        .expect("ranked countdown literal one");
    let literal_guard = Proposition::LessOrEqual(literal_one.clone(), rank.clone());
    let guard_axiom = reconstructed
        .semantic_axioms
        .iter()
        .position(|axiom| *axiom == literal_guard)
        .expect("ranked countdown positive guard is reconstructed as a semantic axiom");
    let one_landing = Proposition::Equal(one.clone(), literal_one);
    let landing_axiom = reconstructed
        .semantic_axioms
        .iter()
        .position(|axiom| *axiom == one_landing)
        .expect("ranked countdown one is reconstructed as a semantic axiom");
    let ordered_guard = Proposition::LessOrEqual(one.clone(), rank.clone());
    ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation: reconstructed.obligation.id,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: id(1, EvidenceIdentity::new),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: reconstructed.obligation.proposition.clone(),
                    rule: ProofRule::IntegerAffineBound {
                        root_bound: Box::new(ProofNode {
                            conclusion: ordered_guard,
                            rule: ProofRule::IntegerOrderSubstitution {
                                relation: Box::new(ProofNode {
                                    conclusion: literal_guard,
                                    rule: ProofRule::SemanticAxiom { index: guard_axiom },
                                }),
                                equality: Box::new(ProofNode {
                                    conclusion: one_landing,
                                    rule: ProofRule::SemanticAxiom {
                                        index: landing_axiom,
                                    },
                                }),
                                endpoint: 0,
                            },
                        }),
                        witness: IntegerAffineWitness {
                            root: one,
                            target: ScalarTerm::exact_integer_subtract(
                                integer_type,
                                rank,
                                ScalarTerm::value(id(5, ValueId::new), scalar_type),
                            )
                            .expect("ranked countdown subtraction"),
                            definition_axioms: Vec::new(),
                            literal_axioms: Vec::new(),
                        },
                    },
                },
            }),
        }],
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
    }
}

fn add_loop_preserved_affine_parameter(module: &mut TerminalModule) -> PlaceId {
    let position = u32::try_from(module.machines[0].structural_parameters.len())
        .expect("fixture structural position fits u32");
    let raw = u64::from(position) + 1;
    let structural_type = id(raw, StructuralTypeId::new);
    let place = id(raw, PlaceId::new);
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type,
        identity: format!("LoopCustody{raw}"),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let machine = &mut module.machines[0];
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place,
            position,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::Parameter {
            position,
            is_self: false,
        },
    });
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut machine.blocks[3].terminator
    else {
        panic!("countdown exit must return Unit")
    };
    trivial_affine_discards.insert(0, place);
    place
}

#[test]
fn ranked_countdown_has_distinct_interpreter_only_authority() {
    let module = ranked_countdown();
    assert_eq!(validate_module_representation(&module), Ok(()));
    let interpretable = validate_module_for_interpretation(&module)
        .expect("exact ranked countdown is interpreter-valid");
    validate_module_for_optimization(&module)
        .expect("exact ranked countdown is independently optimizer-valid");
    let obligations = reconstruct_interpretable_operation_obligations(interpretable)
        .expect("countdown proof question reconstructs over its acyclic skeleton");
    assert_eq!(obligations.len(), 1);
    assert_eq!(obligations[0].obligation.id, id(1, ObligationId::new));
    let scalar_type = module.machines[0].parameters[0].scalar_type;
    let ScalarType::Integer(integer_type) = scalar_type else {
        unreachable!("ranked countdown parameter is an integer")
    };
    assert!(
        obligations[0]
            .semantic_axioms
            .contains(&Proposition::LessOrEqual(
                ScalarTerm::integer(integer_type, IntegerValue::Unsigned(1)).unwrap(),
                ScalarTerm::value(id(2, ValueId::new), scalar_type),
            ))
    );
    assert!(matches!(
        verify_module_for_interpretation(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        ),
        Err(VerificationError::MissingEvidence(obligation))
            if obligation == id(1, ObligationId::new)
    ));
    assert!(matches!(
        verify_module_for_fixed_fuel(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        ),
        Err(VerificationError::MissingEvidence(obligation))
            if obligation == id(1, ObligationId::new)
    ));
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::NonExecutableRankedScc(machine)) if machine == module.entry
    ));
    assert!(matches!(
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        ),
        Err(VerificationError::Module(ModuleError::NonExecutableRankedScc(machine)))
            if machine == module.entry
    ));
}

#[test]
fn native_ranked_countdown_authority_retains_proof_and_structural_frontiers() {
    let mut module = ranked_countdown();
    let place = add_loop_preserved_affine_parameter(&mut module);
    assert!(matches!(
        verify_module_for_native_ranked_countdown(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        ),
        Err(VerificationError::MissingEvidence(obligation))
            if obligation == id(1, ObligationId::new)
    ));
    let proof = ranked_countdown_proof(&module);

    let native =
        verify_module_for_native_ranked_countdown(&module, &proof, &AdmissionProfile::default())
            .expect("exact structural Unit u32 countdown has native authority");
    let optimizable = verify_module_for_optimization(&module, &proof, &AdmissionProfile::default())
        .expect("exact countdown has separate target-neutral optimizer authority");
    assert_eq!(optimizable.module(), &module);
    assert_eq!(optimizable.proof_bundle(), &proof);
    assert_eq!(
        optimizable.reconstructed_obligations().obligations().len(),
        1
    );
    assert_eq!(optimizable.accepted_facts().len(), 1);
    assert_eq!(native.module(), &module);
    assert_eq!(native.module(), &module);
    assert_eq!(native.proof_bundle(), &proof);
    assert_eq!(native.reconstructed_obligations().obligations().len(), 1);
    assert_eq!(native.accepted_facts().len(), 1);
    let header = module.machines[0].ranked_scc.as_ref().unwrap().header;
    let header_frontier = native
        .structural_frontiers()
        .machine(module.entry)
        .and_then(|frontiers| frontiers.block_entry(header))
        .expect("native authority retains the ranked header frontier");
    assert!(
        header_frontier
            .owned_places()
            .iter()
            .any(|owned| owned.place == place)
    );

    verify_module_for_interpretation(&module, &proof, &AdmissionProfile::default())
        .expect("interpreter authority remains independently constructible");
    verify_module_for_fixed_fuel(&module, &proof, &AdmissionProfile::default())
        .expect("fixed-fuel authority remains independently constructible");
    assert!(matches!(
        verify_module(&module, &proof, &AdmissionProfile::default()),
        Err(VerificationError::Module(ModuleError::NonExecutableRankedScc(machine)))
            if machine == module.entry
    ));
}

#[test]
fn native_ranked_countdown_authority_rejects_wider_rank_carriers() {
    let mut module = ranked_countdown_with_width(64);
    add_loop_preserved_affine_parameter(&mut module);
    let proof = ranked_countdown_proof(&module);

    verify_module_for_interpretation(&module, &proof, &AdmissionProfile::default())
        .expect("the reference interpreter retains its wider countdown slice");
    verify_module_for_fixed_fuel(&module, &proof, &AdmissionProfile::default())
        .expect("fixed-fuel verification remains separate from representable ceiling derivation");
    assert!(matches!(
        verify_module_for_native_ranked_countdown(
            &module,
            &proof,
            &AdmissionProfile::default()
        ),
        Err(VerificationError::Module(ModuleError::NonExecutableRankedScc(machine)))
            if machine == module.entry
    ));
}

#[test]
fn native_ranked_countdown_authority_requires_exactly_one_structural_token() {
    let module = ranked_countdown();
    let proof = ranked_countdown_proof(&module);
    verify_module_for_interpretation(&module, &proof, &AdmissionProfile::default())
        .expect("the interpreter permits an empty structural frontier");
    assert!(matches!(
        verify_module_for_native_ranked_countdown(
            &module,
            &proof,
            &AdmissionProfile::default()
        ),
        Err(VerificationError::Module(ModuleError::NonExecutableRankedScc(machine)))
            if machine == module.entry
    ));

    let mut extra = ranked_countdown();
    add_loop_preserved_affine_parameter(&mut extra);
    add_loop_preserved_affine_parameter(&mut extra);
    let proof = ranked_countdown_proof(&extra);
    verify_module_for_interpretation(&extra, &proof, &AdmissionProfile::default())
        .expect("the interpreter keeps its broader preserved-frontier policy");
    assert!(matches!(
        verify_module_for_native_ranked_countdown(
            &extra,
            &proof,
            &AdmissionProfile::default()
        ),
        Err(VerificationError::Module(ModuleError::NonExecutableRankedScc(machine)))
            if machine == extra.entry
    ));
}

#[test]
fn interpreter_ranked_countdown_rejects_extra_mixed_work() {
    let mut module = ranked_countdown();
    let integer = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    module.machines[0].blocks[0].operations.push(Operation {
        id: id(20, OperationId::new),
        result: OperationResult::Scalar(ValueDeclaration {
            id: id(20, ValueId::new),
            scalar_type: ScalarType::Integer(integer),
        }),
        kind: OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(7),
        },
    });
    assert_eq!(validate_module_representation(&module), Ok(()));
    assert!(matches!(
        validate_module_for_interpretation(&module),
        Err(ModuleError::NonExecutableRankedScc(machine)) if machine == module.entry
    ));
}

#[test]
fn ranked_countdown_preserves_a_nonempty_structural_frontier() {
    let mut module = ranked_countdown();
    add_loop_preserved_affine_parameter(&mut module);
    assert_eq!(validate_module_representation(&module), Ok(()));
    validate_module_for_interpretation(&module)
        .expect("interpreter accepts a preserved affine countdown frontier");
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::NonExecutableRankedScc(machine)) if machine == module.entry
    ));
}

#[test]
fn ranked_countdown_rejects_a_cycle_body_that_changes_structural_custody() {
    let mut module = ranked_countdown();
    let place = add_loop_preserved_affine_parameter(&mut module);
    let machine = &mut module.machines[0];
    let header = machine.ranked_scc.as_ref().unwrap().header;
    let Terminator::Conditional { when_true, .. } = &mut machine.blocks[1].terminator else {
        panic!("countdown header must select the cycle path")
    };
    when_true.trivial_affine_discards.push(place);
    assert_eq!(
        validate_module_representation(&module),
        Err(ModuleError::OwnedStructuralFrontierJoinMismatch(header))
    );
}

#[test]
fn ranked_countdown_rejects_uncovered_or_false_arithmetic() {
    let module = ranked_countdown();
    let mut uncovered = module.clone();
    uncovered.machines[0].ranked_scc = None;
    assert!(matches!(
        validate_module_representation(&uncovered),
        Err(ModuleError::ControlCycle(_))
    ));

    let mut forwards_original = module.clone();
    let rank = forwards_original.machines[0]
        .ranked_scc
        .as_ref()
        .unwrap()
        .rank_parameter;
    let decrement = &mut forwards_original.machines[0].blocks[2];
    let Terminator::Jump { arguments, .. } = &mut decrement.terminator else {
        panic!("decrement backedge")
    };
    arguments[0] = rank;
    assert!(matches!(
        validate_module_representation(&forwards_original),
        Err(ModuleError::InvalidRankedScc(_))
    ));

    let mut wrong_guard = module;
    wrong_guard.machines[0].blocks[1].operations[0].kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(1),
    };
    assert!(matches!(
        validate_module_representation(&wrong_guard),
        Err(ModuleError::InvalidRankedScc(_))
    ));
}
