use crate::StateValueRole;
use omega_checked_trees::CheckedTrees;
use omega_checked_trees::expression::{
    BinaryExpression, CallExpression, Expression, ExpressionHandle, ExpressionNode,
    ExpressionTable, IndexedExpression, MemberExpression, NamePath, StructLiteral,
    StructLiteralField,
};
use omega_checked_trees::machine::Machine;
use omega_checked_trees::state::State;
use omega_checked_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use omega_core::arena::Arena;
use omega_core::symbols::SymbolHandle;
use std::sync::Arc;

const INLINE_HELPER_STATE_STACK_COUNT: usize = 16;

struct HelperStateStack {
    inline: [Option<SymbolHandle>; INLINE_HELPER_STATE_STACK_COUNT],
    len: usize,
    overflow: Vec<SymbolHandle>,
}

impl HelperStateStack {
    fn with_capacity(state_capacity: usize) -> Self {
        Self {
            inline: [None; INLINE_HELPER_STATE_STACK_COUNT],
            len: 0,
            overflow: Vec::with_capacity(
                state_capacity.saturating_sub(INLINE_HELPER_STATE_STACK_COUNT),
            ),
        }
    }

    fn contains(&self, symbol: SymbolHandle) -> bool {
        self.inline
            .iter()
            .take(self.len.min(INLINE_HELPER_STATE_STACK_COUNT))
            .flatten()
            .any(|candidate| *candidate == symbol)
            || self.overflow.contains(&symbol)
    }

    fn push(&mut self, symbol: SymbolHandle) {
        if self.len < INLINE_HELPER_STATE_STACK_COUNT {
            self.inline[self.len] = Some(symbol);
        } else {
            self.overflow.push(symbol);
        }

        self.len += 1;
    }

    fn pop(&mut self) {
        if self.len == 0 {
            return;
        }

        self.len -= 1;
        if self.len < INLINE_HELPER_STATE_STACK_COUNT {
            self.inline[self.len] = None;
        } else {
            self.overflow.pop();
        }
    }
}

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
    name: omega_checked_trees::name::Identifier,
    value: Expression,
}

impl Default for Binding {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: omega_checked_trees::name::Identifier::generated_static(""),
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

impl BindingScope for Arena<Binding> {
    fn find_path_binding(&self, path: &NamePath) -> Option<&Binding> {
        self.iter()
            .map(|(_, binding)| binding)
            .find(|binding| binding_matches_path(binding, path))
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
    if binding.symbol.is_valid() && path.head_symbol().is_valid() {
        return binding.symbol == path.head_symbol();
    }

    path.first().is_some_and(|name| *name == binding.name)
}

fn simple_local_bindings(
    program: &CheckedTrees,
    state: &State,
    statement_index: usize,
) -> Arena<Binding> {
    let local_binding_capacity = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
        .filter(|statement| {
            matches!(
                statement,
                StatementNode::LocalData(local_data) if local_data.initial_value.is_valid()
            )
        })
        .count();
    let mut bindings = Arena::with_capacity(local_binding_capacity);

    for statement in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
    {
        let StatementNode::LocalData(local_data) = statement else {
            continue;
        };
        if !local_data.initial_value.is_valid() {
            continue;
        }
        let Some(value) = simple_local_binding_value_from_table(
            &program.expression_table,
            local_data.initial_value,
        ) else {
            continue;
        };
        bindings.insert(Binding {
            symbol: local_data.symbol,
            name: local_data.name.clone(),
            value,
        });
    }

    bindings
}

fn simple_local_binding_value_from_table(
    table: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<Expression> {
    match table.expression(expression) {
        ExpressionNode::Binary(binary) => Some(Expression::Binary(Box::new(BinaryExpression {
            left: simple_local_binding_value_from_table(table, binary.left)?,
            operator: binary.operator,
            right: simple_local_binding_value_from_table(table, binary.right)?,
        }))),
        ExpressionNode::Boolean(value) => Some(Expression::Boolean(*value)),
        ExpressionNode::Float(value) => Some(Expression::Float(*value)),
        ExpressionNode::Integer(value) => Some(Expression::Integer(*value)),
        ExpressionNode::String(value) => Some(Expression::String(value.clone())),
        ExpressionNode::Indexed(indexed) => {
            Some(Expression::Indexed(Box::new(IndexedExpression {
                collection: simple_local_binding_value_from_table(table, indexed.collection)?,
                index: simple_local_binding_value_from_table(table, indexed.index)?,
            })))
        }
        ExpressionNode::Call(call) => Some(Expression::Call(Box::new(CallExpression {
            receiver: call.receiver.is_valid().then(|| {
                simple_local_binding_value_from_table(table, call.receiver).map(Box::new)
            })?,
            target_symbol: call.target_symbol,
            target: call.target.clone(),
            arguments: table
                .expression_handles(call.arguments)
                .iter()
                .map(|argument| simple_local_binding_value_from_table(table, *argument))
                .collect::<Option<Arc<[_]>>>()?,
        }))),
        ExpressionNode::Mutable(inner) => simple_local_binding_value_from_table(table, *inner)
            .map(|value| Expression::Mutable(Box::new(value))),
        ExpressionNode::Name(path) => {
            Some(Expression::Name(NamePath::resolved_with_member_symbols(
                table.name_path_members(path.members).to_vec(),
                table.name_path_member_symbols(path.member_symbols).to_vec(),
                path.head_symbol,
                path.symbol,
            )))
        }
        ExpressionNode::Member(member) => {
            let receiver = simple_local_binding_value_from_table(table, member.receiver)?;
            Some(Expression::Member(Box::new(MemberExpression {
                receiver,
                member_symbol: member.member_symbol,
                member: member.member.clone(),
            })))
        }
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::StructLiteral(_) => None,
    }
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

fn resolve_call_target_machine<'program>(
    program: &'program CheckedTrees,
    current_machine: &'program Machine,
    receiver: Option<&Expression>,
    target_symbol: SymbolHandle,
) -> Option<&'program Machine> {
    let Some(receiver) = receiver else {
        return machine_owning_state_symbol(program, target_symbol).or(Some(current_machine));
    };
    let receiver = strip_mutable_expression_ref(receiver);
    if expression_is_self_reference(current_machine, receiver) {
        if let Some(target_machine) = machine_owning_state_symbol(program, target_symbol)
            && machine_can_receive_self_call(current_machine, target_machine)
        {
            return Some(target_machine);
        }
        return Some(current_machine);
    }

    let contained_symbol = match receiver {
        Expression::Member(member)
            if expression_is_self_reference(current_machine, &member.receiver) =>
        {
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

fn machine_owning_state_symbol<'program>(
    program: &'program CheckedTrees,
    state_symbol: SymbolHandle,
) -> Option<&'program Machine> {
    if !state_symbol.is_valid() {
        return None;
    }

    program.machines().iter().find(|machine| {
        program
            .machine_states(machine)
            .iter()
            .any(|state| state.symbol == state_symbol)
    })
}

fn machine_can_receive_self_call(current: &Machine, target: &Machine) -> bool {
    current.symbol == target.symbol
        || (current.attached_data.is_some() && current.attached_data == target.attached_data)
}

fn strip_mutable_expression_ref(mut expression: &Expression) -> &Expression {
    while let Expression::Mutable(inner) = expression {
        expression = inner.as_ref();
    }
    expression
}

fn resolve_call_target_state<'machine>(
    program: &'machine CheckedTrees,
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

fn expression_is_self_reference(machine: &Machine, expression: &Expression) -> bool {
    match expression {
        Expression::Mutable(inner) => expression_is_self_reference(machine, inner),
        Expression::Name(path) => path.len() == 1 && path.symbol() == machine.symbol,
        _ => false,
    }
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

fn append_name_suffix(
    base: &Expression,
    suffix: &[omega_checked_trees::name::Identifier],
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
        Op::Equal | Op::NotEqual => {
            if let Expression::Boolean(flag) = right {
                let positive = matches!(operator, Op::Equal) == flag;
                return if positive { left } else { boolean_not(left) };
            }
            if let Expression::Boolean(flag) = left {
                let positive = matches!(operator, Op::Equal) == flag;
                return if positive { right } else { boolean_not(right) };
            }
            match operator {
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
                _ => unreachable!(),
            }
        }
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
    if let Some(simplified) = simplify_comparison_disjunction(&left, &right) {
        return simplified;
    }

    if let Some(factored) = factor_common_conjuncts(&left, &right) {
        return factored;
    }

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

fn factor_common_conjuncts(left: &Expression, right: &Expression) -> Option<Expression> {
    let mut left_conjuncts = Vec::new();
    let mut right_conjuncts = Vec::new();
    collect_conjuncts(left, &mut left_conjuncts);
    collect_conjuncts(right, &mut right_conjuncts);

    let mut common = Vec::new();
    let mut remaining_right = right_conjuncts;

    for left_conjunct in left_conjuncts {
        if let Some(index) = remaining_right
            .iter()
            .position(|candidate| expressions_equivalent(left_conjunct, candidate))
        {
            common.push(left_conjunct.clone());
            remaining_right.remove(index);
        }
    }

    if common.is_empty() {
        return None;
    }

    let mut remaining_left = Vec::new();
    collect_unique_non_common_conjuncts(left, &common, &mut remaining_left);

    let left_rest = combine_conjuncts(remaining_left);
    let right_rest = combine_conjuncts(remaining_right.into_iter().cloned().collect::<Vec<_>>());
    let mut factored = boolean_or(left_rest, right_rest);
    for conjunct in common.into_iter().rev() {
        factored = boolean_and(conjunct, factored);
    }
    Some(factored)
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

fn simplify_comparison_disjunction(left: &Expression, right: &Expression) -> Option<Expression> {
    let left_compare = parse_integer_comparison(left)?;
    let right_compare = parse_integer_comparison(right)?;

    if !expressions_equivalent(left_compare.subject, right_compare.subject) {
        return None;
    }

    if left_compare.operator == right_compare.operator && left_compare.value == right_compare.value
    {
        return Some(left.clone());
    }

    use omega_checked_trees::expression::BinaryOperator as Op;
    match (left_compare.operator, right_compare.operator) {
        (Op::Greater, Op::Greater)
        | (Op::Greater, Op::GreaterOrEqual)
        | (Op::GreaterOrEqual, Op::Greater)
        | (Op::GreaterOrEqual, Op::GreaterOrEqual) => {
            let (value, inclusive) = loosen_lower_bound_for_disjunction(
                (
                    left_compare.value,
                    left_compare.operator == Op::GreaterOrEqual,
                ),
                (
                    right_compare.value,
                    right_compare.operator == Op::GreaterOrEqual,
                ),
            );
            Some(Expression::Binary(Box::new(BinaryExpression {
                left: left_compare.subject.clone(),
                operator: if inclusive {
                    Op::GreaterOrEqual
                } else {
                    Op::Greater
                },
                right: Expression::Integer(value),
            })))
        }
        (Op::Less, Op::Less)
        | (Op::Less, Op::LessOrEqual)
        | (Op::LessOrEqual, Op::Less)
        | (Op::LessOrEqual, Op::LessOrEqual) => {
            let (value, inclusive) = loosen_upper_bound_for_disjunction(
                (left_compare.value, left_compare.operator == Op::LessOrEqual),
                (
                    right_compare.value,
                    right_compare.operator == Op::LessOrEqual,
                ),
            );
            Some(Expression::Binary(Box::new(BinaryExpression {
                left: left_compare.subject.clone(),
                operator: if inclusive { Op::LessOrEqual } else { Op::Less },
                right: Expression::Integer(value),
            })))
        }
        (Op::Greater, Op::LessOrEqual)
        | (Op::LessOrEqual, Op::Greater)
        | (Op::GreaterOrEqual, Op::Less)
        | (Op::Less, Op::GreaterOrEqual) => {
            if comparisons_cover_all_integers(left_compare, right_compare) {
                Some(Expression::Boolean(true))
            } else {
                None
            }
        }
        _ => None,
    }
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

fn loosen_lower_bound_for_disjunction(left: (i64, bool), right: (i64, bool)) -> (i64, bool) {
    match left.0.cmp(&right.0) {
        std::cmp::Ordering::Less => left,
        std::cmp::Ordering::Greater => right,
        std::cmp::Ordering::Equal => (left.0, left.1 || right.1),
    }
}

fn loosen_upper_bound_for_disjunction(left: (i64, bool), right: (i64, bool)) -> (i64, bool) {
    match left.0.cmp(&right.0) {
        std::cmp::Ordering::Less => right,
        std::cmp::Ordering::Greater => left,
        std::cmp::Ordering::Equal => (left.0, left.1 || right.1),
    }
}

fn comparisons_cover_all_integers(
    left: IntegerComparison<'_>,
    right: IntegerComparison<'_>,
) -> bool {
    use omega_checked_trees::expression::BinaryOperator as Op;

    let (lower, upper) = match (left.operator, right.operator) {
        (Op::Greater, Op::LessOrEqual)
        | (Op::GreaterOrEqual, Op::Less)
        | (Op::Greater, Op::Less)
        | (Op::GreaterOrEqual, Op::LessOrEqual) => (left, right),
        (Op::LessOrEqual, Op::Greater)
        | (Op::Less, Op::GreaterOrEqual)
        | (Op::Less, Op::Greater)
        | (Op::LessOrEqual, Op::GreaterOrEqual) => (right, left),
        _ => return false,
    };

    let lower_min = match lower.operator {
        Op::Greater => lower.value + 1,
        Op::GreaterOrEqual => lower.value,
        _ => return false,
    };
    let upper_max = match upper.operator {
        Op::Less => upper.value - 1,
        Op::LessOrEqual => upper.value,
        _ => return false,
    };
    lower_min <= upper_max + 1
}

fn collect_conjuncts<'a>(expression: &'a Expression, output: &mut Vec<&'a Expression>) {
    if let Expression::Binary(binary) = expression
        && binary.operator == omega_checked_trees::expression::BinaryOperator::And
    {
        collect_conjuncts(&binary.left, output);
        collect_conjuncts(&binary.right, output);
        return;
    }
    output.push(expression);
}

fn collect_unique_non_common_conjuncts(
    expression: &Expression,
    common: &[Expression],
    output: &mut Vec<Expression>,
) {
    let mut conjuncts = Vec::new();
    collect_conjuncts(expression, &mut conjuncts);
    let mut remaining_common = common.to_vec();
    for conjunct in conjuncts {
        if let Some(index) = remaining_common
            .iter()
            .position(|candidate| expressions_equivalent(conjunct, candidate))
        {
            remaining_common.remove(index);
        } else {
            output.push(conjunct.clone());
        }
    }
}

fn combine_conjuncts(conjuncts: Vec<Expression>) -> Expression {
    conjuncts
        .into_iter()
        .reduce(boolean_and)
        .unwrap_or(Expression::Boolean(true))
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
    if let Some(are_equivalent) = expression_paths_equivalent(left, right) {
        return are_equivalent;
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
            left.target_type.symbol().is_valid()
                && right.target_type.symbol().is_valid()
                && left.target_type.symbol() == right.target_type.symbol()
                && expressions_equivalent(&left.value, &right.value)
        }
        (Expression::Float(left), Expression::Float(right)) => left == right,
        (Expression::Indexed(left), Expression::Indexed(right)) => {
            expressions_equivalent(&left.collection, &right.collection)
                && expressions_equivalent(&left.index, &right.index)
        }
        (Expression::Integer(left), Expression::Integer(right)) => left == right,
        (Expression::Member(left), Expression::Member(right)) => {
            ((left.member_symbol.is_valid()
                && right.member_symbol.is_valid()
                && left.member_symbol == right.member_symbol)
                || (!left.member_symbol.is_valid()
                    && !right.member_symbol.is_valid()
                    && left.member == right.member))
                && expressions_equivalent(&left.receiver, &right.receiver)
        }
        (Expression::Mutable(left), Expression::Mutable(right)) => {
            expressions_equivalent(left, right)
        }
        (Expression::Name(left), Expression::Name(right)) => {
            left.symbol().is_valid() && right.symbol().is_valid() && left.symbol() == right.symbol()
        }
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

fn expression_paths_equivalent(left: &Expression, right: &Expression) -> Option<bool> {
    let left_count = expression_path_segment_count(left)?;
    let right_count = expression_path_segment_count(right)?;

    if left_count != right_count {
        return Some(false);
    }

    Some((0..left_count).all(|index| {
        let left_symbol = expression_path_segment_symbol(left, index);
        let right_symbol = expression_path_segment_symbol(right, index);
        left_symbol.is_valid() && right_symbol.is_valid() && left_symbol == right_symbol
    }))
}

fn expression_path_segment_count(expression: &Expression) -> Option<usize> {
    match expression {
        Expression::Name(path) => Some(path.len()),
        Expression::Member(member) => Some(expression_path_segment_count(&member.receiver)? + 1),
        _ => None,
    }
}

fn expression_path_segment_symbol(expression: &Expression, index: usize) -> SymbolHandle {
    match expression {
        Expression::Name(path) => path.member_symbol(index),
        Expression::Member(member) => {
            let Some(receiver_count) = expression_path_segment_count(&member.receiver) else {
                return SymbolHandle::invalid();
            };
            if index == receiver_count {
                member.member_symbol
            } else {
                expression_path_segment_symbol(&member.receiver, index)
            }
        }
        _ => SymbolHandle::invalid(),
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
