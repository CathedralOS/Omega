use proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemMarker,
};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, EvidenceIdentity, IntegerSign, IntegerType, IntegerValue,
    MachineId, ObligationId, OperationId, Proposition, ScalarTerm, ScalarType, ValueId,
};
use terminal_codec::{encode_module, encode_proof_bundle};
use terminal_fuel::{FuelChargeSite, FuelExhaustion, TerminalFuelMeter, TerminalFuelSchedule};
use terminal_interpreter::{
    TerminalArtifactInterpretError, TerminalCrash, TerminalExecution, TerminalExecutionResult,
    TerminalExecutionStatus, TerminalInterpretError, TerminalScalarValue,
    interpret_terminal_artifact, interpret_terminal_artifact_measured,
};
use terminal_psi::{
    Block, ContractClause, CrashCause, CrashRouteBucket, CrashRouteGuard, MachineContract,
    Operation, OperationKind, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
    ValueDeclaration, VocabularyMarker,
};
use terminal_psi_to_abstract_operations::{ArtifactLoweringError, lower_artifact_sections};
use terminal_verifier::{ObligationEvidence, ProofBundle};

#[test]
fn verified_integer_control_contract_slice_executes_directly() {
    let integer = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let scalar_type = ScalarType::Integer(integer);
    let constant = ValueId::new(1).expect("constant");
    let forwarded = ValueId::new(2).expect("forwarded");
    let result = ValueId::new(3).expect("result");
    let obligation = ObligationId::new(1).expect("obligation");
    let term = |id| ScalarTerm::value(id, scalar_type);
    let seven = || ScalarTerm::integer(integer, IntegerValue::Signed(7)).expect("seven");
    let goal = Proposition::Equal(term(result), seven());

    let machine = TerminalMachine {
        id: MachineId::new(1).expect("machine"),
        attachment: None,
        structural_parameters: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(ValueDeclaration {
            id: result,
            scalar_type,
        }),
        structural_places: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(1).expect("entry"),
        blocks: vec![
            Block {
                id: BlockId::new(1).expect("entry"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(1).expect("operation"),
                    result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                        id: constant,
                        scalar_type,
                    }),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Signed(7),
                    },
                }],
                terminator: Terminator::Jump {
                    edge: EdgeId::new(1).expect("jump"),
                    target: BlockId::new(2).expect("exit"),
                    arguments: vec![constant],
                    trivial_affine_discards: Vec::new(),
                },
            },
            Block {
                id: BlockId::new(2).expect("exit"),
                parameters: vec![ValueDeclaration {
                    id: forwarded,
                    scalar_type,
                }],
                operations: Vec::new(),
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(2).expect("return"),
                    value: forwarded,
                },
            },
        ],
        contract: MachineContract {
            id: ContractId::new(1).expect("contract"),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: vec![ContractClause {
                obligation,
                proposition: goal.clone(),
            }],
            outcome_specific_ensures: Vec::new(),
        },
    };
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine.id,
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
        machines: vec![machine],
    };
    let constant_fact = Proposition::Equal(term(constant), seven());
    let forwarding_fact = Proposition::Equal(term(forwarded), term(constant));
    let return_fact = Proposition::Equal(term(result), term(forwarded));
    let forwarded_is_seven = Proposition::Equal(term(forwarded), seven());
    let proof = ProofNode {
        conclusion: goal,
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(ProofNode {
                conclusion: return_fact,
                rule: ProofRule::SemanticAxiom { index: 2 },
            }),
            middle_equals_right: Box::new(ProofNode {
                conclusion: forwarded_is_seven,
                rule: ProofRule::EqualityTransitivity {
                    left_equals_middle: Box::new(ProofNode {
                        conclusion: forwarding_fact,
                        rule: ProofRule::SemanticAxiom { index: 1 },
                    }),
                    middle_equals_right: Box::new(ProofNode {
                        conclusion: constant_fact,
                        rule: ProofRule::SemanticAxiom { index: 0 },
                    }),
                },
            }),
        },
    };
    let bundle = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(1).expect("certificate"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof,
            }),
        }],
    };
    let expected = TerminalScalarValue::Integer {
        scalar_type: integer,
        value: IntegerValue::Signed(7),
    };
    let semantic_bytes = encode_module(&module).expect("canonical semantic artifact");
    let proof_bytes = encode_proof_bundle(&bundle).expect("canonical proof artifact");
    let first = interpret_terminal_artifact_measured(
        &semantic_bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
        &[],
    )
    .expect("artifact-root interpretation decodes and verifies before execution");
    let second = interpret_terminal_artifact_measured(
        &semantic_bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
        &[],
    )
    .expect("equal artifact execution reproduces deterministic usage");
    assert_eq!(first, second);
    assert_eq!(first.value(), TerminalExecutionResult::Scalar(expected));
    let artifact_abstract =
        lower_artifact_sections(&semantic_bytes, &proof_bytes, &AdmissionProfile::default())
            .expect("artifact-root abstract lowering decodes and verifies first");
    assert_eq!(artifact_abstract.entry, module.entry);
    assert_eq!(
        interpret_terminal_artifact(
            &semantic_bytes,
            &proof_bytes,
            &AdmissionProfile::default(),
            &[],
        )
        .expect("unmeasured artifact entry returns the same result"),
        TerminalExecutionResult::Scalar(expected)
    );
    let mut malformed_semantic = semantic_bytes.clone();
    malformed_semantic.push(0);
    assert!(matches!(
        interpret_terminal_artifact(
            &malformed_semantic,
            &proof_bytes,
            &AdmissionProfile::default(),
            &[],
        ),
        Err(TerminalArtifactInterpretError::SemanticDecode(_))
    ));
    assert!(matches!(
        lower_artifact_sections(
            &malformed_semantic,
            &proof_bytes,
            &AdmissionProfile::default(),
        ),
        Err(ArtifactLoweringError::SemanticDecode(_))
    ));
    let mut malformed_proof = proof_bytes.clone();
    malformed_proof.push(0);
    assert!(matches!(
        interpret_terminal_artifact(
            &semantic_bytes,
            &malformed_proof,
            &AdmissionProfile::default(),
            &[],
        ),
        Err(TerminalArtifactInterpretError::ProofDecode(_))
    ));
    assert!(matches!(
        lower_artifact_sections(
            &semantic_bytes,
            &malformed_proof,
            &AdmissionProfile::default(),
        ),
        Err(ArtifactLoweringError::ProofDecode(_))
    ));
    let empty_proof_bytes =
        encode_proof_bundle(&ProofBundle::default()).expect("canonical empty proof artifact");
    assert!(matches!(
        interpret_terminal_artifact(
            &semantic_bytes,
            &empty_proof_bytes,
            &AdmissionProfile::default(),
            &[],
        ),
        Err(TerminalArtifactInterpretError::Verification(_))
    ));
    assert!(matches!(
        lower_artifact_sections(
            &semantic_bytes,
            &empty_proof_bytes,
            &AdmissionProfile::default(),
        ),
        Err(ArtifactLoweringError::Verification(_))
    ));
    assert!(matches!(
        interpret_terminal_artifact(
            &semantic_bytes,
            &proof_bytes,
            &AdmissionProfile::default(),
            &[TerminalScalarValue::Boolean(true)],
        ),
        Err(TerminalArtifactInterpretError::Execution(
            TerminalInterpretError::ArgumentCount {
                expected: 0,
                actual: 1,
            }
        ))
    ));
    assert_eq!(first.usage().schedule().marker(), 1);
    assert_eq!(first.usage().total_units(), 3);
    assert_eq!(
        first
            .usage()
            .at(FuelChargeSite::Operation(OperationId::new(1).unwrap()))
            .unwrap()
            .units(),
        1
    );
    assert_eq!(
        first
            .usage()
            .at(FuelChargeSite::Edge(EdgeId::new(1).unwrap()))
            .unwrap()
            .units(),
        1
    );
    let mut limited_execution = TerminalExecution::start_artifact(
        &semantic_bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
        &[],
    )
    .expect("limited execution starts at the artifact boundary");
    let mut limited = TerminalFuelMeter::with_allowance(2);
    assert_eq!(
        limited_execution.resume(&mut limited).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
            schedule: TerminalFuelSchedule::CURRENT.identity(),
            site: FuelChargeSite::Edge(EdgeId::new(2).unwrap()),
            required_units: 1,
            remaining_units: 0,
        })
    );
    assert_eq!(limited.usage().total_units(), 2);

    let mut execution = TerminalExecution::start_artifact(
        &semantic_bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
        &[],
    )
    .expect("resumable execution starts at the canonical artifact boundary");
    drop(module);
    drop(bundle);
    drop(semantic_bytes);
    drop(proof_bytes);
    let mut resumable_meter = TerminalFuelMeter::with_allowance(2);
    let exhaustion = FuelExhaustion {
        schedule: TerminalFuelSchedule::CURRENT.identity(),
        site: FuelChargeSite::Edge(EdgeId::new(2).unwrap()),
        required_units: 1,
        remaining_units: 0,
    };
    assert_eq!(
        execution.resume(&mut resumable_meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(exhaustion)
    );
    resumable_meter.replenish(1).unwrap();
    assert_eq!(
        execution.resume(&mut resumable_meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(expected))
    );
    assert_eq!(resumable_meter.usage().total_units(), 3);
    assert_eq!(
        resumable_meter
            .usage()
            .at(FuelChargeSite::Operation(OperationId::new(1).unwrap()))
            .unwrap()
            .executions(),
        1,
        "resume must not replay the already charged constant"
    );
    let completed_usage = resumable_meter.usage().clone();
    assert_eq!(
        execution.resume(&mut resumable_meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(expected))
    );
    assert_eq!(resumable_meter.usage(), &completed_usage);
}

#[test]
fn verified_crashes_are_stable_terminal_outcomes() {
    let integer = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let machine = TerminalMachine {
        id: MachineId::new(90).expect("machine"),
        attachment: None,
        structural_parameters: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(ValueDeclaration {
            id: ValueId::new(90).expect("result"),
            scalar_type: ScalarType::Integer(integer),
        }),
        structural_places: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(90).expect("entry"),
        blocks: vec![Block {
            id: BlockId::new(90).expect("entry"),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Crash {
                edge: EdgeId::new(90).expect("crash edge"),
                cause: CrashCause::Trap,
                site_guard: Vec::new(),
                frontier_lower_bound: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: ContractId::new(90).expect("contract"),
            crash_routes: vec![CrashRouteBucket {
                cause: CrashCause::Trap,
                alternatives: vec![CrashRouteGuard::Truth],
            }],
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine.id,
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
        machines: vec![machine],
    };
    let semantic_bytes = encode_module(&module).expect("crash semantic artifact");
    let proof_bytes = encode_proof_bundle(&ProofBundle::default()).expect("crash proof artifact");
    let expected = TerminalCrash {
        edge: EdgeId::new(90).expect("crash edge"),
        cause: CrashCause::Trap,
        site_guard: Vec::new(),
        frontier_lower_bound: Vec::new(),
    };

    assert!(matches!(
        interpret_terminal_artifact(
            &semantic_bytes,
            &proof_bytes,
            &AdmissionProfile::default(),
            &[],
        ),
        Err(TerminalArtifactInterpretError::Execution(
            TerminalInterpretError::Crash(crash)
        )) if crash == expected
    ));

    let mut execution = TerminalExecution::start_artifact(
        &semantic_bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
        &[],
    )
    .expect("crash execution starts from its artifact");
    let mut meter = TerminalFuelMeter::unbounded();
    assert_eq!(
        execution.resume(&mut meter).expect("crash is an outcome"),
        TerminalExecutionStatus::Crashed(expected.clone())
    );
    assert_eq!(meter.usage().total_units(), 1);
    assert_eq!(
        execution
            .resume(&mut meter)
            .expect("crash remains terminal"),
        TerminalExecutionStatus::Crashed(expected)
    );
    assert_eq!(meter.usage().total_units(), 1, "crash must not replay");
}

#[test]
fn interpreter_rejects_an_out_of_range_integer_argument() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let parameter = ValueId::new(10).expect("parameter");
    let result = ValueId::new(11).expect("result");
    let machine = TerminalMachine {
        id: MachineId::new(10).expect("machine"),
        attachment: None,
        structural_parameters: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: vec![ValueDeclaration {
            id: parameter,
            scalar_type,
        }],
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(ValueDeclaration {
            id: result,
            scalar_type,
        }),
        structural_places: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(10).expect("entry"),
        blocks: vec![Block {
            id: BlockId::new(10).expect("entry"),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Return {
                cleanup_actions: Vec::new(),
                edge: EdgeId::new(10).expect("return"),
                value: parameter,
            },
        }],
        contract: MachineContract {
            id: ContractId::new(10).expect("contract"),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine.id,
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
        machines: vec![machine],
    };
    let semantic_bytes = encode_module(&module).expect("parameter semantic artifact");
    let proof_bytes =
        encode_proof_bundle(&ProofBundle::default()).expect("parameter proof artifact");

    assert!(matches!(
        interpret_terminal_artifact(
            &semantic_bytes,
            &proof_bytes,
            &AdmissionProfile::default(),
            &[TerminalScalarValue::Integer {
                scalar_type: integer,
                value: IntegerValue::Unsigned(300),
            }],
        ),
        Err(TerminalArtifactInterpretError::Execution(
            TerminalInterpretError::ArgumentIntegerOutsideType { value }
        )) if value == parameter
    ));
}
