use super::*;
use checked_trees::{CheckedScalarExpressionBindings, CheckedScalarExpressionRole};
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

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
    let lowered = checked_trees_to_lowered_psi::lower_machine(checked, "value").unwrap();
    (
        encode_module(&lowered.semantic_module).unwrap(),
        encode_proof_bundle(&lowered.proof_bundle).unwrap(),
    )
}

#[test]
fn pure_scalar_custody_keeps_selected_arguments_and_saved_storage_values() {
    for combined in [false, true] {
        let checked = checked_arms(SOURCE, combined);
        let rows = binding_rows(&checked);
        for role in [
            CheckedScalarExpressionRole::StorageInitializer,
            CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 },
            CheckedScalarExpressionRole::CallArgument {
                binding_ordinal: 1,
                argument_ordinal: 0,
            },
            CheckedScalarExpressionRole::AssignmentValue,
            CheckedScalarExpressionRole::Guard,
            CheckedScalarExpressionRole::Return,
            CheckedScalarExpressionRole::TransitionArgument {
                argument_ordinal: 0,
            },
        ] {
            assert!(
                rows.iter().any(|(_, row)| row.role == role),
                "missing role={role:?}, combined={combined}"
            );
        }
        if combined {
            assert!(rows.iter().any(|(_, row)| row.role
                == CheckedScalarExpressionRole::TransitionContinuationArgument {
                    argument_ordinal: 0
                }));
        }
        let artifact = encoded_checked(&checked);
        for selected in [false, true] {
            for flag in [false, true] {
                assert_eq!(
                    execute(
                        &artifact,
                        &[
                            TerminalScalarValue::Boolean(selected),
                            TerminalScalarValue::Boolean(flag)
                        ]
                    )
                    .unwrap(),
                    TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(if selected {
                        !flag
                    } else {
                        flag
                    })),
                    "selected={selected}, flag={flag}, combined={combined}"
                );
            }
        }
    }
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
                    checked_trees_to_lowered_psi::lower_machine(&changed, "value").is_err(),
                    "source-row mutation={mutation}, role={:?}, statement={}, combined={combined}",
                    row.role,
                    row.statement_ordinal
                );
            }
        }
    }
}

#[test]
fn reordered_pure_operand_namespaces_cannot_rebind_equal_carrier_values() {
    for combined in [false, true] {
        let checked = checked_arms(SOURCE, combined);
        encoded_checked(&checked);
        let rows = binding_rows(&checked);
        let mut mutations = 0;
        for (handle, row) in &rows {
            let symbols = checked
                .facts
                .values
                .scalar_expressions
                .binding_symbols
                .span_or_empty(row.symbols);
            if symbols.len() < 2 {
                continue;
            }
            let mut reordered = symbols.to_vec();
            reordered.swap(0, 1);
            let mut changed = checked.clone();
            let plans = &mut changed.facts.values.scalar_expressions;
            let replacement = plans.binding_symbols.insert_many(reordered);
            plans.source_bindings.get_mut(*handle).symbols = replacement;
            assert!(
                checked_trees_to_lowered_psi::lower_machine(&changed, "value").is_err(),
                "reordered namespace role={:?}, statement={}, combined={combined}",
                row.role,
                row.statement_ordinal
            );
            mutations += 1;
        }
        assert!(
            mutations >= 7,
            "all multi-parameter roles need namespace custody"
        );
    }
}

fn replace_authored_expression(
    checked: &mut checked_trees::CheckedTrees,
    row: &CheckedScalarExpressionBindings,
) {
    let state = checked
        .typed
        .machines()
        .iter()
        .flat_map(|machine| checked.typed.machine_states(machine))
        .find(|state| state.symbol == row.state)
        .unwrap();
    let statements = state.statement_nodes;
    let mut statement = checked.typed.statement_table.statements(statements)
        [row.statement_ordinal as usize]
        .clone();
    let replacement = checked
        .typed
        .expression_table
        .insert(typed_trees::expression::ExpressionNode::Boolean(true));
    match (&mut statement, row.role) {
        (
            StatementNode::LocalData(local),
            CheckedScalarExpressionRole::LocalInitializer { .. }
            | CheckedScalarExpressionRole::StorageInitializer,
        ) => local.initial_value = replacement,
        (
            StatementNode::LocalData(local),
            CheckedScalarExpressionRole::CallArgument {
                argument_ordinal, ..
            },
        ) => {
            let typed_trees::expression::ExpressionNode::Call(call) = checked
                .typed
                .expression_table
                .expression(local.initial_value)
            else {
                panic!("direct initializer call");
            };
            let arguments = call.arguments;
            checked
                .typed
                .expression_table
                .set_expression_handle_at_offset(arguments, argument_ordinal, replacement);
        }
        (StatementNode::Assignment(assignment), CheckedScalarExpressionRole::AssignmentValue) => {
            assignment.value = replacement
        }
        (StatementNode::Expression(expression), CheckedScalarExpressionRole::Return) => {
            *expression = replacement
        }
        (StatementNode::Transition(transition), CheckedScalarExpressionRole::Guard) => {
            transition.guard = TransitionGuardNode::When(replacement)
        }
        (StatementNode::Transition(transition), role) => {
            let target = if matches!(
                role,
                CheckedScalarExpressionRole::ContinuationReturn
                    | CheckedScalarExpressionRole::TransitionContinuationArgument { .. }
            ) {
                transition.continuation
            } else {
                transition.target
            };
            match role {
                CheckedScalarExpressionRole::Return
                | CheckedScalarExpressionRole::ContinuationReturn => {
                    let replacement_target = checked
                        .typed
                        .statement_table
                        .insert_transition_target(TransitionTargetNode::Value(replacement));
                    if role == CheckedScalarExpressionRole::ContinuationReturn {
                        transition.continuation = replacement_target;
                    } else {
                        transition.target = replacement_target;
                    }
                }
                CheckedScalarExpressionRole::TransitionArgument { argument_ordinal }
                | CheckedScalarExpressionRole::TransitionContinuationArgument {
                    argument_ordinal,
                } => {
                    let TransitionTargetNode::Named { arguments, .. } =
                        checked.typed.statement_table.transition_target(target)
                    else {
                        panic!("named target");
                    };
                    let arguments = *arguments;
                    checked
                        .typed
                        .statement_table
                        .set_expression_handle_at_offset(arguments, argument_ordinal, replacement);
                }
                _ => panic!("unexpected transition role {role:?}"),
            }
        }
        _ => panic!("unsupported test source role {:?}", row.role),
    }
    checked.typed.statement_table.statements_mut(statements)[row.statement_ordinal as usize] =
        statement;
}

#[test]
fn pure_plans_reject_changed_authored_expression_handles() {
    for combined in [false, true] {
        let checked = checked_arms(SOURCE, combined);
        encoded_checked(&checked);
        for (_, row) in binding_rows(&checked) {
            let mut changed = checked.clone();
            replace_authored_expression(&mut changed, &row);
            assert!(
                checked_trees_to_lowered_psi::lower_machine(&changed, "value").is_err(),
                "changed authored expression role={:?}, statement={}, combined={combined}",
                row.role,
                row.statement_ordinal
            );
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
                checked_trees_to_lowered_psi::lower_machine(&changed, "value").is_err(),
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
fn pure_return_and_continuation_rows_rejoin_the_selected_source_arm() {
    let source = r#"
        machine value(selected: bool, flag: bool) -> bool
        requires true == true
        ensures true == true
        { transition selected { true -> flag false -> !flag } }
    "#;
    for combined in [false, true] {
        let checked = checked_arms(source, combined);
        let artifact = encoded_checked(&checked);
        for selected in [false, true] {
            for flag in [false, true] {
                assert_eq!(
                    execute(
                        &artifact,
                        &[
                            TerminalScalarValue::Boolean(selected),
                            TerminalScalarValue::Boolean(flag)
                        ]
                    )
                    .unwrap(),
                    TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(if selected {
                        flag
                    } else {
                        !flag
                    }))
                );
            }
        }
        let rows = binding_rows(&checked);
        let returns: Vec<_> = rows
            .iter()
            .filter(|(_, row)| {
                matches!(
                    row.role,
                    CheckedScalarExpressionRole::Return
                        | CheckedScalarExpressionRole::ContinuationReturn
                )
            })
            .collect();
        assert_eq!(returns.len(), 2);
        if combined {
            assert!(
                returns
                    .iter()
                    .any(|(_, row)| row.role == CheckedScalarExpressionRole::ContinuationReturn)
            );
        }
        for (handle, row) in &returns {
            let opposite = returns.iter().find(|(other, _)| other != handle).unwrap();
            let mut changed = checked.clone();
            changed
                .facts
                .values
                .scalar_expressions
                .source_bindings
                .get_mut(*handle)
                .expression = opposite.1.expression;
            assert!(
                checked_trees_to_lowered_psi::lower_machine(&changed, "value").is_err(),
                "swapped pure arm binding"
            );
            let mut changed = checked.clone();
            replace_authored_expression(&mut changed, row);
            assert!(
                checked_trees_to_lowered_psi::lower_machine(&changed, "value").is_err(),
                "changed pure arm expression"
            );
        }
    }
}

#[test]
fn pure_guard_custody_is_required_before_selecting_a_crash_fallback() {
    let source = r#"
        machine value(flag: bool) -> bool
        requires true == true
        ensures true == true
        crashes Trap
        {
            let mut current: bool = flag;
            let saved: bool = current;
            current = !current;
            transition { current -> saved }
            crash Trap;
        }
    "#;
    let checked = checked_arms(source, false);
    let artifact = encoded_checked(&checked);
    assert_eq!(
        execute(&artifact, &[TerminalScalarValue::Boolean(false)]).unwrap(),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(false))
    );
    assert!(
        matches!(execute(&artifact, &[TerminalScalarValue::Boolean(true)]),
        Err(TerminalArtifactInterpretError::Execution(TerminalInterpretError::Crash(crash))) if crash.cause == terminal_psi::CrashCause::Trap)
    );
    let rows = binding_rows(&checked);
    let (handle, guard) = rows
        .iter()
        .find(|(_, row)| row.role == CheckedScalarExpressionRole::Guard)
        .unwrap();
    let mut changed = checked.clone();
    changed
        .facts
        .values
        .scalar_expressions
        .source_bindings
        .get_mut(*handle)
        .expression = arena::Handle::invalid();
    assert!(checked_trees_to_lowered_psi::lower_machine(&changed, "value").is_err());
    let mut changed = checked.clone();
    replace_authored_expression(&mut changed, guard);
    assert!(checked_trees_to_lowered_psi::lower_machine(&changed, "value").is_err());
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
                checked_trees_to_lowered_psi::lower_machine(&changed, "value").is_err(),
                "direct-call outer mutation={mutation}, zero_arguments={zero_arguments}"
            );
        }
    }
}

#[test]
fn pure_named_successors_rejoin_source_and_graph_targets_even_without_arguments() {
    for zero_arguments in [false, true] {
        let (parameters, arguments, first_result, second_result) = if zero_arguments {
            ("", "", "true", "false")
        } else {
            ("input: bool", "flag", "input", "!input")
        };
        let source = format!(
            r#"
            machine value(selected: bool, flag: bool) -> bool
            requires true == true
            ensures true == true
            {{
                transition selected {{
                    true -> first({arguments})
                    false -> second({arguments})
                }}
                state first({parameters}) -> bool {{ {first_result} }}
                state second({parameters}) -> bool {{ {second_result} }}
            }}
        "#
        );
        let checked = checked_arms(&source, false);
        let artifact = encoded_checked(&checked);
        for selected in [false, true] {
            for flag in [false, true] {
                let first = zero_arguments || flag;
                assert_eq!(
                    execute(
                        &artifact,
                        &[
                            TerminalScalarValue::Boolean(selected),
                            TerminalScalarValue::Boolean(flag)
                        ]
                    )
                    .unwrap(),
                    TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(if selected {
                        first
                    } else {
                        !first
                    }))
                );
            }
        }
        let machine = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "value")
            .unwrap();
        let states = checked.typed.machine_states(machine);
        let entry = &states[0];
        let alternative = states
            .iter()
            .find(|state| state.name.as_str() == "second")
            .unwrap()
            .symbol;
        let original = states
            .iter()
            .find(|state| state.name.as_str() == "first")
            .unwrap()
            .symbol;
        for graph_target in [false, true] {
            let mut changed = checked.clone();
            if graph_target {
                let graph = changed
                    .facts
                    .flow
                    .terminal_scalar_graphs
                    .machines
                    .iter_mut()
                    .find(|graph| graph.machine == machine.symbol)
                    .unwrap();
                let state = graph
                    .states
                    .iter_mut()
                    .find(|state| state.state == entry.symbol)
                    .unwrap();
                let checked_trees::CheckedScalarStateTerminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } = &mut state.terminator
                else {
                    panic!("authored conditional jumps");
                };
                let (
                    checked_trees::CheckedScalarBranchDestination::Jump(first),
                    checked_trees::CheckedScalarBranchDestination::Jump(second),
                ) = (when_true, when_false)
                else {
                    panic!("two named successors");
                };
                // Swap only targets, retaining the same reachable states and
                // argument coordinates so reachability cannot explain rejection.
                std::mem::swap(&mut first.target, &mut second.target);
            } else {
                for (ordinal, target_symbol) in [(0, alternative), (1, original)] {
                    let StatementNode::Transition(transition) = &checked
                        .typed
                        .statement_table
                        .statements(entry.statement_nodes)[ordinal]
                    else {
                        panic!("authored transition");
                    };
                    let mut target = checked
                        .typed
                        .statement_table
                        .transition_target(transition.target)
                        .clone();
                    let TransitionTargetNode::Named { path, .. } = &mut target else {
                        panic!("named successor");
                    };
                    path.symbol = target_symbol;
                    path.head_symbol = target_symbol;
                    let target = changed
                        .typed
                        .statement_table
                        .insert_transition_target(target);
                    let StatementNode::Transition(transition) = &mut changed
                        .typed
                        .statement_table
                        .statements_mut(entry.statement_nodes)[ordinal]
                    else {
                        unreachable!();
                    };
                    transition.target = target;
                }
            }
            assert!(
                checked_trees_to_lowered_psi::lower_machine(&changed, "value").is_err(),
                "named successor graph_target={graph_target}, zero_arguments={zero_arguments}"
            );
        }
    }
}
