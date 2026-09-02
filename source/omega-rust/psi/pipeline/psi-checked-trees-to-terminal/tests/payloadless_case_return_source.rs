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

fn append_rejoined_selected_evidence_row(
    module: &mut psi_terminal::TerminalModule,
    position: u32,
    label: &str,
) {
    let template = {
        let OperationKind::CallStructural {
            selected_evidence, ..
        } = &module.machines[0].blocks[0].operations[0].kind
        else {
            unreachable!()
        };
        selected_evidence
            .last()
            .expect("template selected row")
            .clone()
    };
    let template_use = template.uses[0].clone();

    let next_application = u64::try_from(module.proposition_applications.len())
        .expect("small proposition-application count")
        + 1;
    let callee_proposition =
        psi_core::PropositionId::new(next_application).expect("new callee proposition");
    let instantiated_proposition =
        psi_core::PropositionId::new(next_application + 1).expect("new instantiated proposition");
    let target_requirement =
        psi_core::PropositionId::new(next_application + 2).expect("new target requirement");
    for (source, id) in [
        (template.callee_proposition, callee_proposition),
        (template.instantiated_proposition, instantiated_proposition),
        (template_use.target_requirement, target_requirement),
    ] {
        let mut application = module
            .proposition_applications
            .iter()
            .find(|application| application.id == source)
            .expect("template proposition application")
            .clone();
        application.id = id;
        module.proposition_applications.push(application);
    }

    let next_term =
        u64::try_from(module.evidence_terms.len()).expect("small evidence-term count") + 1;
    let callee_term = psi_core::EvidenceTermId::new(next_term).expect("new callee evidence term");
    let output = psi_core::EvidenceTermId::new(next_term + 1).expect("new selected output term");
    let target_term = psi_core::EvidenceTermId::new(next_term + 2).expect("new target term");
    for (source, id, proposition) in [
        (template.callee_term, callee_term, callee_proposition),
        (template.output, output, instantiated_proposition),
        (template_use.target_term, target_term, target_requirement),
    ] {
        let mut term = module
            .evidence_terms
            .iter()
            .find(|term| term.id == source)
            .expect("template evidence term")
            .clone();
        term.id = id;
        term.proposition = proposition;
        module.evidence_terms.push(term);
    }

    let mut callee_row = module.machines[1]
        .contract
        .outcome_specific_ensures
        .iter()
        .find(|row| row.guard == template.guard && row.position == template.position)
        .expect("template callee guarantee")
        .clone();
    callee_row.position = position;
    let obligation =
        psi_core::ObligationId::new(u64::MAX - u64::from(position)).expect("new callee obligation");
    callee_row.obligation = obligation;
    callee_row.proposition = psi_core::Proposition::Atom(callee_proposition);
    let output_field = format!("verifier_cardinality_{label}");
    let callee_evidence = callee_row
        .evidence
        .as_mut()
        .expect("template callee guarantee is witness-bearing");
    callee_evidence.term = callee_term;
    callee_evidence.output_field = output_field.clone();
    module.machines[1]
        .contract
        .outcome_specific_ensures
        .push(callee_row);

    module.machines[2]
        .contract
        .requires
        .push(psi_core::Proposition::Atom(target_requirement));

    let mut selected = template;
    selected.position = position;
    selected.callee_obligation = obligation;
    selected.callee_term = callee_term;
    selected.output_field = output_field;
    selected.callee_proposition = callee_proposition;
    selected.instantiated_proposition = instantiated_proposition;
    selected.output = output;
    selected.uses[0].input_position = position;
    selected.uses[0].target_requirement = target_requirement;
    selected.uses[0].target_term = target_term;
    selected.uses[0].source = output;
    selected.uses[0].instantiated_proposition = instantiated_proposition;
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence.push(selected);
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

const RESULT_SUBSTITUTED_GUARDED_CALL_SOURCE: &str = r#"
    trait Evidence {}
    data Outcome [copy] { case Success; case Failure; }
    proposition accepted(value: Outcome) evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}
    data Root {}

    machine Root::produce() -> Outcome
    ensures Outcome::Success -> { selected: accepted(result); }
    { selected = ConcreteEvidence; Outcome::Success }

    machine Root::caller() -> Outcome {
        let saved: Outcome = Root::produce();
        transition saved {
            Outcome::Success { ; selected: local } -> saved
            Outcome::Failure { } -> saved
        }
    }
"#;

const SELECTED_WITNESS_TAIL_USE_SOURCE: &str = r#"
    trait Evidence {}
    data Outcome [copy] { case Success; case Failure; }
    proposition accepted(value: Outcome) evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}
    data Root {}

    machine Root::produce() -> Outcome
    ensures Outcome::Success -> { selected: accepted(result); }
    { selected = ConcreteEvidence; Outcome::Success }

    machine Root::caller() -> Outcome {
        let saved: Outcome = Root::produce();
        transition saved {
            Outcome::Success { ; selected: local } -> finish(saved; local)
            Outcome::Failure { } -> saved
        }
        state finish(value: Outcome) -> Outcome
        requires needed: accepted(value)
        { value }
    }
"#;

const TWO_SELECTED_WITNESS_TAIL_USES_SOURCE: &str = r#"
    trait Evidence {}
    data Outcome [copy] { case Success; case Failure; }
    proposition accepted(value: Outcome) evidence Evidence;
    proposition trusted(value: Outcome) evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}
    data Root {}

    machine Root::produce() -> Outcome
    ensures Outcome::Success -> { first: accepted(result); second: trusted(result); }
    ensures Outcome::Failure -> { sibling: accepted(result); }
    {
        first = ConcreteEvidence;
        second = ConcreteEvidence;
        Outcome::Success
    }

    machine Root::caller() -> Outcome {
        let saved: Outcome = Root::produce();
        transition saved {
            Outcome::Success { ; first: local_first, second: local_second }
                -> finish(saved; local_first, local_second)
            Outcome::Failure { } -> saved
        }
        state finish(value: Outcome) -> Outcome
        requires needed_first: accepted(value)
        requires needed_second: trusted(value)
        { value }
    }
"#;

const THREE_SELECTED_WITNESS_TAIL_USES_SOURCE: &str = r#"
    trait Evidence {}
    data Outcome [copy] { case Success; case Failure; }
    proposition accepted(value: Outcome) evidence Evidence;
    proposition trusted(value: Outcome) evidence Evidence;
    proposition certified(value: Outcome) evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}
    data Root {}

    machine Root::produce() -> Outcome
    ensures Outcome::Success -> {
        first: accepted(result);
        second: trusted(result);
        third: certified(result);
    }
    ensures Outcome::Failure -> { sibling: accepted(result); }
    {
        first = ConcreteEvidence;
        second = ConcreteEvidence;
        third = ConcreteEvidence;
        Outcome::Success
    }

    machine Root::caller() -> Outcome {
        let saved: Outcome = Root::produce();
        transition saved {
            Outcome::Success {
                ; first: local_first, second: local_second, third: local_third
            } -> finish(saved; local_first, local_second, local_third)
            Outcome::Failure { } -> saved
        }
        state finish(value: Outcome) -> Outcome
        requires needed_first: accepted(value)
        requires needed_second: trusted(value)
        requires needed_third: certified(value)
        { value }
    }
"#;

const FOUR_SELECTED_WITNESS_TAIL_USES_SOURCE: &str = r#"
    trait Evidence {}
    data Outcome [copy] { case Success; case Failure; }
    proposition accepted(value: Outcome) evidence Evidence;
    proposition trusted(value: Outcome) evidence Evidence;
    proposition certified(value: Outcome) evidence Evidence;
    proposition reviewed(value: Outcome) evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}
    data Root {}

    machine Root::produce() -> Outcome
    ensures Outcome::Success -> {
        first: accepted(result);
        second: trusted(result);
        third: certified(result);
        fourth: reviewed(result);
    }
    {
        first = ConcreteEvidence;
        second = ConcreteEvidence;
        third = ConcreteEvidence;
        fourth = ConcreteEvidence;
        Outcome::Success
    }

    machine Root::caller() -> Outcome {
        let saved: Outcome = Root::produce();
        transition saved {
            Outcome::Success {
                ; first: local_first, second: local_second,
                  third: local_third, fourth: local_fourth
            } -> finish(saved; local_first, local_second, local_third, local_fourth)
            Outcome::Failure { } -> saved
        }
        state finish(value: Outcome) -> Outcome
        requires needed_first: accepted(value)
        requires needed_second: trusted(value)
        requires needed_third: certified(value)
        requires needed_fourth: reviewed(value)
        { value }
    }
"#;

const FIVE_SELECTED_WITNESS_TAIL_USES_SOURCE: &str = r#"
    trait Evidence {}
    data Outcome [copy] { case Success; case Failure; }
    proposition accepted(value: Outcome) evidence Evidence;
    proposition trusted(value: Outcome) evidence Evidence;
    proposition certified(value: Outcome) evidence Evidence;
    proposition reviewed(value: Outcome) evidence Evidence;
    proposition sealed(value: Outcome) evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}
    data Root {}

    machine Root::produce() -> Outcome
    ensures Outcome::Success -> {
        first: accepted(result);
        second: trusted(result);
        third: certified(result);
        fourth: reviewed(result);
        fifth: sealed(result);
    }
    {
        first = ConcreteEvidence;
        second = ConcreteEvidence;
        third = ConcreteEvidence;
        fourth = ConcreteEvidence;
        fifth = ConcreteEvidence;
        Outcome::Success
    }

    machine Root::caller() -> Outcome {
        let saved: Outcome = Root::produce();
        transition saved {
            Outcome::Success {
                ; first: local_first, second: local_second,
                  third: local_third, fourth: local_fourth, fifth: local_fifth
            } -> finish(
                saved; local_first, local_second, local_third, local_fourth, local_fifth
            )
            Outcome::Failure { } -> saved
        }
        state finish(value: Outcome) -> Outcome
        requires needed_first: accepted(value)
        requires needed_second: trusted(value)
        requires needed_third: certified(value)
        requires needed_fourth: reviewed(value)
        requires needed_fifth: sealed(value)
        { value }
    }
"#;

const SIX_SELECTED_WITNESS_TAIL_USES_SOURCE: &str = r#"
    trait Evidence {}
    data Outcome [copy] { case Success; case Failure; }
    proposition accepted(value: Outcome) evidence Evidence;
    proposition trusted(value: Outcome) evidence Evidence;
    proposition certified(value: Outcome) evidence Evidence;
    proposition reviewed(value: Outcome) evidence Evidence;
    proposition sealed(value: Outcome) evidence Evidence;
    proposition ratified(value: Outcome) evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}
    data Root {}

    machine Root::produce() -> Outcome
    ensures Outcome::Success -> {
        first: accepted(result);
        second: trusted(result);
        third: certified(result);
        fourth: reviewed(result);
        fifth: sealed(result);
        sixth: ratified(result);
    }
    {
        first = ConcreteEvidence;
        second = ConcreteEvidence;
        third = ConcreteEvidence;
        fourth = ConcreteEvidence;
        fifth = ConcreteEvidence;
        sixth = ConcreteEvidence;
        Outcome::Success
    }

    machine Root::caller() -> Outcome {
        let saved: Outcome = Root::produce();
        transition saved {
            Outcome::Success {
                ; first: local_first, second: local_second,
                  third: local_third, fourth: local_fourth,
                  fifth: local_fifth, sixth: local_sixth
            } -> finish(
                saved; local_first, local_second, local_third,
                local_fourth, local_fifth, local_sixth
            )
            Outcome::Failure { } -> saved
        }
        state finish(value: Outcome) -> Outcome
        requires needed_first: accepted(value)
        requires needed_second: trusted(value)
        requires needed_third: certified(value)
        requires needed_fourth: reviewed(value)
        requires needed_fifth: sealed(value)
        requires needed_sixth: ratified(value)
        { value }
    }
"#;

const SEVEN_SELECTED_WITNESS_TAIL_USES_SOURCE: &str = r#"
    trait Evidence {}
    data Outcome [copy] { case Success; case Failure; }
    proposition accepted(value: Outcome) evidence Evidence;
    proposition trusted(value: Outcome) evidence Evidence;
    proposition certified(value: Outcome) evidence Evidence;
    proposition reviewed(value: Outcome) evidence Evidence;
    proposition sealed(value: Outcome) evidence Evidence;
    proposition ratified(value: Outcome) evidence Evidence;
    proposition endorsed(value: Outcome) evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}
    data Root {}

    machine Root::produce() -> Outcome
    ensures Outcome::Success -> {
        first: accepted(result);
        second: trusted(result);
        third: certified(result);
        fourth: reviewed(result);
        fifth: sealed(result);
        sixth: ratified(result);
        seventh: endorsed(result);
    }
    {
        first = ConcreteEvidence;
        second = ConcreteEvidence;
        third = ConcreteEvidence;
        fourth = ConcreteEvidence;
        fifth = ConcreteEvidence;
        sixth = ConcreteEvidence;
        seventh = ConcreteEvidence;
        Outcome::Success
    }

    machine Root::caller() -> Outcome {
        let saved: Outcome = Root::produce();
        transition saved {
            Outcome::Success {
                ; first: local_first, second: local_second,
                  third: local_third, fourth: local_fourth,
                  fifth: local_fifth, sixth: local_sixth,
                  seventh: local_seventh
            } -> finish(
                saved; local_first, local_second, local_third,
                local_fourth, local_fifth, local_sixth, local_seventh
            )
            Outcome::Failure { } -> saved
        }
        state finish(value: Outcome) -> Outcome
        requires needed_first: accepted(value)
        requires needed_second: trusted(value)
        requires needed_third: certified(value)
        requires needed_fourth: reviewed(value)
        requires needed_fifth: sealed(value)
        requires needed_sixth: ratified(value)
        requires needed_seventh: endorsed(value)
        { value }
    }
"#;

const TWELVE_SELECTED_WITNESS_TAIL_USES_SOURCE: &str = r#"
    trait Evidence {}
    data Outcome [copy] { case Success; case Failure; }
    proposition accepted(value: Outcome) evidence Evidence;
    proposition trusted(value: Outcome) evidence Evidence;
    proposition certified(value: Outcome) evidence Evidence;
    proposition reviewed(value: Outcome) evidence Evidence;
    proposition sealed(value: Outcome) evidence Evidence;
    proposition ratified(value: Outcome) evidence Evidence;
    proposition endorsed(value: Outcome) evidence Evidence;
    proposition validated(value: Outcome) evidence Evidence;
    proposition confirmed(value: Outcome) evidence Evidence;
    proposition affirmed(value: Outcome) evidence Evidence;
    proposition attested(value: Outcome) evidence Evidence;
    proposition warranted(value: Outcome) evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}
    data Root {}

    machine Root::produce() -> Outcome
    ensures Outcome::Success -> {
        first: accepted(result);
        second: trusted(result);
        third: certified(result);
        fourth: reviewed(result);
        fifth: sealed(result);
        sixth: ratified(result);
        seventh: endorsed(result);
        eighth: validated(result);
        ninth: confirmed(result);
        tenth: affirmed(result);
        eleventh: attested(result);
        twelfth: warranted(result);
    }
    {
        first = ConcreteEvidence;
        second = ConcreteEvidence;
        third = ConcreteEvidence;
        fourth = ConcreteEvidence;
        fifth = ConcreteEvidence;
        sixth = ConcreteEvidence;
        seventh = ConcreteEvidence;
        eighth = ConcreteEvidence;
        ninth = ConcreteEvidence;
        tenth = ConcreteEvidence;
        eleventh = ConcreteEvidence;
        twelfth = ConcreteEvidence;
        Outcome::Success
    }

    machine Root::caller() -> Outcome {
        let saved: Outcome = Root::produce();
        transition saved {
            Outcome::Success {
                ; first: local_first, second: local_second,
                  third: local_third, fourth: local_fourth,
                  fifth: local_fifth, sixth: local_sixth,
                  seventh: local_seventh, eighth: local_eighth,
                  ninth: local_ninth, tenth: local_tenth,
                  eleventh: local_eleventh, twelfth: local_twelfth
            } -> finish(
                saved; local_first, local_second, local_third,
                local_fourth, local_fifth, local_sixth,
                local_seventh, local_eighth, local_ninth, local_tenth,
                local_eleventh, local_twelfth
            )
            Outcome::Failure { } -> saved
        }
        state finish(value: Outcome) -> Outcome
        requires needed_first: accepted(value)
        requires needed_second: trusted(value)
        requires needed_third: certified(value)
        requires needed_fourth: reviewed(value)
        requires needed_fifth: sealed(value)
        requires needed_sixth: ratified(value)
        requires needed_seventh: endorsed(value)
        requires needed_eighth: validated(value)
        requires needed_ninth: confirmed(value)
        requires needed_tenth: affirmed(value)
        requires needed_eleventh: attested(value)
        requires needed_twelfth: warranted(value)
        { value }
    }
"#;

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

    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::caller")
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
        [psi_core::Proposition::Atom(use_.target_requirement)]
    );
    assert!(target.blocks[0].operations.is_empty());

    let bytes = encode_module(module).expect("selected-witness semantics encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("selected-witness proof encodes");
    let verified = psi_terminal_verifier::verify_module(
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
    let mut execution =
        TerminalExecution::start_artifact(&bytes, &proof, &AdmissionProfile::default(), &[])
            .expect("selected-witness artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(4);
    assert!(matches!(
        execution.resume(&mut meter).expect("artifact completes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::PayloadlessCase(_))
    ));

    let mutate = |mut module: psi_terminal::TerminalModule,
                  change: fn(&mut psi_terminal::OutcomeSpecificCallEvidence)| {
        let OperationKind::CallStructural {
            selected_evidence, ..
        } = &mut module.machines[0].blocks[0].operations[0].kind
        else {
            unreachable!()
        };
        change(&mut selected_evidence[0]);
        assert!(psi_terminal_verifier::validate_module(&module).is_err());
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
        selected.uses[0].target = psi_core::MachineId::new(99).unwrap()
    });
    mutate(module.clone(), |selected| {
        selected.uses[0].input_position = 1
    });
    mutate(module.clone(), |selected| {
        selected.uses[0].target_requirement = selected.instantiated_proposition
    });

    let mut missing_lane = module.clone();
    missing_lane.machines[2].contract.requires.clear();
    assert!(psi_terminal_verifier::validate_module(&missing_lane).is_err());
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
    assert!(psi_terminal_verifier::validate_module(&wrong_interface).is_err());
    let mut wrong_tail = module.clone();
    let result = wrong_tail.machines[2].result.structural().unwrap().place;
    let Terminator::ReturnStructural { source, .. } =
        &mut wrong_tail.machines[2].blocks[0].terminator
    else {
        unreachable!()
    };
    *source = result;
    assert!(psi_terminal_verifier::validate_module(&wrong_tail).is_err());

    let mut omitted_checked_use = checked.clone();
    omitted_checked_use
        .facts
        .flow
        .terminal_structural_call_returns
        .payloadless_guarded_machines[0]
        .selected_evidence[0]
        .tail_use = None;
    assert!(
        psi_checked_trees_to_terminal::lower_machine(&omitted_checked_use, "Root::caller").is_err()
    );
}

#[test]
fn two_selected_witness_tail_uses_are_ordered_distinct_and_runtime_free() {
    let checked = checked(TWO_SELECTED_WITNESS_TAIL_USES_SOURCE);
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

    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::caller")
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
            psi_core::Proposition::Atom(first_use.target_requirement),
            psi_core::Proposition::Atom(second_use.target_requirement),
        ]
    );
    assert!(target.blocks[0].operations.is_empty());

    let bytes = encode_module(module).expect("two selected-witness rows encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof bundle encodes");
    let verified = psi_terminal_verifier::verify_module(
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
    let mut execution =
        TerminalExecution::start_artifact(&bytes, &proof, &AdmissionProfile::default(), &[])
            .expect("two-witness artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(4);
    assert!(matches!(
        execution.resume(&mut meter).expect("artifact completes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::PayloadlessCase(_))
    ));

    let mutate = |mut module: psi_terminal::TerminalModule,
                  change: fn(&mut [psi_terminal::OutcomeSpecificCallEvidence])| {
        let OperationKind::CallStructural {
            selected_evidence, ..
        } = &mut module.machines[0].blocks[0].operations[0].kind
        else {
            unreachable!()
        };
        change(selected_evidence);
        assert!(psi_terminal_verifier::validate_module(&module).is_err());
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
    assert!(psi_terminal_verifier::validate_module(&missing_argument).is_err());
    let mut extra_argument = module.clone();
    let extra = extra_argument.machines[2].contract.requires[0].clone();
    extra_argument.machines[2].contract.requires.push(extra);
    assert!(psi_terminal_verifier::validate_module(&extra_argument).is_err());
}

#[test]
fn three_selected_witness_tail_uses_are_dense_distinct_and_runtime_free() {
    let checked = checked(THREE_SELECTED_WITNESS_TAIL_USES_SOURCE);
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

    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::caller")
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
            psi_core::Proposition::Atom(use_.target_requirement)
        );
    }
    terminal_positions.sort_unstable();
    assert_eq!(terminal_positions, [0, 1, 2]);
    assert!(target.blocks[0].operations.is_empty());

    let bytes = encode_module(module).expect("three selected-witness rows encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof bundle encodes");
    let verified = psi_terminal_verifier::verify_module(
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
    let mut execution =
        TerminalExecution::start_artifact(&bytes, &proof, &AdmissionProfile::default(), &[])
            .expect("three-witness artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(4);
    assert!(matches!(
        execution.resume(&mut meter).expect("artifact completes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::PayloadlessCase(_))
    ));

    let mut reordered = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut reordered.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence.swap(1, 2);
    assert!(psi_terminal_verifier::validate_module(&reordered).is_err());

    let mut duplicated_lane = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut duplicated_lane.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence[2].uses[0].input_position = selected_evidence[1].uses[0].input_position;
    assert!(psi_terminal_verifier::validate_module(&duplicated_lane).is_err());

    let mut missing_requirement = module.clone();
    missing_requirement.machines[2].contract.requires.pop();
    assert!(psi_terminal_verifier::validate_module(&missing_requirement).is_err());
}

#[test]
fn four_selected_witness_tail_uses_are_dense_distinct_and_runtime_free() {
    let checked = checked(FOUR_SELECTED_WITNESS_TAIL_USES_SOURCE);
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

    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::caller")
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
            psi_core::Proposition::Atom(use_.target_requirement)
        );
    }
    terminal_positions.sort_unstable();
    assert_eq!(terminal_positions, [0, 1, 2, 3]);
    assert!(target.blocks[0].operations.is_empty());

    let bytes = encode_module(module).expect("four selected-witness rows encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof bundle encodes");
    let verified = psi_terminal_verifier::verify_module(
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
    let mut execution =
        TerminalExecution::start_artifact(&bytes, &proof, &AdmissionProfile::default(), &[])
            .expect("four-witness artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(4);
    assert!(matches!(
        execution.resume(&mut meter).expect("artifact completes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::PayloadlessCase(_))
    ));

    let mut reordered = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut reordered.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence.swap(2, 3);
    assert!(psi_terminal_verifier::validate_module(&reordered).is_err());

    let mut duplicated_lane = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut duplicated_lane.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence[3].uses[0].input_position = selected_evidence[2].uses[0].input_position;
    assert!(psi_terminal_verifier::validate_module(&duplicated_lane).is_err());

    let mut missing_requirement = module.clone();
    missing_requirement.machines[2].contract.requires.pop();
    assert!(psi_terminal_verifier::validate_module(&missing_requirement).is_err());
}

#[test]
fn five_selected_witness_tail_uses_are_dense_distinct_and_runtime_free() {
    let checked = checked(FIVE_SELECTED_WITNESS_TAIL_USES_SOURCE);
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

    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::caller")
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
            psi_core::Proposition::Atom(use_.target_requirement)
        );
    }
    terminal_positions.sort_unstable();
    assert_eq!(terminal_positions, [0, 1, 2, 3, 4]);
    assert!(target.blocks[0].operations.is_empty());

    let bytes = encode_module(module).expect("five selected-witness rows encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof bundle encodes");
    let verified = psi_terminal_verifier::verify_module(
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
    let mut execution =
        TerminalExecution::start_artifact(&bytes, &proof, &AdmissionProfile::default(), &[])
            .expect("five-witness artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(4);
    assert!(matches!(
        execution.resume(&mut meter).expect("artifact completes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::PayloadlessCase(_))
    ));

    let mut reordered = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut reordered.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence.swap(3, 4);
    assert!(psi_terminal_verifier::validate_module(&reordered).is_err());

    let mut duplicated_lane = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut duplicated_lane.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence[4].uses[0].input_position = selected_evidence[3].uses[0].input_position;
    assert!(psi_terminal_verifier::validate_module(&duplicated_lane).is_err());

    let mut omitted_fifth = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut omitted_fifth.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence.pop();
    assert!(psi_terminal_verifier::validate_module(&omitted_fifth).is_err());

    let mut six_terminal_rows = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut six_terminal_rows.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence.push(selected_evidence[4].clone());
    assert!(psi_terminal_verifier::validate_module(&six_terminal_rows).is_err());
}

#[test]
fn six_selected_witness_tail_uses_are_dense_distinct_and_runtime_free() {
    let checked = checked(SIX_SELECTED_WITNESS_TAIL_USES_SOURCE);
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

    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::caller")
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
            psi_core::Proposition::Atom(use_.target_requirement)
        );
    }
    terminal_positions.sort_unstable();
    assert_eq!(terminal_positions, [0, 1, 2, 3, 4, 5]);
    assert!(target.blocks[0].operations.is_empty());

    let bytes = encode_module(module).expect("six selected-witness rows encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof bundle encodes");
    let verified = psi_terminal_verifier::verify_module(
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
    let mut execution =
        TerminalExecution::start_artifact(&bytes, &proof, &AdmissionProfile::default(), &[])
            .expect("six-witness artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(4);
    assert!(matches!(
        execution.resume(&mut meter).expect("artifact completes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::PayloadlessCase(_))
    ));

    let mut reordered = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut reordered.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence.swap(4, 5);
    assert!(psi_terminal_verifier::validate_module(&reordered).is_err());

    let mut duplicated_lane = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut duplicated_lane.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence[5].uses[0].input_position = selected_evidence[4].uses[0].input_position;
    assert!(psi_terminal_verifier::validate_module(&duplicated_lane).is_err());

    let mut omitted_sixth = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut omitted_sixth.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence.pop();
    assert!(psi_terminal_verifier::validate_module(&omitted_sixth).is_err());

    let mut seven_terminal_rows = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut seven_terminal_rows.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence.push(selected_evidence[5].clone());
    assert!(psi_terminal_verifier::validate_module(&seven_terminal_rows).is_err());
}

#[test]
fn seven_selected_witness_tail_uses_are_dense_distinct_and_runtime_free() {
    let checked = checked(SEVEN_SELECTED_WITNESS_TAIL_USES_SOURCE);
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

    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::caller")
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
            psi_core::Proposition::Atom(use_.target_requirement)
        );
    }
    terminal_positions.sort_unstable();
    assert_eq!(terminal_positions, [0, 1, 2, 3, 4, 5, 6]);
    assert!(target.blocks[0].operations.is_empty());

    let bytes = encode_module(module).expect("seven selected-witness rows encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof bundle encodes");
    let verified = psi_terminal_verifier::verify_module(
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
    let mut execution =
        TerminalExecution::start_artifact(&bytes, &proof, &AdmissionProfile::default(), &[])
            .expect("seven-witness artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(4);
    assert!(matches!(
        execution.resume(&mut meter).expect("artifact completes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::PayloadlessCase(_))
    ));

    let mut reordered = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut reordered.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence.swap(5, 6);
    assert!(psi_terminal_verifier::validate_module(&reordered).is_err());

    let mut duplicated_lane = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut duplicated_lane.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence[6].uses[0].input_position = selected_evidence[5].uses[0].input_position;
    assert!(psi_terminal_verifier::validate_module(&duplicated_lane).is_err());

    let mut omitted_seventh = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut omitted_seventh.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence.pop();
    assert!(psi_terminal_verifier::validate_module(&omitted_seventh).is_err());

    let mut extended_terminal_rows = module.clone();
    append_rejoined_selected_evidence_row(&mut extended_terminal_rows, 7, "eighth");
    psi_terminal_verifier::validate_module(&extended_terminal_rows)
        .expect("a fully rejoined eighth row verifies");

    append_rejoined_selected_evidence_row(&mut extended_terminal_rows, 8, "ninth");
    psi_terminal_verifier::validate_module(&extended_terminal_rows)
        .expect("a fully rejoined ninth row verifies");

    append_rejoined_selected_evidence_row(&mut extended_terminal_rows, 9, "tenth");
    psi_terminal_verifier::validate_module(&extended_terminal_rows)
        .expect("a fully rejoined tenth row verifies");

    append_rejoined_selected_evidence_row(&mut extended_terminal_rows, 10, "eleventh");
    psi_terminal_verifier::validate_module(&extended_terminal_rows)
        .expect("a fully rejoined eleventh row verifies");

    append_rejoined_selected_evidence_row(&mut extended_terminal_rows, 11, "twelfth");
    psi_terminal_verifier::validate_module(&extended_terminal_rows)
        .expect("a fully rejoined twelfth row verifies");

    append_rejoined_selected_evidence_row(&mut extended_terminal_rows, 12, "thirteenth");
    assert!(matches!(
        psi_terminal_verifier::validate_module(&extended_terminal_rows),
        Err(psi_terminal_verifier::ModuleError::InvalidOutcomeSpecificCallEvidence { .. })
    ));
}

#[test]
fn twelve_selected_witness_tail_uses_are_dense_distinct_and_runtime_free() {
    let checked = checked(TWELVE_SELECTED_WITNESS_TAIL_USES_SOURCE);
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
        .expect("the exact twelve-witness tail use has a checked plan");
    assert_eq!(plan.selected_evidence.len(), 12);
    assert_eq!(
        plan.selected_evidence
            .iter()
            .map(|selection| selection.tail_use.as_ref().unwrap().input_position)
            .collect::<Vec<_>>(),
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]
    );
    assert!(
        plan.selected_evidence
            .windows(2)
            .all(|rows| rows[0].selected_term != rows[1].selected_term)
    );

    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::caller")
        .expect("the exact twelve-witness tail use lowers");
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
    assert_eq!(selected_evidence.len(), 12);
    assert_eq!(target.contract.requires.len(), 12);
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
            psi_core::Proposition::Atom(use_.target_requirement)
        );
    }
    terminal_positions.sort_unstable();
    assert_eq!(terminal_positions, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
    assert!(target.blocks[0].operations.is_empty());

    let bytes = encode_module(module).expect("twelve selected-witness rows encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof bundle encodes");
    let verified = psi_terminal_verifier::verify_module(
        module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("all twelve independent tail requirements replay");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, module.entry)
            .expect("proof-only uses do not add fuel")
            .ceiling_units(),
        4
    );
    let mut execution =
        TerminalExecution::start_artifact(&bytes, &proof, &AdmissionProfile::default(), &[])
            .expect("twelve-witness artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(4);
    assert!(matches!(
        execution.resume(&mut meter).expect("artifact completes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::PayloadlessCase(_))
    ));

    let mut reordered = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut reordered.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence.swap(10, 11);
    assert!(psi_terminal_verifier::validate_module(&reordered).is_err());

    let mut duplicated_lane = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut duplicated_lane.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence[11].uses[0].input_position = selected_evidence[10].uses[0].input_position;
    assert!(psi_terminal_verifier::validate_module(&duplicated_lane).is_err());

    let mut omitted_twelfth = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut omitted_twelfth.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence.pop();
    assert!(psi_terminal_verifier::validate_module(&omitted_twelfth).is_err());

    let mut redirected = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut redirected.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence[11].uses[0].target = callee.id;
    assert!(psi_terminal_verifier::validate_module(&redirected).is_err());
}

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
fn guarded_payloadless_call_substitutes_the_exact_whole_result_application() {
    let checked = checked(RESULT_SUBSTITUTED_GUARDED_CALL_SOURCE);
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

    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::caller")
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
        psi_terminal_verifier::reconstruct_terminal_obligations(&lowered.semantic_module)
            .expect("the exact result substitution reconstructs");
    assert!(
        reconstructed.obligations().is_empty(),
        "the selected evidence discharges its guarded row without a proof obligation"
    );
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the substituted selected result verifies");
    let bytes = encode_module(&lowered.semantic_module).expect("substitution encodes");
    assert_eq!(decode_module(&bytes), Ok(lowered.semantic_module.clone()));
    let verified = psi_terminal_verifier::verify_module(
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
    assert!(psi_terminal_verifier::validate_module(&redirected).is_err());

    let mut erased = lowered.semantic_module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut erased.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence[0].result_substitution = None;
    assert!(psi_terminal_verifier::validate_module(&erased).is_err());
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
