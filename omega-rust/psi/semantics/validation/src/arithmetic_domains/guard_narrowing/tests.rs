use super::*;
use typed_trees::statement::StatementNode;

fn arrival_program(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

fn delivered_bounds(source: &str) -> Option<Interval> {
    let program = arrival_program(source);
    let machine = &program.machines()[0];
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.name.as_str() == "append")
        .unwrap();
    incoming_guard_env(&program, machine, state).get("delivered")
}

#[test]
fn arrival_bounds_follow_renamed_argument() {
    assert_eq!(
        delivered_bounds(
            "machine main(value: u32 [0..=4]) -> u32 [0..=4] {
        transition value < 4 { true -> append(value) false -> (value) }
        state append(delivered: u32 [0..=4]) -> u32 [0..=4] { delivered + 1 }
    }"
        ),
        Some(Interval {
            low: Some(0),
            high: Some(3)
        })
    );
}

#[test]
fn arrival_bounds_join_every_predecessor_after_binding() {
    assert_eq!(
        delivered_bounds(
            "machine main(value: u32 [0..=4], choice: bool) -> u32 {
        transition choice { true -> left(value) false -> right(value) }
        state left(first: u32 [0..=4]) -> u32 {
            transition first < 3 { true -> append(first) false -> 0u32 }
        }
        state right(second: u32 [0..=4]) -> u32 {
            transition second < 4 { true -> append(second) false -> 0u32 }
        }
        state append(delivered: u32 [0..=4]) -> u32 { delivered }
    }"
        ),
        Some(Interval {
            low: Some(0),
            high: Some(3)
        })
    );
}

#[test]
fn arrival_bounds_include_unguarded_predecessor() {
    assert_eq!(
        delivered_bounds(
            "machine main(value: u32 [0..=4], choice: bool) -> u32 {
        transition choice { true -> left(value) false -> append(value) }
        state left(first: u32 [0..=4]) -> u32 {
            transition first < 4 { true -> append(first) false -> 0u32 }
        }
        state append(delivered: u32 [0..=4]) -> u32 { delivered }
    }"
        ),
        Some(Interval {
            low: Some(0),
            high: Some(4)
        })
    );
}

#[test]
fn arrival_bounds_do_not_bind_by_parameter_spelling() {
    assert_eq!(
        delivered_bounds(
            "machine main(delivered: u32 [0..=4], other: u32 [0..=4]) -> u32 {
        transition delivered < 4 { true -> append(other) false -> 0u32 }
        state append(delivered: u32 [0..=4]) -> u32 { delivered }
    }"
        ),
        Some(Interval {
            low: Some(0),
            high: Some(4)
        })
    );
}

#[test]
fn arrival_bounds_include_changed_loop_argument() {
    assert_eq!(
        delivered_bounds(
            "machine main(value: u32 [0..=4]) -> u32 {
        transition value < 4 { true -> append(value) false -> 0u32 }
        state append(delivered: u32 [0..=4]) -> u32 {
            transition delivered < 4 { true -> append(delivered + 1) false -> (delivered) }
        }
    }"
        ),
        Some(Interval {
            low: Some(0),
            high: Some(4)
        })
    );
}

#[test]
fn arrival_bounds_keep_guarded_loop_contribution() {
    assert_eq!(
        delivered_bounds(
            "machine main(value: u32 [0..=4]) -> u32 {
        transition value < 4 { true -> append(value) false -> 0u32 }
        state append(delivered: u32 [0..=4]) -> u32 {
            transition delivered < 3 { true -> append(delivered + 1) false -> (delivered) }
        }
    }"
        ),
        Some(Interval {
            low: Some(0),
            high: Some(3)
        })
    );
}

#[test]
fn arrival_bounds_include_statement_and_value_calls() {
    for call in ["append(value);", "let returned: u32 = append(value);"] {
        assert_eq!(
            delivered_bounds(&format!(
                "machine main(value: u32 [0..=4]) -> u32 {{
            {call}
            transition value < 4 {{ true -> append(value) false -> 0u32 }}
            state append(delivered: u32 [0..=4]) -> u32 {{ delivered }}
        }}"
            )),
            Some(Interval {
                low: Some(0),
                high: Some(4)
            }),
            "{call}"
        );
    }
}

#[test]
fn arrival_bounds_propagate_facts_through_call_arguments() {
    for call in ["append(first);", "let returned: u32 = append(first);"] {
        assert_eq!(
            delivered_bounds(&format!(
                "machine main(value: u32 [0..=4]) -> u32 {{
            transition value < 4 {{ true -> relay(value) false -> 0u32 }}
            state relay(first: u32 [0..=4]) -> u32 {{ {call} 0u32 }}
            state append(delivered: u32 [0..=4]) -> u32 {{ delivered }}
        }}"
            )),
            Some(Interval {
                low: Some(0),
                high: Some(3)
            }),
            "{call}"
        );
    }
}

#[test]
fn arrival_bounds_follow_false_continuation_polarity() {
    let mut program = arrival_program(
        "machine main(value: u32 [0..=4]) -> u32 {
        transition value >= 4 { true -> 0u32 false -> append(value) }
        state append(delivered: u32 [0..=4]) -> u32 { delivered }
    }",
    );
    let machine = &program.machines()[0];
    let entry = &program.machine_states(machine)[0];
    let transitions = program
        .statement_table
        .iter_statements(entry.statement_nodes)
        .filter_map(|(handle, statement)| match statement {
            StatementNode::Transition(transition) => Some((handle, *transition)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(transitions.len(), 2);
    let mut first = transitions[0].1;
    first.continuation = transitions[1].1.target;
    *program.statement_table.statement_mut(transitions[0].0) = StatementNode::Transition(first);
    let machine = &program.machines()[0];
    let target = program
        .machine_states(machine)
        .iter()
        .find(|state| state.name.as_str() == "append")
        .unwrap();
    assert_eq!(
        incoming_guard_env(&program, machine, target).get("delivered"),
        Some(Interval {
            low: Some(0),
            high: Some(3)
        })
    );
}

fn terminal_bounds(source: &str) -> Option<(i64, i64)> {
    let program = arrival_program(source);
    terminal_bounds_for_program(&program)
}

fn terminal_bounds_for_program(program: &TypedTrees) -> Option<(i64, i64)> {
    let machine = &program.machines()[0];
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.name.as_str() == "append")
        .unwrap();
    let statements = program.statement_table.statements(state.statement_nodes);
    let StatementNode::Expression(expression) = statements.last().unwrap() else {
        panic!("return");
    };
    arrival_integer_expression_bounds(
        program,
        machine.symbol,
        state.symbol,
        statements.len() - 1,
        *expression,
    )
}

#[test]
fn arrival_return_bounds_cross_overlapping_writes() {
    for (write, expected) in [("", (1, 4)), ("self.value = 4;", (1, 5))] {
        assert_eq!(
            terminal_bounds(&format!(
                "data Main {{ value: u32 [0..=4]; }}
            machine Main::main(&mut self) -> u32 [0..=4] {{
                transition self.value < 4 {{ true -> append() false -> 0u32 }}
                state append(&mut self) -> u32 [0..=4] {{ {write} self.value + 1 }}
            }}"
            )),
            Some(expected),
            "{write}"
        );
    }
}

#[test]
fn arrival_return_bounds_cross_mutable_parameter_writes() {
    assert_eq!(
        terminal_bounds(
            "machine main(value: u32 [0..=4]) -> u32 [0..=4] {
        transition value < 4 { true -> append(value) false -> 0u32 }
        state append(mut delivered: u32 [0..=4]) -> u32 [0..=4] {
            delivered = 4; delivered + 1
        }
    }"
        ),
        Some((1, 5))
    );
}

#[test]
fn arrival_bounds_drop_facts_before_mutated_forwarding() {
    assert_eq!(
        delivered_bounds(
            "machine main(value: u32 [0..=4]) -> u32 {
        transition value < 4 { true -> relay(value) false -> 0u32 }
        state relay(mut first: u32 [0..=4]) -> u32 {
            first = 4; transition { _ -> append(first) }
        }
        state append(delivered: u32 [0..=4]) -> u32 { delivered }
    }"
        ),
        Some(Interval {
            low: Some(0),
            high: Some(4)
        })
    );
}

#[test]
fn arrival_return_bounds_cross_call_writes() {
    assert_eq!(
        terminal_bounds(
            "data Main { value: u32 [0..=4]; }
        machine Main::main(&mut self) -> u32 [0..=4] {
            transition self.value < 4 { true -> append() false -> 0u32 }
            state append(&mut self) -> u32 [0..=4] {
                overwrite(&mut self.value); self.value + 1
            }
        }
        machine overwrite(value: &mut u32 [0..=4]) { value = 4; }
    "
        ),
        Some((1, 5))
    );
}

#[test]
fn arrival_bounds_preserve_scalar_argument_evaluation_position() {
    for (arguments, expected) in [
        ("self.value, overwrite(&mut self.value)", (1, 4)),
        ("overwrite(&mut self.value), self.value", (1, 5)),
    ] {
        let parameters = if expected == (1, 4) {
            "delivered: u32 [0..=4], ignored: u32"
        } else {
            "ignored: u32, delivered: u32 [0..=4]"
        };
        let source = format!(
            "data Main {{ value: u32 [0..=4]; }}
            machine Main::main(&mut self) -> u32 [0..=4] {{
                transition self.value < 4 {{ true -> append({arguments}) false -> 0u32 }}
                state append({parameters}) -> u32 [0..=4] {{ delivered + 1 }}
            }}
            machine overwrite(value: &mut u32 [0..=4]) -> u32 {{ value = 4; 0u32 }}
        "
        );
        // Source normalization currently hoists this call before the complete
        // jump argument list. The actual typed prefix therefore invalidates
        // self.value in both source variants.
        assert_eq!(
            terminal_bounds(&source),
            Some((1, 5)),
            "{arguments}: hoisted call"
        );
        let mut program = arrival_program(&source);
        restore_argument_call_position(&mut program);
        assert_eq!(
            terminal_bounds_for_program(&program),
            Some(expected),
            "{arguments}: direct call"
        );
    }
}

fn restore_argument_call_position(program: &mut TypedTrees) {
    use typed_trees::statement::TransitionTargetNode;
    let machine = &program.machines()[0];
    let arm = program
        .machine_states(machine)
        .iter()
        .find(|state| state.name.as_str().starts_with("__arm_"))
        .unwrap();
    let statements = program
        .statement_table
        .iter_statements(arm.statement_nodes)
        .collect::<Vec<_>>();
    let (local_handle, StatementNode::LocalData(local)) = statements[0] else {
        panic!("call hoist");
    };
    let local = local.clone();
    let StatementNode::Transition(transition) = statements[1].1 else {
        panic!("jump");
    };
    let TransitionTargetNode::Named { arguments, .. } =
        program.statement_table.transition_target(transition.target)
    else {
        panic!("named jump");
    };
    let arguments = *arguments;
    let offset = program
        .statement_table
        .expression_handles(arguments)
        .iter()
        .position(|argument| {
            program.expression_table.display_name(*argument) == local.name.as_str()
        })
        .unwrap();
    let last_machine = program.machines().last().unwrap();
    let last_state = &program.machine_states(last_machine)[0];
    let StatementNode::Expression(zero) = program
        .statement_table
        .statements(last_state.statement_nodes)
        .last()
        .unwrap()
    else {
        panic!("literal return");
    };
    let zero = *zero;
    program.statement_table.set_expression_handle_at_offset(
        arguments,
        offset as u32,
        local.initial_value,
    );
    let StatementNode::LocalData(hoist) = program.statement_table.statement_mut(local_handle)
    else {
        unreachable!();
    };
    hoist.initial_value = zero;
}

#[test]
fn arrival_reference_argument_does_not_freeze_the_referent() {
    assert_eq!(
        terminal_bounds(
            "data Main { value: u32 [0..=4]; }
        machine Main::main(&mut self) -> u32 [0..=4] {
            transition self.value < 4 {
                true -> append(&mut self.value, overwrite(&mut self.value))
                false -> 0u32
            }
            state append(delivered: &mut u32 [0..=4], ignored: u32) -> u32 [0..=4] {
                delivered + 1
            }
        }
        machine overwrite(value: &mut u32 [0..=4]) -> u32 { value = 4; 0u32 }
    "
        ),
        Some((1, 5))
    );
}

#[test]
fn bounded_returns_require_all_arrivals_and_surviving_facts() {
    for (arguments, prefix, accepted) in [
        ("value", "", true),
        ("other", "", false),
        ("value + 1", "", false),
        ("value", "delivered = 4;", false),
    ] {
        let program = arrival_program(&format!(
            "machine main(value: u32 [0..=4], other: u32 [0..=4]) -> u32 [0..=4] {{
            transition value < 4 {{ true -> append({arguments}) false -> 0u32 }}
            state append(mut delivered: u32 [0..=4]) -> u32 [0..=4] {{
                {prefix} delivered + 1
            }}
        }}"
        ));
        let result = crate::validate_program(&program);
        assert_eq!(
            result.is_ok(),
            accepted,
            "{arguments}, {prefix}: {result:?}"
        );
        if !accepted {
            assert!(format!("{result:?}").contains("not provably within its declared range"));
        }
    }
}

#[test]
fn bounded_return_rejects_one_unguarded_predecessor() {
    let program = arrival_program(
        "machine main(value: u32 [0..=4], choice: bool) -> u32 [0..=4] {
            transition choice { true -> guarded(value) false -> append(value) }
            state guarded(source: u32 [0..=4]) -> u32 [0..=4] {
                transition source < 4 { true -> append(source) false -> 0u32 }
            }
            state append(delivered: u32 [0..=4]) -> u32 [0..=4] { delivered + 1 }
        }",
    );
    let diagnostics = crate::validate_program(&program).expect_err("unguarded arrival");
    let report = format!("{diagnostics:?}");
    assert!(report.contains("state `append` terminal expression returns a value not provably within its declared range"), "{report}");
}

#[test]
fn arrival_return_requirement_uses_the_owning_parameter_symbol() {
    let mut program = arrival_program(
        "machine increment(value: u32 [0..=4]) -> u32 [0..=4]
            requires value < 4 { value + 1 }
         machine other(value: u32 [0..=4]) -> u32 { value }",
    );
    let other = &program.machines()[1];
    let other_symbol = program.state_parameters(&program.machine_states(other)[0])[0].symbol;
    let operand = program
        .expression_table
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::Less => {
                Some(binary.left)
            }
            _ => None,
        })
        .unwrap();
    let ExpressionNode::Name(path) = program.expression_table.expression_mut(operand) else {
        panic!("parameter");
    };
    path.symbol = other_symbol;
    path.head_symbol = other_symbol;
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let statements = program.statement_table.statements(state.statement_nodes);
    let StatementNode::Expression(expression) = statements.last().unwrap() else {
        panic!("return");
    };
    assert_eq!(
        arrival_integer_expression_bounds(
            &program,
            machine.symbol,
            state.symbol,
            statements.len() - 1,
            *expression
        ),
        Some((1, 5))
    );
}

fn program_with_guard(condition: &str) -> TypedTrees {
    let source = format!(
        "machine value(remaining: u32) -> u32 {{
             transition {condition} {{ true -> 1u32 false -> 0u32 }}
         }}"
    );
    let tokens = source_files_to_tokens::Lexer::new(&source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

#[test]
fn nested_boolean_guard_wrappers_preserve_integer_bound_polarity() {
    for (condition, positive_rank) in [
        ("remaining > 0", true),
        ("!(remaining > 0)", false),
        ("!!(remaining > 0)", true),
        ("(!(remaining > 0)) == false", true),
        ("false == (!(remaining > 0))", true),
        ("(!(remaining > 0)) != true", true),
        ("true != (!(remaining > 0))", true),
        ("!((remaining > 0) != false)", false),
    ] {
        let program = program_with_guard(condition);
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let StatementNode::Transition(transition) =
            &program.statement_table.statements(state.statement_nodes)[0]
        else {
            panic!("authored transition");
        };
        let selected = guard_narrowed_env(
            &program,
            machine,
            Some(state),
            &transition.guard,
            &ValueEnv::new(),
        );
        let fallback = fall_through_narrowed_env(
            &program,
            machine,
            Some(state),
            &transition.guard,
            &ValueEnv::new(),
        );
        let positive = Interval {
            low: Some(1),
            high: Some(i64::from(u32::MAX)),
        };
        let zero = Interval::constant(0);
        assert_eq!(
            selected.get("remaining"),
            Some(if positive_rank { positive } else { zero }),
            "{condition}: selected"
        );
        assert_eq!(
            fallback.get("remaining"),
            Some(if positive_rank { zero } else { positive }),
            "{condition}: fallback"
        );
    }
}
