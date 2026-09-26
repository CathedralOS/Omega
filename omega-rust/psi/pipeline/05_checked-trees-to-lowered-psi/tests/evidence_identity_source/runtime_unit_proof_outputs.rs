use super::{
    RUNTIME_UNIT_PROOF_OUTPUT_SOURCE, STATIC_REQUIREMENT_BOOL_PROOF_OUTPUT_SOURCE,
    STATIC_REQUIREMENT_BOOL_RUNTIME_BASELINE_SOURCE, STATIC_REQUIREMENT_I32_PROOF_OUTPUT_SOURCE,
    STATIC_REQUIREMENT_I32_RUNTIME_BASELINE_SOURCE, STATIC_REQUIREMENT_PROOF_OUTPUT_SOURCE,
    STATIC_REQUIREMENT_RUNTIME_BASELINE_SOURCE,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerValue, OperationId, ValueId};
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_section};
use terminal_fixed_fuel::derive_fixed_entry_fuel;
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
};
use terminal_psi::OperationKind;

#[test]
fn runtime_unit_proof_output_links_and_executes_its_ordinary_call() {
    let checked = crate::front_end::checked_program(RUNTIME_UNIT_PROOF_OUTPUT_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::relay"),
    )
    .expect("runtime Unit proof output should cross terminal Psi");
    let [invocation] = lowered.semantic_module.proof_output_calls.as_slice() else {
        panic!("one runtime Unit proof-output invocation expected")
    };
    assert_eq!(
        invocation.runtime_result,
        Some(terminal_psi::ProofOutputRuntimeResult::Unit)
    );
    let runtime_call = invocation
        .runtime_call
        .expect("the proof-output row links its ordinary Unit call");
    let caller = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == invocation.caller)
        .expect("proof-output caller machine");
    let operation = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == runtime_call.operation)
        .expect("linked Unit call operation");
    assert!(matches!(
        operation.kind,
        OperationKind::CallUnit { callee, .. } if callee == runtime_call.callee
    ));

    let bytes = encode_module(&lowered.semantic_module).expect("runtime Unit module encodes");
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("runtime Unit proof encodes");
    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the Unit operation and proof-output row verify together");
    assert!(
        derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
            .expect("the retained Unit call has fixed fuel")
            .ceiling_units()
            > 0
    );
    let mut execution = TerminalExecution::start_artifact(
        &bytes,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
    )
    .expect("runtime Unit proof-output artifact starts");
    assert_eq!(
        execution
            .resume(
                &mut TerminalFuelMeter::unbounded(),
                &mut AcceptTerminalEffects
            )
            .expect("execute runtime Unit proof output"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );

    let mut missing_link = lowered.semantic_module.clone();
    missing_link.proof_output_calls[0].runtime_call = None;
    assert!(matches!(
        terminal_verifier::validate_module_representation(&missing_link),
        Err(terminal_verifier::ModuleError::InvalidProofOutputCall { .. })
    ));
}

#[test]
fn static_requirement_proof_output_keeps_public_identity_and_private_dispatch_separate() {
    let checked = crate::front_end::checked_program(STATIC_REQUIREMENT_PROOF_OUTPUT_SOURCE);
    let invocation = checked
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
        .expect("one checked static requirement proof-output call");
    let machine_name = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find_map(|selection| {
            (selection.machine == invocation.caller_machine_symbol)
                .then_some(selection.name.clone())
        })
        .expect("the specialized requirement caller is terminal-selected");
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name(&machine_name),
    )
    .expect("static requirement proof output should cross terminal Psi");
    let baseline_checked =
        crate::front_end::checked_program(STATIC_REQUIREMENT_RUNTIME_BASELINE_SOURCE);
    let baseline_machine_name = baseline_checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find_map(|selection| {
            baseline_checked
                .machine_specializations
                .iter()
                .find(|specialization| {
                    specialization.instance == selection.machine
                        && !specialization.conformance_applications.is_empty()
                })
                .map(|_| selection.name.clone())
        })
        .expect("the baseline specialized requirement caller is terminal-selected");
    let baseline = checked_trees_to_lowered_psi::lower_machine(
        &baseline_checked,
        TerminalMachineSelection::Name(&baseline_machine_name),
    )
    .expect("matching runtime-only static requirement call should lower");
    let [invocation] = lowered.semantic_module.proof_output_calls.as_slice() else {
        panic!("one static requirement proof-output invocation expected")
    };
    let dispatch = invocation
        .static_requirement_dispatch
        .as_ref()
        .expect("the private static realization remains explicit");
    let [argument] = invocation.evidence_arguments.as_slice() else {
        panic!("one public requirement input expected")
    };
    let [output] = invocation.outputs.as_slice() else {
        panic!("one public requirement output expected")
    };
    assert_eq!(
        invocation.target_machine_identity,
        dispatch.public_requirement_identity
    );
    assert_ne!(
        invocation.target_machine_identity, dispatch.realization_identity,
        "the public requirement identity must not become its private realization"
    );
    assert_eq!(output.output_field, "public_out");
    assert!(
        invocation
            .outputs
            .iter()
            .all(|output| output.output_field != "private_out"),
        "the strengthening-only satisfier selector must stay private"
    );
    assert_eq!(output.forwarded_input_position, None);
    assert_eq!(
        output.callee_output, None,
        "no satisfier or requirement callee term identity crosses the static abstraction"
    );
    assert_ne!(output.output, Some(argument.source));
    assert_eq!(
        invocation.runtime_result,
        Some(terminal_psi::ProofOutputRuntimeResult::Unit)
    );
    assert_eq!(
        invocation.runtime_call.map(|call| call.callee),
        Some(dispatch.realization)
    );
    let application = lowered
        .semantic_module
        .closed_conformance_applications
        .iter()
        .find(|application| {
            application.owner == invocation.caller
                && application.report_fingerprint
                    == dispatch.conformance_application_report_fingerprint
                && application.commitment == dispatch.conformance_application_commitment
        })
        .expect("the dispatch rejoins its exact closed application");
    assert_eq!(
        application.trait_identity,
        dispatch.declaring_trait_identity
    );
    let bound_row = application
        .rows
        .iter()
        .find(|row| {
            row.declaring_trait_identity == dispatch.declaring_trait_identity
                && row.requirement_identity == dispatch.requirement_identity
                && row.realization_identity == dispatch.realization_identity
        })
        .expect("the exact conformance row remains present");
    let realization_callable_identity = bound_row
        .realization_callable_identity
        .as_ref()
        .expect("static dispatch references its standalone callable registry");
    assert_eq!(
        realization_callable_identity.as_str(),
        dispatch.realization_callable_identity.as_str()
    );
    let realization_callable = application
        .realization_callables
        .iter()
        .find(|callable| callable.source_callable_identity == *realization_callable_identity)
        .expect("the row reference resolves in its application registry");
    assert_eq!(realization_callable.machine, dispatch.realization);
    assert_eq!(
        realization_callable.result,
        terminal_psi::ClosedConformanceCallableResult::Unit
    );
    assert_eq!(
        lowered
            .semantic_module
            .closed_conformance_applications
            .iter()
            .flat_map(|application| &application.realization_callables)
            .count(),
        1,
        "the bounded static call publishes exactly one canonical registry entry"
    );
    assert_eq!(
        lowered
            .semantic_module
            .closed_conformance_applications
            .iter()
            .flat_map(|application| &application.rows)
            .filter(|row| row.realization_callable_identity.is_some())
            .count(),
        1,
        "exactly the selected row references the canonical registry"
    );
    assert!(
        baseline
            .semantic_module
            .closed_conformance_applications
            .iter()
            .all(|application| {
                application.realization_callables.is_empty()
                    && application
                        .rows
                        .iter()
                        .all(|row| row.realization_callable_identity.is_none())
            }),
        "the matching runtime-only conformance remains map-free"
    );
    let baseline_bytes =
        encode_module(&baseline.semantic_module).expect("map-free baseline module encodes");
    let baseline_decoded =
        decode_module(&baseline_bytes).expect("map-free baseline module decodes");
    assert!(
        baseline_decoded
            .closed_conformance_applications
            .iter()
            .all(|application| {
                application.realization_callables.is_empty()
                    && application
                        .rows
                        .iter()
                        .all(|row| row.realization_callable_identity.is_none())
            }),
        "codec roundtrip preserves absence rather than synthesizing a map"
    );
    let mut spurious_runtime_registry = baseline.semantic_module.clone();
    let spurious_application = spurious_runtime_registry
        .closed_conformance_applications
        .iter_mut()
        .find(|application| !application.rows.is_empty())
        .expect("runtime-only fixture retains its closed application");
    let spurious_identity = "forged::runtime-only-callable".to_owned();
    spurious_application.realization_callables =
        vec![terminal_psi::ClosedConformanceRealizationCallable {
            source_callable_identity: spurious_identity.clone(),
            machine: spurious_application.owner,
            result: terminal_psi::ClosedConformanceCallableResult::Unit,
        }];
    spurious_application.rows[0].realization_callable_identity = Some(spurious_identity);
    spurious_application.report_fingerprint =
        terminal_psi::closed_conformance_application_report_fingerprint(spurious_application);
    spurious_application.commitment =
        terminal_psi::closed_conformance_application_commitment(spurious_application);
    assert!(matches!(
        terminal_verifier::validate_module_representation(&spurious_runtime_registry),
        Err(terminal_verifier::ModuleError::InvalidClosedConformanceApplication { .. })
    ));

    let mut widened_static_registry = lowered.semantic_module.clone();
    let widened_application = widened_static_registry
        .closed_conformance_applications
        .iter_mut()
        .find(|application| application.owner == invocation.caller)
        .expect("entry closed application");
    let second_identity = format!(
        "{}::second",
        widened_application.realization_callables[0].source_callable_identity
    );
    widened_application.realization_callables.push(
        terminal_psi::ClosedConformanceRealizationCallable {
            source_callable_identity: second_identity.clone(),
            machine: widened_application.owner,
            result: terminal_psi::ClosedConformanceCallableResult::Unit,
        },
    );
    widened_application.realization_callables.sort();
    let mut second_row = widened_application.rows[0].clone();
    second_row.requirement_identity.push_str("::second");
    second_row.realization_callable_identity = Some(second_identity);
    widened_application.rows.push(second_row);
    widened_application.report_fingerprint =
        terminal_psi::closed_conformance_application_report_fingerprint(widened_application);
    widened_application.commitment =
        terminal_psi::closed_conformance_application_commitment(widened_application);
    assert!(matches!(
        terminal_verifier::validate_module_representation(&widened_static_registry),
        Err(terminal_verifier::ModuleError::InvalidClosedConformanceApplication { .. })
    ));
    let mut changed_binding = application.clone();
    changed_binding
        .realization_callables
        .first_mut()
        .expect("standalone callable binding")
        .source_callable_identity
        .push_str("::changed");
    assert_ne!(
        terminal_psi::closed_conformance_application_report_fingerprint(&changed_binding),
        application.report_fingerprint
    );
    assert_ne!(
        terminal_psi::closed_conformance_application_commitment(&changed_binding),
        application.commitment
    );
    assert!(
        lowered.proof_bundle.evidence_producers.is_empty(),
        "the satisfier's private forwarded producer must not escape"
    );
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("named static entry");
    let baseline_entry = baseline
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == baseline.semantic_module.entry)
        .expect("runtime baseline entry");
    assert_eq!(entry.parameters, baseline_entry.parameters);
    assert_eq!(
        entry.structural_parameters,
        baseline_entry.structural_parameters
    );
    assert_eq!(entry.result, baseline_entry.result);
    assert_eq!(entry.structural_places, baseline_entry.structural_places);
    let operation_kinds = |machine: &terminal_psi::TerminalMachine| {
        machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .map(|operation| std::mem::discriminant(&operation.kind))
            .collect::<Vec<_>>()
    };
    assert_eq!(operation_kinds(entry), operation_kinds(baseline_entry));
    assert_eq!(
        lowered.semantic_module.machines.len(),
        baseline.semantic_module.machines.len(),
        "named proof lanes do not change the executable closure"
    );
    for (machine, baseline_machine) in lowered
        .semantic_module
        .machines
        .iter()
        .zip(&baseline.semantic_module.machines)
    {
        assert_eq!(machine.parameters, baseline_machine.parameters);
        assert_eq!(
            machine.structural_parameters,
            baseline_machine.structural_parameters
        );
        assert_eq!(machine.result, baseline_machine.result);
        assert_eq!(
            machine.structural_places,
            baseline_machine.structural_places
        );
        assert_eq!(machine.blocks, baseline_machine.blocks);
    }

    let bytes = encode_module(&lowered.semantic_module).expect("static dispatch module encodes");
    assert_eq!(decode_module(&bytes), Ok(lowered.semantic_module.clone()));
    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the exact public/private dispatch split verifies");
    let baseline_verified = terminal_verifier::verify_module(
        &baseline.semantic_module,
        &baseline.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the matching runtime-only static call verifies");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
            .expect("named static call has fixed fuel")
            .ceiling_units(),
        derive_fixed_entry_fuel(&baseline_verified, baseline.semantic_module.entry)
            .expect("baseline static call has fixed fuel")
            .ceiling_units(),
        "erased named lanes add no runtime fuel"
    );

    let rejects = |mut module: terminal_psi::TerminalModule,
                   mutate: fn(&mut terminal_psi::StaticRequirementDispatch)| {
        mutate(
            module.proof_output_calls[0]
                .static_requirement_dispatch
                .as_mut()
                .expect("static dispatch"),
        );
        assert!(matches!(
            terminal_verifier::validate_module_representation(&module),
            Err(
                terminal_verifier::ModuleError::InvalidClosedConformanceApplication { .. }
                    | terminal_verifier::ModuleError::InvalidProofOutputCall { .. }
            )
        ));
    };
    rejects(lowered.semantic_module.clone(), |dispatch| {
        dispatch.conformance_application_report_fingerprint ^= 1;
    });
    let original_application = lowered.semantic_module.closed_conformance_applications[0].clone();
    let mut compact_equal_substitute = original_application.clone();
    compact_equal_substitute
        .trait_arguments
        .push("forged-static-argument".to_owned());
    let substitute_commitment =
        terminal_psi::closed_conformance_application_commitment(&compact_equal_substitute);
    assert_ne!(substitute_commitment, original_application.commitment);
    let mut substituted_dispatch = lowered.semantic_module.clone();
    let dispatch = substituted_dispatch.proof_output_calls[0]
        .static_requirement_dispatch
        .as_mut()
        .expect("static dispatch");
    let compact_coordinate = dispatch.conformance_application_report_fingerprint;
    dispatch.conformance_application_commitment = substitute_commitment;
    assert_eq!(
        dispatch.conformance_application_report_fingerprint,
        compact_coordinate
    );
    assert!(matches!(
        terminal_verifier::validate_module_representation(&substituted_dispatch),
        Err(
            terminal_verifier::ModuleError::InvalidClosedConformanceApplication { .. }
                | terminal_verifier::ModuleError::InvalidProofOutputCall { .. }
        )
    ));
    rejects(lowered.semantic_module.clone(), |dispatch| {
        dispatch.public_requirement_identity.push_str("::forged");
    });
    rejects(lowered.semantic_module.clone(), |dispatch| {
        dispatch.declaring_trait_identity.push_str("::forged");
    });
    rejects(lowered.semantic_module.clone(), |dispatch| {
        dispatch.requirement_identity.push_str("::forged");
    });
    rejects(lowered.semantic_module.clone(), |dispatch| {
        dispatch.realization_identity.push_str("::forged");
    });
    rejects(lowered.semantic_module.clone(), |dispatch| {
        dispatch.realization_callable_identity.push_str("::forged");
    });
    rejects(lowered.semantic_module.clone(), |dispatch| {
        dispatch.realization = semantic_vocabulary::MachineId::new(999).unwrap();
    });
    let mut drifted_callable_binding = lowered.semantic_module.clone();
    let application = drifted_callable_binding
        .closed_conformance_applications
        .iter_mut()
        .find(|application| application.owner == invocation.caller)
        .expect("entry closed application");
    application
        .rows
        .iter_mut()
        .find_map(|row| row.realization_callable_identity.as_mut())
        .expect("standalone callable reference")
        .push_str("::forged");
    application.report_fingerprint =
        terminal_psi::closed_conformance_application_report_fingerprint(application);
    application.commitment = terminal_psi::closed_conformance_application_commitment(application);
    let drifted_dispatch = drifted_callable_binding.proof_output_calls[0]
        .static_requirement_dispatch
        .as_mut()
        .expect("static dispatch");
    drifted_dispatch.conformance_application_report_fingerprint = application.report_fingerprint;
    drifted_dispatch.conformance_application_commitment = application.commitment;
    assert!(matches!(
        terminal_verifier::validate_module_representation(&drifted_callable_binding),
        Err(terminal_verifier::ModuleError::InvalidClosedConformanceApplication { .. })
    ));

    let mut unused_callable_registry = lowered.semantic_module.clone();
    let application = unused_callable_registry
        .closed_conformance_applications
        .iter_mut()
        .find(|application| application.owner == invocation.caller)
        .expect("entry closed application");
    application
        .rows
        .iter_mut()
        .find_map(|row| row.realization_callable_identity.take())
        .expect("selected row registry reference");
    application.report_fingerprint =
        terminal_psi::closed_conformance_application_report_fingerprint(application);
    application.commitment = terminal_psi::closed_conformance_application_commitment(application);
    assert!(matches!(
        terminal_verifier::validate_module_representation(&unused_callable_registry),
        Err(terminal_verifier::ModuleError::InvalidClosedConformanceApplication { .. })
    ));

    let mut duplicate_callable_registry = lowered.semantic_module.clone();
    let application = duplicate_callable_registry
        .closed_conformance_applications
        .iter_mut()
        .find(|application| application.owner == invocation.caller)
        .expect("entry closed application");
    application
        .realization_callables
        .push(application.realization_callables[0].clone());
    application.report_fingerprint =
        terminal_psi::closed_conformance_application_report_fingerprint(application);
    application.commitment = terminal_psi::closed_conformance_application_commitment(application);
    assert!(matches!(
        terminal_verifier::validate_module_representation(&duplicate_callable_registry),
        Err(terminal_verifier::ModuleError::InvalidClosedConformanceApplication { .. })
    ));

    let mut retargeted_callable_registry = lowered.semantic_module.clone();
    let application = retargeted_callable_registry
        .closed_conformance_applications
        .iter_mut()
        .find(|application| application.owner == invocation.caller)
        .expect("entry closed application");
    application.realization_callables[0].machine = application.owner;
    application.report_fingerprint =
        terminal_psi::closed_conformance_application_report_fingerprint(application);
    application.commitment = terminal_psi::closed_conformance_application_commitment(application);
    let report_fingerprint = application.report_fingerprint;
    let commitment = application.commitment;
    let dispatch = retargeted_callable_registry.proof_output_calls[0]
        .static_requirement_dispatch
        .as_mut()
        .expect("static dispatch");
    dispatch.conformance_application_report_fingerprint = report_fingerprint;
    dispatch.conformance_application_commitment = commitment;
    assert!(matches!(
        terminal_verifier::validate_module_representation(&retargeted_callable_registry),
        Err(
            terminal_verifier::ModuleError::InvalidClosedConformanceApplication { .. }
                | terminal_verifier::ModuleError::InvalidProofOutputCall { .. }
        )
    ));

    let mut coordinated_dispatch_retarget = lowered.semantic_module.clone();
    let original_realization = invocation.runtime_call.expect("runtime call").callee;
    let alternate = semantic_vocabulary::MachineId::new(999).expect("alternate machine identity");
    let mut alternate_machine = coordinated_dispatch_retarget
        .machines
        .iter()
        .find(|machine| machine.id == original_realization)
        .expect("static realization machine")
        .clone();
    alternate_machine.id = alternate;
    alternate_machine.contract.id =
        semantic_vocabulary::ContractId::new(999).expect("alternate contract");
    for (ordinal, ensure) in alternate_machine.contract.ensures.iter_mut().enumerate() {
        ensure.obligation = semantic_vocabulary::ObligationId::new(9_900 + ordinal as u64)
            .expect("alternate obligation");
    }
    for (ordinal, ensure) in alternate_machine
        .contract
        .outcome_specific_ensures
        .iter_mut()
        .enumerate()
    {
        ensure.obligation = semantic_vocabulary::ObligationId::new(9_950 + ordinal as u64)
            .expect("alternate guarded obligation");
    }
    coordinated_dispatch_retarget
        .machines
        .push(alternate_machine);
    let runtime_operation = coordinated_dispatch_retarget.proof_output_calls[0]
        .runtime_call
        .expect("runtime call")
        .operation;
    let original_realization_identity = coordinated_dispatch_retarget.proof_output_calls[0]
        .static_requirement_dispatch
        .as_ref()
        .expect("static dispatch")
        .realization_identity
        .clone();
    let application = coordinated_dispatch_retarget
        .closed_conformance_applications
        .iter_mut()
        .find(|application| application.owner == invocation.caller)
        .expect("entry closed application");
    let row = application
        .rows
        .iter_mut()
        .find(|row| row.realization_identity == original_realization_identity)
        .expect("selected static conformance row");
    row.realization_identity.push_str("::coordinated-retarget");
    let retargeted_realization_identity = row.realization_identity.clone();
    let callable_identity = row
        .realization_callable_identity
        .clone()
        .expect("selected row references the independent registry");
    assert_eq!(
        application
            .realization_callables
            .iter()
            .find(|callable| callable.source_callable_identity == callable_identity)
            .expect("standalone callable binding")
            .machine,
        original_realization,
        "the independent source-callable map remains rooted at the original machine"
    );
    application.report_fingerprint =
        terminal_psi::closed_conformance_application_report_fingerprint(application);
    application.commitment = terminal_psi::closed_conformance_application_commitment(application);
    let application_report_fingerprint = application.report_fingerprint;
    let application_commitment = application.commitment;
    coordinated_dispatch_retarget.proof_output_calls[0]
        .runtime_call
        .as_mut()
        .expect("runtime call")
        .callee = alternate;
    let coordinated_dispatch = coordinated_dispatch_retarget.proof_output_calls[0]
        .static_requirement_dispatch
        .as_mut()
        .expect("static dispatch");
    coordinated_dispatch.conformance_application_report_fingerprint =
        application_report_fingerprint;
    coordinated_dispatch.conformance_application_commitment = application_commitment;
    coordinated_dispatch.realization_identity = retargeted_realization_identity;
    coordinated_dispatch.realization = alternate;
    let operation = coordinated_dispatch_retarget
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| operation.id == runtime_operation)
        .expect("linked runtime operation");
    let OperationKind::CallUnit { callee, .. } = &mut operation.kind else {
        panic!("Unit fixture retains one ordinary Unit call")
    };
    *callee = alternate;
    let coordinated_result =
        terminal_verifier::validate_module_representation(&coordinated_dispatch_retarget);
    assert!(
        matches!(
            coordinated_result,
            Err(
                terminal_verifier::ModuleError::InvalidClosedConformanceApplication { .. }
                    | terminal_verifier::ModuleError::InvalidProofOutputCall { .. }
            )
        ),
        "coordinated dispatch retarget must reject through its unchanged callable binding, got {coordinated_result:?}"
    );
    let mut deleted_dispatch = lowered.semantic_module.clone();
    deleted_dispatch.proof_output_calls[0].static_requirement_dispatch = None;
    assert!(matches!(
        terminal_verifier::validate_module_representation(&deleted_dispatch),
        Err(terminal_verifier::ModuleError::InvalidClosedConformanceApplication { .. })
    ));
    let mut leaked_input = lowered.semantic_module.clone();
    let input_source = leaked_input.proof_output_calls[0].evidence_arguments[0].source;
    leaked_input.proof_output_calls[0].outputs[0].output = Some(input_source);
    assert!(matches!(
        terminal_verifier::validate_module_representation(&leaked_input),
        Err(terminal_verifier::ModuleError::InvalidProofOutputCall { .. })
    ));
    let mut exposed_callee = lowered.semantic_module.clone();
    exposed_callee.proof_output_calls[0].outputs[0].callee_output = Some(input_source);
    assert!(matches!(
        terminal_verifier::validate_module_representation(&exposed_callee),
        Err(terminal_verifier::ModuleError::InvalidProofOutputCall { .. })
    ));
    let mut renamed_row = lowered.semantic_module.clone();
    let renamed_entry = renamed_row.entry;
    let application = renamed_row
        .closed_conformance_applications
        .iter_mut()
        .find(|application| application.owner == renamed_entry)
        .expect("entry closed application");
    application.rows[0]
        .public_requirement_identity
        .push_str("::forged");
    application.report_fingerprint =
        terminal_psi::closed_conformance_application_report_fingerprint(application);
    application.commitment = terminal_psi::closed_conformance_application_commitment(application);
    renamed_row.proof_output_calls[0]
        .static_requirement_dispatch
        .as_mut()
        .expect("static dispatch")
        .conformance_application_report_fingerprint = application.report_fingerprint;
    renamed_row.proof_output_calls[0]
        .static_requirement_dispatch
        .as_mut()
        .expect("static dispatch")
        .conformance_application_commitment = application.commitment;
    assert!(matches!(
        terminal_verifier::validate_module_representation(&renamed_row),
        Err(terminal_verifier::ModuleError::InvalidClosedConformanceApplication { .. })
    ));
    let mut deleted_row = lowered.semantic_module.clone();
    let deleted_entry = deleted_row.entry;
    let application = deleted_row
        .closed_conformance_applications
        .iter_mut()
        .find(|application| application.owner == deleted_entry)
        .expect("entry closed application");
    application.rows.clear();
    application.report_fingerprint =
        terminal_psi::closed_conformance_application_report_fingerprint(application);
    application.commitment = terminal_psi::closed_conformance_application_commitment(application);
    deleted_row.proof_output_calls[0]
        .static_requirement_dispatch
        .as_mut()
        .expect("static dispatch")
        .conformance_application_report_fingerprint = application.report_fingerprint;
    deleted_row.proof_output_calls[0]
        .static_requirement_dispatch
        .as_mut()
        .expect("static dispatch")
        .conformance_application_commitment = application.commitment;
    assert!(matches!(
        terminal_verifier::validate_module_representation(&deleted_row),
        Err(terminal_verifier::ModuleError::InvalidClosedConformanceApplication { .. })
    ));
    let mut widened_application = lowered.semantic_module.clone();
    let widened_entry = widened_application.entry;
    let application = widened_application
        .closed_conformance_applications
        .iter_mut()
        .find(|application| application.owner == widened_entry)
        .expect("entry closed application");
    application.trait_arguments.push("forged".to_owned());
    application.report_fingerprint =
        terminal_psi::closed_conformance_application_report_fingerprint(application);
    application.commitment = terminal_psi::closed_conformance_application_commitment(application);
    widened_application.proof_output_calls[0]
        .static_requirement_dispatch
        .as_mut()
        .expect("static dispatch")
        .conformance_application_report_fingerprint = application.report_fingerprint;
    widened_application.proof_output_calls[0]
        .static_requirement_dispatch
        .as_mut()
        .expect("static dispatch")
        .conformance_application_commitment = application.commitment;
    assert!(matches!(
        terminal_verifier::validate_module_representation(&widened_application),
        Err(terminal_verifier::ModuleError::InvalidProofOutputCall { .. })
    ));
}

#[test]
fn static_requirement_i32_result_uses_one_ordinary_scalar_call_without_runtime_overhead() {
    let checked = crate::front_end::checked_program(STATIC_REQUIREMENT_I32_PROOF_OUTPUT_SOURCE);
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
        .expect("one checked i32 static requirement proof-output call");
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
        .expect("the free specialized i32 caller is terminal-selected");
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name(&machine_name),
    )
    .expect("the exact i32 static requirement call crosses Terminal Psi");
    let [invocation] = lowered.semantic_module.proof_output_calls.as_slice() else {
        panic!("one terminal i32 static requirement proof-output call")
    };
    let dispatch = invocation
        .static_requirement_dispatch
        .as_ref()
        .expect("private i32 realization dispatch");
    let i32_type = semantic_vocabulary::ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
            .expect("i32"),
    );
    assert_eq!(
        invocation.runtime_result,
        Some(terminal_psi::ProofOutputRuntimeResult::Scalar(i32_type))
    );
    let runtime_call = invocation
        .runtime_call
        .expect("static i32 proof output links its ordinary call");
    assert_eq!(runtime_call.callee, dispatch.realization);
    let application = lowered
        .semantic_module
        .closed_conformance_applications
        .iter()
        .find(|application| {
            application.owner == invocation.caller
                && application.commitment == dispatch.conformance_application_commitment
        })
        .expect("i32 dispatch retains its closed application");
    let row_identity = application
        .rows
        .iter()
        .find_map(|row| row.realization_callable_identity.as_ref())
        .expect("i32 row references its standalone callable registry");
    let callable = application
        .realization_callables
        .iter()
        .find(|callable| callable.source_callable_identity == *row_identity)
        .expect("i32 registry retains its standalone callable binding");
    assert_eq!(callable.machine, dispatch.realization);
    assert_eq!(
        callable.result,
        terminal_psi::ClosedConformanceCallableResult::I32
    );
    assert_eq!(
        callable.source_callable_identity,
        dispatch.realization_callable_identity
    );
    let caller = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == invocation.caller)
        .expect("i32 proof-output caller");
    assert_eq!(caller.attachment, None, "the scalar caller stays free");
    let realization = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == dispatch.realization)
        .expect("i32 static realization");
    assert_eq!(
        realization.attachment, None,
        "no runtime receiver is emitted"
    );
    assert!(realization.parameters.is_empty());
    assert!(realization.structural_parameters.is_empty());
    assert!(matches!(
        realization.result,
        terminal_psi::TerminalMachineResult::Scalar(result)
            if result.scalar_type == i32_type
    ));
    let operation = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == runtime_call.operation)
        .expect("linked i32 call operation");
    assert!(matches!(
        (&operation.result, &operation.kind),
        (
            terminal_psi::OperationResult::Scalar(result),
            OperationKind::Call {
                callee,
                arguments,
                ..
            }
        ) if result.scalar_type == i32_type
            && *callee == dispatch.realization
            && arguments.is_empty()
    ));

    let semantic_bytes =
        encode_module(&lowered.semantic_module).expect("static i32 module encodes");
    let proof_bytes = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("static i32 proof encodes");
    let decoded = decode_module(&semantic_bytes).expect("static i32 module decodes");
    assert_eq!(decoded, lowered.semantic_module);
    let decoded_proof = decode_proof_bundle(&proof_bytes).expect("static i32 proof decodes");
    assert_eq!(decoded_proof, lowered.proof_bundle);
    let verified =
        terminal_verifier::verify_module(&decoded, &decoded_proof, &AdmissionProfile::default())
            .expect("the exact static i32 operation and dispatch verify together");

    let baseline_checked =
        crate::front_end::checked_program(STATIC_REQUIREMENT_I32_RUNTIME_BASELINE_SOURCE);
    let baseline_name = baseline_checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find_map(|selection| (selection.name == "caller").then(|| selection.name.clone()))
        .expect("the scalar runtime baseline is terminal-selected");
    let baseline = checked_trees_to_lowered_psi::lower_machine(
        &baseline_checked,
        TerminalMachineSelection::Name(&baseline_name),
    )
    .expect("the scalar runtime baseline lowers");
    let baseline_verified = terminal_verifier::verify_module(
        &baseline.semantic_module,
        &baseline.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the scalar runtime baseline verifies");
    let baseline_entry = baseline
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == baseline.semantic_module.entry)
        .expect("scalar baseline entry");
    assert_eq!(caller.parameters, baseline_entry.parameters);
    assert_eq!(caller.result, baseline_entry.result);
    assert_eq!(
        caller
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .map(|operation| std::mem::discriminant(&operation.kind))
            .collect::<Vec<_>>(),
        baseline_entry
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .map(|operation| std::mem::discriminant(&operation.kind))
            .collect::<Vec<_>>(),
        "erased static proof rows add no runtime operations"
    );
    assert_eq!(
        derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
            .expect("static i32 call has fixed fuel")
            .ceiling_units(),
        derive_fixed_entry_fuel(&baseline_verified, baseline.semantic_module.entry)
            .expect("baseline i32 call has fixed fuel")
            .ceiling_units(),
        "erased static proof rows add no runtime fuel"
    );

    let mut execution = TerminalExecution::start_artifact(
        &semantic_bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
    )
    .expect("static i32 artifact starts");
    assert_eq!(
        execution
            .resume(
                &mut TerminalFuelMeter::unbounded(),
                &mut AcceptTerminalEffects
            )
            .expect("execute static i32 requirement call"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Integer {
                scalar_type: semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Signed,
                    32
                )
                .expect("i32"),
                value: IntegerValue::Signed(17),
            }
        ))
    );

    let invalid = |module: &terminal_psi::TerminalModule| {
        matches!(
            terminal_verifier::validate_module_representation(module),
            Err(
                terminal_verifier::ModuleError::InvalidClosedConformanceApplication { .. }
                    | terminal_verifier::ModuleError::InvalidProofOutputCall { .. }
            )
        )
    };
    let mut wrong_type = lowered.semantic_module.clone();
    wrong_type.proof_output_calls[0].runtime_result = Some(
        terminal_psi::ProofOutputRuntimeResult::Scalar(semantic_vocabulary::ScalarType::Boolean),
    );
    assert!(invalid(&wrong_type));
    let mut unit_result = lowered.semantic_module.clone();
    unit_result.proof_output_calls[0].runtime_result =
        Some(terminal_psi::ProofOutputRuntimeResult::Unit);
    assert!(invalid(&unit_result));
    let mut unknown_operation = lowered.semantic_module.clone();
    unknown_operation.proof_output_calls[0]
        .runtime_call
        .as_mut()
        .expect("runtime call")
        .operation = OperationId::new(u64::MAX).expect("nonzero operation ID");
    assert!(invalid(&unknown_operation));
    let mut wrong_callee = lowered.semantic_module.clone();
    wrong_callee.proof_output_calls[0]
        .runtime_call
        .as_mut()
        .expect("runtime call")
        .callee = invocation.caller;
    assert!(invalid(&wrong_callee));
    let mut wrong_realization = lowered.semantic_module.clone();
    wrong_realization.proof_output_calls[0]
        .static_requirement_dispatch
        .as_mut()
        .expect("static dispatch")
        .realization = invocation.caller;
    assert!(invalid(&wrong_realization));
    let mut wrong_operation = lowered.semantic_module.clone();
    wrong_operation
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|candidate| candidate.id == runtime_call.operation)
        .expect("linked operation")
        .kind = OperationKind::IntegerConstant {
        value: IntegerValue::Signed(17),
    };
    assert!(invalid(&wrong_operation));

    let mut forged_argument = lowered.semantic_module.clone();
    let argument = terminal_psi::ValueDeclaration {
        qualifications: Default::default(),
        id: ValueId::new(u64::MAX).expect("nonzero forged argument ID"),
        scalar_type: i32_type,
    };
    forged_argument
        .machines
        .iter_mut()
        .find(|machine| machine.id == invocation.caller)
        .expect("forged caller")
        .parameters
        .push(argument);
    forged_argument
        .machines
        .iter_mut()
        .find(|machine| machine.id == dispatch.realization)
        .expect("forged realization")
        .parameters
        .push(terminal_psi::ValueDeclaration {
            qualifications: Default::default(),
            id: ValueId::new(u64::MAX - 1).expect("nonzero forged parameter ID"),
            scalar_type: i32_type,
        });
    let OperationKind::Call { arguments, .. } = &mut forged_argument
        .machines
        .iter_mut()
        .find(|machine| machine.id == invocation.caller)
        .expect("forged caller")
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find(|candidate| candidate.id == runtime_call.operation)
        .expect("forged linked operation")
        .kind
    else {
        panic!("linked operation remains a scalar call")
    };
    arguments.push(argument.id);
    assert!(
        invalid(&forged_argument),
        "a coherent argument-bearing scalar dispatch remains outside the bounded rung"
    );
}

#[test]
fn static_requirement_bool_result_uses_one_ordinary_scalar_call_without_runtime_overhead() {
    let checked = crate::front_end::checked_program(STATIC_REQUIREMENT_BOOL_PROOF_OUTPUT_SOURCE);
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
        .expect("one checked bool static requirement proof-output call");
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
        .expect("the free specialized bool caller is terminal-selected");
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name(&machine_name),
    )
    .expect("the exact bool static requirement call crosses Terminal Psi");
    let [invocation] = lowered.semantic_module.proof_output_calls.as_slice() else {
        panic!("one terminal bool static requirement proof-output call")
    };
    let dispatch = invocation
        .static_requirement_dispatch
        .as_ref()
        .expect("private bool realization dispatch");
    assert_eq!(
        invocation.runtime_result,
        Some(terminal_psi::ProofOutputRuntimeResult::Scalar(
            semantic_vocabulary::ScalarType::Boolean
        ))
    );
    let runtime_call = invocation
        .runtime_call
        .expect("static bool proof output links its ordinary call");
    assert_eq!(runtime_call.callee, dispatch.realization);
    let application = lowered
        .semantic_module
        .closed_conformance_applications
        .iter()
        .find(|application| {
            application.owner == invocation.caller
                && application.commitment == dispatch.conformance_application_commitment
        })
        .expect("bool dispatch retains its closed application");
    let row_identity = application
        .rows
        .iter()
        .find_map(|row| row.realization_callable_identity.as_ref())
        .expect("bool row references its standalone callable registry");
    assert!(application.realization_callables.iter().any(|callable| {
        callable.source_callable_identity == *row_identity
            && callable.source_callable_identity == dispatch.realization_callable_identity
            && callable.machine == dispatch.realization
            && callable.result == terminal_psi::ClosedConformanceCallableResult::Bool
    }));
    let caller = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == invocation.caller)
        .expect("bool proof-output caller");
    let realization = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == dispatch.realization)
        .expect("bool static realization");
    assert_eq!(caller.attachment, None);
    assert_eq!(realization.attachment, None);
    assert!(realization.parameters.is_empty());
    assert!(realization.structural_parameters.is_empty());
    assert!(matches!(
        realization.result,
        terminal_psi::TerminalMachineResult::Scalar(result)
            if result.scalar_type == semantic_vocabulary::ScalarType::Boolean
    ));
    let operation = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == runtime_call.operation)
        .expect("linked bool call operation");
    assert!(matches!(
        (&operation.result, &operation.kind),
        (
            terminal_psi::OperationResult::Scalar(result),
            OperationKind::Call {
                callee,
                arguments,
                ..
            }
        ) if result.scalar_type == semantic_vocabulary::ScalarType::Boolean
            && *callee == dispatch.realization
            && arguments.is_empty()
    ));

    let semantic_bytes =
        encode_module(&lowered.semantic_module).expect("static bool module encodes");
    let proof_bytes = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("static bool proof encodes");
    let decoded = decode_module(&semantic_bytes).expect("static bool module decodes");
    let decoded_proof = decode_proof_bundle(&proof_bytes).expect("static bool proof decodes");
    assert_eq!(decoded, lowered.semantic_module);
    assert_eq!(decoded_proof, lowered.proof_bundle);
    let mut callable_wire = Vec::new();
    callable_wire.extend(
        u32::try_from(row_identity.len())
            .expect("callable identity length")
            .to_le_bytes(),
    );
    callable_wire.extend(row_identity.as_bytes());
    callable_wire.extend(dispatch.realization.get().to_le_bytes());
    callable_wire.push(3);
    let callable_offset = semantic_bytes
        .windows(callable_wire.len())
        .position(|window| window == callable_wire)
        .expect("canonical Bool callable registry wire row");
    let mut invalid_callable_tag = semantic_bytes.clone();
    invalid_callable_tag[callable_offset + callable_wire.len() - 1] = u8::MAX;
    assert_eq!(
        decode_module(&invalid_callable_tag),
        Err(terminal_codec::CodecError::InvalidTag(
            "ClosedConformanceCallableResult",
            u8::MAX,
        ))
    );
    let verified =
        terminal_verifier::verify_module(&decoded, &decoded_proof, &AdmissionProfile::default())
            .expect("the exact static bool operation and dispatch verify together");

    let baseline_checked =
        crate::front_end::checked_program(STATIC_REQUIREMENT_BOOL_RUNTIME_BASELINE_SOURCE);
    let baseline_name = baseline_checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find_map(|selection| (selection.name == "caller").then(|| selection.name.clone()))
        .expect("the bool runtime baseline is terminal-selected");
    let baseline = checked_trees_to_lowered_psi::lower_machine(
        &baseline_checked,
        TerminalMachineSelection::Name(&baseline_name),
    )
    .expect("the bool runtime baseline lowers");
    let baseline_verified = terminal_verifier::verify_module(
        &baseline.semantic_module,
        &baseline.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the bool runtime baseline verifies");
    let baseline_entry = baseline
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == baseline.semantic_module.entry)
        .expect("bool baseline entry");
    assert_eq!(caller.parameters, baseline_entry.parameters);
    assert_eq!(caller.result, baseline_entry.result);
    assert_eq!(
        caller
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .map(|operation| std::mem::discriminant(&operation.kind))
            .collect::<Vec<_>>(),
        baseline_entry
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .map(|operation| std::mem::discriminant(&operation.kind))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
            .expect("static bool call has fixed fuel")
            .ceiling_units(),
        derive_fixed_entry_fuel(&baseline_verified, baseline.semantic_module.entry)
            .expect("baseline bool call has fixed fuel")
            .ceiling_units()
    );
    let mut execution = TerminalExecution::start_artifact(
        &semantic_bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
    )
    .expect("static bool artifact starts");
    assert_eq!(
        execution
            .resume(
                &mut TerminalFuelMeter::unbounded(),
                &mut AcceptTerminalEffects
            )
            .expect("execute static bool requirement call"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Boolean(true)
        ))
    );

    let invalid = |module: &terminal_psi::TerminalModule| {
        matches!(
            terminal_verifier::validate_module_representation(module),
            Err(
                terminal_verifier::ModuleError::InvalidClosedConformanceApplication { .. }
                    | terminal_verifier::ModuleError::InvalidProofOutputCall { .. }
            )
        )
    };
    let i32_type = semantic_vocabulary::ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
            .expect("i32"),
    );
    let mut wrong_invocation_type = lowered.semantic_module.clone();
    wrong_invocation_type.proof_output_calls[0].runtime_result =
        Some(terminal_psi::ProofOutputRuntimeResult::Scalar(i32_type));
    assert!(invalid(&wrong_invocation_type));
    let mut wrong_realization_type = lowered.semantic_module.clone();
    let terminal_psi::TerminalMachineResult::Scalar(result) = &mut wrong_realization_type
        .machines
        .iter_mut()
        .find(|machine| machine.id == dispatch.realization)
        .expect("bool realization")
        .result
    else {
        panic!("bool realization remains scalar")
    };
    result.scalar_type = i32_type;
    assert!(invalid(&wrong_realization_type));

    let mut coordinated_runtime_retarget = lowered.semantic_module.clone();
    coordinated_runtime_retarget.proof_output_calls[0].runtime_result =
        Some(terminal_psi::ProofOutputRuntimeResult::Scalar(i32_type));
    for machine in &mut coordinated_runtime_retarget.machines {
        if matches!(
            machine.result,
            terminal_psi::TerminalMachineResult::Scalar(_)
        ) {
            let terminal_psi::TerminalMachineResult::Scalar(result) = &mut machine.result else {
                unreachable!()
            };
            result.scalar_type = i32_type;
        }
        for operation in machine
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
        {
            if let terminal_psi::OperationResult::Scalar(result) = &mut operation.result {
                result.scalar_type = i32_type;
            }
            if matches!(operation.kind, OperationKind::BooleanConstant { .. }) {
                operation.kind = OperationKind::IntegerConstant {
                    value: IntegerValue::Signed(1),
                };
            }
        }
    }
    assert!(
        matches!(
            terminal_verifier::validate_module_representation(&coordinated_runtime_retarget),
            Err(terminal_verifier::ModuleError::InvalidClosedConformanceApplication { .. })
        ),
        "the source-derived registry class blocks a coherent Bool-to-i32 runtime retarget"
    );

    let mut retargeted_registry = coordinated_runtime_retarget;
    retargeted_registry
        .closed_conformance_applications
        .iter_mut()
        .find(|application| application.owner == invocation.caller)
        .expect("bool closed application")
        .realization_callables[0]
        .result = terminal_psi::ClosedConformanceCallableResult::I32;
    let application = retargeted_registry
        .closed_conformance_applications
        .iter_mut()
        .find(|application| application.owner == invocation.caller)
        .expect("bool closed application");
    application.report_fingerprint =
        terminal_psi::closed_conformance_application_report_fingerprint(application);
    retargeted_registry.proof_output_calls[0]
        .static_requirement_dispatch
        .as_mut()
        .expect("bool static dispatch")
        .conformance_application_report_fingerprint = application.report_fingerprint;
    assert!(matches!(
        terminal_verifier::validate_module_representation(&retargeted_registry),
        Err(terminal_verifier::ModuleError::ClosedConformanceCommitmentMismatch { .. })
    ));
}
