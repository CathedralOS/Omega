use crate::tests::front_end::{checked_program, typed_program};

#[test]
fn absent_and_stale_expression_handles_are_not_literal_evidence() {
    let mut program = typed_trees::TypedTrees::default();
    let zero = program
        .expression_table
        .insert(typed_trees::expression::ExpressionNode::default());
    let stale = typed_trees::expression::ExpressionHandle::from_parts(
        zero.arena_index(),
        zero.generation() + 1,
    );
    let missing = typed_trees::expression::ExpressionHandle::from_arena_index(u32::MAX);
    for unknown in [Default::default(), stale, missing] {
        assert!(super::literal(&program, unknown).is_none());
    }
    assert!(super::literal(&program, zero).is_some());
}

fn check(source: &str, accepted: bool) {
    let typed = typed_program(source);
    match super::super::builder::tests::check_against_whole_pass(
        typed,
        &crate::CheckingRequest::settled(),
    ) {
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
    use checked_trees::CheckedScalarExpressionRole;
    use typed_trees::statement::{StatementNode, TransitionTargetNode};
    let source = "machine produce() -> u8 { transition { _ -> finish(3u8 + 4u8) } state finish(value: u8) -> u8 { value } }";
    let checked = checked_program(source);
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
        let operators = Default::default();
        let state_mutation_summary_cache = crate::flow::StateMutationSummaryCache::default();
        let mut context = super::FlowBuildContext::new(
            &Default::default(),
            &Default::default(),
            &semantic,
            &plans,
            &operators,
            &[],
            None,
            &state_mutation_summary_cache,
        );
        let value = super::capture_argument(
            program,
            &semantic,
            &mut context,
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
                facts::ScalarValue::Integer(numerics::bignum::BigInt::from_u64(7))
            } else {
                facts::ScalarValue::Unknown
            },
            "mutation {mutation}"
        );
    }
}

#[test]
fn sibling_continuation_values_have_independent_capture() {
    use typed_trees::statement::StatementNode;
    for (other, accepted) in [(4, true), (5, false)] {
        let source = format!(
            "machine produce(flag: bool) -> u8 ensures result == 7 {{ transition flag {{ true -> first(3u8 + 4u8) false -> second(3u8 + {other}u8) }} state first(value: u8) -> u8 {{ value }} state second(value: u8) -> u8 {{ value }} }}"
        );
        let mut typed = typed_program(&source);
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
            arena::HandleSpan::from_parts(nodes.start(), 1);
        let result = crate::lower_typed_trees(typed, &crate::CheckingRequest::settled());
        assert_eq!(result.is_ok(), accepted, "{source}: {result:?}");
    }
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
