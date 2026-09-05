use typed_trees::expression::{ExpressionHandle, ExpressionNode};

mod arguments;
mod guards;

use self::arguments::{
    argument_is_parameter_minus_one, argument_is_parameter_plus_one,
    argument_rebuilds_parameter_with_member_minus_one,
};
use self::guards::{
    guard_is_index_below_limit, guard_is_positive_parameter, guard_is_positive_parameter_member,
};
use super::patterns;
use super::{DecreaseMeasure, DistanceOrientation};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DirectCountdownEdge {
    pub(super) source_parameter: symbols::SymbolHandle,
    pub(super) target_argument_index: usize,
}

/// Return the exact parameter/argument join for the first retained Terminal
/// countdown shape. This is deliberately owned by the Nat ranking prover: a
/// later checked-plan producer may project this judgment, but must not grow a
/// second recognizer for `parameter > 0` and `parameter - 1`.
pub(super) fn direct_countdown_edge(
    program: &typed_trees::TypedTrees,
    source: &typed_trees::state::State,
    target: &typed_trees::state::State,
    guard: &[patterns::GuardFact],
    arguments: &[ExpressionHandle],
    decreases: ExpressionHandle,
) -> Option<DirectCountdownEdge> {
    let (parameter, target_argument_index, argument) =
        countdown_edge_parts(program, source, target, arguments, decreases)?;
    (guard_is_positive_parameter(program, guard, parameter)
        && argument_is_parameter_minus_one(program, argument, parameter))
    .then_some(DirectCountdownEdge {
        source_parameter: parameter.symbol,
        target_argument_index,
    })
}

pub(super) fn state_has_proven_self_loop(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    measure: DecreaseMeasure,
    orientation: DistanceOrientation,
) -> bool {
    let edges = patterns::edges_to_state(program, state, state.symbol);
    !edges.is_empty()
        && edges.iter().all(|edge| {
            edge_decrease_proven(
                program,
                state,
                state,
                &edge.guards,
                edge.arguments,
                measure,
                orientation,
            )
        })
}

/// Prove that the Nat-descending measure strictly decreases across a single
/// guarded transition edge from `source` to `target`. For a self-loop this is
/// the classic countdown / distance proof; for a mutually-recursive cycle the
/// target differs from the source, so the decreasing argument is located by the
/// matching parameter name in the *target* state.
pub(super) fn edge_decrease_proven(
    program: &typed_trees::TypedTrees,
    source: &typed_trees::state::State,
    target: &typed_trees::state::State,
    guard: &[patterns::GuardFact],
    arguments: &[ExpressionHandle],
    measure: DecreaseMeasure,
    orientation: DistanceOrientation,
) -> bool {
    match measure {
        DecreaseMeasure::Single(decreases) => {
            match program.expression_table.expression(decreases) {
                ExpressionNode::Name(_) => {
                    countdown_edge(program, source, target, guard, arguments, decreases)
                }
                ExpressionNode::Member(_) => {
                    member_countdown_edge(program, source, target, guard, arguments, decreases)
                }
                _ => false,
            }
        }
        DecreaseMeasure::Distance { lower, upper } => {
            // The bounded distance reads its `(lower, upper)` subjects in
            // declaration order. The swapped orientation is the
            // inverted-clause probe: it asks whether the subjects written the
            // other way around would have proven.
            let (lower, upper) = match orientation {
                DistanceOrientation::Declared => (lower, upper),
                DistanceOrientation::Swapped => (upper, lower),
            };
            distance_edge(program, source, target, guard, arguments, upper, lower)
        }
    }
}

/// Index of the non-self parameter with the given name in a state's parameter
/// list (the positional argument slot for that parameter on an incoming edge).
fn target_argument_index(
    program: &typed_trees::TypedTrees,
    target: &typed_trees::state::State,
    name: &str,
) -> Option<usize> {
    program
        .state_parameters(target)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .position(|parameter| parameter.name.as_str() == name)
}

fn countdown_edge(
    program: &typed_trees::TypedTrees,
    source: &typed_trees::state::State,
    target: &typed_trees::state::State,
    guard: &[patterns::GuardFact],
    arguments: &[ExpressionHandle],
    decreases: ExpressionHandle,
) -> bool {
    let Some((parameter, _, argument)) =
        countdown_edge_parts(program, source, target, arguments, decreases)
    else {
        return false;
    };

    (guard_is_positive_parameter(program, guard, parameter)
        || declared_floor_at_least_one(program, parameter))
        && argument_is_parameter_minus_one(program, argument, parameter)
}

fn countdown_edge_parts<'program>(
    program: &'program typed_trees::TypedTrees,
    source: &'program typed_trees::state::State,
    target: &typed_trees::state::State,
    arguments: &'program [ExpressionHandle],
    decreases: ExpressionHandle,
) -> Option<(
    &'program typed_trees::signature::StateParameter,
    usize,
    ExpressionHandle,
)> {
    let decreases_path = match program.expression_table.expression(decreases) {
        ExpressionNode::Name(path) => path,
        _ => return None,
    };
    let decrease_name = program
        .expression_table
        .name_path_members(decreases_path.members)
        .last()
        .map(|member| member.as_str())
        .unwrap_or_default();
    let parameter = program
        .state_parameters(source)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .find(|parameter| {
            parameter.symbol == decreases_path.symbol || parameter.name.as_str() == decrease_name
        })?;
    let argument_index = target_argument_index(program, target, parameter.name.as_str())?;
    let argument = arguments.get(argument_index).copied()?;
    Some((parameter, argument_index, argument))
}

/// The parameter's DECLARED `[a..=b]` floor is >= 1 under an Exact (or
/// absent) arithmetic domain -- the positivity source for a decrement on an
/// edge with no usable guard (the MR2 Always loop-back from a sub-state
/// whose param is declared `[1..=N]`). Non-Exact ranges are deliberately
/// permissive (probed live at stores) and must never discharge a bound.
fn declared_floor_at_least_one(
    program: &typed_trees::TypedTrees,
    parameter: &typed_trees::signature::StateParameter,
) -> bool {
    use typed_trees::types::{TypeConstraintNode, TypeReferenceNode};
    let mut handle = parameter.type_reference;
    loop {
        match program.type_reference_table.type_reference(handle) {
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let constraints = program.type_reference_table.constraints(*constraints);
                if constraints.iter().any(|constraint| {
                    matches!(
                        constraint,
                        TypeConstraintNode::ArithmeticDomain(domain)
                            if *domain != numerics::arithmetic::ArithmeticDomain::Exact
                    )
                }) {
                    return false;
                }
                if let Some(minimum) = constraints.iter().find_map(|constraint| match constraint {
                    TypeConstraintNode::Range { minimum, .. } => {
                        program.expression_table.constant_integer_value(*minimum)
                    }
                    _ => None,
                }) {
                    return minimum >= 1;
                }
                handle = *base_type;
            }
            _ => return false,
        }
    }
}

fn member_countdown_edge(
    program: &typed_trees::TypedTrees,
    source: &typed_trees::state::State,
    target: &typed_trees::state::State,
    guard: &[patterns::GuardFact],
    arguments: &[ExpressionHandle],
    decreases: ExpressionHandle,
) -> bool {
    let ExpressionNode::Member(member) = program.expression_table.expression(decreases) else {
        return false;
    };
    let Some(parameter) =
        patterns::parameter_matched_by_expression(program, source, member.receiver)
    else {
        return false;
    };
    let Some(argument_index) = target_argument_index(program, target, parameter.name.as_str())
    else {
        return false;
    };
    let Some(argument) = arguments.get(argument_index).copied() else {
        return false;
    };

    guard_is_positive_parameter_member(program, guard, parameter, member.member.as_str())
        && argument_rebuilds_parameter_with_member_minus_one(
            program,
            argument,
            parameter,
            member.member.as_str(),
        )
}

/// Prove the bounded distance `upper - lower` strictly decreases across one
/// guarded edge: the guard bounds `lower` below `upper`, `upper` is threaded
/// unchanged, and `lower` advances by one.
fn distance_edge(
    program: &typed_trees::TypedTrees,
    source: &typed_trees::state::State,
    target: &typed_trees::state::State,
    guard: &[patterns::GuardFact],
    arguments: &[ExpressionHandle],
    upper: ExpressionHandle,
    lower: ExpressionHandle,
) -> bool {
    let Some(limit_parameter) = patterns::parameter_matched_by_expression(program, source, upper)
    else {
        return false;
    };
    let Some(index_parameter) = patterns::parameter_matched_by_expression(program, source, lower)
    else {
        return false;
    };
    let Some(limit_index) = target_argument_index(program, target, limit_parameter.name.as_str())
    else {
        return false;
    };
    let Some(index_index) = target_argument_index(program, target, index_parameter.name.as_str())
    else {
        return false;
    };
    let Some(limit_argument) = arguments.get(limit_index).copied() else {
        return false;
    };
    let Some(index_argument) = arguments.get(index_index).copied() else {
        return false;
    };

    guard_is_index_below_limit(program, guard, index_parameter, limit_parameter)
        && patterns::expression_is_parameter(program, limit_argument, limit_parameter)
        && argument_is_parameter_plus_one(program, index_argument, index_parameter)
}
