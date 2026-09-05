//! Real-source canaries for exact payloadless structural-call optimizer custody.

use abstract_operations::AbstractOperation;
use abstract_operations_to_abstract_operations::validation::validate_verified_psi_optimization_unit;
use abstract_operations_to_target_operations::{
    LoweringError as TargetLoweringError, lower_to_target_operations,
};
use checked_trees_to_lowered_psi::lower_machine;
use optimization_unit::recompute_psi_optimization_unit_identity;
use optimization_unit_semantics::{
    OptimizationUnitValidationError, validate_psi_optimization_unit,
};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{
    EvidenceTermId, ObligationId, PlaceId, Proposition, ScalarTerm, StructuralCaseId,
};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use target::NativeTarget;
use terminal_codec::{encode_module, encode_proof_bundle};
use terminal_fuel::TerminalFuelSchedule;
use terminal_psi_to_abstract_operations::{
    ArtifactLoweringError, LoweringError, build_verified_psi_optimization_unit,
    lower_artifact_sections, lower_artifact_sections_for_optimization,
};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

const SOURCE: &str = r#"
    data Outcome [copy] {
        case Success;
        case Failure;
    }
    data Root {}

    machine Root::choose() -> Outcome {
        Outcome::Success
    }
"#;

const GUARDED_CALL_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}
    data Outcome [copy] { case Success; case Failure; }
    data Root {}

    machine Root::produce() -> Outcome
    ensures Outcome::Success -> { selected: ready(); true; }
    ensures Outcome::Failure -> { sibling: ready(); }
    { selected = ConcreteEvidence; Outcome::Success }

    machine Root::caller() -> Outcome {
        let saved: Outcome = Root::produce();
        transition saved {
            Outcome::Success { ; selected: local } -> saved
            Outcome::Failure { } -> saved
        }
    }
"#;

fn lowered_source(source: &str, machine: &str) -> lowered_psi::LoweredPsi {
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed).expect("check source");
    lower_machine(&checked, machine).expect("lower exact payloadless source")
}

fn optimizer_unit(
    lowered: &lowered_psi::LoweredPsi,
) -> terminal_psi_to_abstract_operations::VerifiedPsiOptimizationUnit {
    let semantic = encode_module(&lowered.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("encode proof");
    let input =
        lower_artifact_sections_for_optimization(&semantic, &proof, &AdmissionProfile::default())
            .expect("optimizer-only lowering retains exact payloadless semantics");
    build_verified_psi_optimization_unit(input, TerminalFuelSchedule::CURRENT.identity())
        .expect("build verified payloadless optimization unit")
}

fn refresh(unit: &mut optimization_unit::PsiOptimizationUnit) {
    unit.identity = recompute_psi_optimization_unit_identity(unit);
}

fn assert_structural_call_rejects(mut unit: optimization_unit::PsiOptimizationUnit) {
    refresh(&mut unit);
    assert!(matches!(
        validate_psi_optimization_unit(&unit),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { .. })
    ));
}

#[test]
fn source_payloadless_producer_enters_optimizer_while_ordinary_lowering_stays_fenced() {
    let lowered = lowered_source(SOURCE, "Root::choose");
    let semantic = encode_module(&lowered.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("encode proof");
    assert!(matches!(
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default()),
        Err(ArtifactLoweringError::Lowering(
            LoweringError::UnsupportedPayloadlessCase(_)
        ))
    ));

    let optimizer_input =
        lower_artifact_sections_for_optimization(&semantic, &proof, &AdmissionProfile::default())
            .expect("optimizer-only lowering retains the exact producer");
    assert!(matches!(
        lower_to_target_operations(optimizer_input.plan(), NativeTarget::linux_x64()),
        Err(TargetLoweringError::UnsupportedStructuralReturn(_))
    ));

    let verified = optimizer_unit(&lowered);
    validate_verified_psi_optimization_unit(&verified)
        .expect("the exact source producer passes optimizer admission");
    assert!(matches!(
        verified.unit().functions[0].blocks[0].nodes[0].operation,
        AbstractOperation::EstablishPayloadlessCase { .. }
    ));
}

#[test]
fn guarded_source_call_replays_exact_classifier_and_rejects_independent_corruption() {
    let lowered = lowered_source(GUARDED_CALL_SOURCE, "Root::caller");
    let verified = optimizer_unit(&lowered);
    validate_verified_psi_optimization_unit(&verified)
        .expect("the exact guarded payloadless call passes optimizer admission");
    let (_, baseline) = verified.into_parts();

    let caller_index = baseline
        .functions
        .iter()
        .position(|function| {
            function
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .any(|node| matches!(node.operation, AbstractOperation::CallStructural { .. }))
        })
        .expect("source retains its caller");
    let callee_index = baseline
        .functions
        .iter()
        .position(|function| {
            function
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .any(|node| {
                    matches!(
                        node.operation,
                        AbstractOperation::EstablishPayloadlessCase { .. }
                    )
                })
        })
        .expect("source retains its direct producer");

    let call = baseline.functions[caller_index]
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| match &node.operation {
            AbstractOperation::CallStructural {
                selected_evidence, ..
            } => Some(selected_evidence),
            _ => None,
        })
        .expect("caller retains its structural call");
    assert_eq!(
        call.len(),
        1,
        "guarded source retains one selected-evidence row"
    );
    assert!(baseline.functions[callee_index].verified_contract.is_some());
    assert!(
        baseline.functions[callee_index]
            .evidence_contract_lanes
            .is_empty()
    );

    let mut obligation = baseline.clone();
    let AbstractOperation::CallStructural {
        requirement_obligations,
        ..
    } = &mut obligation.functions[caller_index].blocks[0].nodes[0].operation
    else {
        panic!("caller begins with its structural call")
    };
    requirement_obligations.push(ObligationId::new(99_001).unwrap());
    assert_structural_call_rejects(obligation);

    let mut returned_transfer = baseline.clone();
    let AbstractOperation::CallStructural {
        returned_claim_transfers,
        ..
    } = &mut returned_transfer.functions[caller_index].blocks[0].nodes[0].operation
    else {
        panic!("caller begins with its structural call")
    };
    returned_claim_transfers.push(terminal_psi::StructuralResultClaimTransfer {
        callee_claim: semantic_vocabulary::ClaimId::new(99_004).unwrap(),
        caller_claim: semantic_vocabulary::ClaimId::new(99_005).unwrap(),
    });
    assert_structural_call_rejects(returned_transfer);

    let mut crash_continuation = baseline.clone();
    let AbstractOperation::CallStructural {
        crash_continuations,
        ..
    } = &mut crash_continuation.functions[caller_index].blocks[0].nodes[0].operation
    else {
        panic!("caller begins with its structural call")
    };
    crash_continuations.push(terminal_psi::CrashRouteBucket {
        cause: terminal_psi::CrashCause::Trap,
        alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
    });
    assert_structural_call_rejects(crash_continuation);

    let mut missing_contract = baseline.clone();
    missing_contract.functions[callee_index].verified_contract = None;
    assert_structural_call_rejects(missing_contract);

    for mutate in [
        |contract: &mut terminal_psi::MachineContract| {
            contract.requires.push(Proposition::Truth);
        },
        |contract: &mut terminal_psi::MachineContract| {
            contract.ensures.push(terminal_psi::ContractClause {
                obligation: ObligationId::new(99_006).unwrap(),
                proposition: Proposition::Truth,
            });
        },
        |contract: &mut terminal_psi::MachineContract| {
            contract.crash_routes.push(terminal_psi::CrashRouteBucket {
                cause: terminal_psi::CrashCause::Abort,
                alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
            });
        },
    ] {
        let mut contract_lane = baseline.clone();
        mutate(
            contract_lane.functions[callee_index]
                .verified_contract
                .as_mut()
                .unwrap(),
        );
        assert_structural_call_rejects(contract_lane);
    }

    let mut evidence_lane = baseline.clone();
    let callee = evidence_lane.functions[callee_index].machine;
    evidence_lane.functions[callee_index]
        .evidence_contract_lanes
        .push(terminal_psi::EvidenceContractLane {
            machine: callee,
            kind: terminal_psi::EvidenceContractLaneKind::Requires,
            position: 0,
            term: EvidenceTermId::new(99_007).unwrap(),
            output_field: None,
        });
    assert_structural_call_rejects(evidence_lane);

    let mut invalid_case = baseline.clone();
    let producer = invalid_case.functions[callee_index]
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.nodes)
        .find(|node| {
            matches!(
                node.operation,
                AbstractOperation::EstablishPayloadlessCase { .. }
            )
        })
        .expect("callee retains its case producer");
    let AbstractOperation::EstablishPayloadlessCase { result_case, .. } = &mut producer.operation
    else {
        unreachable!()
    };
    *result_case = StructuralCaseId::new(99_002).unwrap();
    assert_structural_call_rejects(invalid_case);

    let mut foreign_root = baseline.clone();
    foreign_root.functions[callee_index]
        .verified_contract
        .as_mut()
        .unwrap()
        .outcome_specific_ensures[0]
        .proposition = Proposition::Equal(
        ScalarTerm::BooleanField {
            root: PlaceId::new(99_003).unwrap(),
            path: Vec::new(),
        },
        ScalarTerm::Boolean(true),
    );
    assert_structural_call_rejects(foreign_root);

    let mut selected_without_rows = baseline;
    selected_without_rows.functions[callee_index]
        .verified_contract
        .as_mut()
        .unwrap()
        .outcome_specific_ensures
        .clear();
    assert_structural_call_rejects(selected_without_rows);
}
