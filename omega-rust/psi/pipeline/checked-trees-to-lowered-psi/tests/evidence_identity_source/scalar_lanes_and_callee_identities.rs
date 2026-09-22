use super::{
    COPY_AND_DISCARD_PROOF_OUTPUT_SOURCE, DISTINCT_PROOF_OUTPUT_PRODUCERS_SOURCE,
    EMPTY_PRODUCER_SOURCE, MULTI_FIELD_PROOF_OUTPUT_SOURCE, ORDINARY_ATTACHED_SCALAR_SOURCE,
    PLURAL_STATIC_REQUIREMENT_PROOF_OUTPUT_SOURCE, PROOF_OUTPUT_SOURCE,
    REPEATED_MULTI_FIELD_PROOF_OUTPUT_SOURCE, REPEATED_PROOF_OUTPUT_SOURCE,
    RUNTIME_VALUE_PROOF_OUTPUT_SOURCE, STATIC_REQUIREMENT_TRAIT_DEFAULT_SOURCE, check,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerValue, OperationId};
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_section};
use terminal_fixed_fuel::derive_fixed_entry_fuel;
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
};
use terminal_psi::{EvidenceContractLaneKind, OperationKind};

#[test]
fn ordinary_attached_scalar_machine_lowers_through_the_unit_closure() {
    let checked = check(ORDINARY_ATTACHED_SCALAR_SOURCE);
    let selection = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find(|selection| selection.name == "Root::f")
        .expect("ordinary attached scalar machine selection");
    assert_eq!(
        selection.signature,
        checked_trees::CheckedTerminalSignatureEligibility::Attached
    );
    // The checker plans the attached scalar body as a Unit effect closure,
    // which is the lane that lowers it; no scalar entry lane is involved.
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(selection.machine)
            .is_some()
    );
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name(&selection.name),
    )
    .expect("attached scalar machine lowers through the Unit closure");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("attached scalar machine verifies");
}

#[test]
fn plural_static_requirement_proof_outputs_preserve_order_identity_and_freshness() {
    let checked = check(PLURAL_STATIC_REQUIREMENT_PROOF_OUTPUT_SOURCE);
    let checked_invocation = checked
        .facts
        .proof
        .proof_output_calls
        .iter()
        .find_map(|(_, invocation)| {
            invocation
                .static_requirement_dispatch
                .as_ref()
                .map(|_| invocation)
        })
        .expect("one checked plural static requirement proof-output call");
    let machine_name = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find_map(|selection| {
            (selection.machine == checked_invocation.caller_machine_symbol)
                .then_some(selection.name.clone())
        })
        .expect("the plural specialized requirement caller is terminal-selected");
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name(&machine_name),
    )
    .expect("plural static requirement proof outputs should cross Terminal Psi");
    let [invocation] = lowered.semantic_module.proof_output_calls.as_slice() else {
        panic!("one plural static requirement proof-output invocation expected")
    };
    assert!(invocation.static_requirement_dispatch.is_some());
    assert_eq!(invocation.evidence_arguments.len(), 2);
    assert_eq!(invocation.outputs.len(), 3);
    assert_eq!(
        invocation
            .evidence_arguments
            .iter()
            .map(|argument| argument.input_position)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    assert_eq!(
        invocation
            .outputs
            .iter()
            .map(|output| (output.output_position, output.output_field.as_str()))
            .collect::<Vec<_>>(),
        vec![(0, "first_copy"), (1, "second_copy"), (2, "third_copy")]
    );
    assert!(
        invocation
            .outputs
            .iter()
            .all(|output| output.forwarded_input_position.is_none()
                && output.callee_output.is_none()
                && output.output.is_some()),
        "static dispatch must expose only fresh public output identities"
    );
    let output_terms = invocation
        .outputs
        .iter()
        .map(|output| output.output.expect("fresh static output term"))
        .collect::<Vec<_>>();
    assert!(output_terms.windows(2).all(|pair| pair[0] != pair[1]));
    assert!(
        invocation
            .evidence_arguments
            .iter()
            .all(|argument| { !output_terms.contains(&argument.source) })
    );

    let semantic_bytes =
        encode_module(&lowered.semantic_module).expect("encode plural static semantics");
    let proof_bytes = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("encode plural static proof bundle");
    let decoded_semantic = decode_module(&semantic_bytes).expect("decode plural static semantics");
    let decoded_proof =
        decode_proof_bundle(&proof_bytes).expect("decode plural static proof bundle");
    assert_eq!(decoded_semantic, lowered.semantic_module);
    assert_eq!(decoded_proof, lowered.proof_bundle);
    terminal_verifier::verify_module(
        &decoded_semantic,
        &decoded_proof,
        &AdmissionProfile::default(),
    )
    .expect("plural subjectless static named-witness lanes verify");

    let rejects = |label: &str, module: &terminal_psi::TerminalModule| {
        let result = terminal_verifier::validate_module_representation(module);
        assert!(
            matches!(
                result,
                Err(terminal_verifier::ModuleError::InvalidProofOutputCall { .. })
            ),
            "{label} mutation must reject as an invalid proof-output call, got {result:?}"
        );
    };

    let mut reordered_arguments = lowered.semantic_module.clone();
    reordered_arguments.proof_output_calls[0]
        .evidence_arguments
        .swap(0, 1);
    rejects("argument order", &reordered_arguments);

    let mut reordered_outputs = lowered.semantic_module.clone();
    reordered_outputs.proof_output_calls[0].outputs.swap(0, 1);
    rejects("output order", &reordered_outputs);

    let mut wrong_proposition = lowered.semantic_module.clone();
    wrong_proposition.proof_output_calls[0].outputs[2].instantiated_proposition =
        semantic_vocabulary::PropositionId::new(999).expect("forged proposition identity");
    rejects("proposition", &wrong_proposition);

    let mut wrong_interface = lowered.semantic_module.clone();
    let third_output = wrong_interface.proof_output_calls[0].outputs[2]
        .output
        .expect("third output term");
    wrong_interface
        .evidence_terms
        .iter_mut()
        .find(|term| term.id == third_output)
        .expect("third output declaration")
        .interface
        .trait_identity
        .push_str("::forged");
    assert_eq!(
        terminal_verifier::validate_module_representation(&wrong_interface),
        Err(terminal_verifier::ModuleError::EvidenceTermInterfaceMismatch(third_output))
    );

    let mut reused_output = lowered.semantic_module.clone();
    let first_output = reused_output.proof_output_calls[0].outputs[0].output;
    reused_output.proof_output_calls[0].outputs[1].output = first_output;
    rejects("freshness", &reused_output);
}

#[test]
fn static_requirement_trait_default_rejoins_exact_application_and_runtime_callee() {
    let checked = check(STATIC_REQUIREMENT_TRAIT_DEFAULT_SOURCE);
    let checked_invocation = checked
        .facts
        .proof
        .proof_output_calls
        .iter()
        .find_map(|(_, invocation)| {
            invocation
                .static_requirement_dispatch
                .as_ref()
                .map(|dispatch| (invocation, dispatch))
        })
        .expect("one checked trait-default static dispatch");
    let expected_public_requirement = checked
        .symbols
        .display_path(checked_invocation.1.requirement, "::");
    let expected_realization = checked
        .symbols
        .display_path(checked_invocation.1.realization_state, "::");
    let machine_name = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find_map(|selection| {
            (selection.machine == checked_invocation.0.caller_machine_symbol)
                .then_some(selection.name.clone())
        })
        .expect("the specialized default caller is terminal-selected");
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name(&machine_name),
    )
    .expect("the exact trait-default dispatch should cross Terminal Psi");
    let [invocation] = lowered.semantic_module.proof_output_calls.as_slice() else {
        panic!("one trait-default proof-output invocation")
    };
    let dispatch = invocation
        .static_requirement_dispatch
        .as_ref()
        .expect("the trait-default dispatch remains explicit");
    assert_eq!(dispatch.requirement_identity, expected_public_requirement);
    assert_eq!(dispatch.realization_identity, expected_realization);
    assert_ne!(dispatch.conformance_application_report_fingerprint, 0);
    assert!(!dispatch.conformance_application_commitment.is_zero());
    assert_eq!(
        invocation.runtime_call.map(|call| call.callee),
        Some(dispatch.realization)
    );
    let application_index = lowered
        .semantic_module
        .closed_conformance_applications
        .iter()
        .position(|application| {
            application.owner == invocation.caller
                && application.commitment == dispatch.conformance_application_commitment
        })
        .expect("the dispatch rejoins its owner-scoped closed application");
    let application = &lowered.semantic_module.closed_conformance_applications[application_index];
    assert!(application.rows.iter().any(|row| {
        row.public_requirement_identity == dispatch.public_requirement_identity
            && row.realization_identity == dispatch.realization_identity
    }));
    let bytes = encode_module(&lowered.semantic_module).expect("default dispatch module encodes");
    assert_eq!(decode_module(&bytes), Ok(lowered.semantic_module.clone()));
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the exact trait-default dispatch verifies");

    let mut declaration_drift = lowered.semantic_module.clone();
    declaration_drift.closed_conformance_applications[application_index]
        .declaration_identity
        .push_str("::forged");
    assert!(matches!(
        terminal_verifier::validate_module_representation(&declaration_drift),
        Err(terminal_verifier::ModuleError::ClosedConformanceFingerprintMismatch { .. })
    ));

    let mut row_drift = lowered.semantic_module.clone();
    row_drift.closed_conformance_applications[application_index].rows[0]
        .realization_identity
        .push_str("::forged");
    assert!(matches!(
        terminal_verifier::validate_module_representation(&row_drift),
        Err(terminal_verifier::ModuleError::InvalidClosedConformanceApplication { .. })
    ));

    let mut runtime_drift = lowered.semantic_module.clone();
    runtime_drift.proof_output_calls[0]
        .runtime_call
        .as_mut()
        .expect("runtime call")
        .callee = runtime_drift.entry;
    assert!(matches!(
        terminal_verifier::validate_module_representation(&runtime_drift),
        Err(terminal_verifier::ModuleError::InvalidProofOutputCall { .. })
    ));
}

#[test]
fn proof_output_retains_copy_and_explicit_discard() {
    let checked = check(COPY_AND_DISCARD_PROOF_OUTPUT_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::relay"),
    )
    .expect("copyable and discarded evidence should cross terminal Psi");
    let [invocation] = lowered.semantic_module.proof_output_calls.as_slice() else {
        panic!("one proof-output call expected")
    };
    let [copied, discarded] = invocation.outputs.as_slice() else {
        panic!("two complete proof selectors expected")
    };
    let copied_term = copied.output.expect("copied field binds a caller term");
    assert_eq!(discarded.output, None);
    let relayed = lowered
        .semantic_module
        .evidence_contract_lanes
        .iter()
        .filter(|lane| {
            lane.machine == lowered.semantic_module.entry
                && lane.kind == EvidenceContractLaneKind::Ensures
        })
        .collect::<Vec<_>>();
    assert_eq!(relayed.len(), 2);
    assert!(relayed.iter().all(|lane| lane.term == copied_term));

    let bytes = encode_module(&lowered.semantic_module).expect("discard disposition encodes");
    assert_eq!(decode_module(&bytes), Ok(lowered.semantic_module.clone()));
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("copy and discard preserve exact evidence provenance");

    let mut omitted = lowered.semantic_module.clone();
    omitted.proof_output_calls[0].outputs.pop();
    assert!(terminal_verifier::validate_module_representation(&omitted).is_err());
}

#[test]
fn runtime_value_proof_output_links_one_scalar_call_and_executes_once() {
    let checked = check(RUNTIME_VALUE_PROOF_OUTPUT_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("relay"),
    )
    .expect("runtime value proof output should cross terminal Psi");
    let [invocation] = lowered.semantic_module.proof_output_calls.as_slice() else {
        panic!("one terminal runtime-value proof output expected")
    };
    let Some(terminal_psi::ProofOutputRuntimeResult::Scalar(runtime_type)) =
        invocation.runtime_result
    else {
        panic!("runtime proof output retains its scalar payload type")
    };
    let runtime_call = invocation
        .runtime_call
        .expect("runtime proof output retains its exact ordinary call operation");
    assert_eq!(runtime_type, semantic_vocabulary::ScalarType::Boolean);
    let caller = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == invocation.caller)
        .expect("proof-output caller machine");
    let calls = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| matches!(operation.kind, OperationKind::Call { .. }))
        .collect::<Vec<_>>();
    assert_eq!(
        calls.len(),
        2,
        "one unrelated call precedes the proof-output call"
    );
    let call = calls
        .iter()
        .copied()
        .find(|operation| operation.id == runtime_call.operation)
        .expect("the retained ID selects the proof-output call, not the earlier call");
    assert_eq!(call.id, runtime_call.operation);
    assert!(matches!(
        call.kind,
        OperationKind::Call { callee, .. } if callee == runtime_call.callee
    ));

    let bytes =
        encode_module(&lowered.semantic_module).expect("runtime proof-output module encodes");
    assert_eq!(decode_module(&bytes), Ok(lowered.semantic_module.clone()));
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("runtime proof-output proof encodes");
    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("runtime proof-output operation and proof group verify together");
    let fuel = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("the ordinary scalar call has fixed fuel");
    assert!(fuel.ceiling_units() > 0);
    let baseline = check(
        r#"
            machine warmup() -> bool
            requires true == true
            ensures true == true
            { true }
            machine produce() -> bool
            requires true == true
            ensures true == true
            { true }
            machine relay() -> bool
            requires true == true
            ensures true == true
            { let warmed: bool = warmup(); let local: bool = produce(); local }
        "#,
    );
    let baseline = checked_trees_to_lowered_psi::lower_machine(
        &baseline,
        TerminalMachineSelection::Name("relay"),
    )
    .expect("ordinary scalar-call baseline lowers");
    let baseline_verified = terminal_verifier::verify_module(
        &baseline.semantic_module,
        &baseline.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("ordinary scalar-call baseline verifies");
    let baseline_fuel = derive_fixed_entry_fuel(&baseline_verified, baseline.semantic_module.entry)
        .expect("ordinary scalar-call baseline has fixed fuel");
    assert_eq!(
        fuel.ceiling_units(),
        baseline_fuel.ceiling_units(),
        "erased proof-output proof metadata adds no runtime fuel"
    );

    let mut execution = TerminalExecution::start_artifact(
        &bytes,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
    )
    .expect("runtime proof-output artifact starts");
    let mut meter = TerminalFuelMeter::unbounded();
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .expect("execute runtime proof output"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Boolean(true)
        ))
    );

    let invalid_proof_output = |module: &terminal_psi::TerminalModule| {
        matches!(
            terminal_verifier::validate_module_representation(module),
            Err(terminal_verifier::ModuleError::InvalidProofOutputCall { .. })
        )
    };
    let mut unknown_operation = lowered.semantic_module.clone();
    unknown_operation.proof_output_calls[0]
        .runtime_call
        .as_mut()
        .expect("runtime link")
        .operation = OperationId::new(u64::MAX).unwrap();
    assert!(invalid_proof_output(&unknown_operation));

    let mut wrong_kind = lowered.semantic_module.clone();
    let linked = wrong_kind.proof_output_calls[0]
        .runtime_call
        .expect("runtime link")
        .operation;
    wrong_kind
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| operation.id == linked)
        .expect("linked operation")
        .kind = OperationKind::IntegerConstant {
        value: IntegerValue::Signed(7),
    };
    assert!(invalid_proof_output(&wrong_kind));

    let mut wrong_caller = lowered.semantic_module.clone();
    wrong_caller.proof_output_calls[0].caller = runtime_call.callee;
    assert!(invalid_proof_output(&wrong_caller));

    let mut missing_link = lowered.semantic_module.clone();
    missing_link.proof_output_calls[0].runtime_call = None;
    assert!(invalid_proof_output(&missing_link));

    let mut mismatched_callee = lowered.semantic_module.clone();
    mismatched_callee.proof_output_calls[0]
        .runtime_call
        .as_mut()
        .expect("runtime link")
        .callee = invocation.caller;
    assert!(invalid_proof_output(&mismatched_callee));

    let proof_only = checked_trees_to_lowered_psi::lower_machine(
        &check(PROOF_OUTPUT_SOURCE),
        TerminalMachineSelection::Name("Root::relay"),
    )
    .expect("proof-only proof output");
    let mut spurious_link = proof_only.semantic_module;
    spurious_link.proof_output_calls[0].runtime_call = Some(runtime_call);
    assert!(invalid_proof_output(&spurious_link));
}

#[test]
fn multi_field_proof_output_is_complete_canonical_and_runtime_erased() {
    let checked = check(MULTI_FIELD_PROOF_OUTPUT_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::relay"),
    )
    .expect("complete multi-field proof output should cross terminal Psi");
    let [invocation] = lowered.semantic_module.proof_output_calls.as_slice() else {
        panic!("one grouped terminal proof-output invocation expected")
    };
    assert_eq!(invocation.ordinal, 0);
    let [first, second] = invocation.outputs.as_slice() else {
        panic!("two terminal proof-output outputs expected")
    };
    assert_eq!((first.output_position, second.output_position), (0, 1));
    assert_eq!(
        (first.output_field.as_str(), second.output_field.as_str()),
        ("first", "second")
    );
    assert_ne!(
        first.callee_output.expect("first producer-backed output"),
        first.output.expect("first bound output")
    );
    assert_ne!(
        second.callee_output.expect("second producer-backed output"),
        second.output.expect("second bound output")
    );
    assert_ne!(first.output, second.output);
    assert_ne!(first.callee_output, second.callee_output);
    assert_eq!(lowered.proof_bundle.evidence_producers.len(), 2);

    let bytes = encode_module(&lowered.semantic_module).expect("multi-field proof output encodes");
    assert_eq!(decode_module(&bytes), Ok(lowered.semantic_module.clone()));
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("multi-field proof encodes");
    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("complete multi-field proof output verifies");
    derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("multi-field proof-only invocation adds no runtime fuel");

    let mut execution = TerminalExecution::start_artifact(
        &bytes,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
    )
    .expect("multi-field proof-only proof output requires no runtime argument");
    let mut meter = TerminalFuelMeter::unbounded();
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .expect("execute erased proof output"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );

    let mut non_dense = lowered.semantic_module.clone();
    non_dense.proof_output_calls[0].outputs[1].output_position = 2;
    assert!(encode_module(&non_dense).is_err());
    assert!(matches!(
        terminal_verifier::validate_module_representation(&non_dense),
        Err(terminal_verifier::ModuleError::InvalidProofOutputCall { .. })
    ));

    let mut aliased = lowered.semantic_module.clone();
    aliased.proof_output_calls[0].outputs[1].output =
        aliased.proof_output_calls[0].outputs[0].output;
    assert!(matches!(
        terminal_verifier::validate_module_representation(&aliased),
        Err(terminal_verifier::ModuleError::InvalidProofOutputCall { .. })
    ));

    let mut duplicate_field = lowered.semantic_module.clone();
    duplicate_field.proof_output_calls[0].outputs[1].output_field =
        duplicate_field.proof_output_calls[0].outputs[0]
            .output_field
            .clone();
    assert!(matches!(
        terminal_verifier::validate_module_representation(&duplicate_field),
        Err(terminal_verifier::ModuleError::InvalidProofOutputCall { .. })
    ));

    let mut incomplete = lowered.semantic_module.clone();
    incomplete.proof_output_calls[0].outputs.pop();
    assert!(terminal_verifier::validate_module_representation(&incomplete).is_err());
}

#[test]
fn repeated_multi_field_proof_outputs_group_calls_and_reuse_callee_producers() {
    let checked = check(REPEATED_MULTI_FIELD_PROOF_OUTPUT_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::relay"),
    )
    .expect("repeated multi-field proof outputs should lower");
    let [first, second] = lowered.semantic_module.proof_output_calls.as_slice() else {
        panic!("two grouped proof-output calls expected")
    };
    assert_eq!((first.ordinal, second.ordinal), (0, 1));
    assert_eq!(first.outputs.len(), 2);
    assert_eq!(second.outputs.len(), 2);
    for position in 0..2 {
        assert_eq!(
            first.outputs[position].callee_output,
            second.outputs[position].callee_output
        );
        assert_ne!(
            first.outputs[position].output,
            second.outputs[position].output
        );
    }
    let caller_outputs = first
        .outputs
        .iter()
        .chain(&second.outputs)
        .map(|output| output.output)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(caller_outputs.len(), 4);
    assert_eq!(lowered.proof_bundle.evidence_producers.len(), 2);
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("two calls reuse each callee producer and mint four caller terms");
}

#[test]
fn repeated_proof_output_calls_have_dense_fresh_outputs_and_one_callee_producer() {
    let checked = check(REPEATED_PROOF_OUTPUT_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::relay"),
    )
    .expect("repeated proof-output calls should retain distinct invocation terms");
    let [first, second] = lowered.semantic_module.proof_output_calls.as_slice() else {
        panic!("two invocation rows expected")
    };
    assert_eq!((first.ordinal, second.ordinal), (0, 1));
    assert_eq!(
        first.outputs[0].callee_output,
        second.outputs[0].callee_output
    );
    assert_ne!(first.outputs[0].output, second.outputs[0].output);
    assert_eq!(lowered.proof_bundle.evidence_producers.len(), 1);
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("one callee declaration needs one producer regardless of invocation count");
}

#[test]
fn same_shape_proof_outputs_retain_distinct_canonical_callee_identities() {
    let checked = check(DISTINCT_PROOF_OUTPUT_PRODUCERS_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::relay"),
    )
    .expect("same-shape producer proof outputs should lower");
    let [first, second] = lowered.semantic_module.proof_output_calls.as_slice() else {
        panic!("two invocation rows expected")
    };
    assert_ne!(
        first.target_machine_identity,
        second.target_machine_identity
    );
    assert!(!first.target_machine_identity.is_empty());
    assert!(!second.target_machine_identity.is_empty());
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("canonical callee identity remains verified semantic data");
}

#[test]
fn empty_complete_evidence_conformance_remains_valid_provenance() {
    let checked = check(EMPTY_PRODUCER_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::produce"),
    )
    .expect("an empty closed conformance is still complete");
    assert_eq!(lowered.proof_bundle.evidence_producers.len(), 1);
    assert!(lowered.proof_bundle.evidence_producers[0].rows.is_empty());
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("empty row set is canonical");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the exact empty conformance verifies");
}
