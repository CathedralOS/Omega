use super::{
    FIFTEEN_SELECTED_WITNESS_TAIL_USES_SOURCE, FIVE_SELECTED_WITNESS_TAIL_USES_SOURCE,
    FOUR_SELECTED_WITNESS_TAIL_USES_SOURCE, GUARDED_CALL_SOURCE,
    MULTI_SELECTED_GUARDED_CALL_SOURCE, SEVEN_SELECTED_WITNESS_TAIL_USES_SOURCE,
    SIX_SELECTED_WITNESS_TAIL_USES_SOURCE, THREE_SELECTED_WITNESS_TAIL_USES_SOURCE,
    TWO_SELECTED_WITNESS_TAIL_USES_SOURCE, append_rejoined_selected_evidence_row,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use terminal_codec::{
    CodecError, decode_module, decode_proof_bundle, encode_module, encode_proof_section,
};
use terminal_fixed_fuel::derive_fixed_entry_fuel;
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarCaseResult,
    TerminalScalarCaseValue,
};
use terminal_psi::OperationKind;

#[test]
fn two_selected_witness_tail_uses_are_ordered_distinct_and_runtime_free() {
    let checked = crate::front_end::checked_program(TWO_SELECTED_WITNESS_TAIL_USES_SOURCE);
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
        .expect("the exact two-witness tail use has a checked plan");
    let [first_plan, second_plan] = plan.selected_evidence.as_slice() else {
        panic!("two selected checked rows")
    };
    assert_ne!(first_plan.selected_term, second_plan.selected_term);
    assert_eq!(first_plan.tail_use.as_ref().unwrap().input_position, 0);
    assert_eq!(second_plan.tail_use.as_ref().unwrap().input_position, 1);

    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::caller"),
    )
    .expect("the exact two-witness tail use lowers");
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
    let [first, second] = selected_evidence.as_slice() else {
        panic!("two selected Terminal rows")
    };
    assert_ne!(first.output, second.output);
    let [first_use] = first.uses.as_slice() else {
        panic!("one use of the first selected row")
    };
    let [second_use] = second.uses.as_slice() else {
        panic!("one use of the second selected row")
    };
    assert_eq!(
        (first.expected_use_count, second.expected_use_count),
        (1, 1)
    );
    assert_eq!(
        (first_use.input_position, second_use.input_position),
        (0, 1)
    );
    assert_eq!(first_use.target, target.id);
    assert_eq!(second_use.target, target.id);
    assert_eq!(first_use.source, first.output);
    assert_eq!(second_use.source, second.output);
    assert_eq!(
        target.contract.requires,
        [
            semantic_vocabulary::Proposition::Atom(first_use.target_requirement),
            semantic_vocabulary::Proposition::Atom(second_use.target_requirement),
        ]
    );
    assert!(target.blocks[0].operations.is_empty());

    let bytes = encode_module(module).expect("two selected-witness rows encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("proof bundle encodes");
    let verified = terminal_verifier::verify_module(
        module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("both independent tail requirements replay");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, module.entry)
            .expect("proof-only uses do not add fuel")
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
    .expect("two-witness artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(4);
    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .expect("artifact completes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::ScalarCase(_))
    ));

    let mutate = |mut module: terminal_psi::TerminalModule,
                  change: fn(&mut [terminal_psi::OutcomeSpecificCallEvidence])| {
        let OperationKind::CallStructural {
            selected_evidence, ..
        } = &mut module.machines[0].blocks[0].operations[0].kind
        else {
            unreachable!()
        };
        change(selected_evidence);
        assert!(terminal_verifier::validate_module(&module).is_err());
    };
    mutate(module.clone(), |rows| {
        let first_position = rows[0].uses[0].input_position;
        rows[0].uses[0].input_position = rows[1].uses[0].input_position;
        rows[1].uses[0].input_position = first_position;
    });
    mutate(module.clone(), |rows| {
        rows[1].uses[0].input_position = rows[0].uses[0].input_position;
    });
    mutate(module.clone(), |rows| {
        rows[1].output = rows[0].output;
        rows[1].uses[0].source = rows[0].output;
    });
    mutate(module.clone(), |rows| rows.swap(0, 1));
    mutate(module.clone(), |rows| {
        rows[1].uses[0].target_requirement = rows[0].uses[0].target_requirement;
    });
    mutate(module.clone(), |rows| {
        rows[1].guard.result_case = rows[0].guard.result_case;
        rows[1].position = 0;
    });

    let mut missing_argument = module.clone();
    missing_argument.machines[2].contract.requires.pop();
    assert!(terminal_verifier::validate_module(&missing_argument).is_err());
    let mut extra_argument = module.clone();
    let extra = extra_argument.machines[2].contract.requires[0].clone();
    extra_argument.machines[2].contract.requires.push(extra);
    assert!(terminal_verifier::validate_module(&extra_argument).is_err());
}

#[test]
fn three_selected_witness_tail_uses_are_dense_distinct_and_runtime_free() {
    let checked = crate::front_end::checked_program(THREE_SELECTED_WITNESS_TAIL_USES_SOURCE);
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
        .expect("the exact three-witness tail use has a checked plan");
    assert_eq!(plan.selected_evidence.len(), 3);
    assert_eq!(
        plan.selected_evidence
            .iter()
            .map(|selection| selection.tail_use.as_ref().unwrap().input_position)
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert!(
        plan.selected_evidence
            .windows(2)
            .all(|rows| { rows[0].selected_term != rows[1].selected_term })
    );

    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::caller"),
    )
    .expect("the exact three-witness tail use lowers");
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
    assert_eq!(selected_evidence.len(), 3);
    assert_eq!(target.contract.requires.len(), 3);
    let mut terminal_positions = Vec::new();
    for selected in selected_evidence {
        let [use_] = selected.uses.as_slice() else {
            panic!("each selected row has one exact use")
        };
        assert_eq!(selected.expected_use_count, 1);
        assert_eq!(use_.target, target.id);
        assert_eq!(use_.source, selected.output);
        let position = usize::try_from(use_.input_position).unwrap();
        terminal_positions.push(use_.input_position);
        assert_eq!(
            target.contract.requires[position],
            semantic_vocabulary::Proposition::Atom(use_.target_requirement)
        );
    }
    terminal_positions.sort_unstable();
    assert_eq!(terminal_positions, [0, 1, 2]);
    assert!(target.blocks[0].operations.is_empty());

    let bytes = encode_module(module).expect("three selected-witness rows encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("proof bundle encodes");
    let verified = terminal_verifier::verify_module(
        module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("all three independent tail requirements replay");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, module.entry)
            .expect("proof-only uses do not add fuel")
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
    .expect("three-witness artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(4);
    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .expect("artifact completes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::ScalarCase(_))
    ));

    let mut reordered = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut reordered.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence.swap(1, 2);
    assert!(terminal_verifier::validate_module(&reordered).is_err());

    let mut duplicated_lane = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut duplicated_lane.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence[2].uses[0].input_position = selected_evidence[1].uses[0].input_position;
    assert!(terminal_verifier::validate_module(&duplicated_lane).is_err());

    let mut missing_requirement = module.clone();
    missing_requirement.machines[2].contract.requires.pop();
    assert!(terminal_verifier::validate_module(&missing_requirement).is_err());
}

#[test]
fn four_selected_witness_tail_uses_are_dense_distinct_and_runtime_free() {
    let checked = crate::front_end::checked_program(FOUR_SELECTED_WITNESS_TAIL_USES_SOURCE);
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
        .expect("the exact four-witness tail use has a checked plan");
    assert_eq!(plan.selected_evidence.len(), 4);
    assert_eq!(
        plan.selected_evidence
            .iter()
            .map(|selection| selection.tail_use.as_ref().unwrap().input_position)
            .collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
    assert!(
        plan.selected_evidence
            .windows(2)
            .all(|rows| rows[0].selected_term != rows[1].selected_term)
    );

    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::caller"),
    )
    .expect("the exact four-witness tail use lowers");
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
    assert_eq!(selected_evidence.len(), 4);
    assert_eq!(target.contract.requires.len(), 4);
    let mut terminal_positions = Vec::new();
    for selected in selected_evidence {
        let [use_] = selected.uses.as_slice() else {
            panic!("each selected row has one exact use")
        };
        assert_eq!(selected.expected_use_count, 1);
        assert_eq!(use_.target, target.id);
        assert_eq!(use_.source, selected.output);
        let position = usize::try_from(use_.input_position).unwrap();
        terminal_positions.push(use_.input_position);
        assert_eq!(
            target.contract.requires[position],
            semantic_vocabulary::Proposition::Atom(use_.target_requirement)
        );
    }
    terminal_positions.sort_unstable();
    assert_eq!(terminal_positions, [0, 1, 2, 3]);
    assert!(target.blocks[0].operations.is_empty());

    let bytes = encode_module(module).expect("four selected-witness rows encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("proof bundle encodes");
    let verified = terminal_verifier::verify_module(
        module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("all four independent tail requirements replay");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, module.entry)
            .expect("proof-only uses do not add fuel")
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
    .expect("four-witness artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(4);
    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .expect("artifact completes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::ScalarCase(_))
    ));

    let mut reordered = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut reordered.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence.swap(2, 3);
    assert!(terminal_verifier::validate_module(&reordered).is_err());

    let mut duplicated_lane = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut duplicated_lane.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence[3].uses[0].input_position = selected_evidence[2].uses[0].input_position;
    assert!(terminal_verifier::validate_module(&duplicated_lane).is_err());

    let mut missing_requirement = module.clone();
    missing_requirement.machines[2].contract.requires.pop();
    assert!(terminal_verifier::validate_module(&missing_requirement).is_err());
}

#[test]
fn five_selected_witness_tail_uses_are_dense_distinct_and_runtime_free() {
    let checked = crate::front_end::checked_program(FIVE_SELECTED_WITNESS_TAIL_USES_SOURCE);
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
        .expect("the exact five-witness tail use has a checked plan");
    assert_eq!(plan.selected_evidence.len(), 5);
    assert_eq!(
        plan.selected_evidence
            .iter()
            .map(|selection| selection.tail_use.as_ref().unwrap().input_position)
            .collect::<Vec<_>>(),
        [0, 1, 2, 3, 4]
    );
    assert!(
        plan.selected_evidence
            .windows(2)
            .all(|rows| rows[0].selected_term != rows[1].selected_term)
    );

    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::caller"),
    )
    .expect("the exact five-witness tail use lowers");
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
    assert_eq!(selected_evidence.len(), 5);
    assert_eq!(target.contract.requires.len(), 5);
    let mut terminal_positions = Vec::new();
    for selected in selected_evidence {
        let [use_] = selected.uses.as_slice() else {
            panic!("each selected row has one exact use")
        };
        assert_eq!(selected.expected_use_count, 1);
        assert_eq!(use_.target, target.id);
        assert_eq!(use_.source, selected.output);
        let position = usize::try_from(use_.input_position).unwrap();
        terminal_positions.push(use_.input_position);
        assert_eq!(
            target.contract.requires[position],
            semantic_vocabulary::Proposition::Atom(use_.target_requirement)
        );
    }
    terminal_positions.sort_unstable();
    assert_eq!(terminal_positions, [0, 1, 2, 3, 4]);
    assert!(target.blocks[0].operations.is_empty());

    let bytes = encode_module(module).expect("five selected-witness rows encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("proof bundle encodes");
    let verified = terminal_verifier::verify_module(
        module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("all five independent tail requirements replay");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, module.entry)
            .expect("proof-only uses do not add fuel")
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
    .expect("five-witness artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(4);
    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .expect("artifact completes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::ScalarCase(_))
    ));

    let mut reordered = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut reordered.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence.swap(3, 4);
    assert!(terminal_verifier::validate_module(&reordered).is_err());

    let mut duplicated_lane = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut duplicated_lane.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence[4].uses[0].input_position = selected_evidence[3].uses[0].input_position;
    assert!(terminal_verifier::validate_module(&duplicated_lane).is_err());

    let mut omitted_fifth = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut omitted_fifth.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence.pop();
    assert!(terminal_verifier::validate_module(&omitted_fifth).is_err());

    let mut six_terminal_rows = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut six_terminal_rows.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence.push(selected_evidence[4].clone());
    assert!(terminal_verifier::validate_module(&six_terminal_rows).is_err());
}

#[test]
fn six_selected_witness_tail_uses_are_dense_distinct_and_runtime_free() {
    let checked = crate::front_end::checked_program(SIX_SELECTED_WITNESS_TAIL_USES_SOURCE);
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
        .expect("the exact six-witness tail use has a checked plan");
    assert_eq!(plan.selected_evidence.len(), 6);
    assert_eq!(
        plan.selected_evidence
            .iter()
            .map(|selection| selection.tail_use.as_ref().unwrap().input_position)
            .collect::<Vec<_>>(),
        [0, 1, 2, 3, 4, 5]
    );
    assert!(
        plan.selected_evidence
            .windows(2)
            .all(|rows| rows[0].selected_term != rows[1].selected_term)
    );

    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::caller"),
    )
    .expect("the exact six-witness tail use lowers");
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
    assert_eq!(selected_evidence.len(), 6);
    assert_eq!(target.contract.requires.len(), 6);
    let mut terminal_positions = Vec::new();
    for selected in selected_evidence {
        let [use_] = selected.uses.as_slice() else {
            panic!("each selected row has one exact use")
        };
        assert_eq!(selected.expected_use_count, 1);
        assert_eq!(use_.target, target.id);
        assert_eq!(use_.source, selected.output);
        let position = usize::try_from(use_.input_position).unwrap();
        terminal_positions.push(use_.input_position);
        assert_eq!(
            target.contract.requires[position],
            semantic_vocabulary::Proposition::Atom(use_.target_requirement)
        );
    }
    terminal_positions.sort_unstable();
    assert_eq!(terminal_positions, [0, 1, 2, 3, 4, 5]);
    assert!(target.blocks[0].operations.is_empty());

    let bytes = encode_module(module).expect("six selected-witness rows encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("proof bundle encodes");
    let verified = terminal_verifier::verify_module(
        module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("all six independent tail requirements replay");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, module.entry)
            .expect("proof-only uses do not add fuel")
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
    .expect("six-witness artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(4);
    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .expect("artifact completes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::ScalarCase(_))
    ));

    let mut reordered = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut reordered.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence.swap(4, 5);
    assert!(terminal_verifier::validate_module(&reordered).is_err());

    let mut duplicated_lane = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut duplicated_lane.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence[5].uses[0].input_position = selected_evidence[4].uses[0].input_position;
    assert!(terminal_verifier::validate_module(&duplicated_lane).is_err());

    let mut omitted_sixth = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut omitted_sixth.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence.pop();
    assert!(terminal_verifier::validate_module(&omitted_sixth).is_err());

    let mut seven_terminal_rows = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut seven_terminal_rows.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence.push(selected_evidence[5].clone());
    assert!(terminal_verifier::validate_module(&seven_terminal_rows).is_err());
}

#[test]
fn seven_selected_witness_tail_uses_are_dense_distinct_and_runtime_free() {
    let checked = crate::front_end::checked_program(SEVEN_SELECTED_WITNESS_TAIL_USES_SOURCE);
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
        .expect("the exact seven-witness tail use has a checked plan");
    assert_eq!(plan.selected_evidence.len(), 7);
    assert_eq!(
        plan.selected_evidence
            .iter()
            .map(|selection| selection.tail_use.as_ref().unwrap().input_position)
            .collect::<Vec<_>>(),
        [0, 1, 2, 3, 4, 5, 6]
    );
    assert!(
        plan.selected_evidence
            .windows(2)
            .all(|rows| rows[0].selected_term != rows[1].selected_term)
    );

    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::caller"),
    )
    .expect("the exact seven-witness tail use lowers");
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
    assert_eq!(selected_evidence.len(), 7);
    assert_eq!(target.contract.requires.len(), 7);
    let mut terminal_positions = Vec::new();
    for selected in selected_evidence {
        let [use_] = selected.uses.as_slice() else {
            panic!("each selected row has one exact use")
        };
        assert_eq!(selected.expected_use_count, 1);
        assert_eq!(use_.target, target.id);
        assert_eq!(use_.source, selected.output);
        let position = usize::try_from(use_.input_position).unwrap();
        terminal_positions.push(use_.input_position);
        assert_eq!(
            target.contract.requires[position],
            semantic_vocabulary::Proposition::Atom(use_.target_requirement)
        );
    }
    terminal_positions.sort_unstable();
    assert_eq!(terminal_positions, [0, 1, 2, 3, 4, 5, 6]);
    assert!(target.blocks[0].operations.is_empty());

    let bytes = encode_module(module).expect("seven selected-witness rows encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("proof bundle encodes");
    let verified = terminal_verifier::verify_module(
        module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("all seven independent tail requirements replay");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, module.entry)
            .expect("proof-only uses do not add fuel")
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
    .expect("seven-witness artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(4);
    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .expect("artifact completes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::ScalarCase(_))
    ));

    let mut reordered = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut reordered.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence.swap(5, 6);
    assert!(terminal_verifier::validate_module(&reordered).is_err());

    let mut duplicated_lane = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut duplicated_lane.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence[6].uses[0].input_position = selected_evidence[5].uses[0].input_position;
    assert!(terminal_verifier::validate_module(&duplicated_lane).is_err());

    let mut omitted_seventh = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut omitted_seventh.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence.pop();
    assert!(terminal_verifier::validate_module(&omitted_seventh).is_err());

    let mut extended_terminal_rows = module.clone();
    append_rejoined_selected_evidence_row(&mut extended_terminal_rows, 7, "eighth");
    terminal_verifier::validate_module(&extended_terminal_rows)
        .expect("a fully rejoined eighth row verifies");

    append_rejoined_selected_evidence_row(&mut extended_terminal_rows, 8, "ninth");
    terminal_verifier::validate_module(&extended_terminal_rows)
        .expect("a fully rejoined ninth row verifies");

    append_rejoined_selected_evidence_row(&mut extended_terminal_rows, 9, "tenth");
    terminal_verifier::validate_module(&extended_terminal_rows)
        .expect("a fully rejoined tenth row verifies");

    append_rejoined_selected_evidence_row(&mut extended_terminal_rows, 10, "eleventh");
    terminal_verifier::validate_module(&extended_terminal_rows)
        .expect("a fully rejoined eleventh row verifies");

    append_rejoined_selected_evidence_row(&mut extended_terminal_rows, 11, "twelfth");
    terminal_verifier::validate_module(&extended_terminal_rows)
        .expect("a fully rejoined twelfth row verifies");

    append_rejoined_selected_evidence_row(&mut extended_terminal_rows, 12, "thirteenth");
    terminal_verifier::validate_module(&extended_terminal_rows)
        .expect("a fully rejoined thirteenth row verifies");

    append_rejoined_selected_evidence_row(&mut extended_terminal_rows, 13, "fourteenth");
    terminal_verifier::validate_module(&extended_terminal_rows)
        .expect("a fully rejoined fourteenth row verifies");

    append_rejoined_selected_evidence_row(&mut extended_terminal_rows, 14, "fifteenth");
    terminal_verifier::validate_module(&extended_terminal_rows)
        .expect("a fully rejoined fifteenth row verifies");

    append_rejoined_selected_evidence_row(&mut extended_terminal_rows, 15, "sixteenth");
    assert!(matches!(
        terminal_verifier::validate_module(&extended_terminal_rows),
        Err(terminal_verifier::ModuleError::InvalidOutcomeSpecificCallEvidence { .. })
    ));
}

#[test]
fn fifteen_selected_witness_tail_uses_are_dense_distinct_and_runtime_free() {
    let checked = crate::front_end::checked_program(FIFTEEN_SELECTED_WITNESS_TAIL_USES_SOURCE);
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
        .expect("the exact fifteen-witness tail use has a checked plan");
    assert_eq!(plan.selected_evidence.len(), 15);
    assert_eq!(
        plan.selected_evidence
            .iter()
            .map(|selection| selection.tail_use.as_ref().unwrap().input_position)
            .collect::<Vec<_>>(),
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]
    );
    assert!(
        plan.selected_evidence
            .windows(2)
            .all(|rows| rows[0].selected_term != rows[1].selected_term)
    );

    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::caller"),
    )
    .expect("the exact fifteen-witness tail use lowers");
    let module = &lowered.semantic_module;
    let [caller, callee, target] = module.machines.as_slice() else {
        panic!("caller, producer, and proof-visible tail target remain canonical")
    };
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &caller.blocks[0].operations[0].kind
    else {
        panic!("the producer call remains structural")
    };
    assert_eq!(selected_evidence.len(), 15);
    assert_eq!(target.contract.requires.len(), 15);
    let mut terminal_positions = Vec::new();
    for selected in selected_evidence {
        let [use_] = selected.uses.as_slice() else {
            panic!("each selected row has one exact use")
        };
        assert_eq!(selected.expected_use_count, 1);
        assert_eq!(use_.target, target.id);
        assert_eq!(use_.source, selected.output);
        let position = usize::try_from(use_.input_position).unwrap();
        terminal_positions.push(use_.input_position);
        assert_eq!(
            target.contract.requires[position],
            semantic_vocabulary::Proposition::Atom(use_.target_requirement)
        );
    }
    terminal_positions.sort_unstable();
    assert_eq!(
        terminal_positions,
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]
    );
    assert!(target.blocks[0].operations.is_empty());

    let bytes = encode_module(module).expect("fifteen selected-witness rows encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("proof bundle encodes");
    let verified = terminal_verifier::verify_module(
        module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("all fifteen independent tail requirements replay");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, module.entry)
            .expect("proof-only uses do not add fuel")
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
    .expect("fifteen-witness artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(4);
    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .expect("artifact completes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::ScalarCase(_))
    ));

    let mut reordered = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut reordered.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence.swap(13, 14);
    assert!(terminal_verifier::validate_module(&reordered).is_err());

    let mut duplicated_lane = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut duplicated_lane.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence[14].uses[0].input_position = selected_evidence[13].uses[0].input_position;
    assert!(terminal_verifier::validate_module(&duplicated_lane).is_err());

    let mut omitted_fifteenth = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut omitted_fifteenth.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence.pop();
    assert!(terminal_verifier::validate_module(&omitted_fifteenth).is_err());

    let mut redirected = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut redirected.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence[14].uses[0].target = callee.id;
    assert!(terminal_verifier::validate_module(&redirected).is_err());
}

#[test]
fn guarded_payloadless_source_call_rejoins_selected_evidence_and_uses_four_fuel() {
    let checked = crate::front_end::checked_program(GUARDED_CALL_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::caller"),
    )
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
    assert_eq!(callee_term.proposition, selected.callee_proposition);
    assert_eq!(output_term.proposition, selected.instantiated_proposition);
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
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("guarded caller proof encodes");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let verified = terminal_verifier::verify_module(
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

    let mut execution = TerminalExecution::start_artifact(
        &bytes,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
    )
    .expect("the guarded caller artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(4);
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .expect("guarded caller completes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::ScalarCase(
            TerminalScalarCaseResult {
                value: TerminalScalarCaseValue {
                    structural_type: caller.result.structural().unwrap().structural_type,
                    result_case: selected.guard.result_case,
                    fields: Vec::new(),
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
    assert!(terminal_verifier::validate_module(&tampered).is_err());

    let mut lost_selection = checked.clone();
    lost_selection
        .facts
        .flow
        .terminal_structural_call_returns
        .payloadless_guarded_machines[0]
        .selected_evidence
        .clear();
    assert!(
        checked_trees_to_lowered_psi::lower_machine(
            &lost_selection,
            TerminalMachineSelection::Name("Root::caller")
        )
        .is_err()
    );

    let mut wrong_arm = checked.clone();
    wrong_arm
        .facts
        .flow
        .terminal_structural_call_returns
        .payloadless_guarded_machines[0]
        .selected_evidence[0]
        .arm_statement_index += 1;
    assert!(
        checked_trees_to_lowered_psi::lower_machine(
            &wrong_arm,
            TerminalMachineSelection::Name("Root::caller")
        )
        .is_err()
    );

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
        checked_trees_to_lowered_psi::lower_machine(
            &wrong_guarantee,
            TerminalMachineSelection::Name("Root::caller")
        )
        .is_err()
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
    assert!(
        checked_trees_to_lowered_psi::lower_machine(
            &wider_validity,
            TerminalMachineSelection::Name("Root::caller")
        )
        .is_err()
    );
}

#[test]
fn guarded_payloadless_source_call_retains_a_canonical_selected_subset_without_runtime_cost() {
    let checked = crate::front_end::checked_program(MULTI_SELECTED_GUARDED_CALL_SOURCE);
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

    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::caller"),
    )
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
    let verified = terminal_verifier::verify_module(
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
    assert!(terminal_verifier::validate_module(&duplicated).is_err());
}
