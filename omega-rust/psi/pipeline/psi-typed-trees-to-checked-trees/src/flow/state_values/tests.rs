use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_tokens_to_syntax_trees::parse_syntax_trees;

#[test]
fn absent_and_stale_expression_handles_are_not_literal_evidence() {
    let mut program = psi_typed_trees::TypedTrees::default();
    let zero = program
        .expression_table
        .insert(psi_typed_trees::expression::ExpressionNode::default());
    let stale = psi_typed_trees::expression::ExpressionHandle::from_parts(
        zero.arena_index(),
        zero.generation() + 1,
    );
    let missing = psi_typed_trees::expression::ExpressionHandle::from_arena_index(u32::MAX);
    for unknown in [Default::default(), stale, missing] {
        assert!(super::literal(&program, unknown).is_none());
    }
    assert!(super::literal(&program, zero).is_some());
}

#[test]
fn cross_owner_named_dispatch_is_an_unknown_incoming_edge() {
    let source = r#"
        machine destination() -> u8 {
            transition { _ -> finish(3) }
            state finish(current: u8) -> u8 { current }
        }
        machine caller() -> u8 {
            transition { _ -> next(4) }
            state next(current: u8) -> u8 { current }
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let program = lower_symbol_resolved_trees(&resolved).expect("type");
    let destination = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "destination")
        .unwrap();
    let finish = &program.machine_states(destination)[1];
    let parameter = program.state_parameters(finish)[0].symbol;
    let literal = program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, node)| {
            matches!(
                node,
                psi_typed_trees::expression::ExpressionNode::Integer(_)
            )
            .then_some(handle)
        })
        .unwrap();
    let caller = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "caller")
        .unwrap();
    let entry = &program.machine_states(caller)[0];
    let scalar_expressions = Default::default();
    let mut context = super::FlowBuildContext::new(
        &Default::default(),
        &Default::default(),
        &Default::default(),
        &scalar_expressions,
    );
    super::join(
        &mut context,
        super::StateValues {
            state: finish.symbol,
            values: vec![(parameter, super::literal(&program, literal).unwrap())],
        },
    );
    // A retained cross-owner selected call target must participate even when
    // its source coordinate is a Named transition, not an ordinary call.
    let call = super::BorrowCallFact {
        target_symbol: finish.symbol,
        ..Default::default()
    };
    assert!(matches!(
        super::find_call_site(&program, caller.symbol, entry.symbol, 0, 0),
        Some(super::CallSite::TransitionNamed { .. })
    ));
    super::record_invocation(&program, &mut context, caller, entry, &call);
    assert_eq!(context.state_value_inputs.len(), 1);
    assert_eq!(
        context.state_value_inputs[0].values[0].1,
        psi_facts::ScalarValue::Unknown
    );
}

fn check(source: &str, accepted: bool) {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    match crate::lower_typed_trees(typed) {
        Ok(_) => assert!(accepted, "unproved input accepted:\n{source}"),
        Err(diagnostics) => {
            assert!(!accepted, "{diagnostics:#?}\n{source}");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("ensures")),
                "expected ensures failure: {diagnostics:#?}\n{source}"
            );
        }
    }
}

#[test]
fn computed_state_arguments_use_selected_scalar_operations() {
    for (scalar_type, body, argument, expected) in [
        ("u8", "let current: u8 = 3;", "current + 4", "7"),
        (
            "u8",
            "let mut current: u8 = 255;",
            "((current as u8 in Wrapping) + 1) as u8",
            "0",
        ),
        (
            "u8",
            "let mut current: u8 = 255;",
            "((current as u8 in Saturating) + 1) as u8",
            "255",
        ),
        ("bool", "let mut current: bool = false;", "!current", "true"),
        ("bool", "let current: u8 = 3;", "current < 4", "true"),
    ] {
        for accepted in [true, false] {
            let comparison = if accepted { "==" } else { "!=" };
            check(
                &format!(
                    "machine produce() -> {scalar_type} ensures result {comparison} {expected} {{ {body} transition {{ _ -> finish({argument}) }} state finish(value: {scalar_type}) -> {scalar_type} {{ value }} }}"
                ),
                accepted,
            );
        }
    }
}

#[test]
fn state_argument_values_are_saved_before_later_argument_writes() {
    for (argument, expected) in [("current", "3"), ("current + 1", "4")] {
        check(
            &format!(
                r#"
                machine replace(target: &mut u8) -> u8 ensures target == 9 {{ target = 9; 0 }}
                machine produce() -> u8 ensures result == {expected} {{
                    let mut current: u8 = 3;
                    transition {{ _ -> finish({argument}, replace(&mut current)) }}
                    state finish(value: u8, ignored: u8) -> u8 {{ value }}
                }}
            "#
            ),
            true,
        );
    }
    check(
        r#"
            data Main { current: u8; }
            machine Main::replace(&mut self) -> u8 ensures self.current == 9 { self.current = 9; 0 }
            machine Main::produce(&mut self) -> u8 ensures result == 3 {
                self.current = 3;
                transition { _ -> finish(self.current, self.replace()) }
                state finish(value: u8, ignored: u8) -> u8 { value }
            }
        "#,
        true,
    );
}

#[test]
fn computed_argument_capture_requires_unique_exact_source_and_destination() {
    use psi_checked_trees::CheckedScalarExpressionRole;
    use psi_typed_trees::statement::{StatementNode, TransitionTargetNode};
    let source = "machine produce() -> u8 { transition { _ -> finish(3u8 + 4u8) } state finish(value: u8) -> u8 { value } }";
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = crate::lower_typed_trees(typed).expect("checked scalar jump");
    let program = &checked.typed;
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let StatementNode::Transition(transition) =
        &program.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("jump")
    };
    let TransitionTargetNode::Named { arguments, .. } =
        program.statement_table.transition_target(transition.target)
    else {
        panic!("named jump")
    };
    let argument = program.statement_table.expression_handles(*arguments)[0];
    for mutation in 0..5 {
        let mut plans = checked.facts.values.scalar_expressions.clone();
        let binding = plans
            .source_bindings
            .iter()
            .find(|(_, binding)| {
                matches!(
                    binding.role,
                    CheckedScalarExpressionRole::TransitionArgument { .. }
                )
            })
            .unwrap()
            .0;
        match mutation {
            1 => plans.source_bindings.get_mut(binding).destination = Default::default(),
            2 => plans.source_bindings.get_mut(binding).expression = Default::default(),
            3 => {
                plans
                    .source_bindings
                    .append(plans.source_bindings.get(binding).clone());
            }
            4 => {
                let expression = plans
                    .expressions
                    .iter()
                    .find(|expression| {
                        matches!(
                            expression.role,
                            CheckedScalarExpressionRole::TransitionArgument { .. }
                        )
                    })
                    .unwrap()
                    .clone();
                plans.expressions.push(expression);
            }
            _ => {}
        }
        let semantic = Default::default();
        let context = super::FlowBuildContext::new(
            &Default::default(),
            &Default::default(),
            &semantic,
            &plans,
        );
        let value = super::capture_argument(
            program,
            &semantic,
            &context,
            machine,
            state,
            0,
            transition.target,
            0,
            argument,
            Default::default(),
        );
        assert_eq!(
            value,
            if mutation == 0 {
                psi_facts::ScalarValue::Integer(psi_numerics::bignum::BigInt::from_u64(7))
            } else {
                psi_facts::ScalarValue::Unknown
            },
            "mutation {mutation}"
        );
    }
}

#[test]
fn sibling_continuation_values_have_independent_capture() {
    use psi_typed_trees::statement::StatementNode;
    for (other, accepted) in [(4, true), (5, false)] {
        let source = format!(
            "machine produce(flag: bool) -> u8 ensures result == 7 {{ transition flag {{ true -> first(3u8 + 4u8) false -> second(3u8 + {other}u8) }} state first(value: u8) -> u8 {{ value }} state second(value: u8) -> u8 {{ value }} }}"
        );
        let tokens = Lexer::new(&source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let mut typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let machine = typed.machines()[0].clone();
        let nodes = typed.machine_states(&machine)[0].statement_nodes;
        let [
            StatementNode::Transition(_),
            StatementNode::Transition(other),
        ] = typed.statement_table.statements(nodes)
        else {
            panic!("two authored arms")
        };
        let continuation = other.target;
        let StatementNode::Transition(first) = &mut typed.statement_table.statements_mut(nodes)[0]
        else {
            panic!("primary arm")
        };
        first.continuation = continuation;
        typed.machine_states_mut(&machine)[0].statement_nodes =
            psi_arena::HandleSpan::from_parts(nodes.start(), 1);
        let result = crate::lower_typed_trees(typed);
        assert_eq!(result.is_ok(), accepted, "{source}: {result:?}");
    }
}

#[test]
fn reached_unknown_input_is_not_a_zero_literal() {
    for alternative in ["unknown", "1"] {
        check(
            &format!(
                r#"
            machine produce(flag: bool, unknown: u8) -> u8
            ensures result == 0
            {{
                transition flag {{ true -> finish(0) false -> finish({alternative}) }}
                state finish(current: u8) -> u8 {{ current }}
            }}
        "#
            ),
            false,
        );
    }
}

#[test]
fn late_predecessors_retire_already_built_constant_contexts() {
    for (alternative, accepted) in [("3", true), ("4", false), ("unknown", false)] {
        check(
            &format!(
                r#"
            machine produce(flag: bool, unknown: u8) -> u8 ensures result == 3 {{
                transition flag {{ true -> first(3) false -> last({alternative}) }}
                state first(value: u8) -> u8 {{ transition {{ _ -> finish(value) }} }}
                state finish(current: u8) -> u8 {{ current }}
                state last(value: u8) -> u8 {{ transition {{ _ -> finish(value) }} }}
            }}
        "#
            ),
            accepted,
        );
    }
}

#[test]
fn reverse_declaration_order_does_not_change_scalar_transfer() {
    check(
        r#"
        machine produce() -> u8 ensures result == 3 {
            transition { _ -> first(3) }
            state finish(current: u8) -> u8 { current }
            state third(value: u8) -> u8 { transition { _ -> finish(value) } }
            state second(value: u8) -> u8 { transition { _ -> third(value) } }
            state first(value: u8) -> u8 { transition { _ -> second(value) } }
        }
    "#,
        true,
    );
}

#[test]
fn scalar_input_meet_compares_integer_payloads() {
    for alternative in ["0x3", "3u8"] {
        check(
            &format!(
                r#"
            machine produce(flag: bool) -> u8
            ensures result == 3
            {{
                transition flag {{ true -> finish(3) false -> finish({alternative}) }}
                state finish(current: u8) -> u8 {{ current }}
            }}
        "#
            ),
            true,
        );
    }
}

#[test]
fn later_argument_mutation_cannot_relabel_an_earlier_captured_value() {
    check(
        r#"
        data Main { value: u8; }
        machine Main::replace(&mut self) -> u8 ensures self.value == 4 {
            self.value = 4;
            0
        }
        machine Main::produce(&mut self) -> u8
        ensures result == 4
        {
            self.value = 3;
            transition { _ -> finish(self.value, self.replace()) }
            state finish(current: u8, ignored: u8) -> u8 { current }
        }
    "#,
        false,
    );
}

#[test]
fn incoming_parameter_facts_have_independent_mutation_lifetimes() {
    check(
        r#"
        machine produce() -> u8 ensures result == 3 {
            transition { _ -> finish(3, 4) }
            state finish(current: u8, mut other: u8) -> u8 {
                other = 5;
                current
            }
        }
    "#,
        true,
    );
}

#[test]
fn scalar_argument_capture_retires_overlap_but_preserves_disjoint_storage() {
    for (target, expected, accepted) in [("value", 4, false), ("other", 3, true)] {
        check(
            &format!(
                r#"
            machine replace(target: &mut u8) -> u8 ensures target == 4 {{
                target = 4;
                0
            }}
            machine produce() -> u8 ensures result == {expected} {{
                let value: u8 = 3;
                let other: u8 = 2;
                transition {{ _ -> finish(value, replace(&mut {target})) }}
                state finish(current: u8, ignored: u8) -> u8 {{ current }}
            }}
        "#
            ),
            accepted,
        );
    }
}

#[test]
fn scalar_argument_capture_closes_later_mutation_over_reference_aliases() {
    check(
        r#"
        machine replace(target: &mut u8) -> u8 ensures target == 4 {
            target = 4;
            0
        }
        machine produce() -> u8 ensures result == 4 {
            let value: u8 = 3;
            let alias: &mut u8 = &mut value;
            transition { _ -> finish(alias, replace(&mut alias)) }
            state finish(current: u8, ignored: u8) -> u8 { current }
        }
    "#,
        false,
    );
}

#[test]
fn reached_loop_preserves_only_agreed_scalar_inputs() {
    for (backedge, accepted) in [("current", true), ("4", false)] {
        check(
            &format!(
                r#"
            machine produce(flag: bool) -> u8
            ensures result == 3
            {{
                transition {{ _ -> repeat(3, flag) }}
                state repeat(current: u8, again: bool) -> u8 {{
                    transition again {{ true -> repeat({backedge}, again) false -> current }}
                }}
            }}
        "#
            ),
            accepted,
        );
    }
}

#[test]
fn self_transition_preserves_a_reached_constant() {
    check(
        r#"
        machine produce(flag: bool) -> u8 ensures result == 3 {
            transition { _ -> repeat(3, flag) }
            state repeat(current: u8, again: bool) -> u8 {
                transition again { true -> self false -> current }
            }
        }
    "#,
        true,
    );
}

#[test]
fn scalar_transfer_converges_beyond_sixty_four_states() {
    let mut source = String::from(
        "machine produce() -> u8 ensures result == 3 {\ntransition { _ -> hop0(3) }\n",
    );
    for index in 0..70 {
        source.push_str(&format!("state hop{index}(current: u8) -> u8 {{\n"));
        if index == 69 {
            source.push_str("current\n");
        } else {
            source.push_str(&format!(
                "transition {{ _ -> hop{}(current) }}\n",
                index + 1
            ));
        }
        source.push_str("}\n");
    }
    source.push_str("}\n");
    check(&source, true);
}
