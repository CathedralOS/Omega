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
    let source = scalar_entry(program, &caller)?;
    let destination = scalar_entry(program, &callee)?;
    let admit_source =
        |expression| meanings::builtin(program, caller.machine, source, expression, 0);
    let admit_destination =
        |expression| meanings::builtin(program, callee.machine, destination, expression, 0);
    let ExpressionNode::Range(source_range) = program.expression_table.expression(caller.range)
    else {
        return None;
    };
    let ExpressionNode::Range(destination_range) =
        program.expression_table.expression(callee.range)
    else {
        return None;
    };
    for expression in [caller.subject, source_range.start, source_range.end] {
        admit_source(expression)?;
    }
    for expression in [
        callee.subject,
        destination_range.start,
        destination_range.end,
    ] {
        admit_destination(expression)?;
    }
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
    let rank = engine.normalize(caller.subject)?;
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
    let next_rank = engine.normalize(callee.subject)?;
    let next_floor = engine.normalize(destination_range.start)?;
    let next_ceiling = engine.normalize(destination_range.end)?;
    if engine.requires_unsatisfiable {
        return Some(RankingRangeCallProgress::Strict);
    }
    let prove = |polynomial: Polynomial, minimum: i64| {
        engine.prove_at_least(&engine.substituted(&polynomial), &BigInt::from_i64(minimum))
    };
    if !prove(rank.clone(), 0)
        || !prove(rank.sub(&floor), 0)
        || !prove(ceiling.sub(&rank), i64::from(!source_range.end_inclusive))
        || !prove(next_rank.clone(), 0)
        || !prove(next_rank.sub(&next_floor), 0)
        || !prove(
            next_ceiling.sub(&next_rank),
            i64::from(!destination_range.end_inclusive),
        )
        || !prove(next_floor.sub(&floor), 0)
        || !prove(floor.sub(&next_floor), 0)
        || !prove(next_ceiling.sub(&ceiling), 0)
        || !prove(ceiling.sub(&next_ceiling), 0)
    {
        return None;
    }
    let descent = rank.sub(&next_rank);
    if prove(descent.clone(), 1) {
        Some(RankingRangeCallProgress::Strict)
    } else {
        prove(descent, 0).then_some(RankingRangeCallProgress::NonIncreasing)
    }
}

fn scalar_entry<'program>(
    program: &'program TypedTrees,
    member: &RankingRangeCallMember<'_>,
) -> Option<&'program State> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == member.machine.symbol)?;
    let witness = machine.termination_plan.implementation_witness.as_ref()?;
    let custody = program.ranking_expression_custody_for(machine.symbol)?;
    if witness.ranking_view != language_semantics::RankingViewId::NAT_DESCENDING
        || Some(witness.view_path.as_str()) != witness.ranking_view.canonical_path()
        || witness.subjects.len() != 1
        || !witness.view_arguments.is_empty()
        || witness.rank_range.is_none()
        || custody.subjects.as_slice() != [member.subject]
        || custody.rank_range != Some(member.range)
        || !custody.view_arguments.is_empty()
    {
        return None;
    }
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
    Some(state)
}
