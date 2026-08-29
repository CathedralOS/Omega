use psi_proof_admission::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::{OperationKind, OperationResult, StructuralTypeShape, Terminator};
use psi_terminal_codec::{
    CodecError, decode_module, decode_proof_bundle, encode_module, encode_proof_bundle,
};
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

const OMITTED_GUARDED_CALL_SOURCE: &str = r#"
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
            Outcome::Success { } -> saved
            Outcome::Failure { } -> saved
        }
    }
"#;

const MULTI_SELECTED_GUARDED_CALL_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}
    data Outcome [copy] { case Success; case Failure; }
    data Root {}

    machine Root::produce() -> Outcome
    ensures Outcome::Success -> { first: ready(); second: ready(); true; }
    ensures Outcome::Failure -> { sibling: ready(); }
    {
        first = ConcreteEvidence;
        second = ConcreteEvidence;
        Outcome::Success
    }

    machine Root::caller() -> Outcome {
        let saved: Outcome = Root::produce();
        transition saved {
            Outcome::Success { ; second: local_second, first: local_first } -> saved
            Outcome::Failure { } -> saved
        }
    }
"#;

#[test]
fn guarded_payloadless_source_call_rejoins_selected_evidence_and_uses_four_fuel() {
    let checked = checked(GUARDED_CALL_SOURCE);
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::caller")
        .expect("the exact guarded source call lowers");
    let module = &lowered.semantic_module;
    let [caller, callee] = module.machines.as_slice() else {
        panic!("the guarded source call retains caller and callee")
    };
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &caller.blocks[0].operations[0].kind
    else {
        panic!("the guarded source call publishes its selected row")
    };
    let [selected] = selected_evidence.as_slice() else {
        panic!("the guarded source call publishes exactly one selected row")
    };
    let callee_row = callee
        .contract
        .outcome_specific_ensures
        .iter()
        .find(|row| {
            row.evidence
                .as_ref()
                .is_some_and(|evidence| evidence.output_field == "selected")
        })
        .expect("the producer named guarded row remains on the callee");
    assert_eq!(selected.guard, callee_row.guard);
    assert_eq!(selected.position, callee_row.position);
    assert_eq!(selected.callee_obligation, callee_row.obligation);
    assert_eq!(
        selected.callee_term,
        callee_row.evidence.as_ref().unwrap().term
    );
    assert_eq!(selected.output_field, "selected");
    assert_ne!(selected.output, selected.callee_term);
    let callee_term = module
        .evidence_terms
        .iter()
        .find(|term| term.id == selected.callee_term)
        .unwrap();
    let output_term = module
        .evidence_terms
        .iter()
        .find(|term| term.id == selected.output)
        .unwrap();
    assert_eq!(callee_term.proposition, selected.proposition);
    assert_eq!(output_term.proposition, selected.proposition);
    assert_eq!(callee_term.interface, output_term.interface);
    assert_eq!(
        selected.validity.result,
        caller.blocks[0].operations[0]
            .result
            .structural()
            .unwrap()
            .place
    );
    assert_eq!(
        selected.validity.proposition_dependencies,
        [selected.validity.result]
    );
    assert!(selected.validity.interface_dependencies.is_empty());
    assert_eq!(module.evidence_terms.len(), 3);
    assert!(module.evidence_contract_lanes.is_empty());
    assert!(module.proof_output_calls.is_empty());
    assert_eq!(lowered.proof_bundle.evidence_producers.len(), 1);
    assert_eq!(lowered.proof_bundle.evidence.len(), 1);

    let bytes = encode_module(module).expect("guarded caller semantics encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("guarded caller proof encodes");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let verified = psi_terminal_verifier::verify_module(
        module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the selected guarded call verifies independently");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, module.entry)
            .expect("the direct guarded call has fixed fuel")
            .ceiling_units(),
        4
    );

    let mut execution =
        TerminalExecution::start_artifact(&bytes, &proof, &AdmissionProfile::default(), &[])
            .expect("the guarded caller artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(4);
    assert_eq!(
        execution
            .resume(&mut meter)
            .expect("guarded caller completes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::PayloadlessCase(
            TerminalPayloadlessCaseResult {
                value: TerminalPayloadlessCaseValue {
                    structural_type: caller.result.structural().unwrap().structural_type,
                    result_case: selected.guard.result_case,
                },
            }
        ))
    );

    let mut tampered = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut tampered.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let [selected] = selected_evidence.as_mut_slice() else {
        unreachable!()
    };
    selected.position = selected.position.checked_add(1).unwrap();
    assert!(psi_terminal_verifier::validate_module(&tampered).is_err());

    let mut lost_selection = checked.clone();
    lost_selection
        .facts
        .flow
        .terminal_structural_call_returns
        .payloadless_guarded_machines[0]
        .selected_evidence
        .clear();
    assert!(psi_checked_trees_to_terminal::lower_machine(&lost_selection, "Root::caller").is_err());

    let mut wrong_arm = checked.clone();
    wrong_arm
        .facts
        .flow
        .terminal_structural_call_returns
        .payloadless_guarded_machines[0]
        .selected_evidence[0]
        .arm_statement_index += 1;
    assert!(psi_checked_trees_to_terminal::lower_machine(&wrong_arm, "Root::caller").is_err());

    let sibling_guarantee = checked
        .facts
        .proof
        .outcome_specific_guarantees
        .iter()
        .find_map(|(handle, guarantee)| {
            (guarantee.public_selector.as_deref() == Some("sibling")).then_some(handle)
        })
        .unwrap();
    let mut wrong_guarantee = checked.clone();
    wrong_guarantee
        .facts
        .flow
        .terminal_structural_call_returns
        .payloadless_guarded_machines[0]
        .selected_evidence[0]
        .guarantee = sibling_guarantee;
    assert!(
        psi_checked_trees_to_terminal::lower_machine(&wrong_guarantee, "Root::caller").is_err()
    );

    let selected_arm = checked
        .facts
        .proof
        .outcome_specific_arms
        .iter()
        .find_map(|(handle, arm)| {
            arm.rows
                .iter()
                .any(|row| row.selected_term.is_some())
                .then_some(handle)
        })
        .unwrap();
    let mut wider_validity = checked.clone();
    let arm = wider_validity
        .facts
        .proof
        .outcome_specific_arms
        .get_mut(selected_arm);
    let row = arm
        .rows
        .iter_mut()
        .find(|row| row.selected_term.is_some())
        .unwrap();
    row.validity
        .referenced_occurrences
        .push(row.validity.result_occurrence);
    assert!(psi_checked_trees_to_terminal::lower_machine(&wider_validity, "Root::caller").is_err());
}

#[test]
fn guarded_payloadless_source_call_retains_a_canonical_selected_subset_without_runtime_cost() {
    let checked = checked(MULTI_SELECTED_GUARDED_CALL_SOURCE);
    let [checked_plan] = checked
        .facts
        .flow
        .terminal_structural_call_returns
        .payloadless_guarded_machines
        .as_slice()
    else {
        panic!("the checked carrier retains one multi-selection call")
    };
    assert_eq!(checked_plan.selected_evidence.len(), 2);

    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::caller")
        .expect("the canonical multi-selection guarded call lowers");
    let [caller, callee] = lowered.semantic_module.machines.as_slice() else {
        panic!("the guarded source call retains caller and callee")
    };
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &caller.blocks[0].operations[0].kind
    else {
        panic!("the source call remains one structural call")
    };
    assert_eq!(selected_evidence.len(), 2);
    assert_eq!(
        selected_evidence
            .iter()
            .map(|binding| (binding.position, binding.output_field.as_str()))
            .collect::<Vec<_>>(),
        [(0, "first"), (1, "second")],
        "caller selector spelling order does not perturb canonical callee-row order"
    );
    assert!(
        selected_evidence
            .windows(2)
            .all(|rows| rows[0].output != rows[1].output)
    );
    for binding in selected_evidence {
        let row = callee
            .contract
            .outcome_specific_ensures
            .iter()
            .find(|row| row.guard == binding.guard && row.position == binding.position)
            .expect("every selected row rejoins one exact callee guarantee");
        let evidence = row.evidence.as_ref().expect("selected row is named");
        assert_eq!(binding.callee_obligation, row.obligation);
        assert_eq!(binding.callee_term, evidence.term);
        assert_eq!(binding.output_field, evidence.output_field);
        assert_ne!(binding.output, binding.callee_term);
        assert_eq!(
            binding.validity.proposition_dependencies,
            [binding.validity.result]
        );
    }
    assert_eq!(caller.blocks[0].operations.len(), 1);
    assert_eq!(lowered.semantic_module.evidence_terms.len(), 5);
    assert_eq!(lowered.proof_bundle.evidence_producers.len(), 2);

    let bytes = encode_module(&lowered.semantic_module).expect("multi-selection module encodes");
    assert_eq!(decode_module(&bytes), Ok(lowered.semantic_module.clone()));
    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("multi-selection guarded call verifies");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
            .unwrap()
            .ceiling_units(),
        4,
        "two erased selections add no runtime charge"
    );

    let mut reordered = lowered.semantic_module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut reordered.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence.swap(0, 1);
    assert!(matches!(
        encode_module(&reordered),
        Err(CodecError::NonCanonicalOrder(
            "guarded-call selections or validity dependency roots"
        ))
    ));

    let mut duplicated = lowered.semantic_module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut duplicated.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence[1] = selected_evidence[0].clone();
    assert!(psi_terminal_verifier::validate_module(&duplicated).is_err());
}

#[test]
fn omitted_guarded_selector_retains_fact_only_callee_without_runtime_delta() {
    let omitted = psi_checked_trees_to_terminal::lower_machine(
        &checked(OMITTED_GUARDED_CALL_SOURCE),
        "Root::caller",
    )
    .expect("the exact omitted-selector guarded call lowers");
    let [caller, callee] = omitted.semantic_module.machines.as_slice() else {
        panic!("the omitted-selector call retains caller and callee")
    };
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &caller.blocks[0].operations[0].kind
    else {
        panic!("omission does not mint caller evidence")
    };
    assert!(
        selected_evidence.is_empty(),
        "omission mints no caller evidence"
    );
    assert_eq!(callee.contract.outcome_specific_ensures.len(), 3);
    assert_eq!(omitted.semantic_module.evidence_terms.len(), 2);
    assert_eq!(omitted.proof_bundle.evidence_producers.len(), 1);
    assert_eq!(omitted.proof_bundle.evidence.len(), 1);
    assert!(omitted.semantic_module.evidence_contract_lanes.is_empty());
    assert!(omitted.semantic_module.proof_output_calls.is_empty());

    let selected =
        psi_checked_trees_to_terminal::lower_machine(&checked(GUARDED_CALL_SOURCE), "Root::caller")
            .expect("selected comparison lowers");
    let mut selected_blocks = selected
        .semantic_module
        .machines
        .iter()
        .map(|machine| machine.blocks.clone())
        .collect::<Vec<_>>();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut selected_blocks[0][0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence.clear();
    let omitted_blocks = omitted
        .semantic_module
        .machines
        .iter()
        .map(|machine| machine.blocks.clone())
        .collect::<Vec<_>>();
    assert_eq!(selected_blocks, omitted_blocks);

    let success_case = match &omitted.semantic_module.structural_types[0].shape {
        StructuralTypeShape::Sum { cases } => {
            cases
                .iter()
                .find(|case| case.identity == "Success")
                .unwrap()
                .id
        }
        _ => panic!("Outcome remains a sum"),
    };
    let selected_rows = callee
        .contract
        .outcome_specific_ensures
        .iter()
        .filter(|row| row.guard.result_case == success_case)
        .collect::<Vec<_>>();
    assert_eq!(selected_rows.len(), 2);
    assert!(selected_rows.iter().any(|row| {
        row.evidence
            .as_ref()
            .is_some_and(|evidence| evidence.output_field == "selected")
    }));
    assert!(callee.contract.outcome_specific_ensures.iter().any(|row| {
        row.guard.result_case != success_case
            && row
                .evidence
                .as_ref()
                .is_some_and(|evidence| evidence.output_field == "sibling")
    }));
    let verified = psi_terminal_verifier::verify_module(
        &omitted.semantic_module,
        &omitted.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the omitted-selector guarded call verifies");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, omitted.semantic_module.entry)
            .unwrap()
            .ceiling_units(),
        4
    );
    let bytes = encode_module(&omitted.semantic_module).unwrap();
    let proof = encode_proof_bundle(&omitted.proof_bundle).unwrap();
    let mut execution =
        TerminalExecution::start_artifact(&bytes, &proof, &AdmissionProfile::default(), &[])
            .unwrap();
    assert_eq!(
        execution
            .resume(&mut TerminalFuelMeter::with_allowance(4))
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::PayloadlessCase(
            TerminalPayloadlessCaseResult {
                value: TerminalPayloadlessCaseValue {
                    structural_type: caller.result.structural().unwrap().structural_type,
                    result_case: success_case,
                },
            }
        ))
    );
}

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
