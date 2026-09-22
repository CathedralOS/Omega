use super::{
    GUARDED_CALL_SOURCE, GUARDED_SOURCE, OMITTED_GUARDED_CALL_SOURCE,
    RESULT_SUBSTITUTED_GUARDED_CALL_SOURCE, checked_source,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_section};
use terminal_fixed_fuel::derive_fixed_entry_fuel;
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarCaseResult,
    TerminalScalarCaseValue,
};
use terminal_psi::{OperationKind, OperationResult, StructuralTypeShape, Terminator};

#[test]
fn guarded_payloadless_call_substitutes_the_exact_whole_result_application() {
    let checked = crate::front_end::checked_program(RESULT_SUBSTITUTED_GUARDED_CALL_SOURCE);
    let [plan] = checked
        .facts
        .flow
        .terminal_structural_call_returns
        .payloadless_guarded_machines
        .as_slice()
    else {
        panic!(
            "one exact result-substituting call plan; arms={:?}; returns={:?}",
            checked.facts.proof.outcome_specific_arms,
            checked
                .facts
                .flow
                .terminal_structural_returns
                .payloadless_case_machines,
        )
    };
    let [checked_selection] = plan.selected_evidence.as_slice() else {
        panic!("one exact checked selection")
    };
    assert!(checked_selection.substitutes_result);

    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::caller"),
    )
    .expect("the whole-result guarded application lowers");
    let [caller, callee] = lowered.semantic_module.machines.as_slice() else {
        panic!("caller and producer remain exact")
    };
    let operation = &caller.blocks[0].operations[0];
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &operation.kind
    else {
        panic!("one structural call")
    };
    let [binding] = selected_evidence.as_slice() else {
        panic!("one substituted selected row")
    };
    assert_ne!(binding.callee_proposition, binding.instantiated_proposition);
    let substitution = binding
        .result_substitution
        .expect("the semantic carrier retains exact result substitution");
    assert_eq!(substitution.argument_position, 0);
    assert_eq!(
        substitution.callee_result,
        callee.result.structural().unwrap().place
    );
    assert_eq!(
        substitution.caller_result,
        operation.result.structural().unwrap().place
    );
    assert_eq!(
        binding.validity.proposition_dependencies,
        [substitution.caller_result]
    );
    assert_eq!(
        binding.validity.interface_dependencies,
        [substitution.caller_result]
    );
    let callee_application = lowered
        .semantic_module
        .proposition_applications
        .iter()
        .find(|application| application.id == binding.callee_proposition)
        .unwrap();
    let instantiated_application = lowered
        .semantic_module
        .proposition_applications
        .iter()
        .find(|application| application.id == binding.instantiated_proposition)
        .unwrap();
    assert_eq!(
        callee_application.declaration,
        instantiated_application.declaration
    );
    assert_eq!(callee_application.arguments.len(), 1);
    assert_eq!(instantiated_application.arguments.len(), 1);

    let reconstructed =
        terminal_verifier::reconstruct_terminal_obligations(&lowered.semantic_module)
            .expect("the exact result substitution reconstructs");
    assert!(
        reconstructed.obligations().is_empty(),
        "the selected evidence discharges its guarded row without a proof obligation"
    );
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the substituted selected result verifies");
    let bytes = encode_module(&lowered.semantic_module).expect("substitution encodes");
    assert_eq!(decode_module(&bytes), Ok(lowered.semantic_module.clone()));
    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .unwrap();
    assert_eq!(
        derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
            .unwrap()
            .ceiling_units(),
        4
    );

    let mut redirected = lowered.semantic_module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut redirected.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence[0]
        .result_substitution
        .as_mut()
        .unwrap()
        .caller_result = callee.result.structural().unwrap().place;
    assert!(terminal_verifier::validate_module(&redirected).is_err());

    let mut erased = lowered.semantic_module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut erased.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence[0].result_substitution = None;
    assert!(terminal_verifier::validate_module(&erased).is_err());
}

#[test]
fn omitted_guarded_selector_retains_fact_only_callee_without_runtime_delta() {
    let omitted = checked_trees_to_lowered_psi::lower_machine(
        &crate::front_end::checked_program(OMITTED_GUARDED_CALL_SOURCE),
        TerminalMachineSelection::Name("Root::caller"),
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

    let selected = checked_trees_to_lowered_psi::lower_machine(
        &crate::front_end::checked_program(GUARDED_CALL_SOURCE),
        TerminalMachineSelection::Name("Root::caller"),
    )
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
    let verified = terminal_verifier::verify_module(
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
    let proof = encode_proof_section(&omitted.semantic_module, &omitted.proof_bundle).unwrap();
    let mut execution = TerminalExecution::start_artifact(
        &bytes,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
    )
    .unwrap();
    assert_eq!(
        execution
            .resume(
                &mut TerminalFuelMeter::with_allowance(4),
                &mut AcceptTerminalEffects
            )
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::ScalarCase(
            TerminalScalarCaseResult {
                value: TerminalScalarCaseValue {
                    structural_type: caller.result.structural().unwrap().structural_type,
                    result_case: success_case,
                    fields: Vec::new(),
                },
            }
        ))
    );
}

#[test]
fn exact_payloadless_case_return_is_canonical_verified_and_executable() {
    let checked = checked_source();
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::choose"),
    )
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
    let OperationKind::EstablishScalarCase { result_case, .. } = &operation.kind else {
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
    let verified = terminal_verifier::verify_module(
        module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("payloadless case construction verifies independently");
    let fixed = derive_fixed_entry_fuel(&verified, module.entry)
        .expect("the exact source producer has fixed fuel");
    assert_eq!(fixed.ceiling_units(), 2);

    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("proof bundle encodes");
    let mut execution = TerminalExecution::start_artifact(
        &bytes,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
    )
    .expect("the payloadless case artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(0);
    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .expect("operation exhaustion is resumable"),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    meter.replenish(1).expect("fund the case constructor");
    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .expect("return exhaustion is resumable"),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    meter.replenish(1).expect("fund the return edge");
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .expect("the case return completes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::ScalarCase(
            TerminalScalarCaseResult {
                value: TerminalScalarCaseValue {
                    structural_type: operation_result.structural_type,
                    result_case: *result_case,
                    fields: Vec::new(),
                },
            }
        ))
    );
}

#[test]
fn guarded_payloadless_case_return_retains_active_evidence_and_vacuous_siblings() {
    let baseline = checked_trees_to_lowered_psi::lower_machine(
        &checked_source(),
        TerminalMachineSelection::Name("Root::choose"),
    )
    .expect("the proof-free payloadless producer lowers");
    let checked = crate::front_end::checked_program(GUARDED_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::choose"),
    )
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
        semantic_vocabulary::Proposition::Atom(_)
    ));
    assert_eq!(
        success[0]
            .evidence
            .as_ref()
            .map(|evidence| evidence.output_field.as_str()),
        Some("selected")
    );
    assert_eq!(
        success[1].proposition,
        semantic_vocabulary::Proposition::Truth
    );
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
        semantic_vocabulary::Proposition::Atom(_)
    ));
    assert_eq!(
        failure_named
            .evidence
            .as_ref()
            .map(|evidence| evidence.output_field.as_str()),
        Some("skipped")
    );
    assert_eq!(
        failure_truth.proposition,
        semantic_vocabulary::Proposition::Truth
    );
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
    let proof_bytes = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("encode guarded proof");
    assert_eq!(
        decode_proof_bundle(&proof_bytes),
        Ok(lowered.proof_bundle.clone())
    );
    let verified = terminal_verifier::verify_module(
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
        TerminalStructuralInputs::default(),
    )
    .expect("the guarded payloadless artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(2);
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .expect("guarded producer completes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::ScalarCase(
            TerminalScalarCaseResult {
                value: TerminalScalarCaseValue {
                    structural_type: machine.result.structural().unwrap().structural_type,
                    result_case: success_case,
                    fields: Vec::new(),
                },
            }
        ))
    );

    let mut missing_producer = lowered.proof_bundle.clone();
    missing_producer.evidence_producers.clear();
    assert!(matches!(
        terminal_verifier::verify_module(module, &missing_producer, &AdmissionProfile::default(),),
        Err(terminal_verifier::VerificationError::MissingEvidenceProducer(_))
    ));

    let mut vacuous_producer = lowered.proof_bundle.clone();
    vacuous_producer.evidence_producers[0].term = failure_named
        .evidence
        .as_ref()
        .expect("named Failure endpoint")
        .term;
    assert!(matches!(
        terminal_verifier::verify_module(module, &vacuous_producer, &AdmissionProfile::default(),),
        Err(terminal_verifier::VerificationError::UnusedEvidenceProducerTerm(_))
    ));

    let mut changed_case = module.clone();
    let OperationKind::EstablishScalarCase { result_case, .. } =
        &mut changed_case.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *result_case = failure_case;
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_case,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::UnusedEvidenceProducerTerm(_))
    ));
    let mut changed_case_bundle = lowered.proof_bundle.clone();
    changed_case_bundle.evidence_producers[0].term = failure_named
        .evidence
        .as_ref()
        .expect("named Failure endpoint")
        .term;
    changed_case_bundle.evidence[0].obligation = failure_truth.obligation;
    terminal_verifier::verify_module(
        &changed_case,
        &changed_case_bundle,
        &AdmissionProfile::default(),
    )
    .expect("changing the exact constructor swaps the active proof and producer set");

    let mut missing_truth = lowered.proof_bundle.clone();
    missing_truth.evidence.clear();
    assert!(matches!(
        terminal_verifier::verify_module(
            module,
            &missing_truth,
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::MissingEvidence(
            obligation
        ))
            if obligation == success[1].obligation
    ));
}
