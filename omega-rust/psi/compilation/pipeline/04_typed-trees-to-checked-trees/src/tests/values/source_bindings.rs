use super::StatementNode;
use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::typed_program;
use checked_trees::CheckedScalarExpressionRole;
use typed_trees::expression::ExpressionNode;

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
        let checked =
            lower_typed_trees(typed_program(&source), &CheckingRequest::settled()).unwrap();
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
        let (owner, _) = checked
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
            assert!(
                checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .for_machine(machine.symbol)
                    .is_none(),
                "authored nominal argument custody does not select an executable boundary"
            );
        }
    }
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
        let checked =
            lower_typed_trees(typed_program(&source), &CheckingRequest::settled()).unwrap();
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
