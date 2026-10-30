use typed_trees::expression::{ExpressionHandle, ExpressionNode};

mod arguments;
mod guards;

use self::arguments::{argument_is_parameter_minus_one, argument_rebuilds_parameter};
use self::guards::{guard_is_positive_parameter, guard_is_positive_parameter_member};
use super::DecreaseMeasure;
use super::patterns;

/// The measure `edge_decrease_proven` reads, stated over one state's own
/// parameters: the countdown subject itself (signed or unsigned), or the
/// distance's unsigned cursor and admitted bound expression.
/// This query uses exact formal symbols in its own declaration frame. State
/// transport is supplied by the retained complete arrival witness.
pub(super) fn state_rank(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    measure: DecreaseMeasure,
) -> Option<checked_trees::CheckedStateNaturalRank> {
    use checked_trees::CheckedNaturalRankMeasure;
    use typed_trees::types::PrimitiveType;
    let plain = |subject: ExpressionHandle| {
        let name = program.expression_table.name_path_node(subject)?;
        program
            .state_parameters(state)
            .iter()
            .enumerate()
            .find(|(_, parameter)| {
                name.symbol.is_valid()
                    && name.head_symbol == name.symbol
                    && parameter.symbol == name.symbol
            })
            .and_then(|(position, parameter)| Some((u32::try_from(position).ok()?, parameter)))
    };
    match measure {
        DecreaseMeasure::Single(subject) => {
            let (position, parameter) = plain(subject)?;
            let primitive_type = program.primitive_type_reference(parameter.type_reference)?;
            if matches!(
                primitive_type,
                PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 | PrimitiveType::Addr
            ) {
                return None;
            }
            Some(checked_trees::CheckedStateNaturalRank {
                function: checked_trees::CheckedNaturalRankFunction::Identity,
                observation: Default::default(),
                state: state.symbol,
                parameter: parameter.symbol,
                parameter_position: position,
                // This query is comparison scratch only, never inserted as
                // an executable observation. The admission writer later binds
                // the exact coordinate, just as it binds observation roots.
                measure: CheckedNaturalRankMeasure::IntegerValue {
                    coordinate: Default::default(),
                    primitive_type,
                },
            })
        }
        DecreaseMeasure::Distance { lower, upper } => {
            let (lower_position, lower) = plain(lower)?;
            let primitive_type = program.primitive_type_reference(lower.type_reference)?;
            if !matches!(
                primitive_type,
                typed_trees::unsigned_integer_primitive_pattern!()
            ) {
                return None;
            }
            Some(checked_trees::CheckedStateNaturalRank {
                function: checked_trees::CheckedNaturalRankFunction::Identity,
                observation: Default::default(),
                state: state.symbol,
                parameter: lower.symbol,
                parameter_position: lower_position,
                measure: CheckedNaturalRankMeasure::BoundedDistance {
                    primitive_type,
                    upper,
                },
            })
        }
    }
}

pub(super) fn state_has_proven_self_loop(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    measure: DecreaseMeasure,
) -> bool {
    let edges = patterns::edges_to_state(program, state, state.symbol);
    !edges.is_empty()
        && edges.iter().all(|edge| {
            edge_decrease_proven(program, state, state, &edge.guards, edge.arguments, measure)
        })
}

/// Prove that the Nat-descending measure strictly decreases across a single
/// guarded transition edge from `source` to `target`. For a self-loop this is
/// the classic countdown proof; for a mutually-recursive cycle the target
/// differs from the source, so the decreasing argument is located by the
/// matching parameter name in the *target* state. Distances have no
/// recognizer here: `current_values` proves every distance edge from the
/// current values the edge actually supplies.
pub(super) fn edge_decrease_proven(
    program: &dyn ::validation::CheckingInput,
    source: &typed_trees::state::State,
    target: &typed_trees::state::State,
    guard: &[patterns::GuardFact],
    arguments: &[ExpressionHandle],
    measure: DecreaseMeasure,
) -> bool {
    let DecreaseMeasure::Single(decreases) = measure else {
        return false;
    };
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

/// Index of the non-self parameter with the given name in a state's parameter
/// list (the positional argument slot for that parameter on an incoming edge).
fn target_argument_index(
    program: &dyn ::validation::CheckingInput,
    target: &typed_trees::state::State,
    name: &str,
) -> Option<usize> {
    program
        .state_parameters(target)
        .iter()
        .position(|parameter| parameter.name.as_str() == name)
}

fn countdown_edge(
    program: &dyn ::validation::CheckingInput,
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

    (guard_is_positive_parameter(program, source, guard, parameter)
        || floor_at_least_one(program, source, parameter))
        && argument_is_parameter_minus_one(program, source, argument, parameter)
}

fn countdown_edge_parts<'program>(
    program: &'program dyn ::validation::CheckingInput,
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
    let parameter = program.state_parameters(source).iter().find(|parameter| {
        parameter.symbol == decreases_path.symbol || parameter.name.as_str() == decrease_name
    })?;
    let argument_index = target_argument_index(program, target, parameter.name.as_str())?;
    let argument = arguments.get(argument_index).copied()?;
    Some((parameter, argument_index, argument))
}

/// The parameter's floor is >= 1 at every evaluation: its declared range or
/// its state's `requires`, under an Exact (or absent) arithmetic domain --
/// the positivity source for a decrement on an edge with no usable guard
/// (an Always loop-back from a sub-state whose parameter starts at one).
/// Non-Exact carriers are permissive (probed live at stores) and never
/// discharge a bound.
fn floor_at_least_one(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    parameter: &typed_trees::signature::StateParameter,
) -> bool {
    validation::find_state(program, state.symbol)
        .and_then(|(machine, state)| {
            validation::state_parameter_integer_interval(program, machine, state, parameter.symbol)
        })
        .and_then(|(low, _)| low)
        .is_some_and(|low| low >= 1)
}

fn member_countdown_edge(
    program: &dyn ::validation::CheckingInput,
    source: &typed_trees::state::State,
    target: &typed_trees::state::State,
    guard: &[patterns::GuardFact],
    arguments: &[ExpressionHandle],
    decreases: ExpressionHandle,
) -> bool {
    let Some(member) = program.expression_table.member_node(decreases) else {
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

    guard_is_positive_parameter_member(program, source, guard, parameter, member.member.as_str())
        && argument_rebuilds_parameter(program, source, argument, parameter, member.member.as_str())
}

/// The syntax tier and relational tier must agree on operation meaning. A
/// `- 1` shape or positive-looking guard is not builtin arithmetic evidence
/// when an authored/selected operator owns that exact expression.
fn has_builtin_meaning(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
) -> bool {
    let Some((machine, _)) = validation::find_state(program, state.symbol) else {
        return false;
    };
    validation::has_builtin_bound_expression_meaning(program, machine, Some(state), expression)
}
