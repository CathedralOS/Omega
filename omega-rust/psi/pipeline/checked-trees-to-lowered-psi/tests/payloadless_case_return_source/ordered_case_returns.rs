use super::{
    SELECTED_WITNESS_TAIL_USE_SOURCE, assert_guarded_case_results, checked,
    checked_ordered_case_returns, integer_case_argument,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_section};
use terminal_fixed_fuel::derive_fixed_entry_fuel;
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::AcceptTerminalEffects;
use terminal_interpreter::TerminalStructuralInputs;
use terminal_interpreter::{TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus};
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi::{OperationKind, Terminator};

#[test]
fn ordered_scalar_guards_return_case_values_and_explicit_fallback() {
    let checked = checked_ordered_case_returns();
    assert_guarded_case_results(
        &checked,
        "MemoryAlignment::from",
        &[
            (integer_case_argument(1), "Alignment1"),
            (integer_case_argument(2), "Alignment2"),
            (integer_case_argument(4), "Alignment4"),
            (integer_case_argument(8), "Alignment8"),
            (integer_case_argument(0), "Alignment1"),
            (integer_case_argument(-1), "Alignment1"),
            (integer_case_argument(3), "Alignment1"),
        ],
    );
}

#[test]
fn ordered_case_returns_reject_changed_construction_control_and_coverage() {
    for mutation in 0..4 {
        let mut checked = checked_ordered_case_returns();
        let state = checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter_mut()
            .find(|plan| {
                plan.states.iter().any(|state| {
                    matches!(
                        state.terminator,
                        checked_trees::CheckedComposedUnitControlTerminatorPlan::Guarded { .. }
                    )
                })
            })
            .unwrap()
            .states
            .first_mut()
            .unwrap();
        let checked_trees::CheckedComposedUnitControlTerminatorPlan::Guarded {
            arms,
            fallback,
            return_values: returns,
        } = &mut state.terminator
        else {
            unreachable!()
        };
        match mutation {
            0 => {
                let checked_trees::CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                    value: replacement,
                    ..
                } = &returns[1]
                else {
                    panic!("selected value")
                };
                let replacement = *replacement;
                let checked_trees::CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                    value,
                    ..
                } = &mut returns[0]
                else {
                    panic!("selected value")
                };
                *value = replacement;
            }
            1 => returns.swap(0, 1),
            2 => {
                let rows = checked
                    .facts
                    .flow
                    .terminal_scalar_graphs
                    .guarded_exits
                    .span_mut(*arms)
                    .unwrap();
                rows.swap(0, 1);
            }
            _ => {
                // Corrupt both consumers of the roster: source coverage, not
                // accidental disagreement between two retained plans, rejects it.
                *fallback = None;
                returns.pop();
                checked
                    .facts
                    .flow
                    .terminal_scalar_graphs
                    .guarded_tails
                    .iter_mut()
                    .find(|tail| tail.state == state.state)
                    .unwrap()
                    .fallback = None;
            }
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(
                &checked,
                TerminalMachineSelection::Name("MemoryAlignment::from")
            )
            .produce(TerminalProductionCustody::artifact_only(
                &mut TerminalProductionTimings::default()
            ))
            .is_err(),
            "mutation {mutation} must fail independent source replay"
        );
    }
}

#[test]
fn ordered_case_returns_reuse_single_guard_and_complementary_pair() {
    for body in [
        "transition flag { true -> (Choice::First) _ -> (Choice::Second) }",
        "transition flag { true -> (Choice::First) false -> (Choice::Second) }",
    ] {
        let program = checked(&format!(
            "data Choice [copy] {{ case First; case Second; }} machine choose(flag: bool) -> Choice {{ {body} }}"
        ));
        assert_guarded_case_results(
            &program,
            "choose",
            &[
                (
                    terminal_interpreter::TerminalScalarValue::Boolean(true),
                    "First",
                ),
                (
                    terminal_interpreter::TerminalScalarValue::Boolean(false),
                    "Second",
                ),
            ],
        );
    }
}

#[test]
fn ordered_case_returns_selectively_evaluate_a_composed_guard() {
    let program = checked(
        r#"
        data Choice [copy] { case First; case Second; }
        machine choose(flag: bool) -> Choice {
            let copied: bool = flag;
            transition flag && copied {
                true -> (Choice::First)
                false -> (Choice::Second)
            }
        }
    "#,
    );
    assert_guarded_case_results(
        &program,
        "choose",
        &[
            (
                terminal_interpreter::TerminalScalarValue::Boolean(true),
                "First",
            ),
            (
                terminal_interpreter::TerminalScalarValue::Boolean(false),
                "Second",
            ),
        ],
    );
}

#[test]
fn borrowed_case_membership_uses_an_observation_operation() {
    for body in [
        "choice in Choice::Empty",
        "!(choice in Choice::Some)",
        "let empty: bool = choice in Choice::Empty; empty && !(choice in Choice::Some)",
    ] {
        let checked = checked(&format!(
            "
            data Choice {{ case Empty; case Some(value: u32); }}
            machine observe(choice: &Choice) -> bool {{ {body} }}
        "
        ));
        let artifact = terminal_production::TerminalProductionRequest::new(
            &checked,
            TerminalMachineSelection::Name("observe"),
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default(),
        ))
        .expect("whole borrowed case membership reaches Terminal")
        .into_artifact();
        let module = decode_module(artifact.semantic_bytes()).unwrap();
        assert!(
            module
                .machines
                .iter()
                .flat_map(|machine| &machine.blocks)
                .flat_map(|block| &block.operations)
                .any(|operation| matches!(
                    operation.kind,
                    OperationKind::StructuralCaseMembership { .. }
                ))
        );
        terminal_verifier::verify_module(
            &module,
            &decode_proof_bundle(artifact.proof_bytes()).unwrap(),
            &AdmissionProfile::default(),
        )
        .unwrap();
    }
}

#[test]
fn selected_witness_tail_use_is_canonical_and_runtime_free() {
    let checked = checked(SELECTED_WITNESS_TAIL_USE_SOURCE);
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
