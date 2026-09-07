use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{encode_module, encode_proof_bundle};
use terminal_interpreter::{
    TerminalExecutionResult, TerminalScalarValue, interpret_terminal_artifact,
};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

fn execute(source: &str) -> TerminalExecutionResult {
    let checked = checked(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "value")
        .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
    let semantics = encode_module(&lowered.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("encode proof");
    interpret_terminal_artifact(&semantics, &proof, &AdmissionProfile::default(), &[])
        .unwrap_or_else(|error| panic!("{source}: {error:#?}"))
}

#[test]
fn scalar_storage_initialization_update_and_saved_values_execute_after_serialization() {
    for (body, expected) in [
        ("let mut current: u8 = 3 + 4; current", 7),
        ("let mut current: u8 = 7; current = current + 1; current", 8),
        (
            "let first: u8 = 2; let mut current: u8 = 7; let saved: u8 = current; current = current + first; let next: u8 = 3; saved + next",
            10,
        ),
        (
            "let mut current: u8 = 255; current = ((current as u8 in Wrapping) + 2) as u8; current",
            1,
        ),
        (
            "let mut current: u8 = 255; current = ((current as u8 in Saturating) + 2) as u8; current",
            255,
        ),
    ] {
        let source = format!(
            "machine value() -> u8\nrequires {expected}u8 == {expected}u8\nensures {expected}u8 == {expected}u8\n{{ {body} }}"
        );
        assert_eq!(
            execute(&source),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                value: IntegerValue::Unsigned(expected)
            }),
            "{source}"
        );
    }
}

#[test]
fn local_storage_call_snapshot_agrees_with_canonical_execution() {
    let source = r#"
        machine halve(input: u8) -> u8
        requires 65u8 == 65u8
        ensures 65u8 == 65u8
        {
            let mut current: u8 = input;
            current = current / 2;
            let saved: u8 = current;
            current = 200;
            saved
        }
        machine value() -> u8
        requires 65u8 == 65u8
        ensures 65u8 == 65u8
        {
            let byte: u8 = halve(130);
            byte
        }
    "#;
    assert_call_snapshot_and_execution(source, 0);
}

#[test]
fn owned_mutable_parameter_snapshot_agrees_with_canonical_execution() {
    let source = r#"
        machine halve(mut current: u8) -> u8
        requires 65u8 == 65u8
        ensures 65u8 == 65u8
        {
            current = current / 2;
            let saved: u8 = current;
            current = 200;
            saved
        }
        machine value() -> u8
        requires 65u8 == 65u8
        ensures 65u8 == 65u8
        {
            let mut input: u8 = 130;
            let byte: u8 = halve(input);
            input = 200;
            byte
        }
    "#;
    assert_call_snapshot_and_execution(source, 1);
}

fn assert_call_snapshot_and_execution(source: &str, statement_index: usize) {
    let checked = checked(source);
    let caller = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let state = &checked.machine_states(caller)[0];
    let point = facts::ProgramPoint::Statement {
        machine_symbol: caller.symbol,
        state_symbol: state.symbol,
        statement_index,
    };
    let snapshots = checked
        .facts
        .semantic
        .facts
        .iter()
        .filter_map(|(_, fact)| {
            if fact.point != point {
                return None;
            }
            let facts::FactPayload::AssignedScalarValue { value } = fact.payload else {
                return None;
            };
            Some(checked.facts.semantic.scalar_values.get(value))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        snapshots,
        vec![&facts::ScalarValue::Integer(
            numerics::bignum::BigInt::from_u64(65)
        )]
    );
    assert_eq!(
        execute(source),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            value: IntegerValue::Unsigned(65),
        })
    );
}

#[test]
fn boolean_storage_updates_keep_prior_immutable_values() {
    let source = "machine value() -> bool\nrequires true == true\nensures true == true\n{ let mut flag: bool = false; let saved: bool = flag; flag = !flag; flag && !saved }";
    assert_eq!(
        execute(source),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true))
    );
}

#[test]
fn scalar_storage_remapping_preserves_direct_calls_and_explicit_state_arguments() {
    for source in [
        "machine identity(input: bool) -> bool\nrequires true == true\nensures true == true\n{ input }\nmachine value() -> bool\nrequires true == true\nensures true == true\n{ let mut flag: bool = false; let saved: bool = flag; flag = true; let returned: bool = identity(flag); returned && !saved }",
        "machine value() -> bool\nrequires true == true\nensures true == true\n{ let mut flag: bool = false; let saved: bool = flag; flag = flag || true; transition { _ -> finish(flag, saved) } state finish(current: bool, prior: bool) -> bool { current && !prior } }",
    ] {
        assert_eq!(
            execute(source),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true))
        );
    }
}

#[test]
fn computed_scalar_state_arguments_execute_with_the_selected_operations() {
    for (initial, argument, expected) in [
        (3, "current + 4", 7),
        (255, "((current as u8 in Wrapping) + 1) as u8", 0),
        (255, "((current as u8 in Saturating) + 1) as u8", 255),
    ] {
        let source = format!(
            "machine value() -> u8\nrequires {expected}u8 == {expected}u8\nensures {expected}u8 == {expected}u8\n{{ let mut current: u8 = {initial}; transition {{ _ -> finish({argument}) }} state finish(input: u8) -> u8 {{ input }} }}"
        );
        assert_eq!(
            execute(&source),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                value: IntegerValue::Unsigned(expected),
            }),
        );
    }
}

#[test]
fn scalar_storage_guards_select_using_the_updated_value() {
    for (initial, replacement) in [(false, true), (true, false)] {
        let source = format!(
            "machine value() -> bool\nrequires true == true\nensures true == true\n{{
                let mut flag: bool = {initial};
                let saved: bool = flag;
                flag = {replacement};
                transition flag {{
                    true -> yes(flag, saved)
                    _ -> no(flag, saved)
                }}
                state yes(current: bool, prior: bool) -> bool {{ current && !prior }}
                state no(current: bool, prior: bool) -> bool {{ !current && prior }}
            }}"
        );
        assert_eq!(
            execute(&source),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true))
        );
    }
}

#[test]
fn changed_scalar_storage_destination_custody_rejects() {
    use checked_trees::CheckedScalarBindingDestination;
    let source = "machine value() -> u8\nrequires 3u8 == 3u8\nensures 3u8 == 3u8\n{ let mut first: u8 = 1; let mut second: u8 = 2; first = 3; first }";
    let original = checked(source);
    checked_trees_to_lowered_psi::lower_machine(&original, "value")
        .expect("unmodified storage graph lowers");
    for mutation in 0..4 {
        let mut changed = original.clone();
        let bindings =
            &mut changed.facts.flow.terminal_scalar_graphs.machines[0].states[0].bindings;
        let CheckedScalarBindingDestination::StorageInitialize { symbol } = bindings[1].destination
        else {
            panic!("second storage")
        };
        match mutation {
            0 => {
                bindings[2].destination = CheckedScalarBindingDestination::StorageAssign { symbol }
            }
            1 => {
                bindings[0].destination = CheckedScalarBindingDestination::StorageAssign { symbol }
            }
            2 => bindings[1].destination = bindings[0].destination,
            _ => bindings[2].primitive_type = typed_trees::types::PrimitiveType::Bool,
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "value").is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn scalar_storage_reads_reject_stale_symbols_and_duplicate_computations() {
    use checked_trees::{CheckedScalarExpression, CheckedScalarExpressionRole};
    let source = "machine value() -> u8\nrequires 7u8 == 7u8\nensures 7u8 == 7u8\n{ let mut current: u8 = 7; current }";
    let original = checked(source);
    checked_trees_to_lowered_psi::lower_machine(&original, "value")
        .expect("unmodified storage read lowers");
    for mutation in 0..3 {
        let mut changed = original.clone();
        let expressions = &mut changed.facts.values.scalar_expressions.expressions;
        let position = expressions
            .iter()
            .position(|expression| expression.role == CheckedScalarExpressionRole::Return)
            .expect("selected return");
        if mutation == 2 {
            expressions.push(expressions[position].clone());
        } else {
            let CheckedScalarExpression::StorageRead { symbol, .. } =
                &mut expressions[position].expression
            else {
                panic!("return reads exact storage")
            };
            *symbol = if mutation == 0 {
                symbols::SymbolHandle::invalid()
            } else {
                symbols::SymbolHandle::from_parts(symbol.arena_index(), symbol.generation() + 1)
            };
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "value").is_err(),
            "mutation {mutation}"
        );
    }
}
