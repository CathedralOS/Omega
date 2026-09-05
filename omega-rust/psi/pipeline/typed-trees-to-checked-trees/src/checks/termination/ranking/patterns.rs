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

/// An UNGUARDED (always) self-loop transition -- the shape the MR2
/// terminal-tail rewrite produces (`{ _ -> countdown(n - 1) }` as the
/// state's fall-through). Its edge facts come from the COMPLEMENTS of the
/// guarded EXIT transitions before it (see `fall_through_exit_guards`),
/// not from its own (absent) guard.
pub(super) struct FallThroughSelfLoop<'program> {
    pub(super) arguments: &'program [ExpressionHandle],
    /// Guards of PRIOR exit transitions: control only reaches the loop when
    /// every one of these was FALSE.
    pub(super) refuted_exit_guards: Vec<ExpressionHandle>,
}

/// Match the state's Always-guarded self-loop reached by FALL-THROUGH, with
/// the prior exit guards whose complements dominate it. A prior transition
/// counts as an EXIT when it is guarded, its true-arm leaves (any valid
/// target), and it has NO fall-through arm of its own (invalid
/// continuation): reaching a later statement then proves the guard false.
pub(super) fn fall_through_self_loop<'program>(
    program: &'program typed_trees::TypedTrees,
    state: &typed_trees::state::State,
) -> Option<FallThroughSelfLoop<'program>> {
    let statements = program.statement_table.statements(state.statement_nodes);
    let mut refuted_exit_guards = Vec::new();
    for statement in statements {
        let StatementNode::Transition(transition) = statement else {
            continue;
        };
        match transition.guard {
            TransitionGuardNode::When(guard) => {
                if transition.target.is_valid() && !transition.continuation.is_valid() {
                    refuted_exit_guards.push(guard);
                }
                continue;
            }
            TransitionGuardNode::Always => {}
        }
        let target = program.statement_table.transition_target(transition.target);
        let TransitionTargetNode::Named {
            path, arguments, ..
        } = target
        else {
            continue;
        };
        if !target_symbol_matches_state(program, state, path.symbol) {
            continue;
        }
        return Some(FallThroughSelfLoop {
            arguments: program.statement_table.expression_handles(*arguments),
            refuted_exit_guards,
        });
    }
    None
}

/// Does refuting `guard` prove `parameter > 0`? True for the base-case
/// spellings `param == 0`, `param < 1`, and `param <= 0` (unsigned or not:
/// the refutation gives param != 0 / param >= 1 / param >= 1, and the Nat
/// ranking only fires for parameters with a well-founded non-negative
/// order).
pub(super) fn refuted_guard_proves_positive(
    program: &typed_trees::TypedTrees,
    guard: ExpressionHandle,
    parameter: &typed_trees::signature::StateParameter,
) -> bool {
    use typed_trees::expression::BinaryOperator;
    let normalized = normalize_boolean_guard(program, guard);
    let ExpressionNode::Binary(binary) = program.expression_table.expression(normalized) else {
        return false;
    };
    if !expression_is_parameter(program, binary.left, parameter) {
        return false;
    }
    let ExpressionNode::Integer(literal) = program.expression_table.expression(binary.right) else {
        return false;
    };
    match binary.operator {
        BinaryOperator::Equal => literal.value_i64() == Some(0),
        BinaryOperator::Less => literal.value_i64() == Some(1),
        BinaryOperator::LessOrEqual => literal.value_i64() == Some(0),
        _ => false,
    }
}

/// A guarded transition edge whose target is a specific state (used for
/// cyclic / mutually-recursive edges where the target differs from the source).
pub(super) struct GuardedEdge<'program> {
    pub(super) guard: ExpressionHandle,
    pub(super) arguments: &'program [ExpressionHandle],
}

/// Match ANY transition edge (guarded or Always) from a statement to
/// `target_symbol`. The Always case (the MR2 terminal-tail rewrite's shape)
/// carries an INVALID guard handle -- provers must not read it as a fact.
pub(super) fn edge_to_any_guard<'program>(
    program: &'program typed_trees::TypedTrees,
    statement: &StatementNode,
    target_symbol: symbols::SymbolHandle,
) -> Option<GuardedEdge<'program>> {
    let StatementNode::Transition(transition) = statement else {
        return None;
    };
    let guard = match transition.guard {
        TransitionGuardNode::When(guard) => guard,
        TransitionGuardNode::Always => ExpressionHandle::invalid(),
    };
    for target_handle in [transition.target, transition.continuation] {
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
        return Some(GuardedEdge {
            guard,
            arguments: program.statement_table.expression_handles(*arguments),
        });
    }
    None
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
    // An Always edge (edge_to_any_guard) carries no guard at all.
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
