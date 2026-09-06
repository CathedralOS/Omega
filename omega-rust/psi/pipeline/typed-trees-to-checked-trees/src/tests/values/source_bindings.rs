use super::*;
use checked_trees::CheckedScalarExpressionRole;
use typed_trees::{expression::ExpressionNode, statement::TransitionGuardNode};

#[test]
fn callable_boundary_arguments_keep_the_nominal_requirement_role() {
    for (result, body) in [
        ("", "Selected(left, right);"),
        ("-> i32", "Selected(left, right)"),
        ("-> i32", "let result: i32 = Selected(left, right); result"),
    ] {
        let source = format!(
            "boundary trait Host {{
                machine invoke(first: i32, second: i32) {result} reaches Host;
             }}
             data Root {{}}
             machine Root::enter<machine Selected>(left: i32, right: i32) {result}
             where machine Selected satisfies Host::invoke;
             reaches Host
             {{ {body} }}"
        );
        let checked = lower_typed_trees(typed_trees(&source)).unwrap();
        let machine = checked
            .machines()
            .iter()
            .find(|machine| {
                machine
                    .attached_data
                    .as_ref()
                    .is_some_and(|name| name.as_str() == "Root")
            })
            .unwrap();
        let state = &checked.machine_states(machine)[0];
        let statement = &checked.statement_table.statements(state.statement_nodes)[0];
        let (target, arguments) = match statement {
            StatementNode::Call(call) => (
                call.target_symbol,
                checked.statement_table.expression_handles(call.arguments),
            ),
            StatementNode::LocalData(_) | StatementNode::Expression(_) => {
                let expression = match statement {
                    StatementNode::LocalData(local) => local.initial_value,
                    StatementNode::Expression(expression) => *expression,
                    _ => unreachable!(),
                };
                let ExpressionNode::Call(call) = checked.expression_table.expression(expression)
                else {
                    panic!("authored callable-parameter expression");
                };
                (
                    call.target_symbol,
                    checked.expression_table.expression_handles(call.arguments),
                )
            }
            _ => panic!("authored callable-parameter root"),
        };
        let (owner, requirement) = checked
            .machine_parameter_signature(target)
            .expect("target retains its nominal callable parameter");
        assert_eq!(owner.symbol, machine.symbol);
        let plans = &checked.facts.values.scalar_expressions;
        for (ordinal, argument) in arguments.iter().enumerate() {
            let argument_ordinal = u32::try_from(ordinal).unwrap();
            let (binding, _) = plans
                .bound_expression_at(
                    state.symbol,
                    0,
                    CheckedScalarExpressionRole::BoundaryCallArgument {
                        call_ordinal: 0,
                        argument_ordinal,
                    },
                )
                .expect("nominal boundary arguments retain exact source custody");
            assert_eq!(binding.expression, *argument);
            assert!(!binding.destination.is_valid());
            assert_eq!(
                plans.binding_symbols.span_or_empty(binding.symbols),
                &checked
                    .state_parameters(state)
                    .iter()
                    .map(|parameter| parameter.symbol)
                    .collect::<Vec<_>>()
            );
            assert!(
                plans
                    .bound_expression_at(
                        state.symbol,
                        0,
                        CheckedScalarExpressionRole::UnitCallArgument {
                            call_ordinal: 0,
                            argument_ordinal,
                        },
                    )
                    .is_none()
            );
        }
        if result.is_empty() {
            let plan = checked
                .facts
                .flow
                .terminal_unit_effects
                .for_machine(machine.symbol)
                .expect("scalar-bearing callable boundary statement retains its Unit plan");
            assert!(matches!(
                plan.operations.first(),
                Some(checked_trees::CheckedUnitEffectOperationPlan::BoundaryCall {
                    target_machine, target_state, scalar_arguments, ..
                }) if *target_machine == requirement.symbol
                    && *target_state == requirement.symbol
                    && scalar_arguments.len() == 2
            ));
        }
    }
}

#[test]
fn ordinary_callable_parameter_arguments_keep_the_unit_role() {
    let source = r#"
        data Root {}
        machine Root::enter<machine Selected>(value: i32)
        where machine Selected(input: i32);
        { Selected(value); }
    "#;
    let checked = lower_typed_trees(typed_trees(source)).unwrap();
    let machine = &checked.machines()[0];
    let state = &checked.machine_states(machine)[0];
    let plans = &checked.facts.values.scalar_expressions;
    assert!(
        plans
            .bound_expression_at(
                state.symbol,
                0,
                CheckedScalarExpressionRole::UnitCallArgument {
                    call_ordinal: 0,
                    argument_ordinal: 0,
                },
            )
            .is_some()
    );
    assert!(
        plans
            .bound_expression_at(
                state.symbol,
                0,
                CheckedScalarExpressionRole::BoundaryCallArgument {
                    call_ordinal: 0,
                    argument_ordinal: 0,
                },
            )
            .is_none()
    );
}

#[test]
fn structural_boundary_initializers_retain_scalar_inputs_without_scalar_result_slots() {
    let source = r#"
        pub data Packet { value: i32; }
        boundary trait Host {
            machine read(first: i32, second: i32) -> Packet reaches Host;
        }
        data Root {}
        machine Root::enter(left: i32, right: i32) reaches Host {
            let before: i32 = left;
            let packet: Packet = Host::read(before, right);
            let after: i32 = before;
        }
    "#;
    let checked = lower_typed_trees(typed_trees(source)).unwrap();
    let machine = checked
        .machines()
        .iter()
        .find(|machine| {
            machine
                .attached_data
                .as_ref()
                .is_some_and(|name| name.as_str() == "Root")
        })
        .unwrap();
    let state = &checked.machine_states(machine)[0];
    let parameters = checked.state_parameters(state);
    let statements = checked.statement_table.statements(state.statement_nodes);
    let StatementNode::LocalData(before) = &statements[0] else {
        panic!("prior scalar local");
    };
    let StatementNode::LocalData(packet) = &statements[1] else {
        panic!("structural call result");
    };
    let ExpressionNode::Call(call) = checked.expression_table.expression(packet.initial_value)
    else {
        panic!("authored boundary call");
    };
    let plans = &checked.facts.values.scalar_expressions;
    for (ordinal, argument) in checked
        .expression_table
        .expression_handles(call.arguments)
        .iter()
        .enumerate()
    {
        let (binding, _) = plans
            .bound_expression_at(
                state.symbol,
                1,
                CheckedScalarExpressionRole::BoundaryCallArgument {
                    call_ordinal: 0,
                    argument_ordinal: u32::try_from(ordinal).unwrap(),
                },
            )
            .expect("one scalar input to structural boundary result");
        assert_eq!(binding.expression, *argument);
        assert!(!binding.destination.is_valid());
        assert_eq!(
            plans.binding_symbols.span_or_empty(binding.symbols),
            &[parameters[0].symbol, parameters[1].symbol, before.symbol]
        );
    }
    let (following, _) = plans
        .bound_expression_at(
            state.symbol,
            2,
            CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 1 },
        )
        .expect("following scalar binding does not count the structural result");
    assert_eq!(
        plans.binding_symbols.span_or_empty(following.symbols),
        &[parameters[0].symbol, parameters[1].symbol, before.symbol]
    );
    assert!(
        !plans
            .source_bindings
            .iter()
            .any(|(_, binding)| binding.destination == packet.symbol)
    );
}

#[test]
fn explicit_compiler_intrinsic_arguments_retain_boundary_custody() {
    let source = r#"
        pub boundary trait Console {
            machine exit_process(return_code: i32) reaches Console;
        }
        pub data ConsoleNativeProvider {}
        machine ConsoleNativeProvider::exit_process(return_code: i32)
            satisfies Console::exit_process
            via Binding::CompilerIntrinsic;
        data Root {}
        machine Root::enter(code: i32) reaches Console {
            ConsoleNativeProvider::exit_process(code);
        }
    "#;
    let checked = lower_typed_trees(typed_trees(source)).unwrap();
    let machine = checked
        .machines()
        .iter()
        .find(|machine| {
            machine
                .attached_data
                .as_ref()
                .is_some_and(|name| name.as_str() == "Root")
        })
        .unwrap();
    let state = &checked.machine_states(machine)[0];
    let StatementNode::Call(call) = &checked.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("authored intrinsic call");
    };
    let (requirement, _) =
        validation::exact_compiler_intrinsic_boundary_requirement(&checked, call.target_symbol)
            .expect("exact explicit intrinsic realization");
    let plans = &checked.facts.values.scalar_expressions;
    let (binding, _) = plans
        .bound_expression_at(
            state.symbol,
            0,
            CheckedScalarExpressionRole::BoundaryCallArgument {
                call_ordinal: 0,
                argument_ordinal: 0,
            },
        )
        .expect("intrinsic arguments have boundary custody");
    assert_eq!(
        binding.expression,
        checked.statement_table.expression_handles(call.arguments)[0]
    );
    assert_eq!(
        plans.binding_symbols.span_or_empty(binding.symbols),
        &[checked.state_parameters(state)[0].symbol]
    );
    assert!(!binding.destination.is_valid());
    assert!(
        plans
            .bound_expression_at(
                state.symbol,
                0,
                CheckedScalarExpressionRole::UnitCallArgument {
                    call_ordinal: 0,
                    argument_ordinal: 0
                },
            )
            .is_none()
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine.symbol)
        .expect("exact intrinsic caller plan");
    assert!(
        matches!(plan.operations.first(), Some(checked_trees::CheckedUnitEffectOperationPlan::BoundaryCall { target_machine, scalar_arguments, .. }) if *target_machine == requirement && scalar_arguments.len() == 1)
    );
}

#[test]
fn boundary_and_unit_call_arguments_keep_dense_callee_and_caller_namespaces() {
    for boundary in [false, true] {
        let declaration = if boundary {
            "boundary machine Sink::consume(first: Packet, flag: bool, second: Packet, number: u8) ensures true;"
        } else {
            "machine Sink::consume(first: Packet, flag: bool, second: Packet, number: u8) {}"
        };
        let source = format!(
            "pub data Packet {{}} data Root {{}} pub data Sink {{}}
             {declaration}
             machine Root::send(first: Packet, flag: bool, second: Packet, number: u8) {{
                 let mut current: bool = flag;
                 let saved: bool = current;
                 current = !flag;
                 Sink::consume(first, saved, second, number);
             }}"
        );
        let checked = lower_typed_trees(typed_trees(&source)).unwrap();
        let machine = checked
            .machines()
            .iter()
            .find(|machine| {
                machine
                    .attached_data
                    .as_ref()
                    .is_some_and(|name| name.as_str() == "Root")
            })
            .unwrap();
        let state = &checked.machine_states(machine)[0];
        let parameters = checked.state_parameters(state);
        let statements = checked.statement_table.statements(state.statement_nodes);
        let StatementNode::LocalData(saved) = &statements[1] else {
            panic!("saved immutable local");
        };
        let StatementNode::Call(call) = &statements[3] else {
            panic!("authored mixed call");
        };
        let arguments = checked.statement_table.expression_handles(call.arguments);
        let plans = &checked.facts.values.scalar_expressions;
        for (dense_ordinal, formal_ordinal) in [(0, 1), (1, 3)] {
            let role = if boundary {
                CheckedScalarExpressionRole::BoundaryCallArgument {
                    call_ordinal: 0,
                    argument_ordinal: dense_ordinal,
                }
            } else {
                CheckedScalarExpressionRole::UnitCallArgument {
                    call_ordinal: 0,
                    argument_ordinal: dense_ordinal,
                }
            };
            let (binding, _) = plans
                .bound_expression_at(state.symbol, 3, role)
                .expect("one exact scalar argument");
            assert_eq!(binding.expression, arguments[formal_ordinal]);
            assert!(!binding.destination.is_valid());
            assert_eq!(
                plans.binding_symbols.span_or_empty(binding.symbols),
                &[parameters[1].symbol, parameters[3].symbol, saved.symbol]
            );
        }
    }
}

#[test]
fn boundary_expression_and_initializer_arguments_keep_authored_handles() {
    for body in [
        "Sink::read(flag)",
        "let result: bool = Sink::read(flag); result",
    ] {
        let source = format!(
            "data Root {{}} pub data Sink {{}}
             boundary machine Sink::read(flag: bool) -> bool;
             machine Root::read(flag: bool) -> bool {{ {body} }}"
        );
        let checked = lower_typed_trees(typed_trees(&source)).unwrap();
        let machine = checked
            .machines()
            .iter()
            .find(|machine| {
                machine
                    .attached_data
                    .as_ref()
                    .is_some_and(|name| name.as_str() == "Root")
            })
            .unwrap();
        let state = &checked.machine_states(machine)[0];
        let statement = &checked.statement_table.statements(state.statement_nodes)[0];
        let expression = match statement {
            StatementNode::LocalData(local) => local.initial_value,
            StatementNode::Expression(expression) => *expression,
            _ => panic!("authored boundary invocation"),
        };
        let ExpressionNode::Call(call) = checked.expression_table.expression(expression) else {
            panic!("boundary call expression");
        };
        let (binding, _) = checked
            .facts
            .values
            .scalar_expressions
            .bound_expression_at(
                state.symbol,
                0,
                CheckedScalarExpressionRole::BoundaryCallArgument {
                    call_ordinal: 0,
                    argument_ordinal: 0,
                },
            )
            .expect("one boundary argument row");
        assert_eq!(
            binding.expression,
            checked.expression_table.expression_handles(call.arguments)[0]
        );
    }
}

#[test]
fn pure_guard_and_direct_call_arguments_keep_exact_source_custody() {
    let source = r#"
        machine identity(first: bool, second: bool) -> bool { first || second }
        machine value(flag: bool, other: bool) -> bool {
            let mut current: bool = flag;
            let saved: bool = current;
            current = other;
            let called: bool = identity(saved, !current);
            transition called && saved { true -> true false -> false }
        }
    "#;
    let checked = lower_typed_trees(typed_trees(source)).unwrap();
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let state = &checked.machine_states(machine)[0];
    let parameters = checked.state_parameters(state);
    let statements = checked.statement_table.statements(state.statement_nodes);
    let StatementNode::LocalData(saved) = &statements[1] else {
        panic!("saved immutable local");
    };
    let StatementNode::LocalData(called) = &statements[3] else {
        panic!("direct call binding");
    };
    let ExpressionNode::Call(call) = checked.expression_table.expression(called.initial_value)
    else {
        panic!("authored direct call");
    };
    let plans = &checked.facts.values.scalar_expressions;
    for (ordinal, argument) in checked
        .expression_table
        .expression_handles(call.arguments)
        .iter()
        .enumerate()
    {
        let (binding, _) = plans
            .bound_expression_at(
                state.symbol,
                3,
                CheckedScalarExpressionRole::CallArgument {
                    binding_ordinal: 1,
                    argument_ordinal: u32::try_from(ordinal).unwrap(),
                },
            )
            .expect("one exact direct argument binding");
        assert_eq!(binding.expression, *argument);
        assert!(!binding.destination.is_valid());
        assert_eq!(
            plans.binding_symbols.span_or_empty(binding.symbols),
            &[parameters[0].symbol, parameters[1].symbol, saved.symbol]
        );
    }
    let StatementNode::Transition(transition) = &statements[4] else {
        panic!("authored guarded return");
    };
    let TransitionGuardNode::When(guard) = transition.guard else {
        panic!("authored guard");
    };
    let (binding, _) = plans
        .bound_expression_at(state.symbol, 4, CheckedScalarExpressionRole::Guard)
        .expect("one exact guard binding");
    assert_eq!(binding.expression, guard);
    assert!(!binding.destination.is_valid());
    assert_eq!(
        plans.binding_symbols.span_or_empty(binding.symbols),
        &[
            parameters[0].symbol,
            parameters[1].symbol,
            saved.symbol,
            called.symbol
        ]
    );
    assert!(
        plans
            .bound_expression_at(
                state.symbol,
                3,
                CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 1 }
            )
            .is_none(),
        "direct call arguments do not manufacture a pure initializer plan"
    );
}
