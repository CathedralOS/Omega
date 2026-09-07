//! Cross-machine scalar rank transport. Each side retains its own authored
//! template; simultaneous call substitution is not local-state correspondence.

use super::*;

pub(crate) struct RankingRangeCallMember<'program> {
    pub(crate) machine: &'program Machine,
    pub(crate) subject: ExpressionHandle,
    pub(crate) range: ExpressionHandle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RankingRangeCallProgress {
    Strict,
    /// A weak edge may preserve or decrease the rank. The component owner must
    /// prove that the graph of these edges contains no complete cycle.
    NonIncreasing,
}

/// Call components establish entry membership without installing the local
/// state query's raw-distance induction hypothesis. IncreasingTo includes its
/// zero plateau even when the entry cursor is already beyond its bound.
pub(crate) fn prove_ranking_range_call_entry(
    program: &TypedTrees,
    member: RankingRangeCallMember<'_>,
) -> bool {
    let prove = || -> Option<bool> {
        let (state, measure) = scalar_entry(program, &member)?;
        admit_member(program, &member, state, measure)?;
        let ExpressionNode::Range(range) = program.expression_table.expression(member.range) else {
            return None;
        };
        let bindings = integer_bindings(program, state)?;
        let mut engine = Engine::strict_with_symbol_bindings(program, member.machine, &bindings);
        if !engine.strict_symbol_bindings_are_valid() {
            return None;
        }
        let comparisons =
            entry_comparisons(program, member.machine, state, &mut engine, &bindings)?;
        let coordinate = rank_coordinate(&mut engine, measure)?;
        let floor = engine.normalize(range.start)?;
        let ceiling = engine.normalize(range.end)?;
        if !engine.install_hypotheses(comparisons) {
            return None;
        }
        Some(
            engine.requires_unsatisfiable
                || membership(
                    &engine,
                    measure,
                    &coordinate,
                    &floor,
                    &ceiling,
                    range.end_inclusive,
                ),
        )
    };
    prove() == Some(true)
}

/// The caller owns prefix stability and whole-component cycle coverage. This
/// judgment reads only caller entry hypotheses, never destination requirements,
/// and proves destination membership plus equality of authored range endpoints.
pub(crate) fn prove_ranking_range_call(
    program: &TypedTrees,
    caller: RankingRangeCallMember<'_>,
    callee: RankingRangeCallMember<'_>,
    guards: &[(ExpressionHandle, bool)],
    arguments: &[ExpressionHandle],
) -> Option<RankingRangeCallProgress> {
    if caller.machine.symbol == callee.machine.symbol {
        return None;
    }
    let (source, source_measure) = scalar_entry(program, &caller)?;
    let (destination, destination_measure) = scalar_entry(program, &callee)?;
    if !matches!(
        (source_measure, destination_measure),
        (
            RankingRangeMeasure::Single(_),
            RankingRangeMeasure::Single(_)
        ) | (
            RankingRangeMeasure::IncreasingTo { .. },
            RankingRangeMeasure::IncreasingTo { .. }
        )
    ) {
        return None;
    }
    let admit_source =
        |expression| meanings::builtin(program, caller.machine, source, expression, 0);
    let ExpressionNode::Range(source_range) = program.expression_table.expression(caller.range)
    else {
        return None;
    };
    let ExpressionNode::Range(destination_range) =
        program.expression_table.expression(callee.range)
    else {
        return None;
    };
    admit_member(program, &caller, source, source_measure)?;
    admit_member(program, &callee, destination, destination_measure)?;
    for &(guard, _) in guards {
        admit_source(guard)?;
    }
    for argument in arguments {
        admit_source(*argument)?;
    }
    let bindings = integer_bindings(program, source)?;
    let mut engine = Engine::strict_with_symbol_bindings(program, caller.machine, &bindings);
    if !engine.strict_symbol_bindings_are_valid() {
        return None;
    }
    let mut comparisons =
        entry_comparisons(program, caller.machine, source, &mut engine, &bindings)?;
    for &(guard, holds) in guards {
        collect_guard(&mut engine, guard, holds, &mut comparisons, 0)?;
    }
    let rank = rank_coordinate(&mut engine, source_measure)?;
    let view_bound = match source_measure {
        RankingRangeMeasure::IncreasingTo { limit, .. } => Some(engine.normalize(limit)?),
        _ => None,
    };
    let floor = engine.normalize(source_range.start)?;
    let ceiling = engine.normalize(source_range.end)?;
    if !engine.install_hypotheses(comparisons) {
        return None;
    }
    let parameters = program
        .state_parameters(destination)
        .iter()
        .filter(|parameter| !parameter.is_self);
    if parameters.clone().count() != arguments.len() {
        return None;
    }
    let mut actuals = Vec::new();
    for (parameter, argument) in parameters.zip(arguments) {
        if exact_integer_parameter(program, parameter.type_reference).is_none() {
            continue;
        }
        if !parameter.symbol.is_valid() || parameter.is_mutable || parameter.is_const {
            return None;
        }
        actuals.push(StrictArithmeticExpressionBinding {
            symbol: parameter.symbol,
            expression: *argument,
        });
    }
    // Resolve every caller actual before adding any callee formal. Foreign
    // references and required nonnumeric inputs therefore fail before vacuity.
    if !engine.bind_strict_arguments(&actuals) {
        return None;
    }
    let next_rank = rank_coordinate(&mut engine, destination_measure)?;
    let pinned_view_bound = match (view_bound, destination_measure) {
        (Some(bound), RankingRangeMeasure::IncreasingTo { limit, .. }) => {
            Some((bound, engine.normalize(limit)?))
        }
        (None, RankingRangeMeasure::Single(_)) => None,
        _ => return None,
    };
    let next_floor = engine.normalize(destination_range.start)?;
    let next_ceiling = engine.normalize(destination_range.end)?;
    if engine.requires_unsatisfiable {
        return Some(RankingRangeCallProgress::Strict);
    }
    let prove = |polynomial: Polynomial, minimum: i64| {
        engine.prove_at_least(&engine.substituted(&polynomial), &BigInt::from_i64(minimum))
    };
    // The view bound is independent of the optional authored range ceiling.
    // Moving it cannot manufacture descent even inside a constant rank range.
    if let Some((bound, next)) = pinned_view_bound
        && (!prove(bound.sub(&next), 0) || !prove(next.sub(&bound), 0))
    {
        return None;
    }
    if !membership(
        &engine,
        source_measure,
        &rank,
        &floor,
        &ceiling,
        source_range.end_inclusive,
    ) || !membership(
        &engine,
        destination_measure,
        &next_rank,
        &next_floor,
        &next_ceiling,
        destination_range.end_inclusive,
    ) || !prove(next_floor.sub(&floor), 0)
        || !prove(floor.sub(&next_floor), 0)
        || !prove(next_ceiling.sub(&ceiling), 0)
        || !prove(ceiling.sub(&next_ceiling), 0)
    {
        return None;
    }
    let descent = rank.sub(&next_rank);
    let clamped = matches!(source_measure, RankingRangeMeasure::IncreasingTo { .. });
    // A raw decrease below zero is only a plateau step. Strict clamped
    // descent requires a positive source coordinate as well as raw descent.
    if (!clamped || prove(rank, 1)) && prove(descent.clone(), 1) {
        Some(RankingRangeCallProgress::Strict)
    } else {
        (prove(descent, 0) || (clamped && prove(Polynomial::default().sub(&next_rank), 0)))
            .then_some(RankingRangeCallProgress::NonIncreasing)
    }
}

fn admit_member(
    program: &TypedTrees,
    member: &RankingRangeCallMember<'_>,
    state: &State,
    measure: RankingRangeMeasure,
) -> Option<()> {
    let ExpressionNode::Range(range) = program.expression_table.expression(member.range) else {
        return None;
    };
    for expression in [member.subject, range.start, range.end] {
        meanings::builtin(program, member.machine, state, expression, 0)?;
    }
    if let RankingRangeMeasure::IncreasingTo { limit, .. } = measure {
        meanings::builtin(program, member.machine, state, limit, 0)?;
    }
    Some(())
}

fn membership(
    engine: &Engine<'_>,
    measure: RankingRangeMeasure,
    coordinate: &Polynomial,
    floor: &Polynomial,
    ceiling: &Polynomial,
    inclusive: bool,
) -> bool {
    let prove = |polynomial: Polynomial, minimum: i64| {
        engine.prove_at_least(&engine.substituted(&polynomial), &BigInt::from_i64(minimum))
    };
    let slack = i64::from(!inclusive);
    match measure {
        RankingRangeMeasure::Single(_) => {
            prove(coordinate.clone(), 0)
                && prove(coordinate.sub(floor), 0)
                && prove(ceiling.sub(coordinate), slack)
        }
        // max(0,d) >= floor follows from either 0>=floor or d>=floor.
        // Both branches must lie below the ceiling, including for strict <.
        RankingRangeMeasure::IncreasingTo { .. } => {
            (prove(Polynomial::default().sub(floor), 0) || prove(coordinate.sub(floor), 0))
                && prove(ceiling.clone(), slack)
                && prove(ceiling.sub(coordinate), slack)
        }
        _ => false,
    }
}

fn rank_coordinate(engine: &mut Engine<'_>, measure: RankingRangeMeasure) -> Option<Polynomial> {
    match measure {
        RankingRangeMeasure::Single(subject) => engine.normalize(subject),
        // Keep the raw coordinate polynomial; the call-owned membership and
        // comparison predicates interpret its max(0,d) normalization.
        RankingRangeMeasure::IncreasingTo { subject, limit } => {
            Some(engine.normalize(limit)?.sub(&engine.normalize(subject)?))
        }
        _ => None,
    }
}

fn scalar_entry<'program>(
    program: &'program TypedTrees,
    member: &RankingRangeCallMember<'_>,
) -> Option<(&'program State, RankingRangeMeasure)> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == member.machine.symbol)?;
    let witness = machine.termination_plan.implementation_witness.as_ref()?;
    let custody = program.ranking_expression_custody_for(machine.symbol)?;
    if Some(witness.view_path.as_str()) != witness.ranking_view.canonical_path()
        || witness.subjects.len() != 1
        || witness.rank_range.is_none()
        || custody.subjects.as_slice() != [member.subject]
        || custody.rank_range != Some(member.range)
    {
        return None;
    }
    let measure = match witness.ranking_view {
        language_semantics::RankingViewId::NAT_DESCENDING
            if witness.view_arguments.is_empty() && custody.view_arguments.is_empty() =>
        {
            RankingRangeMeasure::Single(member.subject)
        }
        language_semantics::RankingViewId::NAT_INCREASING_TO
            if witness.view_arguments.len() == 1 =>
        {
            let [limit] = custody.view_arguments.as_slice() else {
                return None;
            };
            if !limit.is_valid() {
                return None;
            }
            RankingRangeMeasure::IncreasingTo {
                subject: member.subject,
                limit: *limit,
            }
        }
        _ => return None,
    };
    let [state] = program.machine_states(machine) else {
        return None;
    };
    let mut subject = member.subject;
    let mut visited = Vec::new();
    while let ExpressionNode::Atomic(atomic) = program.expression_table.expression(subject) {
        if visited.contains(&subject) {
            return None;
        }
        visited.push(subject);
        subject = atomic.value;
    }
    let ExpressionNode::Name(path) = program.expression_table.expression(subject) else {
        return None;
    };
    if !path.symbol.is_valid()
        || path.symbol != path.head_symbol
        || program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
    {
        return None;
    }
    let parameter = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == path.symbol)?;
    if parameter.is_self
        || parameter.is_mutable
        || parameter.is_const
        || !matches!(
            exact_integer_parameter(program, parameter.type_reference),
            Some(PrimitiveType::U8 | PrimitiveType::U16 | PrimitiveType::U32 | PrimitiveType::U64)
        )
    {
        return None;
    }
    Some((state, measure))
}
