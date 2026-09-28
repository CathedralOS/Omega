use super::{
    TerminalExecutionResult, TerminalScalarValue, checked_arms, encode_module,
    encode_proof_section, execute,
};
use checked_trees::{CheckedScalarExpressionBindings, CheckedScalarExpressionRole};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use typed_trees::statement::StatementNode;

const SOURCE: &str = r#"
    machine identity(input: bool) -> bool
    requires true == true
    ensures true == true
    { input }
    machine value(selected: bool, flag: bool) -> bool
    requires true == true
    ensures true == true
    {
        let mut current: bool = flag;
        let saved: bool = current;
        let direct: bool = identity(saved);
        current = !current;
        transition selected {
            true -> finish(current, saved, direct)
            false -> finish(saved, current, direct)
        }
        state finish(first: bool, second: bool, third: bool) -> bool {
            first && !second && (third || !third)
        }
    }
"#;

fn binding_rows(
    checked: &checked_trees::CheckedTrees,
) -> Vec<(
    arena::Handle<CheckedScalarExpressionBindings>,
    CheckedScalarExpressionBindings,
)> {
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let states = checked.typed.machine_states(machine);
    checked
        .facts
        .values
        .scalar_expressions
        .source_bindings
        .iter()
        .filter(|(_, binding)| states.iter().any(|state| state.symbol == binding.state))
        // This fixture selects a scalar graph, whose direct calls consume
        // CallArgument. Alternative Unit/boundary rows are consumed and
        // mutation-tested separately in call_operand_source_custody.
        .filter(|(_, binding)| {
            !matches!(
                binding.role,
                CheckedScalarExpressionRole::UnitCallArgument { .. }
                    | CheckedScalarExpressionRole::BoundaryCallArgument { .. }
            )
        })
        .map(|(handle, binding)| (handle, binding.clone()))
        .collect()
}

fn encoded_checked(checked: &checked_trees::CheckedTrees) -> (Vec<u8>, Vec<u8>) {
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        checked,
        TerminalMachineSelection::Name("value"),
    )
    .unwrap();
    (
        encode_module(&lowered.semantic_module).unwrap(),
        encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap(),
    )
}

#[test]
fn missing_duplicate_or_rebound_pure_source_rows_reject_for_every_consumed_role() {
    for combined in [false, true] {
        let checked = checked_arms(SOURCE, combined);
        encoded_checked(&checked);
        let rows = binding_rows(&checked);
        assert!(!rows.is_empty());
        for (handle, row) in &rows {
            for mutation in 0..9 {
                let mut changed = checked.clone();
                let plans = &mut changed.facts.values.scalar_expressions;
                match mutation {
                    0 => {
                        let mut retained = arena::Arena::new();
                        for (other, binding) in plans.source_bindings.iter() {
                            if other != *handle {
                                retained.append(binding.clone());
                            }
                        }
                        plans.source_bindings = retained;
                    }
                    1 => {
                        plans.source_bindings.append(row.clone());
                    }
                    2 => {
                        plans.source_bindings.get_mut(*handle).expression = arena::Handle::invalid()
                    }
                    3 => {
                        let replacement = changed
                            .typed
                            .expression_table
                            .insert(typed_trees::expression::ExpressionNode::Boolean(true));
                        plans.source_bindings.get_mut(*handle).expression = replacement;
                    }
                    4 => {
                        plans.source_bindings.get_mut(*handle).state =
                            symbols::SymbolHandle::invalid()
                    }
                    5 => plans.source_bindings.get_mut(*handle).statement_ordinal += 100,
                    6 => {
                        plans.source_bindings.get_mut(*handle).role =
                            if row.role == CheckedScalarExpressionRole::Return {
                                CheckedScalarExpressionRole::Guard
                            } else {
                                CheckedScalarExpressionRole::Return
                            }
                    }
                    7 => {
                        plans.source_bindings.get_mut(*handle).destination =
                            if row.destination.is_valid() {
                                symbols::SymbolHandle::invalid()
                            } else {
                                row.state
                            }
                    }
                    8 => {
                        plans.source_bindings.get_mut(*handle).symbols = arena::HandleSpan::empty()
                    }
                    _ => unreachable!(),
                }
                assert!(
                    checked_trees_to_lowered_psi::lower_machine(
                        &changed,
                        TerminalMachineSelection::Name("value")
                    )
                    .is_err(),
                    "source-row mutation={mutation}, role={:?}, statement={}, combined={combined}",
                    row.role,
                    row.statement_ordinal
                );
            }
        }
    }
}

#[test]
fn pure_local_rows_reject_changed_authored_destinations_and_mutability() {
    let checked = checked_arms(SOURCE, false);
    encoded_checked(&checked);
    let rows = binding_rows(&checked);
    let call_argument = rows
        .iter()
        .find(|(_, row)| matches!(row.role, CheckedScalarExpressionRole::CallArgument { .. }))
        .unwrap()
        .1
        .expression;
    let mut mutations = 0;
    for (_, row) in &rows {
        if !matches!(
            row.role,
            CheckedScalarExpressionRole::StorageInitializer
                | CheckedScalarExpressionRole::LocalInitializer { .. }
                | CheckedScalarExpressionRole::AssignmentValue
        ) {
            continue;
        }
        let state = checked
            .typed
            .machines()
            .iter()
            .flat_map(|machine| checked.typed.machine_states(machine))
            .find(|state| state.symbol == row.state)
            .unwrap();
        for change_mutability in [false, true] {
            let mut changed = checked.clone();
            let statement = &mut changed
                .typed
                .statement_table
                .statements_mut(state.statement_nodes)[row.statement_ordinal as usize];
            match statement {
                StatementNode::LocalData(local) => {
                    if change_mutability {
                        local.is_mutable = !local.is_mutable;
                    } else {
                        local.symbol = row.state;
                    }
                }
                StatementNode::Assignment(assignment) => {
                    assignment.target = if change_mutability {
                        call_argument
                    } else {
                        arena::Handle::invalid()
                    };
                }
                _ => panic!("authored local operation"),
            }
            assert!(
                checked_trees_to_lowered_psi::lower_machine(
                    &changed,
                    TerminalMachineSelection::Name("value")
                )
                .is_err(),
                "typed destination mutation role={:?}, mutability={change_mutability}",
                row.role
            );
            mutations += 1;
        }
    }
    assert_eq!(
        mutations, 6,
        "storage initialization, immutable initialization, and assignment"
    );
}

#[test]
fn direct_call_outer_custody_rejects_changed_targets_and_local_declarations() {
    for zero_arguments in [false, true] {
        let (parameters, arguments, first_result, second_result) = if zero_arguments {
            ("", "", "true", "false")
        } else {
            ("input: bool", "flag", "input", "!input")
        };
        let source = format!(
            r#"
            machine first({parameters}) -> bool
            requires true == true
            ensures true == true
            {{ {first_result} }}
            machine second({parameters}) -> bool
            requires true == true
            ensures true == true
            {{ {second_result} }}
            machine integer_carrier(input: u8) -> u8
            requires 0u8 == 0u8
            ensures 0u8 == 0u8
            {{ input }}
            machine value(flag: bool) -> bool
            requires true == true
            ensures true == true
            {{
                let result_value: bool = first({arguments});
                result_value
            }}
        "#
        );
        let checked = checked_arms(&source, false);
        let artifact = encoded_checked(&checked);
        for flag in [false, true] {
            assert_eq!(
                execute(&artifact, &[TerminalScalarValue::Boolean(flag)]).unwrap(),
                TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(
                    zero_arguments || flag
                ))
            );
        }
        let machine = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "value")
            .unwrap();
        let state = &checked.typed.machine_states(machine)[0];
        let alternative = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "second")
            .unwrap();
        let alternative_target = checked.typed.machine_states(alternative)[0].symbol;
        let integer = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "integer_carrier")
            .unwrap();
        let integer_type = checked.typed.machine_states(integer)[0].return_type;
        let StatementNode::LocalData(local) = &checked
            .typed
            .statement_table
            .statements(state.statement_nodes)[0]
        else {
            panic!("direct-call local");
        };
        let call_expression = local.initial_value;
        for mutation in 0..6 {
            let mut changed = checked.clone();
            match mutation {
                0 => {
                    let typed_trees::expression::ExpressionNode::Call(call) = changed
                        .typed
                        .expression_table
                        .expression_mut(call_expression)
                    else {
                        panic!("authored call");
                    };
                    call.target_symbol = alternative_target;
                }
                1..=3 => {
                    let StatementNode::LocalData(local) = &mut changed
                        .typed
                        .statement_table
                        .statements_mut(state.statement_nodes)[0]
                    else {
                        unreachable!();
                    };
                    match mutation {
                        1 => local.type_reference = integer_type,
                        2 => local.is_mutable = true,
                        3 => local.symbol = state.symbol,
                        _ => unreachable!(),
                    }
                }
                4 => {
                    let replacement = if zero_arguments {
                        let value = changed
                            .typed
                            .expression_table
                            .insert(typed_trees::expression::ExpressionNode::Boolean(true));
                        changed
                            .typed
                            .expression_table
                            .insert_expression_handles([value])
                    } else {
                        arena::HandleSpan::empty()
                    };
                    let typed_trees::expression::ExpressionNode::Call(call) = changed
                        .typed
                        .expression_table
                        .expression_mut(call_expression)
                    else {
                        unreachable!();
                    };
                    call.arguments = replacement;
                }
                5 => {
                    let StatementNode::LocalData(local) = &mut changed
                        .typed
                        .statement_table
                        .statements_mut(state.statement_nodes)[0]
                    else {
                        unreachable!();
                    };
                    local.initial_value = arena::Handle::invalid();
                }
                _ => unreachable!(),
            }
            assert!(
                checked_trees_to_lowered_psi::lower_machine(
                    &changed,
                    TerminalMachineSelection::Name("value")
                )
                .is_err(),
                "direct-call outer mutation={mutation}, zero_arguments={zero_arguments}"
            );
        }
    }
}
