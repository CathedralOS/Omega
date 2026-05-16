use crate::StateValueRole;
use omega_checked_trees::Program;
use omega_checked_trees::expression::{
    BinaryExpression, CallExpression, Expression, IndexedExpression, MemberExpression, NamePath,
    StructLiteral, StructLiteralField,
};
use omega_checked_trees::machine::Machine;
use omega_checked_trees::name::ProgramName;
use omega_checked_trees::state::State;
use omega_checked_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use omega_core::arena::Arena;
use omega_core::symbols::SymbolHandle;

pub fn simplify_expression(
    program: &Program,
    machine: &Machine,
    expression: &Expression,
) -> Expression {
    let bindings: &[Binding] = &[];
    simplify_expression_with_bindings(program, machine, expression, bindings, false)
}

pub fn simplify_state_expression(
    program: &Program,
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
    program: &Program,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    role: StateValueRole,
    expression: &Expression,
) -> Expression {
    let bindings = simple_local_bindings(program, state, statement_index);
    let preserve_call_locals = role == StateValueRole::TransitionArgument;
    simplify_expression_with_bindings(
        program,
        machine,
        expression,
        &bindings,
        preserve_call_locals,
    )
}

#[derive(Debug, Clone)]
struct Binding {
    symbol: SymbolHandle,
    name: Option<ProgramName>,
    value: Expression,
}

impl Default for Binding {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: None,
            value: Expression::Integer(0),
        }
    }
}

trait BindingScope {
    fn find_path_binding(&self, path: &NamePath) -> Option<&Binding>;
}

impl BindingScope for [Binding] {
    fn find_path_binding(&self, path: &NamePath) -> Option<&Binding> {
        self.iter()
            .find(|binding| binding_matches_path(binding, path))
    }
}

impl BindingScope for Vec<Binding> {
    fn find_path_binding(&self, path: &NamePath) -> Option<&Binding> {
        self.as_slice().find_path_binding(path)
    }
}

struct ScopedBindings<'scope, Parent: BindingScope + ?Sized> {
    parent: &'scope Parent,
    locals: &'scope Arena<Binding>,
}

impl<Parent: BindingScope + ?Sized> BindingScope for ScopedBindings<'_, Parent> {
    fn find_path_binding(&self, path: &NamePath) -> Option<&Binding> {
        self.parent.find_path_binding(path).or_else(|| {
            self.locals
                .iter()
                .map(|(_, binding)| binding)
                .find(|binding| binding_matches_path(binding, path))
        })
    }
}

fn binding_matches_path(binding: &Binding, path: &NamePath) -> bool {
    (binding.symbol.is_valid()
        && path.head_symbol().is_valid()
        && binding.symbol == path.head_symbol())
        || (!path.head_symbol().is_valid()
            && path.len() == 1
            && binding
                .name
                .as_ref()
                .is_some_and(|name| path.first().is_some_and(|segment| segment == name)))
}

fn simple_local_bindings(program: &Program, state: &State, statement_index: usize) -> Vec<Binding> {
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
        .filter_map(|statement| {
            let StatementNode::LocalData(local_data) = statement else {
                return None;
            };
            if !local_data.initial_value.is_valid() {
                return None;
            }
            let value = program.expression_table.to_tree(local_data.initial_value);
            simple_local_binding_value(&value).map(|value| Binding {
                symbol: local_data.symbol,
                name: Some(local_data.name.clone()),
                value,
            })
        })
        .collect()
}

fn simple_local_binding_value(expression: &Expression) -> Option<Expression> {
    match expression {
        Expression::Binary(binary) => Some(Expression::Binary(Box::new(BinaryExpression {
            left: simple_local_binding_value(&binary.left)?,
            operator: binary.operator,
            right: simple_local_binding_value(&binary.right)?,
        }))),
        Expression::Boolean(_)
        | Expression::Float(_)
        | Expression::Integer(_)
        | Expression::String(_) => Some(expression.clone()),
        Expression::Indexed(indexed) => Some(Expression::Indexed(Box::new(IndexedExpression {
            collection: simple_local_binding_value(&indexed.collection)?,
            index: simple_local_binding_value(&indexed.index)?,
        }))),
        Expression::Call(call) => Some(Expression::Call(Box::new(CallExpression {
            receiver: call
                .receiver
                .as_ref()
                .map(|receiver| simple_local_binding_value(receiver).map(Box::new))
                .flatten(),
            target_symbol: call.target_symbol,
            target: call.target.clone(),
            arguments: call
                .arguments
                .iter()
                .map(simple_local_binding_value)
                .collect::<Option<Vec<_>>>()?,
        }))),
        Expression::Mutable(inner) => {
            simple_local_binding_value(inner).map(|value| Expression::Mutable(Box::new(value)))
        }
        Expression::Name(_) => Some(expression.clone()),
        Expression::Member(member) => {
            let receiver = simple_local_binding_value(&member.receiver)?;
            Some(Expression::Member(Box::new(MemberExpression {
                receiver,
                member_symbol: member.member_symbol,
                member: member.member.clone(),
            })))
        }
        Expression::ArrayLiteral(_) | Expression::Cast(_) | Expression::StructLiteral(_) => None,
    }
}

fn simplify_expression_with_bindings(
    program: &Program,
    machine: &Machine,
    expression: &Expression,
    bindings: &(impl BindingScope + ?Sized),
    preserve_call_locals: bool,
) -> Expression {
    match expression {
        Expression::ArrayLiteral(values) => Expression::ArrayLiteral(
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
                .collect(),
        ),
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
                        preserve_call_locals,
                    ),
                })
                .collect(),
        }),
    }
}

fn simplify_binary_expression(
    program: &Program,
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
    program: &Program,
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
    let simplified_arguments: Vec<_> = call
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

    if let Some(receiver) = receiver.as_ref()
        && call.arguments.is_empty()
    {
        match call.target.as_str() {
            "is_some" => {
                if let Some(is_none) = expression_match_condition(
                    program,
                    machine,
                    receiver,
                    &none_expression(program),
                ) {
                    return simplify_expression_with_bindings(
                        program,
                        machine,
                        &boolean_not(is_none),
                        bindings,
                        preserve_call_locals,
                    );
                }
                return Expression::Binary(Box::new(BinaryExpression {
                    left: receiver.clone(),
                    operator: omega_checked_trees::expression::BinaryOperator::NotEqual,
                    right: none_expression(program),
                }));
            }
            "is_none" => {
                if let Some(is_none) = expression_match_condition(
                    program,
                    machine,
                    receiver,
                    &none_expression(program),
                ) {
                    return simplify_expression_with_bindings(
                        program,
                        machine,
                        &is_none,
                        bindings,
                        preserve_call_locals,
                    );
                }
                return Expression::Binary(Box::new(BinaryExpression {
                    left: receiver.clone(),
                    operator: omega_checked_trees::expression::BinaryOperator::Equal,
                    right: none_expression(program),
                }));
            }
            _ => {}
        }
    }

    if let Some(target_machine) = resolve_call_target_machine(program, machine, receiver.as_ref())
        && let Some(state) = resolve_call_target_state(program, target_machine, call)
    {
        let argument_bindings: Vec<_> = program
            .state_parameters(state)
            .iter()
            .zip(simplified_arguments.iter())
            .map(|(parameter, argument)| Binding {
                symbol: parameter.symbol,
                name: Some(parameter.name.clone()),
                value: argument.clone(),
            })
            .collect();
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
    program: &Program,
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
    program: &Program,
    machine: &Machine,
    call: &CallExpression,
    expected: &Expression,
    bindings: &(impl BindingScope + ?Sized),
) -> Option<Expression> {
    let receiver = call.receiver.as_ref().map(|receiver| {
        simplify_expression_with_bindings(program, machine, receiver, bindings, false)
    });
    let target_machine = resolve_call_target_machine(program, machine, receiver.as_ref())?;
    let state = resolve_call_target_state(program, target_machine, call)?;
    let argument_values: Vec<_> = call
        .arguments
        .iter()
        .map(|argument| {
            simplify_expression_with_bindings(program, machine, argument, bindings, false)
        })
        .collect();
    let argument_bindings: Vec<_> = program
        .state_parameters(state)
        .iter()
        .zip(argument_values.iter())
        .map(|(parameter, argument)| Binding {
            symbol: parameter.symbol,
            name: Some(parameter.name.clone()),
            value: argument.clone(),
        })
        .collect();

    helper_state_match_condition(state, program, target_machine, &argument_bindings, expected)
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

    let contained_symbol = match receiver {
        Expression::Member(member) if expression_is_self_reference(&member.receiver) => {
            member.member_symbol
        }
        Expression::Name(path) => path.symbol(),
        _ => return None,
    };

    if !contained_symbol.is_valid() {
        return None;
    }

    let contained = program
        .machine_contained_objects(current_machine)
        .iter()
        .find(|contained| contained.symbol == contained_symbol)?;

    program
        .machines()
        .iter()
        .find(|machine| machine.symbol == contained.type_symbol)
}

fn strip_mutable_expression_ref(mut expression: &Expression) -> &Expression {
    while let Expression::Mutable(inner) = expression {
        expression = inner.as_ref();
    }
    expression
}

fn resolve_call_target_state<'machine>(
    program: &'machine Program,
    machine: &'machine Machine,
    call: &CallExpression,
) -> Option<&'machine omega_checked_trees::state::State> {
    if !call.target_symbol.is_valid() {
        return None;
    }

    program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == call.target_symbol)
}

fn expression_is_self_reference(expression: &Expression) -> bool {
    match expression {
        Expression::Mutable(inner) => expression_is_self_reference(inner),
        Expression::Name(path) => {
            path.len() == 1
                && path
                    .first()
                    .is_some_and(|segment| segment.as_str() == "self")
        }
        _ => false,
    }
}

fn helper_state_value(
    state: &State,
    program: &Program,
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
    program: &Program,
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
        &mut Vec::new(),
    )
}

fn helper_state_match_condition_with_stack(
    state: &State,
    program: &Program,
    machine: &Machine,
    bindings: &(impl BindingScope + ?Sized),
    expected: &Expression,
    stack: &mut Vec<SymbolHandle>,
) -> Option<Expression> {
    if state.symbol.is_valid() && stack.contains(&state.symbol) {
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

fn expression_match_condition(
    program: &Program,
    machine: &Machine,
    expression: &Expression,
    expected: &Expression,
) -> Option<Expression> {
    expression_match_condition_with_stack(program, machine, expression, expected, &mut Vec::new())
}

fn expression_match_condition_with_stack(
    program: &Program,
    machine: &Machine,
    expression: &Expression,
    expected: &Expression,
    stack: &mut Vec<SymbolHandle>,
) -> Option<Expression> {
    if expressions_equivalent(expression, expected) {
        return Some(Expression::Boolean(true));
    }

    let Expression::Call(call) = expression else {
        return None;
    };

    let receiver = call.receiver.as_deref();
    let target_machine = resolve_call_target_machine(program, machine, receiver)?;
    let state = resolve_call_target_state(program, target_machine, call)?;
    let argument_bindings: Vec<_> = program
        .state_parameters(state)
        .iter()
        .zip(call.arguments.iter())
        .map(|(parameter, argument)| Binding {
            symbol: parameter.symbol,
            name: Some(parameter.name.clone()),
            value: argument.clone(),
        })
        .collect();

    helper_state_match_condition_with_stack(
        state,
        program,
        target_machine,
        &argument_bindings,
        expected,
        stack,
    )
}

fn none_expression(program: &Program) -> Expression {
    let Some(option) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Option")
    else {
        return Expression::Name(NamePath::unresolved(vec![ProgramName::from("None")]));
    };

    let Some(variant) = program
        .data_members(option)
        .iter()
        .find_map(|member| match member {
            omega_checked_trees::data::DataMember::Variant(variant)
                if variant.name.as_str() == "None" =>
            {
                Some(variant)
            }
            _ => None,
        })
    else {
        return Expression::Name(NamePath::unresolved(vec![ProgramName::from("None")]));
    };

    Expression::Name(NamePath::resolved(
        vec![option.name.clone(), variant.name.clone()],
        option.symbol,
        variant.symbol,
    ))
}

fn helper_state_model(
    state: &State,
    program: &Program,
    machine: &Machine,
    bindings: &(impl BindingScope + ?Sized),
) -> Option<HelperStateModel> {
    let mut local_bindings = Arena::new();
    let mut transitions = Arena::new();
    let mut saw_terminal_expression = false;

    for statement in program.statement_table.statements(state.statement_nodes) {
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
                    name: Some(local.name.clone()),
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

fn append_name_suffix(
    base: &Expression,
    suffix: &[omega_checked_trees::name::ProgramName],
) -> Expression {
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

fn fold_binary_expression(
    operator: omega_checked_trees::expression::BinaryOperator,
    left: Expression,
    right: Expression,
) -> Expression {
    use omega_checked_trees::expression::BinaryOperator as Op;

    match operator {
        Op::And => boolean_and(left, right),
        Op::Or => boolean_or(left, right),
        Op::Equal => match (&left, &right) {
            (Expression::Boolean(a), Expression::Boolean(b)) => Expression::Boolean(a == b),
            (Expression::Integer(a), Expression::Integer(b)) => Expression::Boolean(a == b),
            (Expression::String(a), Expression::String(b)) => Expression::Boolean(a == b),
            _ if left == right => Expression::Boolean(true),
            _ => Expression::Binary(Box::new(BinaryExpression {
                left,
                operator,
                right,
            })),
        },
        Op::NotEqual => match (&left, &right) {
            (Expression::Boolean(a), Expression::Boolean(b)) => Expression::Boolean(a != b),
            (Expression::Integer(a), Expression::Integer(b)) => Expression::Boolean(a != b),
            (Expression::String(a), Expression::String(b)) => Expression::Boolean(a != b),
            _ if left == right => Expression::Boolean(false),
            _ => Expression::Binary(Box::new(BinaryExpression {
                left,
                operator,
                right,
            })),
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
                Expression::Binary(Box::new(BinaryExpression {
                    left,
                    operator,
                    right,
                }))
            }
            (Expression::Integer(a), Expression::Integer(b)) => Expression::Integer(a / b),
            _ => Expression::Binary(Box::new(BinaryExpression {
                left,
                operator,
                right,
            })),
        },
        Op::Modulo => match (&left, &right) {
            (Expression::Integer(_), Expression::Integer(0)) => {
                Expression::Binary(Box::new(BinaryExpression {
                    left,
                    operator,
                    right,
                }))
            }
            (Expression::Integer(a), Expression::Integer(b)) => Expression::Integer(a % b),
            _ => Expression::Binary(Box::new(BinaryExpression {
                left,
                operator,
                right,
            })),
        },
        Op::ShiftLeft => fold_integer_math(left, right, |a, b| a << b, operator),
        Op::ShiftRight => fold_integer_math(left, right, |a, b| a >> b, operator),
    }
}

fn fold_integer_math(
    left: Expression,
    right: Expression,
    operation: impl FnOnce(i64, i64) -> i64,
    operator: omega_checked_trees::expression::BinaryOperator,
) -> Expression {
    match (&left, &right) {
        (Expression::Integer(a), Expression::Integer(b)) => Expression::Integer(operation(*a, *b)),
        _ => Expression::Binary(Box::new(BinaryExpression {
            left,
            operator,
            right,
        })),
    }
}

fn fold_integer_compare(
    left: Expression,
    right: Expression,
    comparison: impl FnOnce(i64, i64) -> bool,
    operator: omega_checked_trees::expression::BinaryOperator,
) -> Expression {
    match (&left, &right) {
        (Expression::Integer(a), Expression::Integer(b)) => Expression::Boolean(comparison(*a, *b)),
        _ => Expression::Binary(Box::new(BinaryExpression {
            left,
            operator,
            right,
        })),
    }
}

fn boolean_and(left: Expression, right: Expression) -> Expression {
    if let Expression::Binary(binary) = &left
        && binary.operator == omega_checked_trees::expression::BinaryOperator::Or
    {
        return boolean_or(
            boolean_and(binary.left.clone(), right.clone()),
            boolean_and(binary.right.clone(), right),
        );
    }

    if let Expression::Binary(binary) = &right
        && binary.operator == omega_checked_trees::expression::BinaryOperator::Or
    {
        return boolean_or(
            boolean_and(left.clone(), binary.left.clone()),
            boolean_and(left, binary.right.clone()),
        );
    }

    if let Some(simplified) = simplify_comparison_conjunction(&left, &right) {
        return simplified;
    }

    match (&left, &right) {
        (Expression::Boolean(false), _) | (_, Expression::Boolean(false)) => {
            Expression::Boolean(false)
        }
        (Expression::Boolean(true), _) => right,
        (_, Expression::Boolean(true)) => left,
        _ if left == right => left,
        _ => Expression::Binary(Box::new(BinaryExpression {
            left,
            operator: omega_checked_trees::expression::BinaryOperator::And,
            right,
        })),
    }
}

fn boolean_or(left: Expression, right: Expression) -> Expression {
    match (&left, &right) {
        (Expression::Boolean(true), _) | (_, Expression::Boolean(true)) => {
            Expression::Boolean(true)
        }
        (Expression::Boolean(false), _) => right,
        (_, Expression::Boolean(false)) => left,
        _ if left == right => left,
        _ => Expression::Binary(Box::new(BinaryExpression {
            left,
            operator: omega_checked_trees::expression::BinaryOperator::Or,
            right,
        })),
    }
}

fn simplify_comparison_conjunction(left: &Expression, right: &Expression) -> Option<Expression> {
    let left_compare = parse_integer_comparison(left)?;
    let right_compare = parse_integer_comparison(right)?;

    if !expressions_equivalent(left_compare.subject, right_compare.subject) {
        return None;
    }

    if left_compare.operator == right_compare.operator && left_compare.value == right_compare.value
    {
        return Some(left.clone());
    }

    let mut lower_bound = None;
    let mut upper_bound = None;

    for comparison in [left_compare, right_compare] {
        match comparison.operator {
            omega_checked_trees::expression::BinaryOperator::Greater => {
                lower_bound = tighten_lower_bound(lower_bound, comparison.value, false);
            }
            omega_checked_trees::expression::BinaryOperator::GreaterOrEqual => {
                lower_bound = tighten_lower_bound(lower_bound, comparison.value, true);
            }
            omega_checked_trees::expression::BinaryOperator::Less => {
                upper_bound = tighten_upper_bound(upper_bound, comparison.value, false);
            }
            omega_checked_trees::expression::BinaryOperator::LessOrEqual => {
                upper_bound = tighten_upper_bound(upper_bound, comparison.value, true);
            }
            _ => return None,
        }
    }

    if let (Some((lower, lower_inclusive)), Some((upper, upper_inclusive))) =
        (lower_bound, upper_bound)
    {
        let impossible =
            lower > upper || (lower == upper && (!lower_inclusive || !upper_inclusive));
        if impossible {
            return Some(Expression::Boolean(false));
        }
    }

    if lower_bound.is_some() && upper_bound.is_none() {
        let (value, inclusive) = lower_bound?;
        return Some(Expression::Binary(Box::new(BinaryExpression {
            left: left_compare.subject.clone(),
            operator: if inclusive {
                omega_checked_trees::expression::BinaryOperator::GreaterOrEqual
            } else {
                omega_checked_trees::expression::BinaryOperator::Greater
            },
            right: Expression::Integer(value),
        })));
    }

    if upper_bound.is_some() && lower_bound.is_none() {
        let (value, inclusive) = upper_bound?;
        return Some(Expression::Binary(Box::new(BinaryExpression {
            left: left_compare.subject.clone(),
            operator: if inclusive {
                omega_checked_trees::expression::BinaryOperator::LessOrEqual
            } else {
                omega_checked_trees::expression::BinaryOperator::Less
            },
            right: Expression::Integer(value),
        })));
    }

    None
}

#[derive(Clone, Copy)]
struct IntegerComparison<'expression> {
    subject: &'expression Expression,
    operator: omega_checked_trees::expression::BinaryOperator,
    value: i64,
}

fn parse_integer_comparison(expression: &Expression) -> Option<IntegerComparison<'_>> {
    let Expression::Binary(binary) = expression else {
        return None;
    };

    let operator = match binary.operator {
        omega_checked_trees::expression::BinaryOperator::Greater
        | omega_checked_trees::expression::BinaryOperator::GreaterOrEqual
        | omega_checked_trees::expression::BinaryOperator::Less
        | omega_checked_trees::expression::BinaryOperator::LessOrEqual => binary.operator,
        _ => return None,
    };

    if let Expression::Integer(value) = &binary.right {
        return Some(IntegerComparison {
            subject: &binary.left,
            operator,
            value: *value,
        });
    }

    if let Expression::Integer(value) = &binary.left {
        let flipped_operator = match binary.operator {
            omega_checked_trees::expression::BinaryOperator::Greater => {
                omega_checked_trees::expression::BinaryOperator::Less
            }
            omega_checked_trees::expression::BinaryOperator::GreaterOrEqual => {
                omega_checked_trees::expression::BinaryOperator::LessOrEqual
            }
            omega_checked_trees::expression::BinaryOperator::Less => {
                omega_checked_trees::expression::BinaryOperator::Greater
            }
            omega_checked_trees::expression::BinaryOperator::LessOrEqual => {
                omega_checked_trees::expression::BinaryOperator::GreaterOrEqual
            }
            _ => unreachable!(),
        };

        return Some(IntegerComparison {
            subject: &binary.right,
            operator: flipped_operator,
            value: *value,
        });
    }

    None
}

fn tighten_lower_bound(
    current: Option<(i64, bool)>,
    candidate_value: i64,
    candidate_inclusive: bool,
) -> Option<(i64, bool)> {
    match current {
        None => Some((candidate_value, candidate_inclusive)),
        Some((current_value, current_inclusive)) => {
            if candidate_value > current_value {
                Some((candidate_value, candidate_inclusive))
            } else if candidate_value < current_value {
                Some((current_value, current_inclusive))
            } else {
                Some((current_value, current_inclusive && candidate_inclusive))
            }
        }
    }
}

fn tighten_upper_bound(
    current: Option<(i64, bool)>,
    candidate_value: i64,
    candidate_inclusive: bool,
) -> Option<(i64, bool)> {
    match current {
        None => Some((candidate_value, candidate_inclusive)),
        Some((current_value, current_inclusive)) => {
            if candidate_value < current_value {
                Some((candidate_value, candidate_inclusive))
            } else if candidate_value > current_value {
                Some((current_value, current_inclusive))
            } else {
                Some((current_value, current_inclusive && candidate_inclusive))
            }
        }
    }
}

fn boolean_not(expression: Expression) -> Expression {
    use omega_checked_trees::expression::BinaryOperator as Op;

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
                Op::Add
                | Op::Divide
                | Op::Modulo
                | Op::Multiply
                | Op::ShiftLeft
                | Op::ShiftRight
                | Op::Subtract => None,
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
            operator: omega_checked_trees::expression::BinaryOperator::Equal,
            right: Expression::Boolean(false),
        })),
    }
}

fn expressions_equivalent(left: &Expression, right: &Expression) -> bool {
    if let (Some(left_path), Some(right_path)) = (
        expression_path_segments(left),
        expression_path_segments(right),
    ) {
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
            left.member == right.member && expressions_equivalent(&left.receiver, &right.receiver)
        }
        (Expression::Mutable(left), Expression::Mutable(right)) => {
            expressions_equivalent(left, right)
        }
        (Expression::Name(left), Expression::Name(right)) => left.members() == right.members(),
        (Expression::StructLiteral(left), Expression::StructLiteral(right)) => {
            left.type_name == right.type_name
                && left.fields.len() == right.fields.len()
                && left
                    .fields
                    .iter()
                    .zip(right.fields.iter())
                    .all(|(left, right)| {
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
    use omega_checked_trees::Program;
    use omega_checked_trees::expression::{
        BinaryExpression, BinaryOperator, CallExpression, Expression, NamePath,
    };
    use omega_checked_trees::machine::Machine;
    use omega_checked_trees::name::ProgramName;
    use omega_checked_trees::signature::StateParameter;
    use omega_checked_trees::state::State;
    use omega_checked_trees::statement::{
        LocalData, Statement, Transition, TransitionGuard, TransitionTarget,
    };
    use omega_checked_trees::types::TypeReference;
    use omega_core::symbols::SymbolHandle;

    #[test]
    fn simplifies_guarded_helper_call_comparison_to_guard_expression() {
        let machine_symbol = SymbolHandle::from_arena_index(1);
        let helper_symbol = SymbolHandle::from_arena_index(2);
        let roll_symbol = SymbolHandle::from_arena_index(3);
        let is_quiet_symbol = SymbolHandle::from_arena_index(4);
        let is_fountain_symbol = SymbolHandle::from_arena_index(5);
        let is_reward_symbol = SymbolHandle::from_arena_index(6);

        let mut helper = State {
            symbol: helper_symbol,
            name: "event_action".into(),
            parameters: Default::default(),
            return_type: None,
            statement_nodes: Default::default(),
        };

        let mut machine = Machine {
            symbol: machine_symbol,
            name: "RoomEvents".into(),
            contains: Default::default(),
            owned_data: Default::default(),
            states: Default::default(),
        };
        let mut program = Program::default();
        push_state_statements(
            &mut program,
            &mut helper,
            [
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
                Statement::LocalData(LocalData {
                    symbol: is_reward_symbol,
                    name: "is_reward".into(),
                    type_reference: TypeReference::Named {
                        symbol: SymbolHandle::invalid(),
                        name: "bool".into(),
                    },
                    initial_value: Some(Expression::Binary(Box::new(BinaryExpression {
                        left: name("roll", roll_symbol),
                        operator: BinaryOperator::Less,
                        right: Expression::Integer(60),
                    }))),
                }),
                Statement::Transition(Transition {
                    target: TransitionTarget::Value(path_expression(&["RoomEventAction", "Quiet"])),
                    continuation: None,
                    guard: TransitionGuard::When(name("is_quiet", is_quiet_symbol)),
                }),
                Statement::Transition(Transition {
                    target: TransitionTarget::Value(path_expression(&[
                        "RoomEventAction",
                        "Fountain",
                    ])),
                    continuation: None,
                    guard: TransitionGuard::When(name("is_fountain", is_fountain_symbol)),
                }),
                Statement::Transition(Transition {
                    target: TransitionTarget::Value(path_expression(&[
                        "RoomEventAction",
                        "Reward",
                    ])),
                    continuation: None,
                    guard: TransitionGuard::When(name("is_reward", is_reward_symbol)),
                }),
                Statement::Transition(Transition {
                    target: TransitionTarget::Value(path_expression(&["RoomEventAction", "Enemy"])),
                    continuation: None,
                    guard: TransitionGuard::Always,
                }),
            ],
        );
        program.typed.push_state_parameter(
            &mut helper,
            StateParameter {
                symbol: roll_symbol,
                name: "roll".into(),
                type_reference: TypeReference::Named {
                    symbol: SymbolHandle::invalid(),
                    name: "u32".into(),
                },
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

        let reward_guard = Expression::Binary(Box::new(BinaryExpression {
            left: Expression::Call(Box::new(CallExpression {
                receiver: None,
                target_symbol: helper_symbol,
                target: "event_action".into(),
                arguments: vec![name("roll", roll_symbol)],
            })),
            operator: BinaryOperator::Equal,
            right: path_expression(&["RoomEventAction", "Reward"]),
        }));

        let enemy_guard = Expression::Binary(Box::new(BinaryExpression {
            left: Expression::Call(Box::new(CallExpression {
                receiver: None,
                target_symbol: helper_symbol,
                target: "event_action".into(),
                arguments: vec![name("roll", roll_symbol)],
            })),
            operator: BinaryOperator::Equal,
            right: path_expression(&["RoomEventAction", "Enemy"]),
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
            contains: Default::default(),
            owned_data: Default::default(),
            states: Default::default(),
        };
        let program = Program::default();
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
    fn simplifies_option_is_some_over_non_recursive_helper() {
        let machine_symbol = SymbolHandle::from_arena_index(20);
        let find_symbol = SymbolHandle::from_arena_index(21);
        let found_symbol = SymbolHandle::from_arena_index(22);

        let mut find = State {
            symbol: find_symbol,
            name: "find_item".into(),
            parameters: Default::default(),
            return_type: None,
            statement_nodes: Default::default(),
        };

        let mut machine = Machine {
            symbol: machine_symbol,
            name: "InventorySystem".into(),
            contains: Default::default(),
            owned_data: Default::default(),
            states: Default::default(),
        };
        let mut program = Program::default();
        push_state_statements(
            &mut program,
            &mut find,
            [
                Statement::Transition(Transition {
                    target: TransitionTarget::Value(Expression::Integer(1)),
                    continuation: None,
                    guard: TransitionGuard::When(name("found", found_symbol)),
                }),
                Statement::Transition(Transition {
                    target: TransitionTarget::Value(path_expression(&["None"])),
                    continuation: None,
                    guard: TransitionGuard::Always,
                }),
            ],
        );
        program.typed.push_state_parameter(
            &mut find,
            StateParameter {
                symbol: found_symbol,
                name: "found".into(),
                type_reference: TypeReference::Named {
                    symbol: SymbolHandle::invalid(),
                    name: "bool".into(),
                },
                is_const: true,
                is_mutable: false,
                is_self: false,
            },
        );
        program.typed.push_machine_state(&mut machine, find);
        program.typed.push_machine(machine.clone());

        let is_some_guard = Expression::Call(Box::new(CallExpression {
            receiver: Some(Box::new(Expression::Call(Box::new(CallExpression {
                receiver: None,
                target_symbol: find_symbol,
                target: "find_item".into(),
                arguments: vec![name("found", found_symbol)],
            })))),
            target_symbol: SymbolHandle::invalid(),
            target: "is_some".into(),
            arguments: vec![],
        }));

        assert_eq!(
            simplify_expression(&program, &machine, &is_some_guard),
            Expression::Binary(Box::new(BinaryExpression {
                left: name("found", found_symbol),
                operator: BinaryOperator::NotEqual,
                right: Expression::Boolean(false),
            }))
        );
    }

    fn push_state_statements<const N: usize>(
        program: &mut Program,
        state: &mut State,
        statements: [Statement; N],
    ) {
        let source_statement_expressions = omega_core::arena::Arena::new();
        let source_statement_path_members = omega_core::arena::Arena::new();

        for statement in statements {
            let statement_node = program.typed.statement_table.insert_tree(
                &statement,
                &mut program.typed.expression_table,
                &mut program.typed.type_reference_table,
                &program.typed.type_constraints,
                &program.typed.type_reference_arguments,
                &source_statement_expressions,
                &source_statement_path_members,
            );
            state.statement_nodes.push_contiguous(statement_node);
        }
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
