use omega_core::symbols::SymbolHandle;
use omega_typed_trees::name::ProgramName;
use omega_typed_trees::Program;
use omega_typed_trees::expression::{BinaryExpression, CallExpression, Expression, IndexedExpression, MemberExpression, StructLiteral, StructLiteralField};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::state::State;
use omega_typed_trees::statement::{Statement, TransitionGuard, TransitionTarget};

pub fn simplify_expression(
    program: &Program,
    machine: &Machine,
    expression: &Expression,
) -> Expression {
    simplify_expression_with_bindings(program, machine, expression, &[])
}

#[derive(Debug, Clone)]
struct Binding {
    symbol: SymbolHandle,
    name: Option<ProgramName>,
    value: Expression,
}

fn simplify_expression_with_bindings(
    program: &Program,
    machine: &Machine,
    expression: &Expression,
    bindings: &[Binding],
) -> Expression {
    match expression {
        Expression::ArrayLiteral(values) => Expression::ArrayLiteral(
            values
                .iter()
                .map(|value| simplify_expression_with_bindings(program, machine, value, bindings))
                .collect(),
        ),
        Expression::Binary(binary) => simplify_binary_expression(
            program,
            machine,
            binary,
            bindings,
        ),
        Expression::Boolean(_)
        | Expression::Float(_)
        | Expression::Integer(_)
        | Expression::String(_) => expression.clone(),
        Expression::Call(call) => simplify_call_expression(program, machine, call, bindings),
        Expression::Cast(cast) => Expression::Cast(Box::new(
            omega_typed_trees::expression::CastExpression {
                value: simplify_expression_with_bindings(program, machine, &cast.value, bindings),
                target_type: cast.target_type.clone(),
            },
        )),
        Expression::Indexed(indexed) => Expression::Indexed(Box::new(IndexedExpression {
            collection: simplify_expression_with_bindings(
                program,
                machine,
                &indexed.collection,
                bindings,
            ),
            index: simplify_expression_with_bindings(program, machine, &indexed.index, bindings),
        })),
        Expression::Member(member) => Expression::Member(Box::new(MemberExpression {
            receiver: simplify_expression_with_bindings(
                program,
                machine,
                &member.receiver,
                bindings,
            ),
            member_symbol: member.member_symbol,
            member: member.member.clone(),
        })),
        Expression::Mutable(inner) => Expression::Mutable(Box::new(
            simplify_expression_with_bindings(program, machine, inner, bindings),
        )),
        Expression::Name(path) => bindings
            .iter()
            .find(|binding| {
                (binding.symbol.is_valid()
                    && path.head_symbol().is_valid()
                    && binding.symbol == path.head_symbol())
                    || (!path.head_symbol().is_valid()
                        && path.len() == 1
                        && binding
                            .name
                            .as_ref()
                            .is_some_and(|name| path.first().is_some_and(|segment| segment == name)))
            })
            .map(|binding| append_name_suffix(&binding.value, &path[1..]))
            .unwrap_or_else(|| expression.clone()),
        Expression::StructLiteral(struct_literal) => {
            Expression::StructLiteral(StructLiteral {
                type_name: struct_literal.type_name.clone(),
                fields: struct_literal
                    .fields
                    .iter()
                    .map(|field| StructLiteralField {
                        name: field.name.clone(),
                        value: simplify_expression_with_bindings(
                            program,
                            machine,
                            &field.value,
                            bindings,
                        ),
                    })
                    .collect(),
            })
        }
    }
}

fn simplify_binary_expression(
    program: &Program,
    machine: &Machine,
    binary: &BinaryExpression,
    bindings: &[Binding],
) -> Expression {
    let left = simplify_expression_with_bindings(program, machine, &binary.left, bindings);
    let right = simplify_expression_with_bindings(program, machine, &binary.right, bindings);

    if let Some(expression) =
        simplify_guarded_helper_comparison(program, machine, binary.operator, &left, &right, bindings)
    {
        return simplify_expression_with_bindings(program, machine, &expression, bindings);
    }

    fold_binary_expression(binary.operator, left, right)
}

fn simplify_call_expression(
    program: &Program,
    machine: &Machine,
    call: &CallExpression,
    bindings: &[Binding],
) -> Expression {
    let receiver = call
        .receiver
        .as_ref()
        .map(|receiver| simplify_expression_with_bindings(program, machine, receiver, bindings));
    let simplified_arguments: Vec<_> = call
        .arguments
        .iter()
        .map(|argument| simplify_expression_with_bindings(program, machine, argument, bindings))
        .collect();

    if let Some(target_machine) = resolve_call_target_machine(
        program,
        machine,
        receiver.as_ref(),
    )
        && let Some(state) = resolve_call_target_state(target_machine, call)
    {
        let argument_bindings: Vec<_> = state
            .parameters
            .iter()
            .zip(simplified_arguments.iter())
            .map(|(parameter, argument)| Binding {
                symbol: parameter.symbol,
                name: Some(parameter.name.clone()),
                value: argument.clone(),
            })
            .collect();
        if let Some(value) = helper_state_value(state, program, target_machine, &argument_bindings) {
            return simplify_expression_with_bindings(program, machine, &value, &argument_bindings);
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
    program: &Program,
    machine: &Machine,
    operator: omega_typed_trees::expression::BinaryOperator,
    left: &Expression,
    right: &Expression,
    bindings: &[Binding],
) -> Option<Expression> {
    use omega_typed_trees::expression::BinaryOperator::{Equal, NotEqual};

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
    program: &Program,
    machine: &Machine,
    call: &CallExpression,
    expected: &Expression,
    bindings: &[Binding],
) -> Option<Expression> {
    let receiver = call
        .receiver
        .as_ref()
        .map(|receiver| simplify_expression_with_bindings(program, machine, receiver, bindings));
    let target_machine = resolve_call_target_machine(program, machine, receiver.as_ref())?;
    let state = resolve_call_target_state(target_machine, call)?;
    let argument_values: Vec<_> = call
        .arguments
        .iter()
        .map(|argument| simplify_expression_with_bindings(program, machine, argument, bindings))
        .collect();
    let argument_bindings: Vec<_> = state
        .parameters
        .iter()
        .zip(argument_values.iter())
        .map(|(parameter, argument)| Binding {
            symbol: parameter.symbol,
            name: Some(parameter.name.clone()),
            value: argument.clone(),
        })
        .collect();

    helper_state_match_condition(
        state,
        program,
        target_machine,
        &argument_bindings,
        expected,
    )
}

fn resolve_call_target_machine<'program>(
    program: &'program Program,
    current_machine: &'program Machine,
    receiver: Option<&Expression>,
) -> Option<&'program Machine> {
    let Some(receiver) = receiver else {
        return Some(current_machine);
    };
    let receiver = strip_mutable_expression_ref(receiver);
    if expression_is_self_reference(receiver) {
        return Some(current_machine);
    }

    let (contained_symbol, contained_name) = match receiver {
        Expression::Member(member) if expression_is_self_reference(&member.receiver) => {
            (member.member_symbol, Some(member.member.as_str()))
        }
        Expression::Name(path) => (path.symbol(), path.last().map(|name| name.as_str())),
        _ => return None,
    };

    let contained = current_machine.contains.iter().find(|contained| {
        (contained_symbol.is_valid() && contained.symbol == contained_symbol)
            || contained_name.is_some_and(|name| contained.name.as_str() == name)
    })?;

    program
        .machines
        .iter()
        .find(|machine| machine.symbol == contained.type_symbol || machine.name == contained.type_name)
}

fn strip_mutable_expression_ref(mut expression: &Expression) -> &Expression {
    while let Expression::Mutable(inner) = expression {
        expression = inner.as_ref();
    }
    expression
}

fn resolve_call_target_state<'machine>(
    machine: &'machine Machine,
    call: &CallExpression,
) -> Option<&'machine omega_typed_trees::state::State> {
    machine.states.iter().find(|state| {
        (call.target_symbol.is_valid() && state.symbol == call.target_symbol)
            || state.name == call.target
    })
}

fn expression_is_self_reference(expression: &Expression) -> bool {
    match expression {
        Expression::Mutable(inner) => expression_is_self_reference(inner),
        Expression::Name(path) => {
            path.len() == 1 && path.first().is_some_and(|segment| segment.as_str() == "self")
        }
        _ => false,
    }
}

fn helper_state_value(
    state: &State,
    program: &Program,
    machine: &Machine,
    bindings: &[Binding],
) -> Option<Expression> {
    let helper = helper_state_model(state, program, machine, bindings)?;
    let mut transitions = helper.transitions.iter();
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
    program: &Program,
    machine: &Machine,
    bindings: &[Binding],
    expected: &Expression,
) -> Option<Expression> {
    let helper = helper_state_model(state, program, machine, bindings)?;
    let mut covered = Expression::Boolean(false);
    let mut matched = Expression::Boolean(false);

    for transition in &helper.transitions {
        let effective_guard = boolean_and(boolean_not(covered.clone()), transition.guard.clone());
        if expressions_equivalent(&transition.value, expected) {
            matched = boolean_or(matched, effective_guard.clone());
        }
        covered = boolean_or(covered, effective_guard);
    }

    Some(matched)
}

fn helper_state_model(
    state: &State,
    program: &Program,
    machine: &Machine,
    bindings: &[Binding],
) -> Option<HelperStateModel> {
    let mut bindings = bindings.to_vec();
    let mut transitions = Vec::new();

    for statement in &state.statements {
        match statement {
            Statement::LocalData(local) => {
                let initial_value = local.initial_value.as_ref()?;
                let value =
                    simplify_expression_with_bindings(program, machine, initial_value, &bindings);
                bindings.push(Binding {
                    symbol: local.symbol,
                    name: Some(local.name.clone()),
                    value,
                });
            }
            Statement::Transition(transition) => {
                if transition.continuation.is_some() {
                    return None;
                }
                let guard = match &transition.guard {
                    TransitionGuard::Always => Expression::Boolean(true),
                    TransitionGuard::When(expression) => {
                        simplify_expression_with_bindings(program, machine, expression, &bindings)
                    }
                };
                let TransitionTarget::Value(value) = &transition.target else {
                    return None;
                };
                let value = simplify_expression_with_bindings(program, machine, value, &bindings);
                transitions.push(HelperTransition { guard, value });
            }
            Statement::Assignment(_) | Statement::Call(_) | Statement::Expression(_) => {
                return None;
            }
        }
    }

    if transitions.is_empty() {
        return None;
    }

    Some(HelperStateModel { transitions })
}

fn append_name_suffix(base: &Expression, suffix: &[omega_typed_trees::name::ProgramName]) -> Expression {
    let mut expression = base.clone();

    for member in suffix {
        expression = Expression::Member(Box::new(MemberExpression {
            receiver: expression,
            member_symbol: SymbolHandle::invalid(),
            member: member.clone(),
        }));
    }

    expression
}

#[derive(Debug, Clone)]
struct HelperStateModel {
    transitions: Vec<HelperTransition>,
}

#[derive(Debug, Clone)]
struct HelperTransition {
    guard: Expression,
    value: Expression,
}

fn fold_binary_expression(
    operator: omega_typed_trees::expression::BinaryOperator,
    left: Expression,
    right: Expression,
) -> Expression {
    use omega_typed_trees::expression::BinaryOperator as Op;

    match operator {
        Op::And => boolean_and(left, right),
        Op::Or => boolean_or(left, right),
        Op::Equal => match (&left, &right) {
            (Expression::Boolean(a), Expression::Boolean(b)) => Expression::Boolean(a == b),
            (Expression::Integer(a), Expression::Integer(b)) => Expression::Boolean(a == b),
            (Expression::String(a), Expression::String(b)) => Expression::Boolean(a == b),
            _ if left == right => Expression::Boolean(true),
            _ => Expression::Binary(Box::new(BinaryExpression { left, operator, right })),
        },
        Op::NotEqual => match (&left, &right) {
            (Expression::Boolean(a), Expression::Boolean(b)) => Expression::Boolean(a != b),
            (Expression::Integer(a), Expression::Integer(b)) => Expression::Boolean(a != b),
            (Expression::String(a), Expression::String(b)) => Expression::Boolean(a != b),
            _ if left == right => Expression::Boolean(false),
            _ => Expression::Binary(Box::new(BinaryExpression { left, operator, right })),
        },
        Op::Greater => fold_integer_compare(left, right, |a, b| a > b, operator),
        Op::GreaterOrEqual => fold_integer_compare(left, right, |a, b| a >= b, operator),
        Op::Less => fold_integer_compare(left, right, |a, b| a < b, operator),
        Op::LessOrEqual => fold_integer_compare(left, right, |a, b| a <= b, operator),
        Op::Add => fold_integer_math(left, right, |a, b| a + b, operator),
        Op::Subtract => fold_integer_math(left, right, |a, b| a - b, operator),
        Op::Multiply => fold_integer_math(left, right, |a, b| a * b, operator),
        Op::Divide => match (&left, &right) {
            (Expression::Integer(_), Expression::Integer(0)) => {
                Expression::Binary(Box::new(BinaryExpression { left, operator, right }))
            }
            (Expression::Integer(a), Expression::Integer(b)) => Expression::Integer(a / b),
            _ => Expression::Binary(Box::new(BinaryExpression { left, operator, right })),
        },
        Op::Modulo => match (&left, &right) {
            (Expression::Integer(_), Expression::Integer(0)) => {
                Expression::Binary(Box::new(BinaryExpression { left, operator, right }))
            }
            (Expression::Integer(a), Expression::Integer(b)) => Expression::Integer(a % b),
            _ => Expression::Binary(Box::new(BinaryExpression { left, operator, right })),
        },
        Op::ShiftLeft => fold_integer_math(left, right, |a, b| a << b, operator),
        Op::ShiftRight => fold_integer_math(left, right, |a, b| a >> b, operator),
    }
}

fn fold_integer_math(
    left: Expression,
    right: Expression,
    operation: impl FnOnce(i64, i64) -> i64,
    operator: omega_typed_trees::expression::BinaryOperator,
) -> Expression {
    match (&left, &right) {
        (Expression::Integer(a), Expression::Integer(b)) => Expression::Integer(operation(*a, *b)),
        _ => Expression::Binary(Box::new(BinaryExpression { left, operator, right })),
    }
}

fn fold_integer_compare(
    left: Expression,
    right: Expression,
    comparison: impl FnOnce(i64, i64) -> bool,
    operator: omega_typed_trees::expression::BinaryOperator,
) -> Expression {
    match (&left, &right) {
        (Expression::Integer(a), Expression::Integer(b)) => Expression::Boolean(comparison(*a, *b)),
        _ => Expression::Binary(Box::new(BinaryExpression { left, operator, right })),
    }
}

fn boolean_and(left: Expression, right: Expression) -> Expression {
    match (&left, &right) {
        (Expression::Boolean(false), _) | (_, Expression::Boolean(false)) => Expression::Boolean(false),
        (Expression::Boolean(true), _) => right,
        (_, Expression::Boolean(true)) => left,
        _ if left == right => left,
        _ => Expression::Binary(Box::new(BinaryExpression {
            left,
            operator: omega_typed_trees::expression::BinaryOperator::And,
            right,
        })),
    }
}

fn boolean_or(left: Expression, right: Expression) -> Expression {
    match (&left, &right) {
        (Expression::Boolean(true), _) | (_, Expression::Boolean(true)) => Expression::Boolean(true),
        (Expression::Boolean(false), _) => right,
        (_, Expression::Boolean(false)) => left,
        _ if left == right => left,
        _ => Expression::Binary(Box::new(BinaryExpression {
            left,
            operator: omega_typed_trees::expression::BinaryOperator::Or,
            right,
        })),
    }
}

fn boolean_not(expression: Expression) -> Expression {
    use omega_typed_trees::expression::BinaryOperator as Op;

    match expression {
        Expression::Boolean(value) => Expression::Boolean(!value),
        Expression::Binary(binary) => {
            let inverted = match binary.operator {
                Op::Equal => Some(Op::NotEqual),
                Op::NotEqual => Some(Op::Equal),
                Op::Greater => Some(Op::LessOrEqual),
                Op::GreaterOrEqual => Some(Op::Less),
                Op::Less => Some(Op::GreaterOrEqual),
                Op::LessOrEqual => Some(Op::Greater),
                Op::And => {
                    return boolean_or(boolean_not(binary.left), boolean_not(binary.right));
                }
                Op::Or => {
                    return boolean_and(boolean_not(binary.left), boolean_not(binary.right));
                }
                Op::Add | Op::Divide | Op::Modulo | Op::Multiply | Op::ShiftLeft | Op::ShiftRight | Op::Subtract => None,
            };

            if let Some(operator) = inverted {
                Expression::Binary(Box::new(BinaryExpression {
                    left: binary.left,
                    operator,
                    right: binary.right,
                }))
            } else {
                Expression::Binary(Box::new(BinaryExpression {
                    left: Expression::Binary(binary),
                    operator: Op::Equal,
                    right: Expression::Boolean(false),
                }))
            }
        }
        other => Expression::Binary(Box::new(BinaryExpression {
            left: other,
            operator: omega_typed_trees::expression::BinaryOperator::Equal,
            right: Expression::Boolean(false),
        })),
    }
}

fn expressions_equivalent(left: &Expression, right: &Expression) -> bool {
    if let (Some(left_path), Some(right_path)) =
        (expression_path_segments(left), expression_path_segments(right))
    {
        return left_path == right_path;
    }

    match (left, right) {
        (Expression::ArrayLiteral(left), Expression::ArrayLiteral(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right.iter())
                    .all(|(left, right)| expressions_equivalent(left, right))
        }
        (Expression::Binary(left), Expression::Binary(right)) => {
            left.operator == right.operator
                && expressions_equivalent(&left.left, &right.left)
                && expressions_equivalent(&left.right, &right.right)
        }
        (Expression::Boolean(left), Expression::Boolean(right)) => left == right,
        (Expression::Call(left), Expression::Call(right)) => {
            left.target == right.target
                && left.arguments.len() == right.arguments.len()
                && left
                    .receiver
                    .as_deref()
                    .zip(right.receiver.as_deref())
                    .map(|(left, right)| expressions_equivalent(left, right))
                    .unwrap_or(left.receiver.is_none() && right.receiver.is_none())
                && left
                    .arguments
                    .iter()
                    .zip(right.arguments.iter())
                    .all(|(left, right)| expressions_equivalent(left, right))
        }
        (Expression::Cast(left), Expression::Cast(right)) => {
            left.target_type.members() == right.target_type.members()
                && expressions_equivalent(&left.value, &right.value)
        }
        (Expression::Float(left), Expression::Float(right)) => left == right,
        (Expression::Indexed(left), Expression::Indexed(right)) => {
            expressions_equivalent(&left.collection, &right.collection)
                && expressions_equivalent(&left.index, &right.index)
        }
        (Expression::Integer(left), Expression::Integer(right)) => left == right,
        (Expression::Member(left), Expression::Member(right)) => {
            left.member == right.member
                && expressions_equivalent(&left.receiver, &right.receiver)
        }
        (Expression::Mutable(left), Expression::Mutable(right)) => {
            expressions_equivalent(left, right)
        }
        (Expression::Name(left), Expression::Name(right)) => left.members() == right.members(),
        (Expression::StructLiteral(left), Expression::StructLiteral(right)) => {
            left.type_name == right.type_name
                && left.fields.len() == right.fields.len()
                && left.fields.iter().zip(right.fields.iter()).all(|(left, right)| {
                    left.name == right.name && expressions_equivalent(&left.value, &right.value)
                })
        }
        (Expression::String(left), Expression::String(right)) => left == right,
        _ => false,
    }
}

fn expression_path_segments(expression: &Expression) -> Option<Vec<String>> {
    match expression {
        Expression::Name(path) => Some(path.iter().map(|segment| segment.to_string()).collect()),
        Expression::Member(member) => {
            let mut path = expression_path_segments(&member.receiver)?;
            path.push(member.member.to_string());
            Some(path)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::simplify_expression;
    use omega_core::symbols::SymbolHandle;
    use omega_typed_trees::expression::{BinaryExpression, BinaryOperator, CallExpression, Expression, NamePath};
    use omega_typed_trees::machine::Machine;
    use omega_typed_trees::name::ProgramName;
    use omega_typed_trees::signature::StateParameter;
    use omega_typed_trees::state::State;
    use omega_typed_trees::statement::{LocalData, Statement, Transition, TransitionGuard, TransitionTarget};
    use omega_typed_trees::types::TypeReference;
    use omega_typed_trees::Program;

    #[test]
    fn simplifies_guarded_helper_call_comparison_to_guard_expression() {
        let machine_symbol = SymbolHandle::from_arena_index(1);
        let helper_symbol = SymbolHandle::from_arena_index(2);
        let roll_symbol = SymbolHandle::from_arena_index(3);
        let is_quiet_symbol = SymbolHandle::from_arena_index(4);
        let is_fountain_symbol = SymbolHandle::from_arena_index(5);

        let helper = State {
            symbol: helper_symbol,
            name: "event_action".into(),
            parameters: vec![StateParameter {
                symbol: roll_symbol,
                name: "roll".into(),
                type_reference: TypeReference::Named {
                    symbol: SymbolHandle::invalid(),
                    name: "u32".into(),
                },
                is_const: true,
                is_mutable: false,
                is_self: false,
            }],
            return_type: None,
            statements: vec![
                Statement::LocalData(LocalData {
                    symbol: is_quiet_symbol,
                    name: "is_quiet".into(),
                    type_reference: TypeReference::Named {
                        symbol: SymbolHandle::invalid(),
                        name: "bool".into(),
                    },
                    initial_value: Some(Expression::Binary(Box::new(BinaryExpression {
                        left: name("roll", roll_symbol),
                        operator: BinaryOperator::Less,
                        right: Expression::Integer(20),
                    }))),
                }),
                Statement::LocalData(LocalData {
                    symbol: is_fountain_symbol,
                    name: "is_fountain".into(),
                    type_reference: TypeReference::Named {
                        symbol: SymbolHandle::invalid(),
                        name: "bool".into(),
                    },
                    initial_value: Some(Expression::Binary(Box::new(BinaryExpression {
                        left: name("roll", roll_symbol),
                        operator: BinaryOperator::Less,
                        right: Expression::Integer(30),
                    }))),
                }),
                Statement::Transition(Transition {
                    target: TransitionTarget::Value(path_expression(&["RoomEventAction", "Quiet"])),
                    continuation: None,
                    guard: TransitionGuard::When(name("is_quiet", is_quiet_symbol)),
                }),
                Statement::Transition(Transition {
                    target: TransitionTarget::Value(path_expression(&["RoomEventAction", "Fountain"])),
                    continuation: None,
                    guard: TransitionGuard::When(name("is_fountain", is_fountain_symbol)),
                }),
                Statement::Transition(Transition {
                    target: TransitionTarget::Value(path_expression(&["RoomEventAction", "Enemy"])),
                    continuation: None,
                    guard: TransitionGuard::Always,
                }),
            ],
            statement_nodes: Default::default(),
        };

        let machine = Machine {
            symbol: machine_symbol,
            name: "RoomEvents".into(),
            contains: vec![],
            owned_data: vec![],
            states: vec![helper],
        };
        let program = Program {
            machines: vec![machine.clone()],
            ..Program::default()
        };

        let quiet_guard = Expression::Binary(Box::new(BinaryExpression {
            left: Expression::Call(Box::new(CallExpression {
                receiver: None,
                target_symbol: helper_symbol,
                target: "event_action".into(),
                arguments: vec![name("roll", roll_symbol)],
            })),
            operator: BinaryOperator::Equal,
            right: path_expression(&["RoomEventAction", "Quiet"]),
        }));

        let fountain_guard = Expression::Binary(Box::new(BinaryExpression {
            left: Expression::Call(Box::new(CallExpression {
                receiver: None,
                target_symbol: helper_symbol,
                target: "event_action".into(),
                arguments: vec![name("roll", roll_symbol)],
            })),
            operator: BinaryOperator::Equal,
            right: path_expression(&["RoomEventAction", "Fountain"]),
        }));

        assert_eq!(
            simplify_expression(&program, &machine, &quiet_guard),
            Expression::Binary(Box::new(BinaryExpression {
                left: name("roll", roll_symbol),
                operator: BinaryOperator::Less,
                right: Expression::Integer(20),
            }))
        );

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
    }

    fn name(name: &str, symbol: SymbolHandle) -> Expression {
        Expression::Name(NamePath::resolved(vec![name.into()], symbol, symbol))
    }

    fn path_expression(segments: &[&str]) -> Expression {
        Expression::Name(path(segments))
    }

    fn path(segments: &[&str]) -> NamePath {
        NamePath::unresolved(
            segments
                .iter()
                .map(|segment| ProgramName::from(*segment))
                .collect(),
        )
    }
}
