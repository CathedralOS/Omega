use omega_core::symbols::SymbolHandle;
use omega_typed_trees::Program;
use omega_typed_trees::expression::{BinaryExpression, CallExpression, Expression, IndexedExpression, MemberExpression, StructLiteral, StructLiteralField};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::statement::{Statement, TransitionGuard, TransitionTarget};

pub(super) fn simplify_expression(
    program: &Program,
    machine: &Machine,
    expression: &Expression,
) -> Expression {
    simplify_expression_with_arguments(program, machine, expression, &[])
}

fn simplify_expression_with_arguments(
    program: &Program,
    machine: &Machine,
    expression: &Expression,
    arguments: &[(SymbolHandle, Expression)],
) -> Expression {
    match expression {
        Expression::ArrayLiteral(values) => Expression::ArrayLiteral(
            values
                .iter()
                .map(|value| simplify_expression_with_arguments(program, machine, value, arguments))
                .collect(),
        ),
        Expression::Binary(binary) => Expression::Binary(Box::new(BinaryExpression {
            left: simplify_expression_with_arguments(program, machine, &binary.left, arguments),
            operator: binary.operator,
            right: simplify_expression_with_arguments(program, machine, &binary.right, arguments),
        })),
        Expression::Boolean(_)
        | Expression::Float(_)
        | Expression::Integer(_)
        | Expression::String(_) => expression.clone(),
        Expression::Call(call) => simplify_call_expression(program, machine, call, arguments),
        Expression::Cast(cast) => Expression::Cast(Box::new(
            omega_typed_trees::expression::CastExpression {
                value: simplify_expression_with_arguments(program, machine, &cast.value, arguments),
                target_type: cast.target_type.clone(),
            },
        )),
        Expression::Indexed(indexed) => Expression::Indexed(Box::new(IndexedExpression {
            collection: simplify_expression_with_arguments(
                program,
                machine,
                &indexed.collection,
                arguments,
            ),
            index: simplify_expression_with_arguments(program, machine, &indexed.index, arguments),
        })),
        Expression::Member(member) => Expression::Member(Box::new(MemberExpression {
            receiver: simplify_expression_with_arguments(
                program,
                machine,
                &member.receiver,
                arguments,
            ),
            member_symbol: member.member_symbol,
            member: member.member.clone(),
        })),
        Expression::Mutable(inner) => Expression::Mutable(Box::new(
            simplify_expression_with_arguments(program, machine, inner, arguments),
        )),
        Expression::Name(path) => arguments
            .iter()
            .find(|(symbol, _)| {
                symbol.is_valid() && path.head_symbol().is_valid() && *symbol == path.head_symbol()
            })
            .map(|(_, value)| append_name_suffix(value, &path[1..]))
            .unwrap_or_else(|| expression.clone()),
        Expression::StructLiteral(struct_literal) => {
            Expression::StructLiteral(StructLiteral {
                type_name: struct_literal.type_name.clone(),
                fields: struct_literal
                    .fields
                    .iter()
                    .map(|field| StructLiteralField {
                        name: field.name.clone(),
                        value: simplify_expression_with_arguments(
                            program,
                            machine,
                            &field.value,
                            arguments,
                        ),
                    })
                    .collect(),
            })
        }
    }
}

fn simplify_call_expression(
    program: &Program,
    machine: &Machine,
    call: &CallExpression,
    arguments: &[(SymbolHandle, Expression)],
) -> Expression {
    let receiver = call
        .receiver
        .as_ref()
        .map(|receiver| simplify_expression_with_arguments(program, machine, receiver, arguments));
    let simplified_arguments: Vec<_> = call
        .arguments
        .iter()
        .map(|argument| simplify_expression_with_arguments(program, machine, argument, arguments))
        .collect();

    let receiver_is_self = receiver.as_ref().is_none_or(|receiver| {
        expression_is_self_reference(receiver)
    });

    if receiver_is_self
        && call.target_symbol.is_valid()
        && let Some(state) = machine
            .states
            .iter()
            .find(|state| state.symbol == call.target_symbol)
        && let Some(value) = unconditional_terminal_value(state)
    {
        let argument_bindings: Vec<_> = state
            .parameters
            .iter()
            .zip(simplified_arguments.iter())
            .map(|(parameter, argument)| (parameter.symbol, argument.clone()))
            .collect();
        return simplify_expression_with_arguments(program, machine, value, &argument_bindings);
    }

    Expression::Call(Box::new(CallExpression {
        receiver: receiver.map(Box::new),
        target_symbol: call.target_symbol,
        target: call.target.clone(),
        arguments: simplified_arguments,
    }))
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

fn unconditional_terminal_value<'state>(
    state: &'state omega_typed_trees::state::State,
) -> Option<&'state Expression> {
    if state
        .statements
        .iter()
        .any(|statement| !matches!(statement, Statement::Transition(_)))
    {
        return None;
    }

    let mut transitions = state.statements.iter().filter_map(|statement| match statement {
        Statement::Transition(transition) => Some(transition),
        _ => None,
    });
    let transition = transitions.next()?;
    if transitions.next().is_some() {
        return None;
    }
    if !matches!(transition.guard, TransitionGuard::Always) {
        return None;
    }
    if transition.continuation.is_some() {
        return None;
    }

    match &transition.target {
        TransitionTarget::Value(value) => Some(value),
        _ => None,
    }
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
