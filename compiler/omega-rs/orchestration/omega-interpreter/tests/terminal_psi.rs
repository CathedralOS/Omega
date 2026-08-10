use omega_interpreter::{
    TerminalArtifactInterpretError, TerminalCrash, TerminalExecution, TerminalExecutionStatus,
    TerminalInterpretError, TerminalScalarValue, interpret_terminal, interpret_terminal_artifact,
    interpret_terminal_artifact_measured, interpret_terminal_measured,
    interpret_terminal_with_meter,
};
use omega_terminal_psi_to_abstract_operations::{
    ArtifactLoweringError, lower_artifact_sections, lower_verified_module,
};
use psi_core::{
    BlockId, ContractId, EdgeId, EvidenceIdentity, IntegerSign, IntegerType, IntegerValue,
    MachineId, ObligationId, OperationId, Proposition, ScalarTerm, ScalarType, ValueId,
};
use psi_proof_kernel::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemMarker,
};
use psi_terminal::{
    Block, ContractClause, CrashCause, CrashRouteBucket, CrashRouteGuard, MachineContract,
    Operation, OperationKind, TerminalMachine, TerminalModule, Terminator, ValueDeclaration,
    VocabularyMarker,
};
use psi_terminal_codec::{encode_module, encode_proof_bundle};
use psi_terminal_fuel::{
    FuelChargeSite, FuelExhaustion, FuelMeterError, TerminalFuelMeter, TerminalFuelSchedule,
};
use psi_terminal_verifier::{ObligationEvidence, ProofBundle, verify_module};

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
        parameters: Vec::new(),
        result: ValueDeclaration {
            id: result,
            scalar_type,
        },
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
                    result: ValueDeclaration {
                        id: constant,
                        scalar_type,
                    },
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Signed(7),
                    },
                }],
                terminator: Terminator::Jump {
                    edge: EdgeId::new(1).expect("jump"),
                    target: BlockId::new(2).expect("exit"),
                    arguments: vec![constant],
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
        },
    };
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine.id,
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
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
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(1).expect("certificate"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof,
            }),
        }],
    };
    let verified = verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("module verifies without source or producer state");

    let expected = TerminalScalarValue::Integer {
        scalar_type: integer,
        value: IntegerValue::Signed(7),
    };
    let first = interpret_terminal_measured(&verified, &[])
        .expect("verified module executes with deterministic usage");
    let second = interpret_terminal_measured(&verified, &[])
        .expect("equal execution reproduces deterministic usage");
    assert_eq!(first, second);
    assert_eq!(first.value(), expected);
    let semantic_bytes = encode_module(&module).expect("canonical semantic artifact");
    let proof_bytes = encode_proof_bundle(&bundle).expect("canonical proof artifact");
    let direct_abstract = lower_verified_module(&verified).expect("lower verified terminal Psi");
    let artifact_abstract =
        lower_artifact_sections(&semantic_bytes, &proof_bytes, &AdmissionProfile::default())
            .expect("artifact-root abstract lowering decodes and verifies first");
    assert_eq!(artifact_abstract, direct_abstract);
    let artifact = interpret_terminal_artifact_measured(
        &semantic_bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
        &[],
    )
    .expect("artifact-root interpretation decodes and verifies before execution");
    assert_eq!(artifact, first);
    assert_eq!(
        interpret_terminal_artifact(
            &semantic_bytes,
            &proof_bytes,
            &AdmissionProfile::default(),
            &[],
        )
        .expect("unmeasured artifact entry returns the same result"),
        expected
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
    assert_eq!(interpret_terminal(&verified, &[]).unwrap(), expected);

    let mut limited = TerminalFuelMeter::with_allowance(2);
    assert_eq!(
        interpret_terminal_with_meter(&verified, &[], &mut limited),
        Err(TerminalInterpretError::Fuel(FuelMeterError::Exhausted(
            FuelExhaustion {
                schedule: TerminalFuelSchedule::CURRENT.identity(),
                site: FuelChargeSite::Edge(EdgeId::new(2).unwrap()),
                required_units: 1,
                remaining_units: 0,
            }
        )))
    );
    assert_eq!(limited.usage().total_units(), 2);

    let mut execution = TerminalExecution::start_artifact(
        &semantic_bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
        &[],
    )
    .expect("resumable execution starts at the canonical artifact boundary");
    drop(verified);
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
        TerminalExecutionStatus::Complete(expected)
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
        TerminalExecutionStatus::Complete(expected)
    );
    assert_eq!(resumable_meter.usage(), &completed_usage);
}

#[test]
fn verified_crashes_are_stable_terminal_outcomes() {
    let integer = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let machine = TerminalMachine {
        id: MachineId::new(90).expect("machine"),
        parameters: Vec::new(),
        result: ValueDeclaration {
            id: ValueId::new(90).expect("result"),
            scalar_type: ScalarType::Integer(integer),
        },
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
        },
    };
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine.id,
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![machine],
    };
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("the explicit crash exit verifies");
    let expected = TerminalCrash {
        edge: EdgeId::new(90).expect("crash edge"),
        cause: CrashCause::Trap,
        site_guard: Vec::new(),
        frontier_lower_bound: Vec::new(),
    };

    assert_eq!(
        interpret_terminal(&verified, &[]),
        Err(TerminalInterpretError::Crash(expected.clone()))
    );

    let mut execution = TerminalExecution::start(&verified, &[]).expect("execution starts");
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
        parameters: vec![ValueDeclaration {
            id: parameter,
            scalar_type,
        }],
        result: ValueDeclaration {
            id: result,
            scalar_type,
        },
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
                edge: EdgeId::new(10).expect("return"),
                value: parameter,
            },
        }],
        contract: MachineContract {
            id: ContractId::new(10).expect("contract"),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
        },
    };
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine.id,
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![machine],
    };
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("parameter-return module verifies");

    assert_eq!(
        interpret_terminal(
            &verified,
            &[TerminalScalarValue::Integer {
                scalar_type: integer,
                value: IntegerValue::Unsigned(300),
            }],
        ),
        Err(TerminalInterpretError::ArgumentIntegerOutsideType { value: parameter })
    );
}
