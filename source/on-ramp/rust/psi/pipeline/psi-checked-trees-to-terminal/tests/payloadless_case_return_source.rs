use psi_proof_admission::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::{OperationKind, OperationResult, StructuralTypeShape, Terminator};
use psi_terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_bundle};
use psi_terminal_fixed_fuel::derive_fixed_entry_fuel;
use psi_terminal_fuel::TerminalFuelMeter;
use psi_terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalPayloadlessCaseResult, TerminalPayloadlessCaseValue,
};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

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

fn checked_source() -> psi_checked_trees::CheckedTrees {
    checked(SOURCE)
}

fn checked(source: &str) -> psi_checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

const GUARDED_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}

    data Outcome [copy] {
        case Success;
        case Failure;
    }
    data Root {}

    machine Root::choose() -> Outcome
    ensures Outcome::Success -> { selected: ready(); true; }
    ensures Outcome::Failure -> { skipped: ready(); true; }
    {
        selected = ConcreteEvidence;
        Outcome::Success
    }
"#;

#[test]
fn exact_payloadless_case_return_is_canonical_verified_and_executable() {
    let checked = checked_source();
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::choose")
        .expect("the exact payloadless case producer lowers");
    let module = &lowered.semantic_module;
    let [machine] = module.machines.as_slice() else {
        panic!("the source producer lowers to one terminal machine")
    };
    let [block] = machine.blocks.as_slice() else {
        panic!("the source producer lowers to one terminal block")
    };
    let [operation] = block.operations.as_slice() else {
        panic!("payloadless construction is one exact structural operation")
    };
    let OperationResult::Structural(operation_result) = &operation.result else {
        panic!("the payloadless case operation must establish a structural place")
    };
    assert!(operation_result.qualifications.is_empty());
    assert!(operation_result.claims.is_empty());
    let OperationKind::EstablishPayloadlessCase { result_case } = &operation.kind else {
        panic!("the source case constructor must remain exact")
    };
    let result_type = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == operation_result.structural_type)
        .expect("the operation result type is declared");
    let StructuralTypeShape::Sum { cases } = &result_type.shape else {
        panic!("the operation result remains a sum")
    };
    assert_eq!(
        cases
            .iter()
            .find(|case| case.id == *result_case)
            .map(|case| case.identity.as_str()),
        Some("Success")
    );
    let Terminator::ReturnStructural {
        source,
        returned_claims,
        trivial_affine_discards,
        ..
    } = &block.terminator
    else {
        panic!("the constructed place returns through structural custody")
    };
    assert_eq!(*source, operation_result.place);
    assert!(returned_claims.is_empty());
    assert!(trivial_affine_discards.is_empty());

    let bytes = encode_module(module).expect("payloadless case semantics encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    let verified = psi_terminal_verifier::verify_module(
        module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("payloadless case construction verifies independently");
    let fixed = derive_fixed_entry_fuel(&verified, module.entry)
        .expect("the exact source producer has fixed fuel");
    assert_eq!(fixed.ceiling_units(), 2);

    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof bundle encodes");
    let mut execution =
        TerminalExecution::start_artifact(&bytes, &proof, &AdmissionProfile::default(), &[])
            .expect("the payloadless case artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(0);
    assert!(matches!(
        execution
            .resume(&mut meter)
            .expect("operation exhaustion is resumable"),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    meter.replenish(1).expect("fund the case constructor");
    assert!(matches!(
        execution
            .resume(&mut meter)
            .expect("return exhaustion is resumable"),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    meter.replenish(1).expect("fund the return edge");
    assert_eq!(
        execution
            .resume(&mut meter)
            .expect("the case return completes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::PayloadlessCase(
            TerminalPayloadlessCaseResult {
                value: TerminalPayloadlessCaseValue {
                    structural_type: operation_result.structural_type,
                    result_case: *result_case,
                },
            }
        ))
    );
}

#[test]
fn guarded_payloadless_case_return_retains_active_evidence_and_vacuous_siblings() {
    let baseline = psi_checked_trees_to_terminal::lower_machine(&checked_source(), "Root::choose")
        .expect("the proof-free payloadless producer lowers");
    let checked = checked(GUARDED_SOURCE);
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::choose")
        .expect("the exact guarded payloadless producer lowers");
    let module = &lowered.semantic_module;
    let [machine] = module.machines.as_slice() else {
        panic!("the guarded producer lowers to one machine")
    };
    assert_eq!(machine.contract.outcome_specific_ensures.len(), 4);
    let success_case = match &module.structural_types[0].shape {
        StructuralTypeShape::Sum { cases } => {
            cases
                .iter()
                .find(|case| case.identity == "Success")
                .expect("Success case")
                .id
        }
        _ => panic!("Outcome remains a sum"),
    };
    let failure_case = match &module.structural_types[0].shape {
        StructuralTypeShape::Sum { cases } => {
            cases
                .iter()
                .find(|case| case.identity == "Failure")
                .expect("Failure case")
                .id
        }
        _ => panic!("Outcome remains a sum"),
    };
    let success = machine
        .contract
        .outcome_specific_ensures
        .iter()
        .filter(|row| row.guard.result_case == success_case)
        .collect::<Vec<_>>();
    assert_eq!(success.len(), 2);
    assert_eq!((success[0].position, success[1].position), (0, 1));
    assert!(matches!(
        success[0].proposition,
        psi_core::Proposition::Atom(_)
    ));
    assert_eq!(
        success[0]
            .evidence
            .as_ref()
            .map(|evidence| evidence.output_field.as_str()),
        Some("selected")
    );
    assert_eq!(success[1].proposition, psi_core::Proposition::Truth);
    assert!(success[1].evidence.is_none());
    let failure_rows = machine
        .contract
        .outcome_specific_ensures
        .iter()
        .filter(|row| row.guard.result_case == failure_case)
        .collect::<Vec<_>>();
    let [failure_named, failure_truth] = failure_rows.as_slice() else {
        panic!("two retained vacuous Failure rows")
    };
    assert!(matches!(
        failure_named.proposition,
        psi_core::Proposition::Atom(_)
    ));
    assert_eq!(
        failure_named
            .evidence
            .as_ref()
            .map(|evidence| evidence.output_field.as_str()),
        Some("skipped")
    );
    assert_eq!(failure_truth.proposition, psi_core::Proposition::Truth);
    assert!(failure_truth.evidence.is_none());
    assert!(module.evidence_contract_lanes.is_empty());
    assert_eq!(module.evidence_terms.len(), 2);
    assert_eq!(lowered.proof_bundle.evidence_producers.len(), 1);
    assert_eq!(lowered.proof_bundle.evidence.len(), 1);
    assert_eq!(
        lowered.proof_bundle.evidence[0].obligation,
        success[1].obligation
    );

    assert_eq!(machine.blocks, baseline.semantic_module.machines[0].blocks);
    let module_bytes = encode_module(module).expect("encode guarded module");
    assert_eq!(decode_module(&module_bytes), Ok(module.clone()));
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle).expect("encode guarded proof");
    assert_eq!(
        decode_proof_bundle(&proof_bytes),
        Ok(lowered.proof_bundle.clone())
    );
    let verified = psi_terminal_verifier::verify_module(
        module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("matching evidence and truth verify while Failure is vacuous");
    assert_eq!(
        verified.reconstructed_obligations().obligations().len(),
        1,
        "only the active unnamed truth row is a logical obligation"
    );
    assert_eq!(
        derive_fixed_entry_fuel(&verified, module.entry)
            .expect("guarded producer has fixed fuel")
            .ceiling_units(),
        2
    );
    let mut execution = TerminalExecution::start_artifact(
        &module_bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
        &[],
    )
    .expect("the guarded payloadless artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(2);
    assert_eq!(
        execution
            .resume(&mut meter)
            .expect("guarded producer completes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::PayloadlessCase(
            TerminalPayloadlessCaseResult {
                value: TerminalPayloadlessCaseValue {
                    structural_type: machine.result.structural().unwrap().structural_type,
                    result_case: success_case,
                },
            }
        ))
    );

    let mut missing_producer = lowered.proof_bundle.clone();
    missing_producer.evidence_producers.clear();
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            module,
            &missing_producer,
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::MissingEvidenceProducer(_))
    ));

    let mut vacuous_producer = lowered.proof_bundle.clone();
    vacuous_producer.evidence_producers[0].term = failure_named
        .evidence
        .as_ref()
        .expect("named Failure endpoint")
        .term;
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            module,
            &vacuous_producer,
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::UnusedEvidenceProducerTerm(_))
    ));

    let mut changed_case = module.clone();
    let OperationKind::EstablishPayloadlessCase { result_case } =
        &mut changed_case.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *result_case = failure_case;
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &changed_case,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::UnusedEvidenceProducerTerm(_))
    ));
    let mut changed_case_bundle = lowered.proof_bundle.clone();
    changed_case_bundle.evidence_producers[0].term = failure_named
        .evidence
        .as_ref()
        .expect("named Failure endpoint")
        .term;
    changed_case_bundle.evidence[0].obligation = failure_truth.obligation;
    psi_terminal_verifier::verify_module(
        &changed_case,
        &changed_case_bundle,
        &AdmissionProfile::default(),
    )
    .expect("changing the exact constructor swaps the active proof and producer set");

    let mut missing_truth = lowered.proof_bundle.clone();
    missing_truth.evidence.clear();
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            module,
            &missing_truth,
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::MissingEvidence(
            obligation
        ))
            if obligation == success[1].obligation
    ));
}
