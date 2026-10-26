//! Invocation rank transport with optional authored ranges. Each side retains
//! its own template, including when a recursive invocation reuses declaration
//! handles. Actual values are captured in the caller namespace before the
//! destination's formal and projection namespace replaces it. Source ranks and
//! hypotheses are already polynomials, so rebinding cannot rewrite their past.

use super::coordinate::{CoordinateFamily, CoordinateSet};
use super::{
    BigInt, BinaryOperator, Comparison, Engine, ExpressionHandle, ExpressionNode, Machine,
    MeasureBodyShape, Polynomial, RankingRangeMeasure, State, collect_guard, entry_comparisons,
    field_coordinates, fields, find_declared_measure, integer_bindings, integer_parameter, lengths,
    meanings, measure_body_shape, parameter_comparisons, projections, saved_arguments,
    validate_mapping,
};
use crate::{StrictArithmeticExpressionBinding, StrictArithmeticSymbolBinding};
use symbols::SymbolHandle;
mod endpoint_pins;
pub(crate) use endpoint_pins::{RankingRangeCallEdge, mixed_call_endpoints_are_pinned};

/// The exact transition site inside the caller's machine. `state` is the state
/// whose statements carry the call; `entry_parameters` is the source-admitted
/// assignment of full entry coordinates to that state's completed readings.
/// Authored subjects and endpoints retain their exact entry identity; no type,
/// duplicated leaf or arithmetic shape supplies a missing correspondence.
pub(crate) struct RankingRangeCallSite<'program> {
    pub(crate) state: &'program State,
    pub(crate) entry_parameters: &'program super::RankStateBindings,
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

/// The caller owns prefix stability and whole-component cycle coverage. This
/// judgment reads only caller entry hypotheses, never destination requirements,
/// and proves nonincrease or strict descent with pinned view bounds. Authored
/// ranges additionally require membership and equality of their endpoints.
/// When only one side authors a range, the component owner must separately
/// conserve its endpoint inputs through every unranged participant.
pub(crate) fn prove_ranking_range_call(
    program: &dyn crate::CheckingInput,
    caller: RankingRangeCallMember<'_>,
    caller_site: &RankingRangeCallSite<'_>,
    callee: RankingRangeCallMember<'_>,
    guards: &[(ExpressionHandle, bool)],
    arguments: &[ExpressionHandle],
) -> Option<RankingRangeCallProgress> {
    let (entry, source_measure) = scalar_entry(program, &caller)?;
    let (destination, destination_measure) = scalar_entry(program, &callee)?;
    let source = caller_site.state;
    if !match (source_measure, destination_measure) {
        (RankingRangeMeasure::Single(_), RankingRangeMeasure::Single(_))
        | (RankingRangeMeasure::IncreasingTo { .. }, RankingRangeMeasure::IncreasingTo { .. })
        | (RankingRangeMeasure::Distance { .. }, RankingRangeMeasure::Distance { .. })
        | (RankingRangeMeasure::SliceLength(_), RankingRangeMeasure::SliceLength(_))
        | (RankingRangeMeasure::Computed { .. }, RankingRangeMeasure::Computed { .. }) => true,
        // A field-view pair shares one produced coordinate only through the
        // same declared measure; same-shaped projections of different
        // declarations name different orders.
        (
            RankingRangeMeasure::Field {
                measure: source_measure,
                ..
            },
            RankingRangeMeasure::Field {
                measure: destination_measure,
                ..
            },
        ) => source_measure == destination_measure,
        _ => false,
    } {
        return None;
    }
    let admit_source =
        |expression| meanings::admitted_carrier(program, caller.machine, source, expression, 0);
    // Both selected scalar views already produce natural ranks. An absent
    // optional range adds no endpoint obligations or synthetic bound. Authored
    // subjects, endpoints, and limits name entry parameters, so their builtin
    // meaning is admitted at the entry state even when the call sits inside a
    // subordinate state.
    admit_member(program, &caller, entry, source_measure)?;
    admit_member(program, &callee, destination, destination_measure)?;
    // An authored range keeps the state-edge owner's endpoint formation bar
    // on both sides: an exact integer formal, an exact declared projection,
    // or a bounded integer expression, each in its own entry scope -- never a
    // selected computation smuggled through the call. An endpoint declaration
    // bounds alone cannot place is deferred, not rejected: each side's
    // installed hypotheses still owe it a carrier landing, exactly as at a
    // state edge.
    let mut caller_deferred_endpoints = Vec::new();
    let mut callee_deferred_endpoints = Vec::new();
    for (machine, state, range, deferred) in [
        (
            caller.machine,
            entry,
            caller.range,
            &mut caller_deferred_endpoints,
        ),
        (
            callee.machine,
            destination,
            callee.range,
            &mut callee_deferred_endpoints,
        ),
    ] {
        if range.is_valid()
            && let ExpressionNode::Range(range) = program.expression_table.expression(range)
        {
            for endpoint in [range.start, range.end] {
                if !fields::endpoint_statically_formed(program, machine, state, endpoint) {
                    deferred.push(endpoint);
                }
            }
        }
    }
    for &(guard, _) in guards {
        admit_source(guard)?;
    }
    for argument in arguments {
        admit_source(*argument)?;
    }
    let at_entry = source.symbol == entry.symbol;
    let bindings = if at_entry {
        integer_bindings(program, source)?
    } else {
        validate_mapping(
            program,
            caller.machine,
            source,
            caller_site.entry_parameters,
        )?;
        telescoped_bindings(
            program,
            caller.machine,
            source,
            caller_site.entry_parameters,
        )?
    };
    let mut engine = Engine::strict(program, caller.machine, &bindings);
    if !engine.strict_symbol_bindings_are_valid() {
        return None;
    }
    // A slice-length member's rank is its collection's length coordinate:
    // distinct atoms, never the scalar value of the slice parameter itself.
    // Every other measure still owes a `.len` inside a guard, actual,
    // endpoint, or requires fact the same produced-length binding: the leaf
    // is a non-polynomial input, not an operand polynomial.
    let mut length_bindings = lengths::bindings(program, caller.machine, source, None);
    if !at_entry {
        // The ranked slice subject stays entry-spelled: alias its entry
        // parameter to the site carrier's length atom.
        let roles = telescoped_length_bindings(
            program,
            caller.machine,
            source,
            caller_site.entry_parameters,
        );
        for (symbol, identity) in roles {
            if !length_bindings.iter().any(|(bound, _)| *bound == symbol) {
                length_bindings.push((symbol, identity));
            }
        }
    }
    // Declared array extents need installation even without dynamic slices.
    let expressions = installation_expressions(
        program,
        &caller,
        source,
        scalar_subjects(&caller, source_measure),
        guards,
        arguments,
    );
    lengths::install(
        program,
        caller.machine,
        source,
        entry,
        &length_bindings,
        &mut engine,
        &expressions,
    )?;
    // A field-view member's rank is its record's exact projection coordinate:
    // a fresh atom per carrier formal, shared with every authored
    // `record.field` chain the site can read. Resolve the measure's chain on
    // the member's entry formal, then through this site's telescope onto the
    // formal that actually carries the role -- an unmapped or duplicated
    // carrier resolves nothing, so the coordinate cannot borrow a foreign
    // record's lineage. Install it like the named-state judgment so a member
    // projection inside a guard, actual, endpoint, or requires fact names
    // the same atom.
    let field_ranks = if let RankingRangeMeasure::Field { subject, measure } = source_measure {
        let coordinate = fields::FieldCoordinate::resolve_field(program, entry, subject, measure)?;
        let coordinate = coordinate.arrive_unless_at_entry(
            program,
            source,
            caller_site.entry_parameters,
            at_entry,
        )?;
        let mut coordinates = field_coordinates::FieldCoordinates::new(coordinate);
        install_call_coordinates(
            program,
            &caller,
            source,
            entry,
            caller_site,
            at_entry,
            &mut engine,
            &mut coordinates,
            vec![subject],
            guards,
            arguments,
        )?;
        Some(coordinates)
    } else {
        // A member-chain scalar subject produces its rank from projected
        // storage: the chain resolves against the entry formal's own
        // declaration, then re-resolves through this site's telescope onto
        // the formal actually carrying the record role. Guards, actuals, and
        // endpoints spelling the same member chain bind to the same atom.
        let subjects = scalar_subjects(&caller, source_measure);
        let mut coordinates = field_coordinates::FieldCoordinates::empty();
        field_coordinates::include_subject_coordinates(
            program,
            entry,
            source,
            (!at_entry).then_some(caller_site.entry_parameters),
            &subjects,
            &mut coordinates,
        )?;
        install_call_coordinates(
            program,
            &caller,
            source,
            entry,
            caller_site,
            at_entry,
            &mut engine,
            &mut coordinates,
            subjects.clone(),
            guards,
            arguments,
        )?;
        Some(coordinates)
    };
    // A member-chain slice subject produces its length from the exact
    // projected coordinate of the carrier formal's record, resolved at the
    // entry scope and re-resolved onto the site's carrier. Bare slice
    // formals keep the `length_bindings` coordinate installed above.
    let slice_ranks = if let RankingRangeMeasure::SliceLength(subject) = source_measure {
        let resolved = match lengths::SliceCoordinate::resolve_length(program, entry, subject) {
            Some(coordinate) => {
                let coordinate = coordinate.arrive_unless_at_entry(
                    program,
                    source,
                    caller_site.entry_parameters,
                    at_entry,
                )?;
                Some(coordinate)
            }
            // Bare and projected descriptors use the same selected full
            // coordinate; another slice leaf supplies no correspondence.
            None if !at_entry => lengths::parameter(program, entry, subject)
                .and_then(|formal| {
                    caller_site
                        .entry_parameters
                        .destination(&super::RankSubjectCoordinate {
                            parameter: formal.symbol,
                            path: Vec::new(),
                        })
                })
                .and_then(|destination| {
                    if destination.path.is_empty() {
                        None
                    } else {
                        lengths::SliceCoordinate::at_coordinate(
                            program,
                            source,
                            &destination,
                            super::coordinate::CoordinateKind::Length,
                        )
                    }
                }),
            None => None,
        };
        match resolved {
            Some(coordinate) => {
                let mut coordinates = lengths::SliceCoordinates::new(coordinate);
                install_call_coordinates(
                    program,
                    &caller,
                    source,
                    entry,
                    caller_site,
                    at_entry,
                    &mut engine,
                    &mut coordinates,
                    vec![subject],
                    guards,
                    arguments,
                )?;
                Some(coordinates)
            }
            None => None,
        }
    } else {
        // `.len` spellings inside endpoints, guards, or actuals on another
        // measure name the same projected slice coordinates a SliceLength
        // subject does; install them over the caller's whole read surface.
        let mut coordinates = lengths::SliceCoordinates::empty();
        install_call_coordinates(
            program,
            &caller,
            source,
            entry,
            caller_site,
            at_entry,
            &mut engine,
            &mut coordinates,
            scalar_subjects(&caller, source_measure),
            guards,
            arguments,
        )?;
        Some(coordinates)
    };
    // Saved observations consume the complete caller coordinate roster.
    // In particular, a saved Boolean over a field must see that field's
    // exact atom before its comparison is captured. Written or exclusively
    // exposed bindings remain excluded by the saved-observation owner.
    saved_arguments::install(
        program,
        caller.machine,
        source,
        &length_bindings,
        &mut engine,
    );
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
    comparisons.extend(super::length_bound_comparisons(&length_bindings));
    if let Some(coordinates) = &field_ranks {
        // The produced coordinate's own natural bounds and declared field
        // constraints are hypotheses, as at a named-state edge.
        comparisons.extend(coordinates.comparisons(program));
    }
    if let Some(coordinates) = &slice_ranks {
        comparisons.extend(coordinates.comparisons(program));
    }
    for &(guard, holds) in guards {
        if !engine.capture_expression_calls(guard) {
            return None;
        }
        collect_guard(&mut engine, guard, holds, &mut comparisons, 0)?;
    }
    let rank = match source_measure {
        RankingRangeMeasure::SliceLength(subject) => match &slice_ranks {
            Some(slice) => slice.value()?,
            None => length_coordinate(program, entry, subject, &length_bindings)?,
        },
        RankingRangeMeasure::Field { .. } => field_ranks.as_ref()?.value()?,
        _ => rank_coordinate(program, caller.machine, &mut engine, source_measure)?,
    };
    let view_bound = match source_measure {
        RankingRangeMeasure::IncreasingTo { limit, .. }
        | RankingRangeMeasure::Distance { upper: limit, .. } => Some(engine.normalize(limit)?),
        _ => None,
    };
    // The entry-spelled endpoint reads the site's already-established
    // caller coordinates; neither callee formals nor callee requirements are
    // available yet.
    let source_range = member_call_range(program, &caller, entry, &mut engine)?;
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
    engine.refresh_opaque_intervals();
    // A caller endpoint that declaration bounds alone could not place owes
    // its carrier landing under this site's hypotheses: requires facts at
    // entry, or the member's own range invariant re-established at a
    // subordinate arrival. A dead site stays vacuous.
    if !engine.requires_unsatisfiable
        && let Some((floor, ceiling, _)) = &source_range
    {
        let range = program.expression_table.range_node(caller.range)?;
        for endpoint in &caller_deferred_endpoints {
            let polynomial = if *endpoint == range.start {
                floor
            } else {
                ceiling
            };
            if !fields::endpoint_lands_under(
                &mut engine,
                program,
                caller.machine,
                entry,
                *endpoint,
                polynomial,
            ) {
                return None;
            }
        }
    }
    let parameters = program.state_parameters(destination).iter();
    if parameters.clone().count() != arguments.len() {
        return None;
    }
    let demanded = super::ranking_range_required_coordinates(
        program,
        callee.machine,
        callee.range,
        destination_measure,
        super::RankingRangePremises::RankInvariant,
    )?;
    let mut actuals = Vec::new();
    for (parameter, argument) in parameters.zip(arguments) {
        if !demanded.iter().any(|coordinate| {
            coordinate.parameter == parameter.symbol && coordinate.path.is_empty()
        }) {
            continue;
        }
        if super::integer_parameter(program, parameter.type_reference).is_none() {
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
    // A scalar destination's actual may itself be a quotient, remainder, or
    // shift tree over caller inputs: bind each term's exact-integer meaning
    // the same way the endpoints were bound, so normalization keeps both
    // operands' identity instead of refusing the term. Formation stays with
    // the actual's own evaluation; a literal zero modulus or out-of-width
    // constant shift count fails the bind.
    for binding in &actuals {
        if let Some(observed) = fields::completed_scalar_actual(
            program,
            caller.machine,
            source,
            &mut engine,
            binding.expression,
            &[],
        ) && !engine.bind_strict_occurrence(binding.expression, observed)
        {
            return None;
        }
        if !engine.capture_expression_calls(binding.expression) {
            return None;
        }
        meanings::install_nonpolynomial_terms(
            program,
            caller.machine,
            source,
            &mut engine,
            binding.expression,
            0,
        )?;
    }
    // Resolve every demanded caller value before adding any callee formal.
    // Unrelated accumulators remain payload; required unknown values still
    // fail before vacuity. Whole-argument execution and effects belong to the
    // source checking and completed-operand owners.
    let destination_formals = engine.exact_argument_values(&actuals)?;
    // Authored member projections in the callee's entry scope bind to the
    // value the exact actual installs: a member-chain subject, a view bound,
    // or an endpoint names the coordinate its own declaration resolves,
    // walked over the actual. A member the coordinate cannot read stays
    // unbound and fails normalization below.
    let mut destination_expressions = vec![callee.subject];
    if callee.paired_subject.is_valid() {
        destination_expressions.push(callee.paired_subject);
    }
    if let RankingRangeMeasure::IncreasingTo { limit, .. } = destination_measure {
        destination_expressions.push(limit);
    }
    if callee.range.is_valid()
        && let ExpressionNode::Range(range) = program.expression_table.expression(callee.range)
    {
        destination_expressions.extend([range.start, range.end]);
    }
    let mut destination_projections = Vec::new();
    for expression in destination_expressions {
        collect_destination_fields(
            program,
            caller.machine,
            destination,
            source,
            &mut engine,
            arguments,
            expression,
            &mut destination_projections,
        )?;
        collect_destination_lengths(
            program,
            caller.machine,
            destination,
            source,
            &mut engine,
            arguments,
            &length_bindings,
            expression,
            &mut destination_projections,
        )?;
    }
    let captured_destination_rank = match destination_measure {
        // The callee's ranked slice formal arrives as this call's exact
        // actual; its produced length is the actual's own coordinate, not a
        // forwarded caller parameter. A member-chain subject reads the
        // projected coordinate its carrier formal's declaration names, walked
        // over the actual's forward or rebuilt literal.
        RankingRangeMeasure::SliceLength(subject) => {
            if let Some(coordinate) =
                lengths::SliceCoordinate::resolve_length(program, destination, subject)
            {
                let position = typed_trees::signature::state_parameter_position(
                    program.state_parameters(destination),
                    coordinate.parameter.symbol,
                )?;
                Some(coordinate.arrived_length(
                    program,
                    caller.machine,
                    source,
                    &mut engine,
                    arguments[position],
                    coordinate.borrowed,
                    &length_bindings,
                )?)
            } else {
                let formal = lengths::parameter(program, destination, subject)?;
                let position = typed_trees::signature::state_parameter_position(
                    program.state_parameters(destination),
                    formal.symbol,
                )?;
                Some(lengths::actual(
                    program,
                    caller.machine,
                    source,
                    arguments[position],
                    &length_bindings,
                    &mut engine,
                )?)
            }
        }
        // The callee's ranked record formal arrives as this call's exact
        // actual; its produced coordinate is the shared measure's chain
        // walked over that actual -- a forwarded carrier, a borrow of one, a
        // prefix member of a larger carrier, or a literal rebuilt declaration
        // by declaration. The destination formal's own reference boundary
        // decides whether the actual reads under one `&`.
        RankingRangeMeasure::Field { subject, measure } => {
            let destination_coordinate =
                fields::FieldCoordinate::resolve_field(program, destination, subject, measure)?;
            let position = program
                .state_parameters(destination)
                .iter()
                .position(|parameter| {
                    parameter.symbol == destination_coordinate.parameter.symbol
                })?;
            Some(destination_coordinate.field_actual(
                program,
                caller.machine,
                source,
                &mut engine,
                arguments[position],
            )?)
        }
        _ => None,
    };
    // This is an invocation boundary, not a same-frame consistency binding.
    // All actuals and projected actuals were resolved before any formal changes.
    engine.bind_goal_formals(destination_formals);
    for (expression, value) in destination_projections {
        if !engine.bind_strict_projection(expression, value) {
            return None;
        }
    }
    let next_rank = match captured_destination_rank {
        Some(rank) => rank,
        None => rank_coordinate(program, caller.machine, &mut engine, destination_measure)?,
    };
    let pinned_view_bound = match (view_bound, destination_measure) {
        (Some(bound), RankingRangeMeasure::IncreasingTo { limit, .. })
        | (Some(bound), RankingRangeMeasure::Distance { upper: limit, .. }) => {
            Some((bound, engine.normalize(limit)?))
        }
        (None, RankingRangeMeasure::Single(_))
        | (None, RankingRangeMeasure::Computed { .. })
        | (None, RankingRangeMeasure::Field { .. })
        | (None, RankingRangeMeasure::SliceLength(_)) => None,
        _ => return None,
    };
    // Meaning belongs to the callee, but these formals and projections now
    // denote the actuals installed by this call. Bind only now: an earlier
    // occurrence binding would freeze the wrong namespace.
    let destination_range = member_call_range(program, &callee, destination, &mut engine)?;
    engine.refresh_opaque_intervals();
    if engine.requires_unsatisfiable {
        return Some(RankingRangeCallProgress::Strict);
    }
    // The destination's deferred endpoints land under the caller's
    // hypotheses with this call's actuals already bound: the range the call
    // feeds must be a defined interval of the callee's carrier here, not only
    // inside the callee's own entry proof.
    if let Some((floor, ceiling, _)) = &destination_range {
        let range = program.expression_table.range_node(callee.range)?;
        for endpoint in &callee_deferred_endpoints {
            let polynomial = if *endpoint == range.start {
                floor
            } else {
                ceiling
            };
            if !fields::endpoint_lands_under(
                &mut engine,
                program,
                callee.machine,
                destination,
                *endpoint,
                polynomial,
            ) {
                return None;
            }
        }
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
    program: &dyn crate::CheckingInput,
    member: &RankingRangeCallMember<'_>,
) -> Option<Vec<super::RankPremiseInput>> {
    let (_, measure) = scalar_entry(program, member)?;
    super::ranking_range_premise_symbols(program, member.machine, member.range, measure)
}

fn admit_member(
    program: &dyn crate::CheckingInput,
    member: &RankingRangeCallMember<'_>,
    state: &State,
    measure: RankingRangeMeasure,
) -> Option<()> {
    meanings::admitted_carrier(program, member.machine, state, member.subject, 0)?;
    if member.paired_subject.is_valid() {
        meanings::admitted_carrier(program, member.machine, state, member.paired_subject, 0)?;
    }
    if member.range.is_valid() {
        let range = program.expression_table.range_node(member.range)?;
        for expression in [range.start, range.end] {
            meanings::admitted_carrier(program, member.machine, state, expression, 0)?;
        }
    }
    if let RankingRangeMeasure::IncreasingTo { limit, .. } = measure {
        meanings::admitted_carrier(program, member.machine, state, limit, 0)?;
    }
    Some(())
}

/// The facts a member's own arrival judgment establishes for its authored
/// range at every internal arrival: exactly what `membership` proves for the
/// produced rank, plus carrier formation for a computed rank. The floor side
/// of `Nat::IncreasingTo` membership is a disjunction (`max(0, d) >= floor`),
/// which no single comparison states, so only its ceiling facts are consumed.
pub(super) fn arrival_invariant(
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
        | RankingRangeMeasure::Field { .. }
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
    }
    if let RankingRangeMeasure::Computed { carrier, .. } = measure
        && let Some(maximum) = super::carrier_maximum(carrier)
    {
        facts.push((BinaryOperator::LessOrEqual, rank.clone(), maximum));
    }
    facts
}

pub(super) fn membership(
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
        // A slice length or a field projection is already the produced
        // natural coordinate; its membership shape matches the scalar views.
        RankingRangeMeasure::Single(_)
        | RankingRangeMeasure::Computed { .. }
        | RankingRangeMeasure::Distance { .. }
        | RankingRangeMeasure::Field { .. }
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
    }
}

/// Capture every exact member projection inside `expression` that is rooted at
/// a `destination` record formal as the value its call actual installs: the
/// resolved chain re-resolved over the actual's own carrier. Members that do
/// not resolve -- or whose actual the coordinate cannot read -- stay unbound,
/// so an opaque endpoint still fails `normalize` rather than borrowing a
/// foreign record's lineage.
fn collect_destination_fields(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    destination: &State,
    source: &State,
    engine: &mut Engine<'_>,
    arguments: &[ExpressionHandle],
    expression: ExpressionHandle,
    projections: &mut Vec<(ExpressionHandle, Polynomial)>,
) -> Option<()> {
    let mut pending = vec![(expression, 0usize)];
    while let Some((expression, depth)) = pending.pop() {
        let node = program
            .expression_table
            .expression_at_depth(expression, depth)?;
        match node {
            ExpressionNode::Member(member) => {
                if let Some(coordinate) = fields::FieldCoordinate::resolve_field_projection(
                    program,
                    destination,
                    expression,
                ) && let Some(position) = typed_trees::signature::state_parameter_position(
                    program.state_parameters(destination),
                    coordinate.parameter.symbol,
                ) && let Some(actual) =
                    coordinate.field_actual(program, machine, source, engine, arguments[position])
                {
                    projections.push((expression, actual));
                }
                pending.push((member.receiver, depth + 1));
            }
            ExpressionNode::Borrow(borrow) => pending.push((borrow.target, depth + 1)),
            ExpressionNode::Binary(binary) => {
                pending.push((binary.left, depth + 1));
                pending.push((binary.right, depth + 1));
            }
            ExpressionNode::Unary(unary) => pending.push((unary.operand, depth + 1)),
            ExpressionNode::Atomic(atomic) => pending.push((atomic.value, depth + 1)),
            ExpressionNode::Range(range) => {
                pending.push((range.start, depth + 1));
                pending.push((range.end, depth + 1));
            }
            _ => {}
        }
    }
    Some(())
}

/// Capture every authored `.len` spelling inside `expression` whose receiver is
/// an exact member chain of a `destination` record formal as the produced
/// length its call actual installs: the projected slice coordinate walked
/// over the actual's forward, borrow, or rebuilt literal. A `.len` whose
/// receiver resolves no projected coordinate -- or whose actual the
/// coordinate cannot read -- stays unbound, so an opaque endpoint still
/// fails `normalize` rather than naming a guessed length.
fn collect_destination_lengths(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    destination: &State,
    source: &State,
    engine: &mut Engine<'_>,
    arguments: &[ExpressionHandle],
    bindings: &[(SymbolHandle, String)],
    expression: ExpressionHandle,
    projections: &mut Vec<(ExpressionHandle, Polynomial)>,
) -> Option<()> {
    let mut pending = vec![(expression, 0usize)];
    while let Some((expression, depth)) = pending.pop() {
        let node = program
            .expression_table
            .expression_at_depth(expression, depth)?;
        match node {
            ExpressionNode::Member(member) => {
                if member.case_variant.is_none()
                    && let Some(receiver) = crate::value_custody::places::collection_length_receiver(
                        program,
                        machine,
                        Some(destination),
                        expression,
                    )
                {
                    let actual = if let Some(extent) =
                        crate::value_custody::places::declared_collection_extent(
                            program,
                            machine,
                            Some(destination),
                            receiver,
                        ) {
                        // The formal's fixed carrier requires this extent of
                        // every actual; unlike a slice, it has no live length
                        // value to transport from the caller.
                        Some(Polynomial::constant(BigInt::from_u64(extent as u64)))
                    } else if let Some(coordinate) =
                        lengths::SliceCoordinate::resolve_length(program, destination, receiver)
                    {
                        typed_trees::signature::state_parameter_position(
                            program.state_parameters(destination),
                            coordinate.parameter.symbol,
                        )
                        .and_then(|position| {
                            coordinate.arrived_length(
                                program,
                                machine,
                                source,
                                engine,
                                arguments[position],
                                coordinate.borrowed,
                                bindings,
                            )
                        })
                    } else {
                        // A bare slice formal's `.len` names the produced
                        // length its call actual installs, subslice geometry
                        // included.
                        lengths::parameter(program, destination, receiver).and_then(|formal| {
                            typed_trees::signature::state_parameter_position(
                                program.state_parameters(destination),
                                formal.symbol,
                            )
                            .and_then(|position| {
                                lengths::actual(
                                    program,
                                    machine,
                                    source,
                                    arguments[position],
                                    bindings,
                                    engine,
                                )
                            })
                        })
                    };
                    if let Some(actual) = actual {
                        projections.push((expression, actual));
                    }
                }
                pending.push((member.receiver, depth + 1));
            }
            ExpressionNode::Borrow(borrow) => pending.push((borrow.target, depth + 1)),
            ExpressionNode::Binary(binary) => {
                pending.push((binary.left, depth + 1));
                pending.push((binary.right, depth + 1));
            }
            ExpressionNode::Unary(unary) => pending.push((unary.operand, depth + 1)),
            ExpressionNode::Atomic(atomic) => pending.push((atomic.value, depth + 1)),
            ExpressionNode::Indexed(indexed) => {
                pending.push((indexed.collection, depth + 1));
                pending.push((indexed.index, depth + 1));
            }
            ExpressionNode::Range(range) => {
                pending.push((range.start, depth + 1));
                pending.push((range.end, depth + 1));
            }
            _ => {}
        }
    }
    Some(())
}

/// The produced length coordinate of a slice-typed entry parameter: the
/// metadata atom shared with `.len` projections, never the collection value.
fn length_coordinate(
    program: &dyn crate::CheckingInput,
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

pub(super) fn rank_coordinate(
    program: &dyn crate::CheckingInput,
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

/// Each site formal retains its own atom. Exact selected empty-entry
/// coordinates additionally bind the entry template to that site's selected
/// scalar value. Other descendants with the same ancestry remain independent;
/// correspondence never supplies a duplicate-copy equality hypothesis.
pub(super) fn telescoped_bindings(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    entry_parameters: &super::RankStateBindings,
) -> Option<Vec<StrictArithmeticSymbolBinding>> {
    let root = program.machine_states(machine).first()?;
    let mut bindings = integer_bindings(program, state)?;
    for selected in &entry_parameters.coordinates {
        if !selected.entry.path.is_empty() {
            continue;
        }
        let entry = program.state_parameter_by_symbol(root, selected.entry.parameter)?;
        if super::integer_parameter(program, entry.type_reference).is_none() {
            continue;
        }
        let value =
            super::scalar_coordinate_value(program, state, &selected.destination, &bindings)?;
        if let Some(existing) = bindings
            .iter()
            .find(|binding| binding.symbol == entry.symbol)
        {
            if existing.value != value {
                return None;
            }
            continue;
        }
        bindings.push(StrictArithmeticSymbolBinding {
            symbol: entry.symbol,
            value,
        });
    }
    Some(bindings)
}

/// Length aliases use the same exact selected coordinates as scalar aliases.
/// A second descriptor sharing ancestry contributes no equality premise.
pub(super) fn telescoped_length_bindings(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    entry_parameters: &super::RankStateBindings,
) -> Vec<(SymbolHandle, String)> {
    lengths::bindings(program, machine, state, Some(entry_parameters))
}

/// The scalar expressions whose member chains may carry a produced rank or
/// bound: the subject, a paired distance subject, and an `IncreasingTo`
/// view's authored limit.
/// The shared installation read surface: the seed subjects plus the
/// caller's range endpoints when spelled, the spelled arguments, the guard
/// conditions, and the caller's entry projections.
fn installation_expressions(
    program: &dyn crate::CheckingInput,
    caller: &RankingRangeCallMember<'_>,
    source: &State,
    mut expressions: Vec<ExpressionHandle>,
    guards: &[(ExpressionHandle, bool)],
    arguments: &[ExpressionHandle],
) -> Vec<ExpressionHandle> {
    if caller.range.is_valid()
        && let ExpressionNode::Range(range) = program.expression_table.expression(caller.range)
    {
        expressions.extend([range.start, range.end]);
    }
    expressions.extend(arguments.iter().copied());
    expressions.extend(guards.iter().map(|(guard, _)| *guard));
    // Occurrence bindings are exact: a field read inside a saved predicate
    // is distinct from another spelling of that field in a call actual.
    // Install the admitted initializer's coordinates before capturing it.
    expressions.extend(
        program
            .statement_table
            .statements(source.statement_nodes)
            .iter()
            .filter_map(|statement| statement.as_local_data())
            .filter(|local| saved_arguments::saved(program, source, local.symbol).is_some())
            .map(|local| local.initial_value),
    );
    expressions.extend(projections::entry_expressions(
        program,
        caller.machine,
        source,
    ));
    expressions
}

fn scalar_subjects(
    member: &RankingRangeCallMember<'_>,
    measure: RankingRangeMeasure,
) -> Vec<ExpressionHandle> {
    let mut subjects = vec![member.subject];
    if member.paired_subject.is_valid() {
        subjects.push(member.paired_subject);
    }
    if let RankingRangeMeasure::IncreasingTo { limit, .. } = measure {
        subjects.push(limit);
    }
    subjects
}

fn scalar_entry<'program>(
    program: &'program dyn crate::CheckingInput,
    member: &RankingRangeCallMember<'_>,
) -> Option<(&'program State, RankingRangeMeasure)> {
    let machine = program.machine(member.machine.symbol)?;
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
        if let Some(view) =
            super::declared_scalar_view(program, state, *subject, &witness.view_path)
        {
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
        // A declared field view produces the measure's exact `u64`
        // projection of the subject's record. The coordinate owner
        // re-resolves the chain against the subject formal's own
        // declaration; a non-formal subject or a foreign record has no
        // scalar transport either.
        let path = witness
            .view_path
            .split("::")
            .filter(|member| !member.is_empty())
            .collect::<Vec<_>>();
        let measure = find_declared_measure(program, &path)?;
        if measure.lexicographic
            || !matches!(
                measure_body_shape(program, measure),
                Some(MeasureBodyShape::FieldProjection { .. })
            )
        {
            return None;
        }
        fields::FieldCoordinate::resolve_field(program, state, *subject, measure.symbol)?;
        return Some((
            state,
            RankingRangeMeasure::Field {
                subject: member.subject,
                measure: measure.symbol,
            },
        ));
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
    // The ranked storage subject has an exact entry coordinate. A distance
    // bound is an independently admitted expression; requiring it to be a
    // formal rejects literal and computed bounds before their proof can run.
    {
        let subject = member.subject;
        if matches!(measure, RankingRangeMeasure::SliceLength(_)) {
            // The ranked subject is the exact slice-typed entry parameter or
            // a projected slice leaf of an entry record formal; its produced
            // length coordinate, not the collection value, carries the rank.
            if lengths::parameter(program, state, subject).is_none()
                && lengths::SliceCoordinate::resolve_length(program, state, subject).is_none()
            {
                return None;
            }
        } else {
            entry_scalar_parameter(program, state, subject)?;
        }
    }
    Some((state, measure))
}

/// A scalar view subject must arrive as the machine's own fixed-integer
/// entry input: a bare (possibly atomically wrapped) name bound to a
/// non-const formal, or an exact member chain rooted at such a formal whose
/// declared leaf is a fixed integer. The leaf's arithmetic policy is not a
/// coordinate property: a `u64 in Wrapping` formal holds one carrier value
/// per arrival exactly as an Exact one does, and the edge judgment proves
/// every update's no-wrap premise and natural formation under the site's
/// guards. Nothing is inferred from unsigned-only storage eligibility.
fn entry_scalar_parameter(
    program: &dyn crate::CheckingInput,
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
    if let Some(_) = program.expression_table.member_node(subject) {
        // The projected leaf's declared type is the subject's carrier; every
        // step is an exact declared field of a non-self, non-const entry
        // formal's record.
        let reference = fields::projected_type(program, state, subject)?;
        return integer_parameter(program, reference).map(|_| ());
    }
    let Some(symbol) = crate::expression_traversal::single_segment_name_symbol(program, subject)
    else {
        return None;
    };
    let parameter = program.state_parameter_by_symbol(state, symbol)?;
    if parameter.is_const || integer_parameter(program, parameter.type_reference).is_none() {
        return None;
    }
    Some(())
}

/// A call member's authored range read at one site: endpoint meanings are
/// installed against the member machine's own coordinates, then normalized.
/// `Some(None)` = no authored range; outer `None` = the range is unreadable.
fn member_call_range(
    program: &dyn crate::CheckingInput,
    member: &RankingRangeCallMember<'_>,
    state: &State,
    engine: &mut Engine<'_>,
) -> Option<Option<(Polynomial, Polynomial, bool)>> {
    if !member.range.is_valid() {
        return Some(None);
    }
    let range = program.expression_table.range_node(member.range)?;
    for endpoint in [range.start, range.end] {
        meanings::install_nonpolynomial_terms(program, member.machine, state, engine, endpoint, 0)?;
    }
    Some(Some((
        engine.normalize(range.start)?,
        engine.normalize(range.end)?,
        range.end_inclusive,
    )))
}

/// Build a call's projected-coordinate expressions and install them against
/// the caller's machine coordinates — the shared surface under the
/// subject-wide and `.len`-spelled reads. The family fixes which authored
/// reading each expression names.
#[allow(clippy::too_many_arguments)]
fn install_call_coordinates<'program, F: CoordinateFamily<'program>>(
    program: &'program dyn crate::CheckingInput,
    caller: &RankingRangeCallMember<'_>,
    source: &State,
    entry: &State,
    caller_site: &RankingRangeCallSite<'_>,
    at_entry: bool,
    engine: &mut Engine<'_>,
    coordinates: &mut CoordinateSet<'program, F>,
    subjects: Vec<ExpressionHandle>,
    guards: &[(ExpressionHandle, bool)],
    arguments: &[ExpressionHandle],
) -> Option<()> {
    let expressions =
        installation_expressions(program, caller, source, subjects, guards, arguments);
    coordinates.install(
        program,
        caller.machine,
        source,
        entry,
        (!at_entry).then_some(caller_site.entry_parameters),
        engine,
        &expressions,
    )
}
