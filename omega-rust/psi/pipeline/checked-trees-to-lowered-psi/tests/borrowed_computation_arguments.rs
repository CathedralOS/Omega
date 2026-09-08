//! Computed call operands retain primitive borrows and actual evaluation order.

use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralPrimitiveValue, TerminalStructuralValue,
};

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap()
}

fn unsigned(value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

fn execute(source: &str, expected: &[u128]) {
    execute_with_arguments(source, &[], expected, 1);
}

fn execute_with_arguments(
    source: &str,
    arguments: &[TerminalScalarValue],
    expected: &[u128],
    expected_borrow_calls: u64,
) {
    let checked = checked(source);
    let artifact = terminal_production::produce_terminal_artifact(&checked, "enter")
        .expect("nested primitive borrow reaches the existing Terminal call closure");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let caller = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let parameter = &caller.structural_parameters[0];
    let mut execution =
        TerminalExecution::start_artifact_with_structural_arguments_and_primitive_values(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            arguments,
            &[TerminalStructuralValue {
                opaque_identity: 71,
                structural_type: parameter.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            &[TerminalStructuralPrimitiveValue {
                argument_index: 0,
                value: unsigned(201),
            }],
        )
        .unwrap();
    let mut meter = terminal_fuel::TerminalFuelMeter::with_allowance(0);
    let mut observations = Vec::new();
    let mut completed = false;
    for _ in 0..128 {
        let status = execution.resume(&mut meter).unwrap();
        observations.push(execution.structural_primitive_values()[0].value);
        match status {
            TerminalExecutionStatus::Complete(result) => {
                assert_eq!(result, TerminalExecutionResult::Unit);
                completed = true;
                break;
            }
            TerminalExecutionStatus::SponsorExhausted(_) => meter.replenish(1).unwrap(),
            other => panic!("unexpected computed borrow result: {other:?}"),
        }
    }
    assert!(completed);
    observations.dedup();
    assert_eq!(
        observations,
        expected.iter().copied().map(unsigned).collect::<Vec<_>>()
    );
    let mut invocations = std::collections::BTreeMap::from([(module.entry, 1)]);
    for operation in module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
    {
        let callee = match operation.kind {
            terminal_psi::OperationKind::Call { callee, .. }
            | terminal_psi::OperationKind::CallUnit { callee, .. }
            | terminal_psi::OperationKind::CallStructuralScalar { callee, .. } => callee,
            _ => continue,
        };
        if let Some(usage) = meter
            .usage()
            .at(terminal_fuel::FuelChargeSite::Operation(operation.id))
        {
            *invocations.entry(callee).or_default() += usage.executions();
        }
    }
    let mut borrowed_calls = 0;
    for (machine, operation) in module.machines.iter().flat_map(|machine| {
        machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .map(move |operation| (machine.id, operation))
    }) {
        let Some(usage) = meter
            .usage()
            .at(terminal_fuel::FuelChargeSite::Operation(operation.id))
        else {
            continue;
        };
        assert_eq!(
            usage.executions(),
            invocations[&machine],
            "completed operation must not replay within an invocation"
        );
        if matches!(
            operation.kind,
            terminal_psi::OperationKind::CallStructuralScalar { .. }
        ) {
            assert_eq!(
                usage.units(),
                1,
                "borrowed call uses the existing one-unit charge"
            );
            borrowed_calls += usage.executions();
        }
    }
    assert_eq!(
        borrowed_calls, expected_borrow_calls,
        "selected borrowed invocations"
    );
}

#[test]
fn computed_borrow_mutates_before_the_following_scalar_operand() {
    execute(
        r#"
        machine stamp(value: &mut u64, number: u64) -> u64 { value = number; number }
        machine second(first: u64, second: u64) -> u64 { second }
        machine consume(output: &mut u64, value: u64) { output = value; }
        machine enter(output: &mut u64) {
            let mut scratch: u64 = 91;
            consume(&mut output, second(stamp(&mut scratch, 7), scratch));
            output = scratch;
        }
    "#,
        &[201, 7],
    );
}

#[test]
fn earlier_scalar_operand_is_not_reread_after_the_borrowed_mutation() {
    execute(
        r#"
        machine stamp(value: &mut u64, number: u64) -> u64 { value = number; number }
        machine first(first: u64, second: u64) -> u64 { first }
        machine consume(output: &mut u64, value: u64) { output = value; }
        machine enter(output: &mut u64) {
            let mut scratch: u64 = 91;
            consume(&mut output, first(scratch, stamp(&mut scratch, 7)));
            output = scratch;
        }
    "#,
        &[201, 91, 7],
    );
}

#[test]
fn short_circuit_selection_skips_the_borrowed_mutator() {
    let source = r#"
        machine stamp(value: &mut u64) -> bool { value = 7; true }
        machine consume(value: bool) { }
        machine enter(enabled: bool, output: &mut u64) {
            let mut scratch: u64 = 91;
            consume(enabled && stamp(&mut scratch));
            output = scratch;
        }
    "#;
    execute_with_arguments(
        source,
        &[TerminalScalarValue::Boolean(false)],
        &[201, 91],
        0,
    );
    execute_with_arguments(source, &[TerminalScalarValue::Boolean(true)], &[201, 7], 1);
}

#[test]
fn two_nested_mutators_keep_distinct_invocations_and_one_shared_callee() {
    execute_with_arguments(
        r#"
        machine stamp(value: &mut u64, number: u64) -> u64 { value = number; number }
        machine first(first: u64, second: u64) -> u64 { first }
        machine consume(output: &mut u64, value: u64) { output = value; }
        machine enter(output: &mut u64) {
            let mut scratch: u64 = 91;
            consume(&mut output, first(stamp(&mut scratch, 7), stamp(&mut scratch, 11)));
            output = scratch;
        }
    "#,
        &[],
        &[201, 7, 11],
        2,
    );
}

#[test]
fn nested_primitive_parameter_borrow_preserves_dense_scalar_positions() {
    execute_with_arguments(
        r#"
        machine stamp(value: &mut u64, number: u64) -> u64 { value = number; number }
        machine consume(value: u64) { }
        machine enter(number: u64, output: &mut u64) {
            consume(stamp(&mut output, number));
        }
    "#,
        &[unsigned(17)],
        &[201, 17],
        1,
    );
}

#[test]
fn nested_write_only_local_borrow_updates_the_original_referent() {
    execute(
        r#"
        machine stamp(value: &write u64, number: u64) -> u64 { value = number; number }
        machine consume(output: &mut u64, value: u64) { output = value; }
        machine enter(output: &mut u64) {
            let mut scratch: u64 = 91;
            consume(&mut output, stamp(&write scratch, 7));
            output = scratch;
        }
    "#,
        &[201, 7],
    );
}

#[test]
fn nested_shared_local_borrow_preserves_contents() {
    execute(
        r#"
        machine hold(value: &u64) -> u64 { 11 }
        machine consume(output: &mut u64, value: u64) { output = value; }
        machine enter(output: &mut u64) {
            let mut scratch: u64 = 91;
            consume(&mut output, hold(&scratch));
            output = scratch;
        }
    "#,
        &[201, 11, 91],
    );
}

#[test]
fn repeated_shared_actuals_keep_their_formal_positions() {
    execute(
        r#"
        machine hold(first: &u64, number: u64, second: &u64) -> u64 { number }
        machine consume(output: &mut u64, value: u64) { output = value; }
        machine enter(output: &mut u64) {
            let mut scratch: u64 = 91;
            consume(&mut output, hold(&scratch, 11, &scratch));
            output = scratch;
        }
    "#,
        &[201, 11, 91],
    );
}

#[test]
fn folded_boolean_prefix_keeps_the_selected_comparison_operands() {
    for expression in [
        "true && (scratch == stamp(&mut scratch))",
        "false || (scratch > stamp(&mut scratch))",
        "true && !(scratch == stamp(&mut scratch))",
    ] {
        execute(
            &r#"
            machine stamp(value: &mut u64) -> u64 { value = 7; 7 }
            machine consume(value: bool) { }
            machine enter(output: &mut u64) {
                let mut scratch: u64 = 91;
                consume(EXPRESSION);
                output = scratch;
            }
        "#
            .replace("EXPRESSION", expression),
            &[201, 7],
        );
    }
}

#[test]
fn comparison_operand_cannot_substitute_a_different_mutable_read() {
    let original = checked(
        r#"
        machine stamp(value: &mut u64) -> u64 { value = 7; 7 }
        machine consume(value: bool) { }
        machine enter(output: &mut u64) {
            let mut first: u64 = 41;
            let mut other: u64 = 53;
            consume(first == stamp(&mut first));
            output = other;
        }
    "#,
    );
    let _artifact = terminal_production::produce_terminal_artifact(&original, "enter").unwrap();
    let caller = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "enter")
        .unwrap();
    let state = &original.machine_states(caller)[0];
    let locals = original
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .filter_map(|statement| match statement {
            checked_trees::statement::StatementNode::LocalData(local) => Some(local.symbol),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut changed = original.clone();
    let mut mutations = 0;
    for (handle, node) in original.facts.values.scalar_computations.nodes.iter() {
        if let checked_trees::CheckedScalarComputationKind::Value(
            checked_trees::CheckedScalarExpression::StorageRead {
                symbol,
                primitive_type,
            },
        ) = &node.kind
            && *symbol == locals[0]
        {
            changed
                .facts
                .values
                .scalar_computations
                .nodes
                .get_mut(handle)
                .kind = checked_trees::CheckedScalarComputationKind::Value(
                checked_trees::CheckedScalarExpression::StorageRead {
                    symbol: locals[1],
                    primitive_type: *primitive_type,
                },
            );
            mutations += 1;
        }
    }
    assert!(mutations > 0);
    assert!(terminal_production::produce_terminal_artifact(&changed, "enter").is_err());
}
