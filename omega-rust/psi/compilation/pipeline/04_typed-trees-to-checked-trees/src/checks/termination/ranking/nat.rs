use typed_trees::expression::{ExpressionHandle, ExpressionNode};

mod arguments;
mod guards;

use self::arguments::{
    argument_is_parameter_minus_one, argument_rebuilds_parameter_with_member_minus_one,
};
use self::guards::{guard_is_positive_parameter, guard_is_positive_parameter_member};
use super::DecreaseMeasure;
use super::patterns;

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
    (guard_is_positive_parameter(program, source, guard, parameter)
        && argument_is_parameter_minus_one(program, source, argument, parameter))
    .then_some(DirectCountdownEdge {
        source_parameter: parameter.symbol,
        target_argument_index,
    })
}

/// The measure `edge_decrease_proven` reads, stated over one state's own
/// parameters: the countdown subject itself (signed or unsigned), or the
/// distance's unsigned parameters or unsigned cursor and collection length.
/// Edge proofs locate a target's subjects by name, so every state of a cyclic
/// component carries parameters of these names.
pub(super) fn state_rank(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    measure: DecreaseMeasure,
) -> Option<checked_trees::CheckedStateNaturalRank> {
    use checked_trees::CheckedNaturalRankMeasure;
    use typed_trees::types::PrimitiveType;
    let plain = |subject: ExpressionHandle| {
        let ExpressionNode::Name(_) = program.expression_table.expression(subject) else {
            return None;
        };
        program
            .state_parameters(state)
            .iter()
            .enumerate()
            .find(|(_, parameter)| patterns::expression_is_parameter(program, subject, parameter))
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
                state: state.symbol,
                parameter: parameter.symbol,
                parameter_position: position,
                measure: CheckedNaturalRankMeasure::IntegerParameter { primitive_type },
            })
        }
        DecreaseMeasure::Distance { lower, upper } => {
            let (lower_position, lower) = plain(lower)?;
            if matches!(
                program.expression_table.expression(upper),
                ExpressionNode::Member(_)
            ) {
                // This is the same semantic collection-length selection the
                // edge prover uses, not a record field merely named `len`.
                let collection = patterns::parameter_matched_by_expression(program, state, upper)?;
                if program.primitive_type_reference(lower.type_reference)
                    != Some(PrimitiveType::U64)
                {
                    return None;
                }
                let collection_position = program
                    .state_parameters(state)
                    .iter()
                    .position(|parameter| parameter.symbol == collection.symbol)?;
                return Some(checked_trees::CheckedStateNaturalRank {
                    state: state.symbol,
                    parameter: lower.symbol,
                    parameter_position: lower_position,
                    measure: CheckedNaturalRankMeasure::CollectionLengthDistance {
                        collection: collection.symbol,
                        collection_position: u32::try_from(collection_position).ok()?,
                    },
                });
            }
            let primitive_type = program.primitive_type_reference(lower.type_reference)?;
            if let ExpressionNode::Integer(literal) = program.expression_table.expression(upper) {
                // A constant bound: the rank is the cursor's distance below a
                // fixed value, so no upper parameter rides along the cycle.
                let upper = literal.value_u64()?;
                let maximum = super::unsigned_maximum(primitive_type)?;
                return (u128::from(upper) <= maximum).then_some(
                    checked_trees::CheckedStateNaturalRank {
                        state: state.symbol,
                        parameter: lower.symbol,
                        parameter_position: lower_position,
                        measure: CheckedNaturalRankMeasure::ConstantDistance {
                            primitive_type,
                            upper,
                        },
                    },
                );
            }
            let (upper_position, upper) = plain(upper)?;
            if lower.symbol == upper.symbol
                || program.primitive_type_reference(upper.type_reference) != Some(primitive_type)
                || !matches!(
                    primitive_type,
                    PrimitiveType::U8
                        | PrimitiveType::U16
                        | PrimitiveType::U32
                        | PrimitiveType::U64
                )
            {
                return None;
            }
            Some(checked_trees::CheckedStateNaturalRank {
                state: state.symbol,
                parameter: lower.symbol,
                parameter_position: lower_position,
                measure: CheckedNaturalRankMeasure::UnsignedDistance {
                    primitive_type,
                    upper: upper.symbol,
                    upper_position,
                },
            })
        }
    }
}

/// Resolve an authored subject's root name to the formal of `state` that
/// carries its entry role under the discovered telescope (`entries` holds
/// one entry-parameter symbol per non-self formal, the same correspondence
/// the relational range judgment proved). A cyclic state may rename or
/// reorder the ranked input — `pending` where the witness reads `remaining`
/// — so the role's carrier, not a name twin, is the parameter the
/// executable rank measures. The index returned counts every formal, self
/// included, matching `state_rank`'s `parameter_position`.
pub(super) fn carried_subject<'program>(
    program: &'program typed_trees::TypedTrees,
    state: &'program typed_trees::state::State,
    entries: &[symbols::SymbolHandle],
    subject: ExpressionHandle,
) -> Option<(usize, &'program typed_trees::signature::StateParameter)> {
    let ExpressionNode::Name(name) = program.expression_table.expression(subject) else {
        return None;
    };
    if !name.symbol.is_valid() || name.head_symbol != name.symbol {
        return None;
    }
    let position = entries.iter().position(|entry| *entry == name.symbol)?;
    program
        .state_parameters(state)
        .iter()
        .enumerate()
        .nth(position)
}

/// The `Single` rank for a cyclic state whose entry telescope assigns the
/// subject's role to a renamed or reordered formal. Distance measures keep
/// the name-based `state_rank` judgment until one needs this route.
pub(super) fn carried_state_rank(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    measure: DecreaseMeasure,
    entries: &[symbols::SymbolHandle],
) -> Option<checked_trees::CheckedStateNaturalRank> {
    use checked_trees::CheckedNaturalRankMeasure;
    use typed_trees::types::PrimitiveType;
    let DecreaseMeasure::Single(subject) = measure else {
        return None;
    };
    let (position, parameter) = carried_subject(program, state, entries, subject)?;
    let primitive_type = program.primitive_type_reference(parameter.type_reference)?;
    if matches!(
        primitive_type,
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 | PrimitiveType::Addr
    ) {
        return None;
    }
    Some(checked_trees::CheckedStateNaturalRank {
        state: state.symbol,
        parameter: parameter.symbol,
        parameter_position: u32::try_from(position).ok()?,
        measure: CheckedNaturalRankMeasure::IntegerParameter { primitive_type },
    })
}

/// The `direct_countdown_edge` judgment under the entry telescope: the
/// source carrier is the formal claiming the subject's entry role and the
/// stepped actual must land in the `target` slot claiming that same role.
/// The guard and `- 1` recognizers are the name-based edge's own — only the
/// parameter/slot selection differs, so a renamed or reordered header still
/// reports the exact parameter/argument join.
pub(super) fn carried_direct_countdown_edge(
    program: &typed_trees::TypedTrees,
    source: &typed_trees::state::State,
    guard: &[patterns::GuardFact],
    arguments: &[ExpressionHandle],
    decreases: ExpressionHandle,
    source_entries: &[symbols::SymbolHandle],
    target_entries: &[symbols::SymbolHandle],
) -> Option<DirectCountdownEdge> {
    let ExpressionNode::Name(name) = program.expression_table.expression(decreases) else {
        return None;
    };
    if !name.symbol.is_valid() || name.head_symbol != name.symbol {
        return None;
    }
    let (_, parameter) = carried_subject(program, source, source_entries, decreases)?;
    let target_argument_index = target_entries
        .iter()
        .position(|entry| *entry == name.symbol)?;
    let argument = arguments.get(target_argument_index).copied()?;
    (guard_is_positive_parameter(program, source, guard, parameter)
        && argument_is_parameter_minus_one(program, source, argument, parameter))
    .then_some(DirectCountdownEdge {
        source_parameter: parameter.symbol,
        target_argument_index,
    })
}

/// The `parameter > 0`/`parameter - 1` countdown edge under the entry
/// telescope: the source carrier is found by entry role and its stepped
/// actual must land in the `target` slot claiming the same role. The guard,
/// positivity floor, and `- 1` recognizers are the name-based edge's own —
/// only the parameter/slot selection differs.
pub(super) fn carried_edge_decrease_proven(
    program: &typed_trees::TypedTrees,
    source: &typed_trees::state::State,
    guard: &[patterns::GuardFact],
    arguments: &[ExpressionHandle],
    measure: DecreaseMeasure,
    source_entries: &[symbols::SymbolHandle],
    target_entries: &[symbols::SymbolHandle],
) -> bool {
    let DecreaseMeasure::Single(subject) = measure else {
        return false;
    };
    let ExpressionNode::Name(name) = program.expression_table.expression(subject) else {
        return false;
    };
    if !name.symbol.is_valid() || name.head_symbol != name.symbol {
        return false;
    }
    let Some((_, parameter)) = carried_subject(program, source, source_entries, subject) else {
        return false;
    };
    let Some(argument_index) = target_entries
        .iter()
        .position(|entry| *entry == name.symbol)
    else {
        return false;
    };
    let Some(argument) = arguments.get(argument_index).copied() else {
        return false;
    };
    (guard_is_positive_parameter(program, source, guard, parameter)
        || floor_at_least_one(program, source, parameter))
        && argument_is_parameter_minus_one(program, source, argument, parameter)
}

pub(super) fn state_has_proven_self_loop(
    program: &typed_trees::TypedTrees,
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
    program: &typed_trees::TypedTrees,
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
    program: &typed_trees::TypedTrees,
    target: &typed_trees::state::State,
    name: &str,
) -> Option<usize> {
    program
        .state_parameters(target)
        .iter()
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

    (guard_is_positive_parameter(program, source, guard, parameter)
        || floor_at_least_one(program, source, parameter))
        && argument_is_parameter_minus_one(program, source, argument, parameter)
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
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    parameter: &typed_trees::signature::StateParameter,
) -> bool {
    crate::semantic::calls::find_state_with_machine(program, state.symbol)
        .and_then(|(machine, state)| {
            validation::state_parameter_integer_interval(program, machine, state, parameter.symbol)
        })
        .and_then(|(low, _)| low)
        .is_some_and(|low| low >= 1)
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

    guard_is_positive_parameter_member(program, source, guard, parameter, member.member.as_str())
        && argument_rebuilds_parameter_with_member_minus_one(
            program,
            source,
            argument,
            parameter,
            member.member.as_str(),
        )
}

/// The syntax tier and relational tier must agree on operation meaning. A
/// `- 1` shape or positive-looking guard is not builtin arithmetic evidence
/// when an authored/selected operator owns that exact expression.
fn has_builtin_meaning(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
) -> bool {
    let Some((machine, _)) = crate::semantic::calls::find_state_with_machine(program, state.symbol)
    else {
        return false;
    };
    validation::has_builtin_bound_expression_meaning(program, machine, Some(state), expression)
}
