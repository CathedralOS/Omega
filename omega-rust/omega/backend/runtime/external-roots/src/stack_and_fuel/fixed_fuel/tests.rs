use super::{
    BTreeSet, EntryStubId, FixedFuelCall, FixedFuelLocalEvidence, FixedFuelProviderSummary,
    InstalledCode, ProviderFuelSummaryId, ProviderFuelValidationReceiptId, RootProviderId,
    bind_installed_segment_fuel, bind_installed_segment_fuel_catalog, compose_fixed_fuel,
    validate_installed_segment_fuel, validate_installed_segment_fuel_catalog,
};
use executable_installation::{
    AdmissionReceiptId, Artifact, ArtifactAdmissionEvidence, ArtifactEntry, ArtifactId,
    CodePlacementAuthority, CodePlacementId, EntrySetId, FinalValidationCertificate,
    FinalValidationId, InstallAuthority, InstallationAudience, InstallationReceipt,
    InstallationScopeId, InstalledCodeId, MachineContractSetId, MachineFootprintId,
    MaterializationReceipt, PlacementPlanId, RelocationSetId, WxEnforcement, admit_executable,
    install_validated, materialize_admitted_artifact, materialize_and_freeze,
    validate_final_placement,
};
use extents::{
    AddressSpaceId, ExtentLineageId, ExtentProvenanceId, ExtentRightId, ExtentRights,
    ExtentRootGrant, MappingEraId,
};
use installation_evidence::ObjectEvidence;
use layout_plans::{
    ArtifactInstallationScopeId, PlacementConstraints, PlacementPhase, PlacementSite,
};
use proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, IntegerAffineWitness, PrimitiveJudgment,
    ProofNode, ProofRule, ProofSystemMarker, RecursiveComponentCertificate,
    RecursiveEdgeCertificate,
};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, EvidenceIdentity, IntegerSign, IntegerType, IntegerValue,
    MachineId, ObligationId, OperationId, Proposition, ScalarTerm, ScalarType, ValueId,
};
use terminal_codec::terminal_psi_identity;
use terminal_fixed_fuel::{derive_fixed_segment_fuel, derive_validated_fixed_safe_point_segments};
use terminal_psi::{
    Block, ControlCycleEvidence, MachineContract, Operation, OperationKind, OperationResult,
    SuccessorEdge, TerminalBlockNaturalRank, TerminalMachine, TerminalMachineResult,
    TerminalModule, TerminalNaturalCycle, TerminalNaturalRankComparison, TerminalNaturalRankEdge,
    TerminalRankedScc, Terminator, ValueDeclaration, VocabularyMarker,
};
use terminal_verifier::{
    ObligationEvidence, ProofBundle, reconstruct_control_cycle_obligations,
    reconstruct_interpretable_operation_obligations, validate_module_for_interpretation,
    verify_module,
};

#[derive(Debug)]
struct TestObject {
    identity: terminal_psi::TerminalPsiIdentity,
    machine: MachineId,
    bytes: Vec<u8>,
}

impl ObjectEvidence for TestObject {
    fn psi(&self) -> terminal_psi::TerminalPsiIdentity {
        self.identity
    }

    fn target(&self) -> target::NativeTarget {
        target::NativeTarget::linux_x64()
    }

    fn text_bytes(&self) -> &[u8] {
        &self.bytes
    }

    fn function_text_offset(&self, machine: MachineId) -> Option<usize> {
        (machine == self.machine).then_some(16)
    }
}

fn normalized<T, E: std::fmt::Debug>(identity: u64, constructor: fn(u64) -> Result<T, E>) -> T {
    constructor(identity).expect("normalized test identity")
}

fn entry(identity: u64) -> EntryStubId {
    EntryStubId::from_normalized_identity(identity).expect("entry identity")
}

fn constraints() -> PlacementConstraints {
    PlacementConstraints::new(
        None,
        16,
        PlacementPhase::Load,
        None,
        Some(
            ArtifactInstallationScopeId::from_normalized_identity(0x7100)
                .expect("artifact installation scope"),
        ),
    )
    .expect("placement constraints")
}

fn installed_code(
    artifact_identity: u64,
    installed_identity: u64,
    entry: EntryStubId,
) -> InstalledCode {
    let artifact_constraints = constraints();
    let contracts = normalized(0x7110, MachineContractSetId::from_normalized_identity);
    let footprint = normalized(0x7111, MachineFootprintId::from_normalized_identity);
    let artifact = Artifact::from_canonical_decode(
        normalized(artifact_identity, ArtifactId::from_normalized_identity),
        target::Architecture::X86_64,
        vec![0; 64],
        contracts,
        footprint,
        normalized(0x7112, PlacementPlanId::from_normalized_identity),
        artifact_constraints,
        normalized(0x7113, EntrySetId::from_normalized_identity),
        vec![ArtifactEntry::from_canonical_decode(entry, 16)],
        normalized(0x7114, RelocationSetId::from_normalized_identity),
        Vec::new(),
        executable_installation::ArtifactAuthorityCommitments::from_canonical_evidence(
            contracts,
            b"test-machine-contracts-v1",
            footprint,
            b"test-machine-footprint-v1",
            None,
            artifact_constraints
                .installation_scope()
                .map(|scope| (scope, b"test-installation-scope-v1".as_slice())),
        ),
    )
    .expect("test artifact");
    let admitted = admit_executable(
        &artifact,
        ArtifactAdmissionEvidence::from_validator(
            normalized(0x7120, AdmissionReceiptId::from_normalized_identity),
            &artifact,
            true,
        ),
    )
    .expect("admitted artifact");
    let rights = ExtentRights::from_normalized_identities([normalized(
        0x7130,
        ExtentRightId::from_normalized_identity,
    )]);
    let issuance = extents::ExtentProviderIssuance::from_normalized_identities([
        0x7141, 0x7142, 0x7143, 0x7144, 0x7145, 0x7146, 0x7147, 0x7148, 0x7149, 0x714a, 0x714b,
        0x714c, 0x714d,
    ])
    .expect("extent provider issuance");
    let extent = ExtentRootGrant::from_admitted_provider(
        issuance,
        normalized(0x7150, ExtentLineageId::from_normalized_identity),
        normalized(0x7151, AddressSpaceId::from_normalized_identity),
        rights.clone(),
        normalized(0x7152, ExtentProvenanceId::from_normalized_identity),
        normalized(0x7153, MappingEraId::from_normalized_identity),
    )
    .mint(0x1000, 4096)
    .expect("placement extent");
    let placement = CodePlacementAuthority::from_admitted_provider(
        normalized(0x7160, CodePlacementId::from_normalized_identity),
        normalized(0x7100, InstallationScopeId::from_normalized_identity),
        InstallationAudience::DormantLocal,
        &extent,
        rights,
        constraints(),
        PlacementSite {
            base_address: 0x1000,
            phase: PlacementPhase::Load,
            machine_regime: None,
            installation_scope: Some(
                ArtifactInstallationScopeId::from_normalized_identity(0x7100)
                    .expect("artifact installation scope"),
            ),
        },
    )
    .claim(extent)
    .expect("code placement");
    let materialized = materialize_admitted_artifact(&admitted, &placement, |_| None)
        .expect("relocation-free materialization");
    let frozen = materialize_and_freeze(
        &admitted,
        placement,
        materialized.clone(),
        MaterializationReceipt::from_materialized(
            &materialized,
            normalized(0x7161, MachineFootprintId::from_normalized_identity),
            true,
        ),
    )
    .expect("frozen placement");
    let validation = FinalValidationCertificate::from_validator(
        normalized(0x7162, FinalValidationId::from_normalized_identity),
        &frozen,
        true,
    );
    let validated = validate_final_placement(frozen, &validation).expect("validated placement");
    let authority = InstallAuthority::from_admitted_provider(&validated);
    let receipt = InstallationReceipt::from_provider(
        normalized(
            installed_identity,
            InstalledCodeId::from_normalized_identity,
        ),
        &validated,
        true,
        WxEnforcement::HardwareEnforced,
    );
    install_validated(validated, authority, receipt).expect("installed code")
}

fn terminal_fixture() -> TerminalModule {
    let machine = MachineId::new(0x7200).expect("machine identity");
    let block = BlockId::new(0x7201).expect("block identity");
    let final_block = BlockId::new(0x7202).expect("final block identity");
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
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
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: machine,
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block,
            blocks: vec![
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: block,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Jump {
                        erased_arguments: Vec::new(),
                        structural_arguments: Vec::new(),
                        edge: EdgeId::new(0x7203).expect("jump edge identity"),
                        target: final_block,
                        arguments: Vec::new(),
                        residual_affine_discards: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: final_block,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: EdgeId::new(0x7204).expect("return edge identity"),
                        trivial_affine_discards: Vec::new(),
                    },
                },
            ],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: ContractId::new(0x7205).expect("contract identity"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

fn core_id<T>(raw: u64, constructor: impl FnOnce(u64) -> Option<T>) -> T {
    constructor(raw).expect("nonzero fixture identity")
}

fn ranked_terminal_fixture() -> TerminalModule {
    let machine = core_id(0x7400, MachineId::new);
    let preheader = core_id(0x7401, BlockId::new);
    let header = core_id(0x7402, BlockId::new);
    let decrement = core_id(0x7403, BlockId::new);
    let done = core_id(0x7404, BlockId::new);
    let initial = core_id(0x7411, ValueId::new);
    let rank = core_id(0x7412, ValueId::new);
    let zero = core_id(0x7413, ValueId::new);
    let condition = core_id(0x7414, ValueId::new);
    let one = core_id(0x7415, ValueId::new);
    let next = core_id(0x7416, ValueId::new);
    let integer = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32 fixture type");
    let scalar = ScalarType::Integer(integer);
    let preheader_edge = core_id(0x7421, EdgeId::new);
    let guard_edge = core_id(0x7422, EdgeId::new);
    let exit_edge = core_id(0x7423, EdgeId::new);
    let backedge = core_id(0x7424, EdgeId::new);
    let return_edge = core_id(0x7425, EdgeId::new);

    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
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
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: machine,
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![ValueDeclaration {
                qualifications: Default::default(),
                id: initial,
                scalar_type: scalar,
            }],
            ranked_scc: Some(TerminalRankedScc::Natural(vec![TerminalNaturalCycle {
                rank_type: integer,
                ranks: vec![
                    TerminalBlockNaturalRank {
                        block: header,
                        value: rank,
                    },
                    TerminalBlockNaturalRank {
                        block: decrement,
                        value: rank,
                    },
                ],
                edges: vec![
                    TerminalNaturalRankEdge {
                        edge: guard_edge,
                        source: header,
                        target: decrement,
                        successor_rank: rank,
                        comparison: TerminalNaturalRankComparison::Preserving,
                    },
                    TerminalNaturalRankEdge {
                        edge: backedge,
                        source: decrement,
                        target: header,
                        successor_rank: next,
                        comparison: TerminalNaturalRankComparison::Strict,
                    },
                ],
            }])),
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: preheader,
            blocks: vec![
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: preheader,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Jump {
                        erased_arguments: Vec::new(),
                        structural_arguments: Vec::new(),
                        edge: preheader_edge,
                        target: header,
                        arguments: vec![initial],
                        residual_affine_discards: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: header,
                    parameters: vec![ValueDeclaration {
                        qualifications: Default::default(),
                        id: rank,
                        scalar_type: scalar,
                    }],
                    operations: vec![
                        Operation {
                            static_reach_binding: None,
                            id: core_id(0x7431, OperationId::new),
                            result: OperationResult::Scalar(ValueDeclaration {
                                qualifications: Default::default(),
                                id: zero,
                                scalar_type: scalar,
                            }),
                            kind: OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(0),
                            },
                        },
                        Operation {
                            static_reach_binding: None,
                            id: core_id(0x7432, OperationId::new),
                            result: OperationResult::Scalar(ValueDeclaration {
                                qualifications: Default::default(),
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
                            erased_arguments: Vec::new(),
                            structural_arguments: Vec::new(),
                            edge: guard_edge,
                            target: decrement,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            erased_arguments: Vec::new(),
                            structural_arguments: Vec::new(),
                            edge: exit_edge,
                            target: done,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: decrement,
                    parameters: Vec::new(),
                    operations: vec![
                        Operation {
                            static_reach_binding: None,
                            id: core_id(0x7433, OperationId::new),
                            result: OperationResult::Scalar(ValueDeclaration {
                                qualifications: Default::default(),
                                id: one,
                                scalar_type: scalar,
                            }),
                            kind: OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(1),
                            },
                        },
                        Operation {
                            static_reach_binding: None,
                            id: core_id(0x7434, OperationId::new),
                            result: OperationResult::Scalar(ValueDeclaration {
                                qualifications: Default::default(),
                                id: next,
                                scalar_type: scalar,
                            }),
                            kind: OperationKind::ExactIntegerSubtract {
                                left: rank,
                                right: one,
                                obligation: core_id(0x7441, ObligationId::new),
                            },
                        },
                    ],
                    terminator: Terminator::Jump {
                        erased_arguments: Vec::new(),
                        structural_arguments: Vec::new(),
                        edge: backedge,
                        target: header,
                        arguments: vec![next],
                        residual_affine_discards: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
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
                erased_scalar_formals: Vec::new(),
                id: core_id(0x7451, ContractId::new),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

fn ranked_terminal_proof(module: &TerminalModule) -> ProofBundle {
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
    let rank = ScalarTerm::value(core_id(0x7412, ValueId::new), scalar_type);
    let one = ScalarTerm::value(core_id(0x7415, ValueId::new), scalar_type);
    let literal_one = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(1))
        .expect("ranked countdown literal one");
    let literal_guard = Proposition::LessOrEqual(literal_one.clone(), rank.clone());
    let guard_axiom = reconstructed
        .semantic_axioms
        .iter()
        .position(|axiom| *axiom == literal_guard)
        .expect("ranked countdown positive guard is reconstructed");
    let one_landing = Proposition::Equal(one.clone(), literal_one);
    let landing_axiom = reconstructed
        .semantic_axioms
        .iter()
        .position(|axiom| *axiom == one_landing)
        .expect("ranked countdown literal one is reconstructed");
    let ordered_guard = Proposition::LessOrEqual(one.clone(), rank.clone());
    ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation: reconstructed.obligation.id,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: core_id(0x7461, EvidenceIdentity::new),
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
                                ScalarTerm::value(core_id(0x7415, ValueId::new), scalar_type),
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
        control_cycles: vec![ranked_cycle_evidence(module)],
        evidence_producers: Vec::new(),
    }
}

/// Certificate for the verifier-reconstructed natural-order question the
/// retained countdown claims: forward edges preserve the header's rank and the
/// covered backedge carries the strict `rank - 1 < rank` descent.
fn ranked_cycle_evidence(module: &TerminalModule) -> ControlCycleEvidence {
    let questions = reconstruct_control_cycle_obligations(module)
        .expect("ranked countdown cycle question reconstructs");
    let [question] = questions.as_slice() else {
        panic!("ranked countdown reconstructs exactly one cycle question")
    };
    let obligation = &question.obligation;
    let scalar_type = module.machines[0].parameters[0].scalar_type;
    let ScalarType::Integer(integer_type) = scalar_type else {
        unreachable!("ranked countdown parameter is an integer")
    };
    let one = ScalarTerm::value(core_id(0x7415, ValueId::new), scalar_type);
    let zero_literal = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(0))
        .expect("ranked countdown literal zero");
    let one_literal = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(1))
        .expect("ranked countdown literal one");
    let axiom = |axioms: &[Proposition], conclusion: Proposition| -> ProofNode {
        let index = axioms
            .iter()
            .position(|axiom| *axiom == conclusion)
            .expect("countdown cycle premise is reconstructed as a semantic axiom");
        ProofNode {
            conclusion,
            rule: ProofRule::SemanticAxiom { index },
        }
    };
    let mut envelope = 0x7472_u64;
    let mut wrap = |proof: ProofNode| {
        envelope += 1;
        EvidenceRoute::CertificateDerived(CertificateEnvelope {
            identity: core_id(envelope, EvidenceIdentity::new),
            proof_system_marker: ProofSystemMarker::CURRENT,
            proof,
        })
    };
    let well_foundedness = wrap(ProofNode {
        conclusion: obligation.well_foundedness.obligation.proposition.clone(),
        rule: ProofRule::SemanticAxiom { index: 0 },
    });
    let edges = obligation
        .edges
        .iter()
        .map(|edge| {
            let decrease = &edge.decrease;
            let proof = match &decrease.obligation.proposition {
                Proposition::LessOrEqual(left, right) if left == right => ProofNode {
                    conclusion: decrease.obligation.proposition.clone(),
                    rule: ProofRule::IntegerOrderWeakening {
                        relation: Box::new(ProofNode {
                            conclusion: Proposition::Equal(left.clone(), right.clone()),
                            rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
                        }),
                    },
                },
                Proposition::LessThan(after, before) => ProofNode {
                    conclusion: decrease.obligation.proposition.clone(),
                    rule: ProofRule::IntegerSubtractOrder {
                        difference: Box::new(axiom(
                            &decrease.semantic_axioms,
                            Proposition::Equal(
                                after.clone(),
                                ScalarTerm::exact_integer_subtract(
                                    integer_type,
                                    before.clone(),
                                    one.clone(),
                                )
                                .expect("ranked countdown subtraction term"),
                            ),
                        )),
                        positive: Box::new(ProofNode {
                            conclusion: Proposition::LessThan(zero_literal.clone(), one.clone()),
                            rule: ProofRule::IntegerOrderSubstitution {
                                relation: Box::new(ProofNode {
                                    conclusion: Proposition::LessThan(
                                        zero_literal.clone(),
                                        one_literal.clone(),
                                    ),
                                    rule: ProofRule::Primitive(
                                        PrimitiveJudgment::ClosedIntegerRelation,
                                    ),
                                }),
                                equality: Box::new(axiom(
                                    &decrease.semantic_axioms,
                                    Proposition::Equal(one.clone(), one_literal.clone()),
                                )),
                                endpoint: 1,
                            },
                        }),
                    },
                },
                other => panic!("unexpected countdown decrease question {other:?}"),
            };
            RecursiveEdgeCertificate {
                obligation: decrease.obligation.id,
                evidence: wrap(proof),
            }
        })
        .collect();
    ControlCycleEvidence {
        component: question.component,
        certificate: RecursiveComponentCertificate {
            identity: core_id(0x7471, EvidenceIdentity::new),
            ranking_relation: obligation
                .ranking_relation
                .expect("ranked countdown reconstructs its natural relation"),
            well_foundedness,
            edges,
        },
    }
}

#[test]
fn installed_segment_replay_is_exact_and_never_becomes_entry_authority() {
    let module = terminal_fixture();
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("terminal fixture verifies");
    let machine = module.entry;
    let block = module.machines[0].entry;
    let edge = match module.machines[0].blocks[0].terminator {
        Terminator::Jump { edge, .. } => edge,
        _ => unreachable!("fixture begins with one jump"),
    };
    let certificate = derive_fixed_segment_fuel(&verified, machine, block, edge)
        .expect("one exact terminal segment");
    let terminal = TestObject {
        identity: terminal_psi_identity(&module).expect("terminal identity"),
        machine,
        bytes: vec![0; 64],
    };
    let selected_entry = entry(0x7300);
    let installed = installed_code(0x7310, 0x7320, selected_entry);
    let binding = bind_installed_segment_fuel(certificate, &terminal, &installed, selected_entry)
        .expect("segment binds exact installed function");

    validate_installed_segment_fuel(&binding, &installed, selected_entry)
        .expect("exact installed segment replays");
    assert_eq!(binding.certificate().machine(), machine);
    assert_eq!(binding.certificate().start_block(), block);
    assert_eq!(binding.certificate().end_edge(), edge);
    let segment_identity = ProviderFuelSummaryId::from_normalized_identity(0x7330).unwrap();
    let provider = RootProviderId::from_normalized_identity(0x7331).unwrap();
    let summary = FixedFuelProviderSummary::from_segment(
        segment_identity,
        provider,
        binding.clone(),
        BTreeSet::new(),
    );
    assert!(matches!(
        &summary.local_evidence,
        FixedFuelLocalEvidence::TerminalSegment(_)
    ));
    let direct_error = compose_fixed_fuel(segment_identity, [&summary])
        .expect_err("one path segment cannot stand in for whole-entry work");
    assert!(direct_error.0.contains("path-segment"));

    let root_identity = ProviderFuelSummaryId::from_normalized_identity(0x7332).unwrap();
    let root = FixedFuelProviderSummary::from_admitted_provider(
        root_identity,
        provider,
        binding.certificate().schedule(),
        1,
        BTreeSet::from([FixedFuelCall {
            callee: segment_identity,
            maximum_invocations: 1,
        }]),
        ProviderFuelValidationReceiptId::from_normalized_identity(0x7333).unwrap(),
    );
    let transitive_error = compose_fixed_fuel(root_identity, [&root, &summary])
        .expect_err("a whole-entry root cannot absorb a path-segment callee ceiling");
    assert_eq!(transitive_error.0, direct_error.0);
    assert_eq!(
        compose_fixed_fuel(root_identity, [&root, &summary])
            .expect_err("rejection leaves the input summaries reusable")
            .0,
        direct_error.0
    );
    let opaque_only = FixedFuelProviderSummary::from_admitted_provider(
        root_identity,
        provider,
        binding.certificate().schedule(),
        1,
        BTreeSet::new(),
        ProviderFuelValidationReceiptId::from_normalized_identity(0x7333).unwrap(),
    );
    assert_eq!(
        compose_fixed_fuel(root_identity, [&opaque_only])
            .expect("ordinary whole-entry provider summaries remain admissible")
            .units(),
        1
    );

    let wrong_occurrence = installed_code(0x7310, 0x7321, selected_entry);
    assert!(
        validate_installed_segment_fuel(&binding, &wrong_occurrence, selected_entry)
            .unwrap_err()
            .0
            .contains("selected installed code")
    );
    let wrong_artifact = installed_code(0x7312, 0x7322, selected_entry);
    assert!(validate_installed_segment_fuel(&binding, &wrong_artifact, selected_entry).is_err());
    assert!(validate_installed_segment_fuel(&binding, &installed, entry(0x7301)).is_err());
    validate_installed_segment_fuel(&binding, &installed, selected_entry)
        .expect("failed replay leaves the exact segment binding reusable");
}

#[test]
fn installed_segment_catalog_binds_one_complete_partition_to_one_occurrence() {
    let module = terminal_fixture();
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("terminal fixture verifies");
    let catalog = derive_validated_fixed_safe_point_segments(&verified, module.entry)
        .expect("complete two-segment partition");
    assert_eq!(catalog.certificates().len(), 2);
    assert_eq!(
        catalog.certificates()[0].start_block(),
        module.machines[0].entry
    );
    assert_eq!(
        catalog.certificates()[1].start_block(),
        module.machines[0].blocks[1].id
    );

    let terminal = TestObject {
        identity: terminal_psi_identity(&module).expect("terminal identity"),
        machine: module.entry,
        bytes: vec![0; 64],
    };
    let selected_entry = entry(0x7340);
    let installed = installed_code(0x7350, 0x7360, selected_entry);
    let binding =
        bind_installed_segment_fuel_catalog(catalog, &terminal, &installed, selected_entry)
            .expect("complete partition binds to one installed function occurrence");

    assert_eq!(binding.psi(), terminal.identity);
    assert_eq!(binding.machine(), module.entry);
    assert_eq!(binding.segments().len(), 2);
    assert_eq!(binding.installed_code(), installed.identity());
    assert_eq!(binding.artifact(), installed.artifact());
    assert_eq!(binding.entry(), selected_entry);
    validate_installed_segment_fuel_catalog(&binding, &installed, selected_entry)
        .expect("exact installed occurrence replays");

    let wrong_occurrence = installed_code(0x7350, 0x7361, selected_entry);
    assert!(
        validate_installed_segment_fuel_catalog(&binding, &wrong_occurrence, selected_entry,)
            .is_err()
    );
    let wrong_artifact = installed_code(0x7352, 0x7362, selected_entry);
    assert!(
        validate_installed_segment_fuel_catalog(&binding, &wrong_artifact, selected_entry,)
            .is_err()
    );
    assert!(validate_installed_segment_fuel_catalog(&binding, &installed, entry(0x7341),).is_err());
    validate_installed_segment_fuel_catalog(&binding, &installed, selected_entry)
        .expect("failed borrowed replay leaves the installed catalog intact");
}

#[test]
fn installed_natural_cycle_safe_point_catalog_binds_to_one_occurrence() {
    let module = ranked_terminal_fixture();
    let proof = ranked_terminal_proof(&module);
    let verified = verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("natural-cycle countdown verifies under ordinary authority");
    let catalog = derive_validated_fixed_safe_point_segments(&verified, module.entry)
        .expect("exact safe-point roster for the ranked component");
    assert_eq!(catalog.certificates().len(), 5);

    let terminal = TestObject {
        identity: terminal_psi_identity(&module).expect("terminal identity"),
        machine: module.entry,
        bytes: vec![0; 64],
    };
    let selected_entry = entry(0x7470);
    let installed = installed_code(0x7480, 0x7490, selected_entry);
    let binding =
        bind_installed_segment_fuel_catalog(catalog, &terminal, &installed, selected_entry)
            .expect("ranked roster binds to one exact installed occurrence");

    assert_eq!(binding.psi(), terminal.identity);
    assert_eq!(binding.machine(), module.entry);
    assert_eq!(binding.segments().len(), 5);
    assert_eq!(binding.installed_code(), installed.identity());
    assert_eq!(binding.artifact(), installed.artifact());
    assert_eq!(binding.entry(), selected_entry);
    assert_eq!(
        binding
            .segments()
            .iter()
            .map(|segment| segment.ceiling_units())
            .collect::<Vec<_>>(),
        vec![1, 3, 3, 3, 1],
    );
    validate_installed_segment_fuel_catalog(&binding, &installed, selected_entry)
        .expect("exact installed ranked roster replays");

    let wrong_occurrence = installed_code(0x7480, 0x7491, selected_entry);
    assert!(
        validate_installed_segment_fuel_catalog(&binding, &wrong_occurrence, selected_entry)
            .is_err()
    );
    let wrong_artifact = installed_code(0x7482, 0x7492, selected_entry);
    assert!(
        validate_installed_segment_fuel_catalog(&binding, &wrong_artifact, selected_entry).is_err()
    );
    assert!(validate_installed_segment_fuel_catalog(&binding, &installed, entry(0x7471),).is_err());
    validate_installed_segment_fuel_catalog(&binding, &installed, selected_entry)
        .expect("failed replay leaves the ranked roster intact");
}
