//! Fixtures shared by the payloadless case return source tests.

use terminal_interpreter::AcceptTerminalEffects;
use terminal_interpreter::TerminalStructuralInputs;
#[path = "payloadless_case_return_source/guarded_payloadless_calls.rs"]
mod guarded_payloadless_calls;
#[path = "payloadless_case_return_source/ordered_case_returns.rs"]
mod ordered_case_returns;
#[path = "payloadless_case_return_source/selected_witness_tail_uses.rs"]
mod selected_witness_tail_uses;

use proof_admission::AdmissionProfile;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use terminal_codec::{decode_module, decode_proof_bundle};
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarCaseValue,
};
use terminal_psi::{OperationKind, StructuralTypeShape};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::CheckingRequest;
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

fn checked_source() -> checked_trees::CheckedTrees {
    checked(SOURCE)
}

fn checked_ordered_case_returns() -> checked_trees::CheckedTrees {
    checked(
        r#"
        data MemoryAlignment [copy] {
            case Alignment1; case Alignment2; case Alignment4; case Alignment8;
        }
        machine MemoryAlignment::from(size: i32) -> MemoryAlignment {
            transition size {
                1 -> (MemoryAlignment::Alignment1)
                2 -> (MemoryAlignment::Alignment2)
                4 -> (MemoryAlignment::Alignment4)
                8 -> (MemoryAlignment::Alignment8)
                _ -> (MemoryAlignment::Alignment1)
            }
        }
    "#,
    )
}

fn integer_case_argument(value: i128) -> terminal_interpreter::TerminalScalarValue {
    terminal_interpreter::TerminalScalarValue::Integer {
        scalar_type: semantic_vocabulary::IntegerType::new(
            semantic_vocabulary::IntegerSign::Signed,
            32,
        )
        .unwrap(),
        value: semantic_vocabulary::IntegerValue::Signed(value),
    }
}

fn assert_guarded_case_results(
    checked: &checked_trees::CheckedTrees,
    entry: &str,
    cases_to_run: &[(terminal_interpreter::TerminalScalarValue, &str)],
) {
    let artifact = terminal_production::TerminalProductionRequest::new(checked, entry)
        .produce_artifact()
        .expect("ordered scalar guards retain selected case construction");
    let artifact =
        terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes()).unwrap();
    let module = decode_module(artifact.semantic_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &decode_proof_bundle(artifact.proof_bytes()).unwrap(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let structural_type = machine.result.structural().unwrap().structural_type;
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == structural_type)
        .unwrap();
    let StructuralTypeShape::Sum { cases } = &declaration.shape else {
        panic!("alignment sum");
    };
    for (input, expected) in cases_to_run {
        let expected_case = cases
            .iter()
            .find(|case| case.identity == *expected)
            .unwrap()
            .id;
        let arguments = [*input];
        let mut execution = TerminalExecution::start_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &AdmissionProfile::default(),
            &arguments,
            TerminalStructuralInputs::default(),
        )
        .unwrap();
        let mut fuel = TerminalFuelMeter::with_allowance(0);
        let mut completed = false;
        for _ in 0..100 {
            match execution
                .resume(&mut fuel, &mut AcceptTerminalEffects)
                .unwrap()
            {
                TerminalExecutionStatus::Complete(TerminalExecutionResult::ScalarCase(result)) => {
                    assert_eq!(
                        result.value,
                        TerminalScalarCaseValue {
                            structural_type,
                            result_case: expected_case,
                            fields: Vec::new()
                        }
                    );
                    completed = true;
                    break;
                }
                TerminalExecutionStatus::Complete(other) => {
                    panic!("unexpected case result {other:?}")
                }
                _ => {
                    fuel.replenish(1).unwrap();
                }
            }
        }
        assert!(
            completed,
            "input {input:?} completes with bounded one-unit fuel resumptions"
        );
    }
}

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed, &CheckingRequest::settled()).expect("check")
}

fn append_rejoined_selected_evidence_row(
    module: &mut terminal_psi::TerminalModule,
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
        semantic_vocabulary::PropositionId::new(next_application).expect("new callee proposition");
    let instantiated_proposition = semantic_vocabulary::PropositionId::new(next_application + 1)
        .expect("new instantiated proposition");
    let target_requirement = semantic_vocabulary::PropositionId::new(next_application + 2)
        .expect("new target requirement");
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
    let callee_term =
        semantic_vocabulary::EvidenceTermId::new(next_term).expect("new callee evidence term");
    let output =
        semantic_vocabulary::EvidenceTermId::new(next_term + 1).expect("new selected output term");
    let target_term =
        semantic_vocabulary::EvidenceTermId::new(next_term + 2).expect("new target term");
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
    let obligation = semantic_vocabulary::ObligationId::new(u64::MAX - u64::from(position))
        .expect("new callee obligation");
    callee_row.obligation = obligation;
    callee_row.proposition = semantic_vocabulary::Proposition::Atom(callee_proposition);
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
        .push(semantic_vocabulary::Proposition::Atom(target_requirement));

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

const FIFTEEN_SELECTED_WITNESS_TAIL_USES_SOURCE: &str = r#"
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
    proposition substantiated(value: Outcome) evidence Evidence;
    proposition documented(value: Outcome) evidence Evidence;
    proposition corroborated(value: Outcome) evidence Evidence;
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
        thirteenth: substantiated(result);
        fourteenth: documented(result);
        fifteenth: corroborated(result);
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
        thirteenth = ConcreteEvidence;
        fourteenth = ConcreteEvidence;
        fifteenth = ConcreteEvidence;
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
                  eleventh: local_eleventh, twelfth: local_twelfth,
                  thirteenth: local_thirteenth,
                  fourteenth: local_fourteenth,
                  fifteenth: local_fifteenth
            } -> finish(
                saved; local_first, local_second, local_third,
                local_fourth, local_fifth, local_sixth,
                local_seventh, local_eighth, local_ninth, local_tenth,
                local_eleventh, local_twelfth, local_thirteenth,
                local_fourteenth, local_fifteenth
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
        requires needed_thirteenth: substantiated(value)
        requires needed_fourteenth: documented(value)
        requires needed_fifteenth: corroborated(value)
        { value }
    }
"#;
