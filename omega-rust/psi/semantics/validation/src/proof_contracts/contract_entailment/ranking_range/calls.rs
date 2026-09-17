//! Cross-machine scalar rank transport with optional authored ranges. Each side
//! retains its own template; call substitution is not local-state correspondence.

use super::super::{
    StrictArithmeticBindingValue, StrictArithmeticExpressionBinding, StrictArithmeticSymbolBinding,
};
use super::{
    BigInt, BinaryOperator, Comparison, Engine, ExpressionHandle, ExpressionNode, Machine,
    Polynomial, PrimitiveType, RankingRangeMeasure, State, TypedTrees, collect_guard,
    entry_comparisons, exact_integer_parameter, integer_bindings, lengths, meanings,
    parameter_comparisons, projections, validate_mapping,
};
use symbols::SymbolHandle;
mod endpoint_pins;
pub(crate) use endpoint_pins::{RankingRangeCallEdge, mixed_call_endpoints_are_pinned};

/// The exact transition site inside the caller's machine. `state` is the state
/// whose statements carry the call; `entry_parameters` is that state's
/// discovered telescope: one entry parameter symbol per non-self formal
/// (`SymbolHandle::default()` marks a formal with no entry role). Authored
/// subjects and endpoints stay entry-spelled; the site aliases each carried
/// role onto the atom for the formal that actually holds it here.
pub(crate) struct RankingRangeCallSite<'program> {
    pub(crate) state: &'program State,
    pub(crate) entry_parameters: &'program [SymbolHandle],
}

pub(crate) struct RankingRangeCallMember<'program> {
    pub(crate) machine: &'program Machine,
    pub(crate) subject: ExpressionHandle,
    /// The upper subject of a two-subject view (`Nat::BoundedDistance` ranks
    /// `(subject, paired_subject)` by `paired_subject - subject`). Invalid for
    /// single-subject views.
    pub(crate) paired_subject: ExpressionHandle,
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
        let length_bindings = if matches!(measure, RankingRangeMeasure::SliceLength(_)) {
            lengths::bindings(program, state, None)
        } else {
            Vec::new()
        };
        if !length_bindings.is_empty() {
            let mut expressions = vec![range.start, range.end];
            if let RankingRangeMeasure::SliceLength(subject) = measure {
                expressions.push(subject);
            }
            expressions.extend(projections::entry_expressions(
                program,
                member.machine,
                state,
            ));
            lengths::install(
                program,
                member.machine,
                state,
                state,
                &length_bindings,
                &mut engine,
                &expressions,
            )?;
        }
        let mut comparisons =
            entry_comparisons(program, member.machine, state, &mut engine, &bindings)?;
        comparisons.extend(length_bindings.iter().map(|(_, identity)| {
            (
                BinaryOperator::GreaterOrEqual,
                Polynomial::atom(identity.clone()),
                Polynomial::default(),
            )
        }));
        let coordinate = match measure {
            RankingRangeMeasure::SliceLength(subject) => {
                length_coordinate(program, state, subject, &length_bindings)?
            }
            _ => rank_coordinate(program, member.machine, &mut engine, measure)?,
        };
        let floor = engine.normalize(range.start)?;
        let ceiling = engine.normalize(range.end)?;
        if !engine.install_hypotheses(comparisons) {
            return None;
        }
        Some(
            engine.requires_unsatisfiable
                || (membership(
                    &engine,
                    measure,
                    &coordinate,
                    &floor,
                    &ceiling,
                    range.end_inclusive,
                ) && forms_in_carrier(&engine, measure, &coordinate)),
        )
    };
    prove() == Some(true)
}

/// The caller owns prefix stability and whole-component cycle coverage. This
/// judgment reads only caller entry hypotheses, never destination requirements,
/// and proves nonincrease or strict descent with pinned view bounds. Authored
/// ranges additionally require membership and equality of their endpoints.
/// When only one side authors a range, the component owner must separately
/// conserve its endpoint inputs through every unranged participant.
pub(crate) fn prove_ranking_range_call(
    program: &TypedTrees,
    caller: RankingRangeCallMember<'_>,
    caller_site: &RankingRangeCallSite<'_>,
    callee: RankingRangeCallMember<'_>,
    guards: &[(ExpressionHandle, bool)],
    arguments: &[ExpressionHandle],
) -> Option<RankingRangeCallProgress> {
    if caller.machine.symbol == callee.machine.symbol {
        return None;
    }
    let (entry, source_measure) = scalar_entry(program, &caller)?;
    let (destination, destination_measure) = scalar_entry(program, &callee)?;
    let source = caller_site.state;
    if !matches!(
        (source_measure, destination_measure),
        (
            RankingRangeMeasure::Single(_),
            RankingRangeMeasure::Single(_)
        ) | (
            RankingRangeMeasure::IncreasingTo { .. },
            RankingRangeMeasure::IncreasingTo { .. }
        ) | (
            RankingRangeMeasure::Distance { .. },
            RankingRangeMeasure::Distance { .. }
        ) | (
            RankingRangeMeasure::SliceLength(_),
            RankingRangeMeasure::SliceLength(_)
        ) | (
            RankingRangeMeasure::Computed { .. },
            RankingRangeMeasure::Computed { .. }
        )
    ) {
        return None;
    }
    let admit_source =
        |expression| meanings::builtin(program, caller.machine, source, expression, 0);
    // Both selected scalar views already produce natural ranks. An absent
    // optional range adds no endpoint obligations or synthetic bound. Authored
    // subjects, endpoints, and limits name entry parameters, so their builtin
    // meaning is admitted at the entry state even when the call sits inside a
    // subordinate state.
    admit_member(program, &caller, entry, source_measure)?;
    admit_member(program, &callee, destination, destination_measure)?;
    for &(guard, _) in guards {
        admit_source(guard)?;
    }
    for argument in arguments {
        admit_source(*argument)?;
    }
    let at_entry = source.symbol == entry.symbol;
    let (bindings, mut carrier_equalities) = if at_entry {
        (integer_bindings(program, source)?, Vec::new())
    } else {
        validate_mapping(
            program,
            caller.machine,
            source,
            caller_site.entry_parameters,
        )?;
        telescoped_bindings(program, source, caller_site.entry_parameters)?
    };
    let mut engine = Engine::strict_with_symbol_bindings(program, caller.machine, &bindings);
    if !engine.strict_symbol_bindings_are_valid() {
        return None;
    }
    // A slice-length member's rank is its collection's length coordinate:
    // distinct atoms, never the scalar value of the slice parameter itself.
    // Install the same projections the named-state judgment uses so a `.len`
    // inside a guard, actual, endpoint, or requires fact names the same atom.
    let mut length_bindings = if matches!(source_measure, RankingRangeMeasure::SliceLength(_)) {
        lengths::bindings(program, source, None)
    } else {
        Vec::new()
    };
    if !at_entry {
        // The ranked slice subject stays entry-spelled: alias its entry
        // parameter to the site carrier's length atom.
        let (roles, equalities) =
            telescoped_length_bindings(program, source, caller_site.entry_parameters);
        carrier_equalities.extend(equalities);
        for (symbol, identity) in roles {
            if !length_bindings.iter().any(|(bound, _)| *bound == symbol) {
                length_bindings.push((symbol, identity));
            }
        }
    }
    if !length_bindings.is_empty() {
        let mut expressions = Vec::new();
        if let RankingRangeMeasure::SliceLength(subject) = source_measure {
            expressions.push(subject);
        }
        if caller.range.is_valid()
            && let ExpressionNode::Range(range) = program.expression_table.expression(caller.range)
        {
            expressions.extend([range.start, range.end]);
        }
        expressions.extend(arguments.iter().copied());
        expressions.extend(guards.iter().map(|(guard, _)| *guard));
        expressions.extend(projections::entry_expressions(
            program,
            caller.machine,
            source,
        ));
        lengths::install(
            program,
            caller.machine,
            source,
            source,
            &length_bindings,
            &mut engine,
            &expressions,
        )?;
    }
    let mut comparisons = if at_entry {
        entry_comparisons(program, caller.machine, source, &mut engine, &bindings)?
    } else {
        // Requires clauses describe the entry parameters, not whatever formal
        // carries the role here; only the member's own entry-invariant proof
        // may substitute them past an internal arrival. The site's own
        // constrained-type facts still hold on every arrival, and so does the
        // member's authored range (installed below, once the rank is formed).
        parameter_comparisons(program, caller.machine, source, &mut engine, &bindings)?
    };
    comparisons.extend(carrier_equalities);
    comparisons.extend(length_bindings.iter().map(|(_, identity)| {
        (
            BinaryOperator::GreaterOrEqual,
            Polynomial::atom(identity.clone()),
            Polynomial::default(),
        )
    }));
    for &(guard, holds) in guards {
        collect_guard(&mut engine, guard, holds, &mut comparisons, 0)?;
    }
    let rank = match source_measure {
        RankingRangeMeasure::SliceLength(subject) => {
            length_coordinate(program, entry, subject, &length_bindings)?
        }
        _ => rank_coordinate(program, caller.machine, &mut engine, source_measure)?,
    };
    let view_bound = match source_measure {
        RankingRangeMeasure::IncreasingTo { limit, .. } => Some(engine.normalize(limit)?),
        _ => None,
    };
    let source_range = if caller.range.is_valid() {
        let ExpressionNode::Range(range) = program.expression_table.expression(caller.range) else {
            return None;
        };
        Some((
            engine.normalize(range.start)?,
            engine.normalize(range.end)?,
            range.end_inclusive,
        ))
    } else {
        None
    };
    if !at_entry && let Some((floor, ceiling, inclusive)) = &source_range {
        // A subordinate site holds the member's own authored range on
        // arrival. Entry facts stop at the entry state, but the range is the
        // member's private witness invariant: its state-edge judgment
        // re-establishes membership and endpoint pinning at every internal
        // arrival, and the checked stage runs that judgment for every ranged
        // member with internal arrivals. Consume exactly the facts that
        // judgment proves -- never a destination requirement, never a caller
        // guarantee -- so an arrival the member cannot prove fails at the
        // member's own judgment rather than being assumed here.
        comparisons.extend(arrival_invariant(
            source_measure,
            &rank,
            floor,
            ceiling,
            *inclusive,
        ));
    }
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
        if !parameter.symbol.is_valid() || parameter.is_const {
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
    let next_rank = match destination_measure {
        // The callee's ranked slice formal arrives as this call's exact
        // actual; its produced length is the actual's own coordinate, not a
        // forwarded caller parameter.
        RankingRangeMeasure::SliceLength(subject) => {
            let formal = lengths::parameter(program, destination, subject)?;
            let position = program
                .state_parameters(destination)
                .iter()
                .filter(|parameter| !parameter.is_self)
                .position(|parameter| parameter.symbol == formal.symbol)?;
            lengths::actual(
                program,
                caller.machine,
                source,
                arguments[position],
                &length_bindings,
                &mut engine,
            )?
        }
        _ => rank_coordinate(program, caller.machine, &mut engine, destination_measure)?,
    };
    let pinned_view_bound = match (view_bound, destination_measure) {
        (Some(bound), RankingRangeMeasure::IncreasingTo { limit, .. }) => {
            Some((bound, engine.normalize(limit)?))
        }
        (None, RankingRangeMeasure::Single(_))
        | (None, RankingRangeMeasure::Computed { .. })
        | (None, RankingRangeMeasure::Distance { .. })
        | (None, RankingRangeMeasure::SliceLength(_)) => None,
        _ => return None,
    };
    let destination_range = if callee.range.is_valid() {
        let ExpressionNode::Range(range) = program.expression_table.expression(callee.range) else {
            return None;
        };
        Some((
            engine.normalize(range.start)?,
            engine.normalize(range.end)?,
            range.end_inclusive,
        ))
    } else {
        None
    };
    if engine.requires_unsatisfiable {
        return Some(RankingRangeCallProgress::Strict);
    }
    // A computed rank is a carrier value only while its body forms there,
    // on both sides of the call, from the caller's own hypotheses.
    if !forms_in_carrier(&engine, source_measure, &rank)
        || !forms_in_carrier(&engine, destination_measure, &next_rank)
    {
        return None;
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
    if let Some((floor, ceiling, inclusive)) = &source_range
        && !membership(&engine, source_measure, &rank, floor, ceiling, *inclusive)
    {
        return None;
    }
    if let Some((floor, ceiling, inclusive)) = &destination_range
        && !membership(
            &engine,
            destination_measure,
            &next_rank,
            floor,
            ceiling,
            *inclusive,
        )
    {
        return None;
    }
    if let (Some((floor, ceiling, _)), Some((next_floor, next_ceiling, _))) =
        (source_range, destination_range)
        && (!prove(next_floor.sub(&floor), 0)
            || !prove(floor.sub(&next_floor), 0)
            || !prove(next_ceiling.sub(&ceiling), 0)
            || !prove(ceiling.sub(&next_ceiling), 0))
    {
        return None;
    }
    let clamped = matches!(source_measure, RankingRangeMeasure::IncreasingTo { .. });
    // Without an authored range, natural subtraction still cannot descend out
    // of the well-founded carrier. Do not count mathematical underflow as a
    // valid unsigned rank merely because its raw polynomial decreases.
    if !clamped && (!prove(rank.clone(), 0) || !prove(next_rank.clone(), 0)) {
        return None;
    }
    let descent = rank.sub(&next_rank);
    // A raw decrease below zero is only a plateau step. Strict clamped
    // descent requires a positive source coordinate as well as raw descent.
    if (!clamped || prove(rank, 1)) && prove(descent.clone(), 1) {
        Some(RankingRangeCallProgress::Strict)
    } else {
        (prove(descent, 0) || (clamped && prove(Polynomial::default().sub(&next_rank), 0)))
            .then_some(RankingRangeCallProgress::NonIncreasing)
    }
}

/// The caller-side premise carriers a call-site judgment may read, resolved
/// through the member's exact scalar entry (see `ranking_range_premise_symbols`).
/// `None` when the member has no supported scalar witness.
pub(crate) fn call_member_premise_symbols(
    program: &TypedTrees,
    member: &RankingRangeCallMember<'_>,
) -> Option<Vec<SymbolHandle>> {
    let (_, measure) = scalar_entry(program, member)?;
    super::ranking_range_premise_symbols(program, member.machine, member.range, measure)
}

fn admit_member(
    program: &TypedTrees,
    member: &RankingRangeCallMember<'_>,
    state: &State,
    measure: RankingRangeMeasure,
) -> Option<()> {
    meanings::builtin(program, member.machine, state, member.subject, 0)?;
    if member.paired_subject.is_valid() {
        meanings::builtin(program, member.machine, state, member.paired_subject, 0)?;
    }
    if member.range.is_valid() {
        let ExpressionNode::Range(range) = program.expression_table.expression(member.range) else {
            return None;
        };
        for expression in [range.start, range.end] {
            meanings::builtin(program, member.machine, state, expression, 0)?;
        }
    }
    if let RankingRangeMeasure::IncreasingTo { limit, .. } = measure {
        meanings::builtin(program, member.machine, state, limit, 0)?;
    }
    Some(())
}

/// The facts a member's own arrival judgment establishes for its authored
/// range at every internal arrival: exactly what `membership` proves for the
/// produced rank, plus carrier formation for a computed rank. The floor side
/// of `Nat::IncreasingTo` membership is a disjunction (`max(0, d) >= floor`),
/// which no single comparison states, so only its ceiling facts are consumed.
fn arrival_invariant(
    measure: RankingRangeMeasure,
    rank: &Polynomial,
    floor: &Polynomial,
    ceiling: &Polynomial,
    inclusive: bool,
) -> Vec<Comparison> {
    let below_ceiling = if inclusive {
        BinaryOperator::LessOrEqual
    } else {
        BinaryOperator::Less
    };
    let mut facts = Vec::new();
    match measure {
        RankingRangeMeasure::Single(_)
        | RankingRangeMeasure::Computed { .. }
        | RankingRangeMeasure::Distance { .. }
        | RankingRangeMeasure::SliceLength(_) => {
            facts.push((
                BinaryOperator::GreaterOrEqual,
                rank.clone(),
                Polynomial::default(),
            ));
            facts.push((BinaryOperator::GreaterOrEqual, rank.clone(), floor.clone()));
            facts.push((below_ceiling, rank.clone(), ceiling.clone()));
        }
        RankingRangeMeasure::IncreasingTo { .. } => {
            facts.push((below_ceiling, Polynomial::default(), ceiling.clone()));
            facts.push((below_ceiling, rank.clone(), ceiling.clone()));
        }
        RankingRangeMeasure::Field { .. } => {}
    }
    if let RankingRangeMeasure::Computed { carrier, .. } = measure
        && let Some(maximum) = super::carrier_maximum(carrier)
    {
        facts.push((BinaryOperator::LessOrEqual, rank.clone(), maximum));
    }
    facts
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
        // A slice length is already the produced natural coordinate; its
        // membership shape matches the scalar views.
        RankingRangeMeasure::Single(_)
        | RankingRangeMeasure::Computed { .. }
        | RankingRangeMeasure::Distance { .. }
        | RankingRangeMeasure::SliceLength(_) => {
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

/// The produced length coordinate of a slice-typed entry parameter: the
/// metadata atom shared with `.len` projections, never the collection value.
fn length_coordinate(
    program: &TypedTrees,
    state: &State,
    subject: ExpressionHandle,
    length_bindings: &[(symbols::SymbolHandle, String)],
) -> Option<Polynomial> {
    let parameter = lengths::parameter(program, state, subject)?;
    let (_, identity) = length_bindings
        .iter()
        .find(|(symbol, _)| *symbol == parameter.symbol)?;
    Some(Polynomial::atom(identity.clone()))
}

/// A computed rank forms inside its carrier; every other measure already
/// denotes a carrier value or a produced natural.
fn forms_in_carrier(engine: &Engine<'_>, measure: RankingRangeMeasure, rank: &Polynomial) -> bool {
    let RankingRangeMeasure::Computed { carrier, .. } = measure else {
        return true;
    };
    super::carrier_maximum(carrier).is_some_and(|maximum| {
        engine.prove_at_least(
            &engine.substituted(&maximum.sub(rank)),
            &BigInt::from_i64(0),
        )
    })
}

fn rank_coordinate(
    program: &TypedTrees,
    machine: &Machine,
    engine: &mut Engine<'_>,
    measure: RankingRangeMeasure,
) -> Option<Polynomial> {
    match measure {
        RankingRangeMeasure::Single(subject) => engine.normalize(subject),
        RankingRangeMeasure::Computed {
            subject,
            parameter,
            body,
            ..
        } => super::computed_rank(program, machine, engine, subject, parameter, body),
        // Keep the raw coordinate polynomial; the call-owned membership and
        // comparison predicates interpret its max(0,d) normalization.
        RankingRangeMeasure::IncreasingTo { subject, limit } => {
            Some(engine.normalize(limit)?.sub(&engine.normalize(subject)?))
        }
        RankingRangeMeasure::Distance { lower, upper } => {
            Some(engine.normalize(upper)?.sub(&engine.normalize(lower)?))
        }
        _ => None,
    }
}

/// Site-scoped arithmetic atoms. Every site formal keeps its own binding;
/// each carried entry role additionally binds to its first carrier's atom, so
/// entry-spelled expressions normalize to the value the site holds. A role
/// with several carriers denotes copies the telescope kept only because every
/// arrival forwarded a bare name — a contested computed claim demotes before
/// reaching this judgment — so the extra carriers hold that same value and
/// contribute an explicit equality hypothesis rather than a second binding.
fn telescoped_bindings(
    program: &TypedTrees,
    state: &State,
    entry_parameters: &[SymbolHandle],
) -> Option<(Vec<StrictArithmeticSymbolBinding>, Vec<Comparison>)> {
    let mut bindings = integer_bindings(program, state)?;
    let mut equalities = Vec::new();
    let formals = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    if formals.len() != entry_parameters.len() {
        return None;
    }
    for (formal, role) in formals.iter().zip(entry_parameters) {
        if !role.is_valid() {
            continue;
        }
        let Some(binding) = bindings
            .iter()
            .find(|binding| binding.symbol == formal.symbol)
        else {
            continue;
        };
        let StrictArithmeticBindingValue::Atom { identity, unsigned } = &binding.value else {
            continue;
        };
        if let Some(existing) = bindings.iter().find(|binding| binding.symbol == *role) {
            let StrictArithmeticBindingValue::Atom {
                identity: existing, ..
            } = &existing.value
            else {
                return None;
            };
            if *existing != *identity {
                equalities.push((
                    BinaryOperator::Equal,
                    Polynomial::atom(existing.clone()),
                    Polynomial::atom(identity.clone()),
                ));
            }
            continue;
        }
        bindings.push(StrictArithmeticSymbolBinding {
            symbol: *role,
            value: StrictArithmeticBindingValue::Atom {
                identity: identity.clone(),
                unsigned: *unsigned,
            },
        });
    }
    Some((bindings, equalities))
}

/// The same role aliasing applied to produced length coordinates: each carried
/// entry role of a slice-typed site formal binds its first carrier's length
/// atom, and the remaining carriers contribute equalities between their length
/// coordinates. A copied collection keeps the same produced length; a windowed
/// or diverging claimant was already demoted by discovery.
fn telescoped_length_bindings(
    program: &TypedTrees,
    state: &State,
    entry_parameters: &[SymbolHandle],
) -> (Vec<(SymbolHandle, String)>, Vec<Comparison>) {
    let bindings = lengths::bindings(program, state, None);
    let mut roles: Vec<(SymbolHandle, String)> = Vec::new();
    let mut equalities = Vec::new();
    for (formal, role) in program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .zip(entry_parameters)
    {
        if !role.is_valid() {
            continue;
        }
        let Some((_, identity)) = bindings.iter().find(|(symbol, _)| *symbol == formal.symbol)
        else {
            continue;
        };
        if let Some((_, existing)) = roles.iter().find(|(symbol, _)| *symbol == *role) {
            if *existing != *identity {
                equalities.push((
                    BinaryOperator::Equal,
                    Polynomial::atom(existing.clone()),
                    Polynomial::atom(identity.clone()),
                ));
            }
            continue;
        }
        roles.push((*role, identity.clone()));
    }
    (roles, equalities)
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
    if witness.subjects.len() != custody.subjects.len()
        || witness.rank_range.is_some() != custody.rank_range.is_some()
        || custody.rank_range.unwrap_or_default() != member.range
        || custody.rank_range.is_some_and(|range| !range.is_valid())
    {
        return None;
    }
    let state = program.machine_states(machine).first()?;
    if Some(witness.view_path.as_str()) != witness.ranking_view.canonical_path() {
        // A declared scalar view: the shared classification admits the
        // measure for this exact subject. An identity forward produces the
        // subject itself; a computation produces its body over the subject,
        // and the judgments prove that rank's formation inside the carrier.
        // Any other authored path has no scalar transport.
        let [subject] = custody.subjects.as_slice() else {
            return None;
        };
        if witness.ranking_view.is_valid()
            || !witness.view_arguments.is_empty()
            || !custody.view_arguments.is_empty()
            || *subject != member.subject
            || member.paired_subject.is_valid()
        {
            return None;
        }
        let view = super::declared_scalar_view(program, state, *subject, &witness.view_path)?;
        entry_scalar_parameter(program, state, *subject)?;
        let measure = match view.computation {
            None => RankingRangeMeasure::Single(member.subject),
            Some(computation) => RankingRangeMeasure::Computed {
                subject: member.subject,
                parameter: computation.parameter,
                body: computation.body,
                carrier: view.carrier,
            },
        };
        return Some((state, measure));
    }
    let measure = match witness.ranking_view {
        language_semantics::RankingViewId::NAT_DESCENDING
            if witness.view_arguments.is_empty() && custody.view_arguments.is_empty() =>
        {
            let [subject] = custody.subjects.as_slice() else {
                return None;
            };
            if *subject != member.subject || member.paired_subject.is_valid() {
                return None;
            }
            RankingRangeMeasure::Single(member.subject)
        }
        language_semantics::RankingViewId::NAT_INCREASING_TO
            if witness.view_arguments.len() == 1 =>
        {
            let [subject] = custody.subjects.as_slice() else {
                return None;
            };
            if *subject != member.subject || member.paired_subject.is_valid() {
                return None;
            }
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
        language_semantics::RankingViewId::NAT_BOUNDED_DISTANCE
            if witness.view_arguments.is_empty() && custody.view_arguments.is_empty() =>
        {
            let [lower, upper] = custody.subjects.as_slice() else {
                return None;
            };
            if *lower != member.subject || *upper != member.paired_subject {
                return None;
            }
            RankingRangeMeasure::Distance {
                lower: member.subject,
                upper: member.paired_subject,
            }
        }
        language_semantics::RankingViewId::SLICE_LENGTH
            if witness.view_arguments.is_empty() && custody.view_arguments.is_empty() =>
        {
            let [subject] = custody.subjects.as_slice() else {
                return None;
            };
            if *subject != member.subject || member.paired_subject.is_valid() {
                return None;
            }
            RankingRangeMeasure::SliceLength(member.subject)
        }
        _ => return None,
    };
    // Authored subjects and endpoint expressions name entry parameters; the
    // member's own witness owns every internal arrival, so only the entry
    // binding is checked here.
    for subject in [member.subject, member.paired_subject]
        .into_iter()
        .filter(|subject| subject.is_valid())
    {
        if matches!(measure, RankingRangeMeasure::SliceLength(_)) {
            // The ranked subject is the exact slice-typed entry parameter;
            // its produced length coordinate, not the collection value,
            // carries the rank.
            lengths::parameter(program, state, subject)?;
        } else {
            entry_scalar_parameter(program, state, subject)?;
        }
    }
    Some((state, measure))
}

/// A scalar view subject must arrive as the machine's own exact unsigned
/// entry parameter: a bare (possibly atomically wrapped) name bound to a
/// non-self, non-const `u8..=u64` formal.
fn entry_scalar_parameter(
    program: &TypedTrees,
    state: &State,
    subject: ExpressionHandle,
) -> Option<()> {
    let mut subject = subject;
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
        || parameter.is_const
        || !matches!(
            exact_integer_parameter(program, parameter.type_reference),
            Some(PrimitiveType::U8 | PrimitiveType::U16 | PrimitiveType::U32 | PrimitiveType::U64)
        )
    {
        return None;
    }
    Some(())
}
