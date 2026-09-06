//! Mutable owned formals receive values once and retain separate current storage.

use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{encode_module, encode_proof_bundle};
use terminal_interpreter::{
    TerminalArtifactInterpretError, TerminalEffect, TerminalEffectHandler, TerminalEffectRejection,
    TerminalExecutionResult, TerminalInterpretError, TerminalScalarValue,
    interpret_terminal_artifact, interpret_terminal_artifact_with_effect_handler_measured,
};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

fn typed(source: &str) -> typed_trees::TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

fn assert_execution(source: &str, expected: TerminalScalarValue) {
    let artifact = {
        let checked = lower_typed_trees(typed(source))
            .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
        let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "value")
            .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
        (
            encode_module(&lowered.semantic_module).expect("canonical semantic bytes"),
            encode_proof_bundle(&lowered.proof_bundle).expect("canonical proof bytes"),
        )
    };
    // No typed trees or producer-owned evidence objects reach execution.
    let result =
        interpret_terminal_artifact(&artifact.0, &artifact.1, &AdmissionProfile::default(), &[])
            .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
    assert_eq!(
        result,
        TerminalExecutionResult::Scalar(expected),
        "{source}"
    );
}

fn encoded(source: &str) -> (Vec<u8>, Vec<u8>) {
    let checked = lower_typed_trees(typed(source))
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "value")
        .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
    (
        encode_module(&lowered.semantic_module).unwrap(),
        encode_proof_bundle(&lowered.proof_bundle).unwrap(),
    )
}

fn integer(destination: &str, value: u32) -> TerminalScalarValue {
    let (sign, value) = match destination {
        "i32" => (IntegerSign::Signed, IntegerValue::Signed(i128::from(value))),
        "u32" => (
            IntegerSign::Unsigned,
            IntegerValue::Unsigned(u128::from(value)),
        ),
        _ => panic!("fixture has a fixed integer carrier"),
    };
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(sign, 32).unwrap(),
        value,
    }
}

#[test]
fn mutable_owned_call_arguments_land_exact_values_and_preserve_typed_division() {
    for (destination, policy, argument, expected) in [
        ("i32", "", "7 / 2 * 2", 7),
        ("u32", "", "(4097 / 4096) * 4096", 4097),
        // Wrapping is explicit: this control needs no ExactMultiply proof.
        (
            "i32",
            " in Wrapping",
            "(7i32 as i32 in Wrapping) / 2 * 2",
            6,
        ),
    ] {
        let source = format!(
            r#"
            machine receive(mut input: {destination}{policy}) -> {destination}{policy}
            requires 0{destination} == 0{destination}
            ensures 0{destination} == 0{destination}
            {{ input }}
            machine value() -> {destination}{policy}
            requires 0{destination} == 0{destination}
            ensures 0{destination} == 0{destination}
            {{ receive({argument}) }}
            "#,
        );
        assert_execution(&source, integer(destination, expected));
    }
}

#[test]
fn reassignment_updates_reads_and_calls_without_rebinding_saved_values() {
    for (body, expected) in [
        ("input = 9; input", 9),
        ("let saved: i32 = input; input = 9; saved", 7),
        ("input = 9; identity(input)", 9),
        ("let saved: i32 = input; input = 9; identity(saved)", 7),
    ] {
        let source = format!(
            r#"
            machine identity(input: i32) -> i32
            requires {expected}i32 == {expected}i32
            ensures {expected}i32 == {expected}i32
            {{ input }}
            machine rewrite(mut input: i32) -> i32
            requires {expected}i32 == {expected}i32
            ensures {expected}i32 == {expected}i32
            {{ {body} }}
            machine value() -> i32
            requires {expected}i32 == {expected}i32
            ensures {expected}i32 == {expected}i32
            {{ let returned: i32 = rewrite(7 / 2 * 2); returned }}
            "#,
        );
        assert_execution(&source, integer("i32", expected));
    }
}

#[test]
fn mixed_formal_positions_keep_independent_mutable_storage_and_immutable_values() {
    for (arguments, returned, expected) in [
        ("7, 4, 9", "first", 9),
        ("9, 4, 7", "first", 7),
        ("7, 4, 9", "last", 4),
        ("7, 4, 9", "saved", 7),
        ("7, 4, 9", "middle", 4),
    ] {
        let source = format!(
            r#"
            machine select(mut first: i32, middle: i32, mut last: i32) -> i32
            requires 0i32 == 0i32
            ensures 0i32 == 0i32
            {{ let saved: i32 = first; first = last; last = middle; {returned} }}
            machine value() -> i32
            requires 0i32 == 0i32
            ensures 0i32 == 0i32
            {{ select({arguments}) }}
            "#,
        );
        assert_execution(&source, integer("i32", expected));
    }
}

#[test]
fn state_entry_reinitializes_mutable_storage_and_boolean_updates_use_current_values() {
    let states = r#"
        machine value() -> i32
        requires 9i32 == 9i32
        ensures 9i32 == 9i32
        {
            transition { _ -> change(7 / 2 * 2) }
            state change(mut input: i32) -> i32 {
                input = 9;
                transition { _ -> finish(input) }
            }
            state finish(mut input: i32) -> i32 {
                let saved: i32 = input;
                input = 11;
                saved
            }
        }
    "#;
    assert_execution(states, integer("i32", 9));
    for (returned, expected) in [("input", true), ("saved", false)] {
        let source = format!(
            r#"
            machine flip(mut input: bool) -> bool
            requires {expected} == {expected}
            ensures {expected} == {expected}
            {{ let saved: bool = input; input = !input; {returned} }}
            machine value() -> bool
            requires {expected} == {expected}
            ensures {expected} == {expected}
            {{ flip(false) }}
            "#,
        );
        assert_execution(&source, TerminalScalarValue::Boolean(expected));
    }
}

#[test]
fn final_mutable_formal_guarantee_cannot_prove_equality_with_the_original_argument() {
    let helper = r#"
        machine rewrite(mut input: i32) -> i32
        ensures result == input
        { input = 9; input }
    "#;
    lower_typed_trees(typed(helper))
        .expect("the helper's result equals its final mutable input, not an implicit old value");
    let source = format!(
        r#"
        {helper}
        machine value() -> i32
        ensures result == 7
        {{ rewrite(7) }}
        "#,
    );
    assert!(
        lower_typed_trees(typed(&source)).is_err(),
        "a final-formal guarantee must not be substituted with the earlier call argument",
    );
}

#[derive(Default)]
struct ObserveArguments(Vec<Vec<TerminalScalarValue>>);

#[test]
fn guard_uses_current_mutable_boolean_instead_of_its_opposite_entry_value() {
    let source = r#"
        machine value(mut input: bool) -> bool
        requires true == true
        ensures true == true
        crashes Trap
        {
            input = !input;
            transition input {
                true -> true
                false -> failed()
            }
            state failed() -> bool { crash Trap; }
        }
    "#;
    let artifact = encoded(source);
    assert_eq!(
        interpret_terminal_artifact(
            &artifact.0,
            &artifact.1,
            &AdmissionProfile::default(),
            &[TerminalScalarValue::Boolean(false)],
        )
        .unwrap(),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true)),
    );
    let result = interpret_terminal_artifact(
        &artifact.0,
        &artifact.1,
        &AdmissionProfile::default(),
        &[TerminalScalarValue::Boolean(true)],
    );
    assert!(
        matches!(result,
        Err(TerminalArtifactInterpretError::Execution(TerminalInterpretError::Crash(crash)))
            if crash.cause == terminal_psi::CrashCause::Trap),
        "the true entry value becomes false before the guard and selects the crashing state"
    );
}

impl TerminalEffectHandler for ObserveArguments {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall {
            arguments,
            structural_arguments,
            ..
        } = effect
        else {
            panic!("fixture emits only a checked boundary call");
        };
        assert!(structural_arguments.is_empty());
        self.0.push(arguments.clone());
        Ok(())
    }
}

#[test]
fn unit_callers_deliver_initial_mutable_boundary_values_and_updated_scalar_results() {
    for (argument, expected) in [("7 / 2 * 2", 7), ("rewrite(7 / 2 * 2)", 9)] {
        let source = format!(
            r#"
            boundary trait Sink {{ machine observe(mut input: i32) reaches Sink; }}
            machine rewrite(mut input: i32) -> i32
            requires 9i32 == 9i32
            ensures 9i32 == 9i32
            {{ input = 9; input }}
            machine value() reaches Sink {{ Sink::observe({argument}); }}
            "#,
        );
        let artifact = encoded(&source);
        let mut observer = ObserveArguments::default();
        let result = interpret_terminal_artifact_with_effect_handler_measured(
            &artifact.0,
            &artifact.1,
            &AdmissionProfile::default(),
            &[],
            &[],
            &mut observer,
        )
        .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
        assert_eq!(result.value(), TerminalExecutionResult::Unit);
        assert_eq!(observer.0, vec![vec![integer("i32", expected)]]);
    }
}

#[test]
fn mutable_middle_argument_does_not_reorder_crashing_siblings() {
    for (first, second, expected) in [
        ("Abort", "Trap", terminal_psi::CrashCause::Abort),
        ("Trap", "Abort", terminal_psi::CrashCause::Trap),
    ] {
        let source = format!(
            r#"
            boundary trait Sink {{
                machine observe(first: bool, mut middle: i32, last: bool) reaches Sink;
            }}
            machine first() -> bool crashes {first} {{ crash {first}; }}
            machine second() -> bool crashes {second} {{ crash {second}; }}
            machine value() reaches Sink crashes Abort crashes Trap {{
                Sink::observe(first(), 7 / 2 * 2, second());
            }}
            "#,
        );
        let artifact = encoded(&source);
        let mut observer = ObserveArguments::default();
        let result = interpret_terminal_artifact_with_effect_handler_measured(
            &artifact.0,
            &artifact.1,
            &AdmissionProfile::default(),
            &[],
            &[],
            &mut observer,
        );
        assert!(
            matches!(result,
            Err(TerminalArtifactInterpretError::Execution(TerminalInterpretError::Crash(crash)))
                if crash.cause == expected),
            "{source}"
        );
        assert!(
            observer.0.is_empty(),
            "the outer call cannot run after a sibling crashes"
        );
    }
}

#[test]
fn checked_entry_storage_custody_rejects_missing_stale_and_rebound_parameter_rows() {
    let source = r#"
        machine select(mut first: i32, middle: i32, mut last: i32) -> i32
        requires 0i32 == 0i32
        ensures 0i32 == 0i32
        {
            first = last;
            transition { _ -> finish(first) }
            state finish(mut input: i32) -> i32 { input }
        }
        machine value() -> i32
        requires 0i32 == 0i32
        ensures 0i32 == 0i32
        { select(7, 4, 9) }
    "#;
    assert_execution(source, integer("i32", 9));
    let checked = lower_typed_trees(typed(source)).unwrap();
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "select")
        .unwrap();
    let states = checked.typed.machine_states(machine);
    let entry = states[0].clone();
    let other_state = states[1].symbol;
    let other_parameter = checked.typed.state_parameters(&states[1])[0].symbol;
    let graph_index = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .machines
        .iter()
        .position(|graph| graph.machine == machine.symbol)
        .unwrap();
    let graph = &checked.facts.flow.terminal_scalar_graphs.machines[graph_index];
    let state_index = graph
        .states
        .iter()
        .position(|state| state.state == entry.symbol)
        .unwrap();
    let span = graph.states[state_index].parameter_storage;
    let rows = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .parameter_storage
        .span(span)
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows.iter()
            .map(|row| row.parameter_ordinal)
            .collect::<Vec<_>>(),
        vec![0, 2]
    );
    for mutation in 0..10 {
        let mut candidate = checked.clone();
        let plans = &mut candidate.facts.flow.terminal_scalar_graphs;
        match mutation {
            0 => {
                plans.machines[graph_index].states[state_index].parameter_storage =
                    arena::HandleSpan::empty()
            }
            1 => {
                plans.machines[graph_index].states[state_index].parameter_storage =
                    arena::HandleSpan::from_parts(span.start(), 1)
            }
            2 => {
                plans.machines[graph_index].states[state_index].parameter_storage =
                    arena::HandleSpan::from_parts(
                        arena::Handle::from_parts(
                            span.start().arena_index(),
                            span.start().generation() + 1,
                        ),
                        span.count(),
                    )
            }
            3 => plans.parameter_storage.span_mut(span).unwrap().swap(0, 1),
            4 => {
                let rows = plans.parameter_storage.span_mut(span).unwrap();
                rows[1] = rows[0];
            }
            5 => {
                plans
                    .parameter_storage
                    .get_mut(span.start())
                    .parameter_ordinal = 1
            }
            6 => plans.parameter_storage.get_mut(span.start()).symbol = other_parameter,
            7 => {
                plans.parameter_storage.get_mut(span.start()).primitive_type =
                    typed_trees::types::PrimitiveType::Bool
            }
            8 => {
                candidate
                    .typed
                    .state_parameters
                    .get_mut(entry.parameters.start())
                    .is_mutable = false
            }
            9 => plans.machines[graph_index].states[state_index].state = other_state,
            _ => unreachable!(),
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&candidate, "value").is_err(),
            "entry storage mutation {mutation} must reject before Terminal publication"
        );
    }
}
