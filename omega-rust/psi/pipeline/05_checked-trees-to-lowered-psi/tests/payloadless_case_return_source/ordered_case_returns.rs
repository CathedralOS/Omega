use super::SELECTED_WITNESS_TAIL_USE_SOURCE;
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use terminal_codec::{decode_module, encode_module, encode_proof_section};
use terminal_fixed_fuel::derive_fixed_entry_fuel;
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::AcceptTerminalEffects;
use terminal_interpreter::TerminalStructuralInputs;
use terminal_interpreter::{TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus};
use terminal_psi::{OperationKind, Terminator};

#[test]
fn selected_witness_tail_use_is_canonical_and_runtime_free() {
    let checked = crate::front_end::checked_program(SELECTED_WITNESS_TAIL_USE_SOURCE);
    let caller_symbol = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("caller"))
        .unwrap()
        .symbol;
    let plan = checked
        .facts
        .flow
        .terminal_structural_call_returns
        .payloadless_guarded_for_machine(caller_symbol)
        .expect("the exact selected-witness tail use has a checked plan");
    let [checked_selection] = plan.selected_evidence.as_slice() else {
        panic!("one selected checked row")
    };
    assert!(checked_selection.tail_use.is_some());

    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::caller"),
    )
    .expect("the exact selected-witness tail use lowers");
    let module = &lowered.semantic_module;
    let [caller, _callee, target] = module.machines.as_slice() else {
        panic!("caller, producer, and proof-visible tail target remain canonical")
    };
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &caller.blocks[0].operations[0].kind
    else {
        panic!("the producer call remains structural")
    };
    let [selected] = selected_evidence.as_slice() else {
        panic!("one selected Terminal row")
    };
    let [use_] = selected.uses.as_slice() else {
        panic!("one exact selected-term use")
    };
    assert_eq!(selected.expected_use_count, 1);
    assert_eq!(use_.target, target.id);
    assert_eq!(use_.source, selected.output);
    assert_eq!(
        use_.instantiated_proposition,
        selected.instantiated_proposition
    );
    assert_eq!(
        target.contract.requires,
        [semantic_vocabulary::Proposition::Atom(
            use_.target_requirement
        )]
    );
    assert!(target.blocks[0].operations.is_empty());

    let bytes = encode_module(module).expect("selected-witness semantics encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("selected-witness proof encodes");
    let verified = terminal_verifier::verify_module(
        module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the independent tail requirement replays");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, module.entry)
            .expect("proof-only tail use does not add fuel")
            .ceiling_units(),
        4
    );
    let mut execution = TerminalExecution::start_artifact(
        &bytes,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
    )
    .expect("selected-witness artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(4);
    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .expect("artifact completes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::ScalarCase(_))
    ));

    let mutate = |mut module: terminal_psi::TerminalModule,
                  change: fn(&mut terminal_psi::OutcomeSpecificCallEvidence)| {
        let OperationKind::CallStructural {
            selected_evidence, ..
        } = &mut module.machines[0].blocks[0].operations[0].kind
        else {
            unreachable!()
        };
        change(&mut selected_evidence[0]);
        assert!(terminal_verifier::validate_module(&module).is_err());
    };
    mutate(module.clone(), |selected| selected.uses.clear());
    mutate(module.clone(), |selected| {
        selected.expected_use_count = 0;
        selected.uses.clear();
    });
    mutate(module.clone(), |selected| {
        selected.uses.push(selected.uses[0].clone())
    });
    mutate(module.clone(), |selected| {
        selected.uses[0].target = semantic_vocabulary::MachineId::new(99).unwrap()
    });
    mutate(module.clone(), |selected| {
        selected.uses[0].input_position = 1
    });
    mutate(module.clone(), |selected| {
        selected.uses[0].target_requirement = selected.instantiated_proposition
    });

    let mut missing_lane = module.clone();
    missing_lane.machines[2].contract.requires.clear();
    assert!(terminal_verifier::validate_module(&missing_lane).is_err());
    let mut wrong_interface = module.clone();
    let target_term = selected.uses[0].target_term;
    wrong_interface
        .evidence_terms
        .iter_mut()
        .find(|term| term.id == target_term)
        .unwrap()
        .interface
        .trait_identity
        .push_str("::mutated");
    assert!(terminal_verifier::validate_module(&wrong_interface).is_err());
    let mut wrong_tail = module.clone();
    let result = wrong_tail.machines[2].result.structural().unwrap().place;
    let Terminator::ReturnStructural { source, .. } =
        &mut wrong_tail.machines[2].blocks[0].terminator
    else {
        unreachable!()
    };
    *source = result;
    assert!(terminal_verifier::validate_module(&wrong_tail).is_err());

    let mut omitted_checked_use = checked.clone();
    omitted_checked_use
        .facts
        .flow
        .terminal_structural_call_returns
        .payloadless_guarded_machines[0]
        .selected_evidence[0]
        .tail_use = None;
    assert!(
        checked_trees_to_lowered_psi::lower_machine(
            &omitted_checked_use,
            TerminalMachineSelection::Name("Root::caller")
        )
        .is_err()
    );
}
