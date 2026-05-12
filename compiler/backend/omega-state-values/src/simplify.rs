use omega_core::symbols::SymbolHandle;
use omega_typed_trees::Program;
use omega_typed_trees::expression::{BinaryExpression, CallExpression, Expression, IndexedExpression, MemberExpression, StructLiteral, StructLiteralField};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::statement::{Statement, TransitionGuard, TransitionTarget};

pub fn simplify_expression(
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

    if let Some(target_machine) = resolve_call_target_machine(
        program,
        machine,
        receiver.as_ref(),
    )
        && let Some(state) = resolve_call_target_state(target_machine, call)
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
