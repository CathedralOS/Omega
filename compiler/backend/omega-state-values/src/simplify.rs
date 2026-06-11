mod bindings;
mod call_targets;
mod folding;
mod helper_stack;

use self::bindings::{
    Binding, BindingScope, ScopedBindings, append_name_suffix, simple_local_bindings,
};
use self::call_targets::{resolve_call_target_machine, resolve_call_target_state};
use self::folding::{
    boolean_and, boolean_not, boolean_or, expressions_equivalent, fold_binary_expression,
};
use self::helper_stack::HelperStateStack;
use crate::StateValueRole;
use omega_checked_trees::CheckedTrees;
use omega_checked_trees::expression::{
    BinaryExpression, CallExpression, Expression, IndexedExpression, MemberExpression,
    StructLiteral, StructLiteralField, UnaryOperator,
};
use omega_checked_trees::machine::Machine;
use omega_checked_trees::state::State;
use omega_checked_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use omega_core::arena::Arena;
use std::sync::Arc;

pub fn simplify_expression(
    program: &CheckedTrees,
    machine: &Machine,
    expression: &Expression,
) -> Expression {
    let bindings: &[Binding] = &[];
    simplify_expression_with_bindings(program, machine, expression, bindings, false)
}

pub fn simplify_state_expression(
    program: &CheckedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: &Expression,
) -> Expression {
    simplify_state_expression_for_role(
        program,
        machine,
        state,
        statement_index,
        StateValueRole::AssignmentValue,
        expression,
    )
}

pub fn simplify_state_expression_for_role(
    program: &CheckedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    role: StateValueRole,
    expression: &Expression,
) -> Expression {
    let bindings = simple_local_bindings(program, state, statement_index);
    // Never fold a call-result local into its use site, for ANY role. The call's
    // result is materialized once into that local's own call-result slot at the `let`
    // statement; substituting the call expression into a later statement (e.g.
    // `let index = self.f(c); ... = max(.., index + 1)`) re-references a call that is
    // not collected there, so its result slot can't be resolved and the value/write
    // fails to lower (especially in a dispatched callee). Keeping the local a Name
    // lets it resolve to its populated slot. (Previously only Call/Transition
    // arguments preserved call locals; AssignmentValue folded them -- the source of
    // the dispatched carve_room `max(.., self.cell_index(cell) + 1)` failure.)
    let _ = role;
    let preserve_call_locals = true;
    simplify_expression_with_bindings(
        program,
        machine,
        expression,
        &bindings,
        preserve_call_locals,
    )
}

fn simplify_expression_with_bindings(
    program: &CheckedTrees,
    machine: &Machine,
    expression: &Expression,
    bindings: &(impl BindingScope + ?Sized),
    preserve_call_locals: bool,
) -> Expression {
    match expression {
        Expression::ArrayLiteral(values) => Expression::ArrayLiteral(Arc::from(
            values
                .iter()
                .map(|value| {
                    simplify_expression_with_bindings(
                        program,
                        machine,
                        value,
                        bindings,
                        preserve_call_locals,
                    )
                })
                .collect::<Arc<[_]>>(),
        )),
        Expression::Binary(binary) => {
            simplify_binary_expression(program, machine, binary, bindings, preserve_call_locals)
        }
        Expression::Boolean(_)
        | Expression::Float(_)
        | Expression::Integer(_)
        | Expression::String(_) => expression.clone(),
        Expression::Call(call) => {
            simplify_call_expression(program, machine, call, bindings, preserve_call_locals)
        }
        Expression::Cast(cast) => {
            Expression::Cast(Box::new(omega_checked_trees::expression::CastExpression {
                value: simplify_expression_with_bindings(
                    program,
                    machine,
                    &cast.value,
                    bindings,
                    preserve_call_locals,
                ),
                target_type: cast.target_type.clone(),
            }))
        }
        Expression::Indexed(indexed) => Expression::Indexed(Box::new(IndexedExpression {
            collection: simplify_expression_with_bindings(
                program,
                machine,
                &indexed.collection,
                bindings,
                preserve_call_locals,
            ),
            index: simplify_expression_with_bindings(
                program,
                machine,
                &indexed.index,
                bindings,
                preserve_call_locals,
            ),
        })),
        Expression::Range(range) => {
            Expression::Range(Box::new(omega_checked_trees::expression::RangeExpression {
                start: range.start.as_ref().map(|start| {
                    Box::new(simplify_expression_with_bindings(
                        program,
                        machine,
                        start,
                        bindings,
                        preserve_call_locals,
                    ))
                }),
                end: range.end.as_ref().map(|end| {
                    Box::new(simplify_expression_with_bindings(
                        program,
                        machine,
                        end,
                        bindings,
                        preserve_call_locals,
                    ))
                }),
                end_inclusive: range.end_inclusive,
            }))
        }
        Expression::Member(member) => Expression::Member(Box::new(MemberExpression {
            receiver: simplify_expression_with_bindings(
                program,
                machine,
                &member.receiver,
                bindings,
                preserve_call_locals,
            ),
            member_symbol: member.member_symbol,
            member: member.member.clone(),
        })),
        Expression::Mutable(inner) => {
            Expression::Mutable(Box::new(simplify_expression_with_bindings(
                program,
                machine,
                inner,
                bindings,
                preserve_call_locals,
            )))
        }
        Expression::Unary(unary) => {
            let operand = simplify_expression_with_bindings(
                program,
                machine,
                &unary.operand,
                bindings,
                preserve_call_locals,
            );
            match unary.operator {
                UnaryOperator::LogicalNot => boolean_not(operand),
            }
        }
        Expression::Name(path) => bindings
            .find_path_binding(path)
            .and_then(|binding| {
                if preserve_call_locals && matches!(binding.value, Expression::Call(_)) {
                    return None;
                }
                Some(append_name_suffix(&binding.value, &path[1..]))
            })
            .unwrap_or_else(|| expression.clone()),
        Expression::StructLiteral(struct_literal) => Expression::StructLiteral(StructLiteral {
            type_name: struct_literal.type_name.clone(),
            case_name: struct_literal.case_name.clone(),
            fields: Arc::from(
                struct_literal
                    .fields
                    .iter()
                    .map(|field| StructLiteralField {
                        name: field.name.clone(),
                        value: simplify_expression_with_bindings(
                            program,
                            machine,
                            &field.value,
                            bindings,
                            preserve_call_locals,
                        ),
                    })
                    .collect::<Arc<[_]>>(),
            ),
        }),
    }
}

fn simplify_binary_expression(
    program: &CheckedTrees,
    machine: &Machine,
    binary: &BinaryExpression,
    bindings: &(impl BindingScope + ?Sized),
    preserve_call_locals: bool,
) -> Expression {
    let left = simplify_expression_with_bindings(
        program,
        machine,
        &binary.left,
        bindings,
        preserve_call_locals,
    );
    let right = simplify_expression_with_bindings(
        program,
        machine,
        &binary.right,
        bindings,
        preserve_call_locals,
    );

    if let Some(expression) = simplify_guarded_helper_comparison(
        program,
        machine,
        binary.operator,
        &left,
        &right,
        bindings,
    ) {
        return simplify_expression_with_bindings(
            program,
            machine,
            &expression,
            bindings,
            preserve_call_locals,
        );
    }

    fold_binary_expression(binary.operator, left, right)
}

fn simplify_call_expression(
    program: &CheckedTrees,
    machine: &Machine,
    call: &CallExpression,
    bindings: &(impl BindingScope + ?Sized),
    preserve_call_locals: bool,
) -> Expression {
    let receiver = call.receiver.as_ref().map(|receiver| {
        simplify_expression_with_bindings(
            program,
            machine,
            receiver,
            bindings,
            preserve_call_locals,
        )
    });
    let simplified_arguments: Arc<[_]> = call
        .arguments
        .iter()
        .map(|argument| {
            simplify_expression_with_bindings(
                program,
                machine,
                argument,
                bindings,
                preserve_call_locals,
            )
        })
        .collect();

    if let Some(target_machine) =
        resolve_call_target_machine(program, machine, receiver.as_ref(), call.target_symbol)
        && let Some(state) = resolve_call_target_state(program, target_machine, call)
    {
        let parameters = program.state_parameters(state);
        let mut argument_bindings = Arena::with_capacity(parameters.len());
        for (parameter, argument) in parameters.iter().zip(simplified_arguments.iter()) {
            argument_bindings.insert(Binding {
                symbol: parameter.symbol,
                name: parameter.name.clone(),
                value: argument.clone(),
            });
        }
        if let Some(value) = helper_state_value(state, program, target_machine, &argument_bindings)
        {
            return simplify_expression_with_bindings(
                program,
                machine,
                &value,
                &argument_bindings,
                preserve_call_locals,
            );
        }
    }

    Expression::Call(Box::new(CallExpression {
        receiver: receiver.map(Box::new),
        target_symbol: call.target_symbol,
        target: call.target.clone(),
        arguments: simplified_arguments,
    }))
}

fn simplify_guarded_helper_comparison(
    program: &CheckedTrees,
    machine: &Machine,
    operator: omega_checked_trees::expression::BinaryOperator,
    left: &Expression,
    right: &Expression,
    bindings: &(impl BindingScope + ?Sized),
) -> Option<Expression> {
    use omega_checked_trees::expression::BinaryOperator::{Equal, NotEqual};

    if !matches!(operator, Equal | NotEqual) {
        return None;
    }

    if let Expression::Call(call) = left
        && let Some(condition) =
            simplify_helper_call_comparison(program, machine, call, right, bindings)
    {
        return Some(match operator {
            Equal => condition,
            NotEqual => boolean_not(condition),
            _ => unreachable!(),
        });
    }

    if let Expression::Call(call) = right
        && let Some(condition) =
            simplify_helper_call_comparison(program, machine, call, left, bindings)
    {
        return Some(match operator {
            Equal => condition,
            NotEqual => boolean_not(condition),
            _ => unreachable!(),
        });
    }

    None
}

fn simplify_helper_call_comparison(
    program: &CheckedTrees,
    machine: &Machine,
    call: &CallExpression,
    expected: &Expression,
    bindings: &(impl BindingScope + ?Sized),
) -> Option<Expression> {
    let receiver = call.receiver.as_ref().map(|receiver| {
        simplify_expression_with_bindings(program, machine, receiver, bindings, false)
    });
    let target_machine =
        resolve_call_target_machine(program, machine, receiver.as_ref(), call.target_symbol)?;
    let state = resolve_call_target_state(program, target_machine, call)?;
    let parameters = program.state_parameters(state);
    let mut argument_bindings = Arena::with_capacity(parameters.len());
    for (parameter, argument) in parameters.iter().zip(call.arguments.iter()) {
        argument_bindings.insert(Binding {
            symbol: parameter.symbol,
            name: parameter.name.clone(),
            value: simplify_expression_with_bindings(program, machine, argument, bindings, false),
        });
    }

    helper_state_match_condition(state, program, target_machine, &argument_bindings, expected)
}

fn helper_state_value(
    state: &State,
    program: &CheckedTrees,
    machine: &Machine,
    bindings: &(impl BindingScope + ?Sized),
) -> Option<Expression> {
    let helper = helper_state_model(state, program, machine, bindings)?;
    let mut transitions = helper.transitions.iter().map(|(_, transition)| transition);
    let transition = transitions.next()?;
    if transitions.next().is_some() {
        return None;
    }
    if !matches!(transition.guard, Expression::Boolean(true)) {
        return None;
    }
    Some(transition.value.clone())
}

fn helper_state_match_condition(
    state: &State,
    program: &CheckedTrees,
    machine: &Machine,
    bindings: &(impl BindingScope + ?Sized),
    expected: &Expression,
) -> Option<Expression> {
    helper_state_match_condition_with_stack(
        state,
        program,
        machine,
        bindings,
        expected,
        &mut HelperStateStack::with_capacity(program.machine_states.len()),
    )
}

fn helper_state_match_condition_with_stack(
    state: &State,
    program: &CheckedTrees,
    machine: &Machine,
    bindings: &(impl BindingScope + ?Sized),
    expected: &Expression,
    stack: &mut HelperStateStack,
) -> Option<Expression> {
    if state.symbol.is_valid() && stack.contains(state.symbol) {
        return None;
    }
    let pushed = state.symbol.is_valid();
    if pushed {
        stack.push(state.symbol);
    }

    let helper = match helper_state_model(state, program, machine, bindings) {
        Some(helper) => helper,
        None => {
            if pushed {
                stack.pop();
            }
            return None;
        }
    };
    let mut covered = Expression::Boolean(false);
    let mut matched = Expression::Boolean(false);

    for (_, transition) in helper.transitions.iter() {
        let effective_guard = boolean_and(boolean_not(covered.clone()), transition.guard.clone());
        if let Some(value_matches) = expression_match_condition_with_stack(
            program,
            machine,
            &transition.value,
            expected,
            stack,
        ) {
            matched = boolean_or(matched, boolean_and(effective_guard.clone(), value_matches));
        }
        covered = boolean_or(covered, effective_guard);
    }

    if pushed {
        stack.pop();
    }
    Some(matched)
}

fn expression_match_condition_with_stack(
    program: &CheckedTrees,
    machine: &Machine,
    expression: &Expression,
    expected: &Expression,
    stack: &mut HelperStateStack,
) -> Option<Expression> {
    if expressions_equivalent(expression, expected) {
        return Some(Expression::Boolean(true));
    }

    let Expression::Call(call) = expression else {
        return None;
    };

    let receiver = call.receiver.as_deref();
    let target_machine =
        resolve_call_target_machine(program, machine, receiver, call.target_symbol)?;
    let state = resolve_call_target_state(program, target_machine, call)?;
    let parameters = program.state_parameters(state);
    let mut argument_bindings = Arena::with_capacity(parameters.len());
    for (parameter, argument) in parameters.iter().zip(call.arguments.iter()) {
        argument_bindings.insert(Binding {
            symbol: parameter.symbol,
            name: parameter.name.clone(),
            value: argument.clone(),
        });
    }

    helper_state_match_condition_with_stack(
        state,
        program,
        target_machine,
        &argument_bindings,
        expected,
        stack,
    )
}

fn helper_state_model(
    state: &State,
    program: &CheckedTrees,
    machine: &Machine,
    bindings: &(impl BindingScope + ?Sized),
) -> Option<HelperStateModel> {
    let statements = program.statement_table.statements(state.statement_nodes);
    let local_binding_capacity = statements
        .iter()
        .filter(|statement| matches!(statement, StatementNode::LocalData(_)))
        .count();
    let transition_capacity = statements
        .iter()
        .filter(|statement| {
            matches!(
                statement,
                StatementNode::Transition(_) | StatementNode::Expression(_)
            )
        })
        .count();
    let mut local_bindings = Arena::with_capacity(local_binding_capacity);
    let mut transitions = Arena::with_capacity(transition_capacity);
    let mut saw_terminal_expression = false;

    for statement in statements {
        let scoped_bindings = ScopedBindings {
            parent: bindings,
            locals: &local_bindings,
        };

        match statement {
            StatementNode::LocalData(local) => {
                if saw_terminal_expression {
                    return None;
                }
                if !local.initial_value.is_valid() {
                    return None;
                }
                let initial_value = program.expression_table.to_tree(local.initial_value);
                let value = simplify_expression_with_bindings(
                    program,
                    machine,
                    &initial_value,
                    &scoped_bindings,
                    false,
                );
                local_bindings.insert(Binding {
                    symbol: local.symbol,
                    name: local.name.clone(),
                    value,
                });
            }
            StatementNode::Transition(transition) => {
                if saw_terminal_expression {
                    return None;
                }
                if transition.continuation.is_valid() {
                    return None;
                }
                let guard = match transition.guard {
                    TransitionGuardNode::Always => Expression::Boolean(true),
                    TransitionGuardNode::When(expression) => {
                        let expression = program.expression_table.to_tree(expression);
                        simplify_expression_with_bindings(
                            program,
                            machine,
                            &expression,
                            &scoped_bindings,
                            false,
                        )
                    }
                };
                let TransitionTargetNode::Value(value) =
                    program.statement_table.transition_target(transition.target)
                else {
                    return None;
                };
                let value = program.expression_table.to_tree(*value);
                let value = simplify_expression_with_bindings(
                    program,
                    machine,
                    &value,
                    &scoped_bindings,
                    false,
                );
                transitions.insert(HelperTransition { guard, value });
            }
            StatementNode::Expression(expression) => {
                let expression = program.expression_table.to_tree(*expression);
                let value = simplify_expression_with_bindings(
                    program,
                    machine,
                    &expression,
                    &scoped_bindings,
                    false,
                );
                transitions.insert(HelperTransition {
                    guard: Expression::Boolean(true),
                    value,
                });
                saw_terminal_expression = true;
            }
            StatementNode::Assignment(_) | StatementNode::Call(_) => {
                return None;
            }
        }
    }

    if transitions.is_empty() {
        return None;
    }

    Some(HelperStateModel { transitions })
}

#[derive(Debug, Clone)]
struct HelperStateModel {
    transitions: Arena<HelperTransition>,
}

#[derive(Debug, Clone)]
struct HelperTransition {
    guard: Expression,
    value: Expression,
}

impl Default for HelperTransition {
    fn default() -> Self {
        Self {
            guard: Expression::Boolean(true),
            value: Expression::Integer(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::simplify_expression;
    use omega_checked_trees::CheckedTrees;
    use omega_checked_trees::expression::{
        BinaryExpression, BinaryOperator, CallExpression, Expression, NamePath,
    };
    use omega_checked_trees::machine::Machine;
    use omega_checked_trees::name::Identifier;
    use omega_checked_trees::signature::StateParameter;
    use omega_checked_trees::state::State;
    use omega_checked_trees::statement::{
        StatementNode, TableLocalData, TableTransition, TransitionGuardNode, TransitionTargetNode,
    };
    use omega_checked_trees::types::{TypeReferenceHandle, TypeReferenceNode};
    use omega_core::symbols::SymbolHandle;
    use std::sync::Arc;

    #[test]
    fn simplifies_guarded_helper_call_comparison_to_guard_expression() {
        let machine_symbol = SymbolHandle::from_arena_index(1);
        let helper_symbol = SymbolHandle::from_arena_index(2);
        let roll_symbol = SymbolHandle::from_arena_index(3);
        let is_quiet_symbol = SymbolHandle::from_arena_index(4);
        let is_fountain_symbol = SymbolHandle::from_arena_index(5);
        let is_reward_symbol = SymbolHandle::from_arena_index(6);
        let event_action_symbol = SymbolHandle::from_arena_index(7);
        let quiet_symbol = SymbolHandle::from_arena_index(8);
        let fountain_symbol = SymbolHandle::from_arena_index(9);
        let reward_symbol = SymbolHandle::from_arena_index(10);
        let enemy_symbol = SymbolHandle::from_arena_index(11);

        let mut helper = State {
            symbol: helper_symbol,
            name: "event_action".into(),
            parameters: Default::default(),
            return_type: TypeReferenceHandle::invalid(),
            statement_nodes: Default::default(),
        };

        let mut machine = Machine {
            symbol: machine_symbol,
            name: "RoomEvents".into(),
            attached_data: None,
            contains: Default::default(),
            contracts: Default::default(),
            effects: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            states: Default::default(),
            ..Machine::default()
        };
        let mut program = CheckedTrees::default();
        push_state_statements(
            &mut program,
            &mut helper,
            [
                TestStatement::LocalData {
                    symbol: is_quiet_symbol,
                    name: "is_quiet".into(),
                    type_reference: TypeReferenceNode::Named {
                        symbol: SymbolHandle::invalid(),
                        name: "bool".into(),
                    },
                    initial_value: Some(Expression::Binary(Box::new(BinaryExpression {
                        left: name("roll", roll_symbol),
                        operator: BinaryOperator::Less,
                        right: Expression::Integer(20),
                    }))),
                },
                TestStatement::LocalData {
                    symbol: is_fountain_symbol,
                    name: "is_fountain".into(),
                    type_reference: TypeReferenceNode::Named {
                        symbol: SymbolHandle::invalid(),
                        name: "bool".into(),
                    },
                    initial_value: Some(Expression::Binary(Box::new(BinaryExpression {
                        left: name("roll", roll_symbol),
                        operator: BinaryOperator::Less,
                        right: Expression::Integer(30),
                    }))),
                },
                TestStatement::LocalData {
                    symbol: is_reward_symbol,
                    name: "is_reward".into(),
                    type_reference: TypeReferenceNode::Named {
                        symbol: SymbolHandle::invalid(),
                        name: "bool".into(),
                    },
                    initial_value: Some(Expression::Binary(Box::new(BinaryExpression {
                        left: name("roll", roll_symbol),
                        operator: BinaryOperator::Less,
                        right: Expression::Integer(60),
                    }))),
                },
                TestStatement::TransitionValue {
                    target: resolved_path_expression(
                        &["RoomEventAction", "Quiet"],
                        event_action_symbol,
                        quiet_symbol,
                    ),
                    guard: Some(name("is_quiet", is_quiet_symbol)),
                },
                TestStatement::TransitionValue {
                    target: resolved_path_expression(
                        &["RoomEventAction", "Fountain"],
                        event_action_symbol,
                        fountain_symbol,
                    ),
                    guard: Some(name("is_fountain", is_fountain_symbol)),
                },
                TestStatement::TransitionValue {
                    target: resolved_path_expression(
                        &["RoomEventAction", "Reward"],
                        event_action_symbol,
                        reward_symbol,
                    ),
                    guard: Some(name("is_reward", is_reward_symbol)),
                },
                TestStatement::TransitionValue {
                    target: resolved_path_expression(
                        &["RoomEventAction", "Enemy"],
                        event_action_symbol,
                        enemy_symbol,
                    ),
                    guard: None,
                },
            ],
        );
        let roll_type = named_type_reference(&mut program, "u32");
        program.typed.push_state_parameter(
            &mut helper,
            StateParameter {
                symbol: roll_symbol,
                name: "roll".into(),
                type_reference: roll_type,
                is_const: true,
                is_mutable: false,
                is_self: false,
            },
        );
        program.typed.push_machine_state(&mut machine, helper);
        program.typed.push_machine(machine.clone());

        let quiet_guard = Expression::Binary(Box::new(BinaryExpression {
            left: Expression::Call(Box::new(CallExpression {
                receiver: None,
                target_symbol: helper_symbol,
                target: "event_action".into(),
                arguments: Arc::from(vec![name("roll", roll_symbol)].into_boxed_slice()),
            })),
            operator: BinaryOperator::Equal,
            right: resolved_path_expression(
                &["RoomEventAction", "Quiet"],
                event_action_symbol,
                quiet_symbol,
            ),
        }));

        let fountain_guard = Expression::Binary(Box::new(BinaryExpression {
            left: Expression::Call(Box::new(CallExpression {
                receiver: None,
                target_symbol: helper_symbol,
                target: "event_action".into(),
                arguments: Arc::from(vec![name("roll", roll_symbol)].into_boxed_slice()),
            })),
            operator: BinaryOperator::Equal,
            right: resolved_path_expression(
                &["RoomEventAction", "Fountain"],
                event_action_symbol,
                fountain_symbol,
            ),
        }));

        assert_eq!(
            simplify_expression(&program, &machine, &quiet_guard),
            Expression::Binary(Box::new(BinaryExpression {
                left: name("roll", roll_symbol),
                operator: BinaryOperator::Less,
                right: Expression::Integer(20),
            }))
        );

        let reward_guard = Expression::Binary(Box::new(BinaryExpression {
            left: Expression::Call(Box::new(CallExpression {
                receiver: None,
                target_symbol: helper_symbol,
                target: "event_action".into(),
                arguments: Arc::from(vec![name("roll", roll_symbol)].into_boxed_slice()),
            })),
            operator: BinaryOperator::Equal,
            right: resolved_path_expression(
                &["RoomEventAction", "Reward"],
                event_action_symbol,
                reward_symbol,
            ),
        }));

        let enemy_guard = Expression::Binary(Box::new(BinaryExpression {
            left: Expression::Call(Box::new(CallExpression {
                receiver: None,
                target_symbol: helper_symbol,
                target: "event_action".into(),
                arguments: Arc::from(vec![name("roll", roll_symbol)].into_boxed_slice()),
            })),
            operator: BinaryOperator::Equal,
            right: resolved_path_expression(
                &["RoomEventAction", "Enemy"],
                event_action_symbol,
                enemy_symbol,
            ),
        }));

        assert_eq!(
            simplify_expression(&program, &machine, &fountain_guard),
            Expression::Binary(Box::new(BinaryExpression {
                left: Expression::Binary(Box::new(BinaryExpression {
                    left: name("roll", roll_symbol),
                    operator: BinaryOperator::GreaterOrEqual,
                    right: Expression::Integer(20),
                })),
                operator: BinaryOperator::And,
                right: Expression::Binary(Box::new(BinaryExpression {
                    left: name("roll", roll_symbol),
                    operator: BinaryOperator::Less,
                    right: Expression::Integer(30),
                })),
            }))
        );

        assert_eq!(
            simplify_expression(&program, &machine, &reward_guard),
            Expression::Binary(Box::new(BinaryExpression {
                left: Expression::Binary(Box::new(BinaryExpression {
                    left: name("roll", roll_symbol),
                    operator: BinaryOperator::GreaterOrEqual,
                    right: Expression::Integer(30),
                })),
                operator: BinaryOperator::And,
                right: Expression::Binary(Box::new(BinaryExpression {
                    left: name("roll", roll_symbol),
                    operator: BinaryOperator::Less,
                    right: Expression::Integer(60),
                })),
            }))
        );

        assert_eq!(
            simplify_expression(&program, &machine, &enemy_guard),
            Expression::Binary(Box::new(BinaryExpression {
                left: name("roll", roll_symbol),
                operator: BinaryOperator::GreaterOrEqual,
                right: Expression::Integer(60),
            }))
        );
    }

    #[test]
    fn simplifies_impossible_integer_range_conjunction_to_false() {
        let machine = Machine {
            symbol: SymbolHandle::invalid(),
            name: "main".into(),
            attached_data: None,
            contains: Default::default(),
            contracts: Default::default(),
            effects: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            states: Default::default(),
            ..Machine::default()
        };
        let program = CheckedTrees::default();
        let roll_symbol = SymbolHandle::from_arena_index(99);

        let expression = Expression::Binary(Box::new(BinaryExpression {
            left: Expression::Binary(Box::new(BinaryExpression {
                left: name("roll", roll_symbol),
                operator: BinaryOperator::GreaterOrEqual,
                right: Expression::Integer(20),
            })),
            operator: BinaryOperator::And,
            right: Expression::Binary(Box::new(BinaryExpression {
                left: name("roll", roll_symbol),
                operator: BinaryOperator::Less,
                right: Expression::Integer(20),
            })),
        }));

        assert_eq!(
            simplify_expression(&program, &machine, &expression),
            Expression::Boolean(false)
        );
    }

    #[test]
    fn simplifies_negated_redundant_integer_range_conjunction_to_single_comparison() {
        let machine = Machine {
            symbol: SymbolHandle::invalid(),
            name: "main".into(),
            attached_data: None,
            contains: Default::default(),
            contracts: Default::default(),
            effects: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            states: Default::default(),
            ..Machine::default()
        };
        let program = CheckedTrees::default();
        let health_symbol = SymbolHandle::from_arena_index(100);

        let expression = Expression::Binary(Box::new(BinaryExpression {
            left: Expression::Binary(Box::new(BinaryExpression {
                left: Expression::Binary(Box::new(BinaryExpression {
                    left: name("health", health_symbol),
                    operator: BinaryOperator::GreaterOrEqual,
                    right: Expression::Integer(0),
                })),
                operator: BinaryOperator::And,
                right: Expression::Binary(Box::new(BinaryExpression {
                    left: name("health", health_symbol),
                    operator: BinaryOperator::Greater,
                    right: Expression::Integer(0),
                })),
            })),
            operator: BinaryOperator::Equal,
            right: Expression::Boolean(false),
        }));

        assert_eq!(
            simplify_expression(&program, &machine, &expression),
            Expression::Binary(Box::new(BinaryExpression {
                left: name("health", health_symbol),
                operator: BinaryOperator::LessOrEqual,
                right: Expression::Integer(0),
            }))
        );
    }

    enum TestStatement {
        LocalData {
            symbol: SymbolHandle,
            name: Identifier,
            type_reference: TypeReferenceNode,
            initial_value: Option<Expression>,
        },
        TransitionValue {
            target: Expression,
            guard: Option<Expression>,
        },
    }

    fn push_state_statements<const N: usize>(
        program: &mut CheckedTrees,
        state: &mut State,
        statements: [TestStatement; N],
    ) {
        for statement in statements {
            let statement = match statement {
                TestStatement::LocalData {
                    symbol,
                    name,
                    type_reference,
                    initial_value,
                } => {
                    let type_reference = program.typed.type_reference_table.insert(type_reference);
                    let initial_value = initial_value
                        .as_ref()
                        .map(|value| program.typed.expression_table.insert_tree(value))
                        .unwrap_or_else(omega_checked_trees::expression::ExpressionHandle::invalid);

                    StatementNode::LocalData(TableLocalData {
                        symbol,
                        name,
                        type_reference,
                        initial_value,
                    })
                }
                TestStatement::TransitionValue { target, guard } => {
                    let target = program.typed.expression_table.insert_tree(&target);
                    let target = program
                        .typed
                        .statement_table
                        .insert_transition_target(TransitionTargetNode::Value(target));
                    let guard = guard
                        .as_ref()
                        .map(|guard| {
                            TransitionGuardNode::When(
                                program.typed.expression_table.insert_tree(guard),
                            )
                        })
                        .unwrap_or(TransitionGuardNode::Always);

                    StatementNode::Transition(TableTransition {
                        target,
                        continuation:
                            omega_checked_trees::statement::TransitionTargetHandle::invalid(),
                        guard,
                    })
                }
            };
            program
                .typed
                .statement_table
                .push_statement(&mut state.statement_nodes, statement);
        }
    }

    fn name(name: &str, symbol: SymbolHandle) -> Expression {
        Expression::Name(NamePath::resolved(vec![name.into()], symbol, symbol))
    }

    fn named_type_reference(program: &mut CheckedTrees, name: &str) -> TypeReferenceHandle {
        program
            .typed
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: name.into(),
            })
    }

    fn resolved_path_expression(
        segments: &[&str],
        head_symbol: SymbolHandle,
        symbol: SymbolHandle,
    ) -> Expression {
        Expression::Name(NamePath::resolved(
            segments
                .iter()
                .map(|segment| Identifier::from(*segment))
                .collect(),
            head_symbol,
            symbol,
        ))
    }
}
