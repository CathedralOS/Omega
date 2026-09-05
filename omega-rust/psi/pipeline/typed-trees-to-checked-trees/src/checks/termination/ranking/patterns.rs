use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

pub(super) struct GuardedSelfLoop<'program> {
    pub(super) guard: ExpressionHandle,
    pub(super) arguments: &'program [ExpressionHandle],
}

pub(super) fn guarded_self_loop<'program>(
    program: &'program typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    statement: &StatementNode,
) -> Option<GuardedSelfLoop<'program>> {
    let StatementNode::Transition(transition) = statement else {
        return None;
    };
    let TransitionGuardNode::When(guard) = transition.guard else {
        return None;
    };
    let target = program.statement_table.transition_target(transition.target);
    let TransitionTargetNode::Named {
        path, arguments, ..
    } = target
    else {
        return None;
    };
    if !target_symbol_matches_state(program, state, path.symbol) {
        return None;
    }

    Some(GuardedSelfLoop {
        guard,
        arguments: program.statement_table.expression_handles(*arguments),
    })
}

/// A fact at one exact edge, including failed earlier tests.
#[derive(Clone, Copy)]
pub(super) struct GuardFact {
    pub(super) expression: ExpressionHandle,
    pub(super) holds: bool,
}

pub(super) struct GuardedEdge<'program> {
    pub(super) statement_ordinal: usize,
    pub(super) is_continuation: bool,
    pub(super) guards: Vec<GuardFact>,
    pub(super) arguments: &'program [ExpressionHandle],
}

/// Collect every occurrence, not merely the first matching arm. An Always
/// fallback retains failed earlier tests; a continuation refutes its own test.
/// Intervening statements discard earlier facts instead of assuming no write.
pub(super) fn edges_to_state<'program>(
    program: &'program typed_trees::TypedTrees,
    source: &typed_trees::state::State,
    target_symbol: symbols::SymbolHandle,
) -> Vec<GuardedEdge<'program>> {
    let mut edges = Vec::new();
    let mut previous = Vec::new();
    for (statement_ordinal, statement) in program
        .statement_table
        .statements(source.statement_nodes)
        .iter()
        .enumerate()
    {
        let StatementNode::Transition(transition) = statement else {
            previous.clear();
            continue;
        };
        let guard = match transition.guard {
            TransitionGuardNode::When(expression) => Some(expression),
            TransitionGuardNode::Always => None,
        };
        if guard.is_some_and(|guard| !stable_guard(program, source, guard)) {
            previous.clear();
        }
        for (target_handle, is_continuation) in
            [(transition.target, false), (transition.continuation, true)]
        {
            if !target_handle.is_valid() {
                continue;
            }
            let TransitionTargetNode::Named {
                path, arguments, ..
            } = program.statement_table.transition_target(target_handle)
            else {
                continue;
            };
            if !target_symbol_matches_state_symbol(program, target_symbol, path.symbol) {
                continue;
            }
            let mut guards = previous.clone();
            if let Some(expression) = guard {
                guards.push(GuardFact {
                    expression,
                    holds: !is_continuation,
                });
            }
            edges.push(GuardedEdge {
                statement_ordinal,
                is_continuation,
                guards,
                arguments: program.statement_table.expression_handles(*arguments),
            });
        }
        if let Some(expression) = guard
            && transition.target.is_valid()
            && !transition.continuation.is_valid()
            && stable_guard(program, source, expression)
        {
            previous.push(GuardFact {
                expression,
                holds: false,
            });
        } else {
            previous.clear();
        }
    }
    edges
}

/// Only immutable primitive parameters and literal Boolean/comparison structure
/// supply facts across statements. Calls, storage reads, and projections are not
/// assumed to retain their observed value.
fn stable_guard(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
) -> bool {
    use typed_trees::expression::{BinaryOperator, UnaryOperator};
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(name) => program.state_parameters(state).iter().any(|parameter| {
            parameter.symbol == name.symbol
                && !parameter.is_mutable
                && !parameter.is_self
                && program
                    .primitive_type_reference(parameter.type_reference)
                    .is_some()
        }),
        ExpressionNode::Boolean(_) | ExpressionNode::Integer(_) => true,
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            stable_guard(program, state, unary.operand)
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
            ) =>
        {
            stable_guard(program, state, binary.left) && stable_guard(program, state, binary.right)
        }
        _ => false,
    }
}

fn target_symbol_matches_state(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    target_symbol: symbols::SymbolHandle,
) -> bool {
    target_symbol_matches_state_symbol(program, state.symbol, target_symbol)
}

fn target_symbol_matches_state_symbol(
    program: &typed_trees::TypedTrees,
    state_symbol: symbols::SymbolHandle,
    target_symbol: symbols::SymbolHandle,
) -> bool {
    if target_symbol == state_symbol {
        return true;
    }
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == target_symbol)
    else {
        return false;
    };
    let entry_name = machine
        .name
        .as_str()
        .rsplit("::")
        .next()
        .unwrap_or_default();
    program
        .machine_states(machine)
        .iter()
        .find(|candidate| candidate.name.as_str() == entry_name)
        .or_else(|| program.machine_states(machine).first())
        .is_some_and(|entry| entry.symbol == state_symbol)
}

pub(super) fn parameter_matched_by_expression<'program>(
    program: &'program typed_trees::TypedTrees,
    state: &'program typed_trees::state::State,
    expression: ExpressionHandle,
) -> Option<&'program typed_trees::signature::StateParameter> {
    program.state_parameters(state).iter().find(|parameter| {
        !parameter.is_self && expression_matches_parameter(program, expression, parameter)
    })
}

pub(super) fn parameter_and_argument_index_matched_by_expression<'program>(
    program: &'program typed_trees::TypedTrees,
    state: &'program typed_trees::state::State,
    expression: ExpressionHandle,
) -> Option<(&'program typed_trees::signature::StateParameter, usize)> {
    program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .enumerate()
        .find_map(|(index, parameter)| {
            expression_matches_parameter(program, expression, parameter)
                .then_some((parameter, index))
        })
}

pub(super) fn non_self_parameter_index(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    parameter: &typed_trees::signature::StateParameter,
) -> Option<usize> {
    program
        .state_parameters(state)
        .iter()
        .filter(|candidate| !candidate.is_self)
        .position(|candidate| candidate.symbol == parameter.symbol)
}

pub(super) fn normalize_boolean_guard(
    program: &typed_trees::TypedTrees,
    guard: ExpressionHandle,
) -> ExpressionHandle {
    // An unguarded edge carries no positive expression fact.
    if !guard.is_valid() {
        return guard;
    }
    match program.expression_table.expression(guard) {
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                typed_trees::expression::BinaryOperator::Equal
            ) && matches!(
                program.expression_table.expression(binary.right),
                ExpressionNode::Boolean(true)
            ) =>
        {
            binary.left
        }
        _ => guard,
    }
}

pub(super) fn expression_is_parameter(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
    parameter: &typed_trees::signature::StateParameter,
) -> bool {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return false;
    };

    path.symbol == parameter.symbol
        || program
            .expression_table
            .name_path_members(path.members)
            .last()
            .is_some_and(|member| member.as_str() == parameter.name.as_str())
}

pub(super) fn expression_is_parameter_member(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
    parameter: &typed_trees::signature::StateParameter,
    member_name: &str,
) -> bool {
    matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Member(member)
            if member.member.as_str() == member_name
                && expression_is_parameter(program, member.receiver, parameter)
    )
}

pub(super) fn expression_matches_parameter(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
    parameter: &typed_trees::signature::StateParameter,
) -> bool {
    expression_is_parameter(program, expression, parameter)
        || matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Member(member)
                if member.member.as_str() == "len"
                    && expression_is_parameter(program, member.receiver, parameter)
        )
}
