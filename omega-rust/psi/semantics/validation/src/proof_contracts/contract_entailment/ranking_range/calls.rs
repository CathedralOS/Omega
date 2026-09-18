//! Cross-machine scalar rank transport with optional authored ranges. Each side
//! retains its own template; call substitution is not local-state correspondence.

use super::super::{
    StrictArithmeticBindingValue, StrictArithmeticExpressionBinding, StrictArithmeticSymbolBinding,
};
use super::{
    BigInt, BinaryOperator, Comparison, Engine, ExpressionHandle, ExpressionNode, Machine,
    MeasureBodyShape, Polynomial, PrimitiveType, RankingRangeMeasure, RankingRangeState, State,
    TypedTrees, collect_guard, entry_comparisons, exact_integer_parameter, field_coordinates,
    fields, find_declared_measure, integer_bindings, lengths, meanings, measure_body_shape,
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
        // Every authored range keeps the state-edge owner's endpoint
        // formation bar: an exact integer formal, an exact declared
        // projection, or a bounded integer expression -- never a selected
        // computation. An endpoint declaration bounds alone cannot place is
        // deferred, not rejected: the entry hypotheses installed below still
        // owe it a carrier landing, exactly as at a state edge.
        let mut deferred_endpoints = Vec::new();
        for endpoint in [range.start, range.end] {
            if !fields::endpoint_statically_formed(program, member.machine, state, endpoint) {
                deferred_endpoints.push(endpoint);
            }
        }
        let bindings = integer_bindings(program, state)?;
        let mut engine = Engine::strict_with_symbol_bindings(program, member.machine, &bindings);
        if !engine.strict_symbol_bindings_are_valid() {
            return None;
        }
        // A field-view rank is the exact coordinate atom; authored member
        // projections inside endpoints and requires facts bind to it before
        // any hypothesis normalizes them. A member-chain scalar subject reads
        // the same kind of coordinate: its chain resolves against the carrier
        // formal's own declaration and binds here as the produced rank atom.
        let field_rank = if let RankingRangeMeasure::Field { subject, measure } = measure {
            let coordinate = fields::FieldCoordinate::resolve(program, state, subject, measure)?;
            let mut coordinates = field_coordinates::FieldCoordinates::new(coordinate);
            let mut expressions = vec![range.start, range.end, subject];
            expressions.extend(projections::entry_expressions(
                program,
                member.machine,
                state,
            ));
            coordinates.install(program, state, state, None, &mut engine, &expressions)?;
            Some(coordinates)
        } else {
            let subjects = scalar_subjects(&member, measure);
            let mut coordinates =
                member_subject_coordinates(program, state, state, None, &subjects)?;
            let mut expressions = vec![range.start, range.end];
            expressions.extend(subjects.iter().copied());
            expressions.extend(projections::entry_expressions(
                program,
                member.machine,
                state,
            ));
            coordinates.install(program, state, state, None, &mut engine, &expressions)?;
            Some(coordinates)
        };
        // A slice over projected storage produces its length from the member
        // chain's exact leaf coordinate, as at a named-state edge. Every other
        // measure still owes authored `.len` spellings inside endpoints,
        // requires facts, or view bounds the same auxiliary coordinates: a
        // produced length is a non-polynomial input, not an operand polynomial.
        let mut slice_rank = if let RankingRangeMeasure::SliceLength(subject) = measure {
            lengths::SliceCoordinate::resolve(program, state, subject)
                .map(lengths::SliceCoordinates::new)
        } else {
            Some(lengths::SliceCoordinates::empty())
        };
        if let Some(coordinates) = &mut slice_rank {
            let mut expressions = vec![range.start, range.end];
            if let RankingRangeMeasure::SliceLength(subject) = measure {
                expressions.push(subject);
            } else {
                expressions.extend(scalar_subjects(&member, measure));
            }
            expressions.extend(projections::entry_expressions(
                program,
                member.machine,
                state,
            ));
            coordinates.install(
                program,
                member.machine,
                state,
                state,
                None,
                &mut engine,
                &expressions,
            )?;
        }
        // A bare slice formal's `.len` names the produced length atom its
        // formal binds here, whatever measure the member ranks by.
        let length_bindings = lengths::bindings(program, member.machine, state, None);
        if !length_bindings.is_empty() {
            let mut expressions = vec![range.start, range.end];
            expressions.extend(scalar_subjects(&member, measure));
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
        if let Some(coordinates) = &field_rank {
            comparisons.extend(coordinates.comparisons(program));
        }
        if let Some(coordinates) = &slice_rank {
            comparisons.extend(coordinates.comparisons());
        }
        comparisons.extend(length_bindings.iter().map(|(_, identity)| {
            (
                BinaryOperator::GreaterOrEqual,
                Polynomial::atom(identity.clone()),
                Polynomial::default(),
            )
        }));
        let coordinate = match measure {
            RankingRangeMeasure::SliceLength(subject) => match &slice_rank {
                Some(slice) => slice.value()?,
                None => length_coordinate(program, state, subject, &length_bindings)?,
            },
            RankingRangeMeasure::Field { .. } => field_rank.as_ref()?.value()?,
            _ => rank_coordinate(program, member.machine, &mut engine, measure)?,
        };
        let floor = engine.normalize(range.start)?;
        let ceiling = engine.normalize(range.end)?;
        if !engine.install_hypotheses(comparisons) {
            return None;
        }
        if !engine.requires_unsatisfiable {
            for endpoint in &deferred_endpoints {
                let polynomial = if *endpoint == range.start {
                    &floor
                } else {
                    &ceiling
                };
                if !fields::endpoint_lands_under(
                    &mut engine,
                    program,
                    member.machine,
                    state,
                    *endpoint,
                    polynomial,
                ) {
                    return None;
                }
            }
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
        |expression| meanings::builtin(program, caller.machine, source, expression, 0);
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
    let (bindings, mut carrier_equalities) = if at_entry {
        (integer_bindings(program, source)?, Vec::new())
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
    let mut engine = Engine::strict_with_symbol_bindings(program, caller.machine, &bindings);
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
        let (roles, equalities) = telescoped_length_bindings(
            program,
            caller.machine,
            source,
            caller_site.entry_parameters,
        );
        carrier_equalities.extend(equalities);
        for (symbol, identity) in roles {
            if !length_bindings.iter().any(|(bound, _)| *bound == symbol) {
                length_bindings.push((symbol, identity));
            }
        }
    }
    if !length_bindings.is_empty() {
        let mut expressions = scalar_subjects(&caller, source_measure);
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
            entry,
            &length_bindings,
            &mut engine,
            &expressions,
        )?;
    }
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
        let coordinate = fields::FieldCoordinate::resolve(program, entry, subject, measure)?;
        let coordinate = if at_entry {
            coordinate
        } else {
            coordinate.at_arrival(
                program,
                RankingRangeState {
                    state: source,
                    entry_parameters: caller_site.entry_parameters,
                },
                coordinate.parameter.symbol,
            )?
        };
        let mut coordinates = field_coordinates::FieldCoordinates::new(coordinate);
        let mut expressions = vec![subject];
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
        coordinates.install(
            program,
            source,
            entry,
            (!at_entry).then_some(caller_site.entry_parameters),
            &mut engine,
            &expressions,
        )?;
        Some(coordinates)
    } else {
        // A member-chain scalar subject produces its rank from projected
        // storage: the chain resolves against the entry formal's own
        // declaration, then re-resolves through this site's telescope onto
        // the formal actually carrying the record role. Guards, actuals, and
        // endpoints spelling the same member chain bind to the same atom.
        let subjects = scalar_subjects(&caller, source_measure);
        let mut coordinates = member_subject_coordinates(
            program,
            entry,
            source,
            (!at_entry).then_some(caller_site.entry_parameters),
            &subjects,
        )?;
        let mut expressions = subjects.clone();
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
        coordinates.install(
            program,
            source,
            entry,
            (!at_entry).then_some(caller_site.entry_parameters),
            &mut engine,
            &expressions,
        )?;
        Some(coordinates)
    };
    // A member-chain slice subject produces its length from the exact
    // projected coordinate of the carrier formal's record, resolved at the
    // entry scope and re-resolved onto the site's carrier. Bare slice
    // formals keep the `length_bindings` coordinate installed above.
    let slice_ranks = if let RankingRangeMeasure::SliceLength(subject) = source_measure {
        let resolved = match lengths::SliceCoordinate::resolve(program, entry, subject) {
            Some(coordinate) => {
                let coordinate = if at_entry {
                    coordinate
                } else {
                    coordinate.at_arrival(
                        program,
                        RankingRangeState {
                            state: source,
                            entry_parameters: caller_site.entry_parameters,
                        },
                        coordinate.parameter.symbol,
                    )?
                };
                Some(coordinate)
            }
            // A bare slice subject can still name a coordinate at this site:
            // its entry role may arrive packed inside a record carrier's
            // unique slice leaf.
            None if !at_entry => lengths::parameter(program, entry, subject)
                .and_then(|formal| {
                    fields::unique_entry_carrier(
                        program,
                        source,
                        caller_site.entry_parameters,
                        formal.symbol,
                    )
                })
                .and_then(|carrier| lengths::slice_leaf_coordinate(program, carrier)),
            None => None,
        };
        match resolved {
            Some(coordinate) => {
                let mut coordinates = lengths::SliceCoordinates::new(coordinate);
                let mut expressions = vec![subject];
                if caller.range.is_valid()
                    && let ExpressionNode::Range(range) =
                        program.expression_table.expression(caller.range)
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
                coordinates.install(
                    program,
                    caller.machine,
                    source,
                    entry,
                    (!at_entry).then_some(caller_site.entry_parameters),
                    &mut engine,
                    &expressions,
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
        let mut expressions = scalar_subjects(&caller, source_measure);
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
        coordinates.install(
            program,
            caller.machine,
            source,
            entry,
            (!at_entry).then_some(caller_site.entry_parameters),
            &mut engine,
            &expressions,
        )?;
        Some(coordinates)
    };
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
    if let Some(coordinates) = &field_ranks {
        // The produced coordinate's own natural bounds and declared field
        // constraints are hypotheses, as at a named-state edge.
        comparisons.extend(coordinates.comparisons(program));
    }
    if let Some(coordinates) = &slice_ranks {
        comparisons.extend(coordinates.comparisons());
    }
    for &(guard, holds) in guards {
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
    // A caller endpoint that declaration bounds alone could not place owes
    // its carrier landing under this site's hypotheses: requires facts at
    // entry, or the member's own range invariant re-established at a
    // subordinate arrival. A dead site stays vacuous.
    if !engine.requires_unsatisfiable
        && let Some((floor, ceiling, _)) = &source_range
    {
        let ExpressionNode::Range(range) = program.expression_table.expression(caller.range) else {
            return None;
        };
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
    for expression in destination_expressions {
        bind_destination_fields(
            program,
            destination,
            source,
            &mut engine,
            arguments,
            expression,
        )?;
        bind_destination_lengths(
            program,
            caller.machine,
            destination,
            source,
            &mut engine,
            arguments,
            &length_bindings,
            expression,
        )?;
    }
    let next_rank = match destination_measure {
        // The callee's ranked slice formal arrives as this call's exact
        // actual; its produced length is the actual's own coordinate, not a
        // forwarded caller parameter. A member-chain subject reads the
        // projected coordinate its carrier formal's declaration names, walked
        // over the actual's forward or rebuilt literal.
        RankingRangeMeasure::SliceLength(subject) => {
            if let Some(coordinate) =
                lengths::SliceCoordinate::resolve(program, destination, subject)
            {
                let position = program
                    .state_parameters(destination)
                    .iter()
                    .filter(|parameter| !parameter.is_self)
                    .position(|parameter| parameter.symbol == coordinate.parameter.symbol)?;
                coordinate.arrived(
                    program,
                    caller.machine,
                    source,
                    &mut engine,
                    arguments[position],
                    coordinate.borrowed,
                    &length_bindings,
                )?
            } else {
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
        }
        // The callee's ranked record formal arrives as this call's exact
        // actual; its produced coordinate is the shared measure's chain
        // walked over that actual -- a forwarded carrier, a borrow of one, a
        // prefix member of a larger carrier, or a literal rebuilt declaration
        // by declaration. The destination formal's own reference boundary
        // decides whether the actual reads under one `&`.
        RankingRangeMeasure::Field { subject, measure } => {
            let destination_coordinate =
                fields::FieldCoordinate::resolve(program, destination, subject, measure)?;
            let position = program
                .state_parameters(destination)
                .iter()
                .filter(|parameter| !parameter.is_self)
                .position(|parameter| {
                    parameter.symbol == destination_coordinate.parameter.symbol
                })?;
            destination_coordinate.arrived(
                program,
                source,
                &mut engine,
                arguments[position],
                destination_coordinate.borrowed,
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
        | (None, RankingRangeMeasure::Field { .. })
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
    // The destination's deferred endpoints land under the caller's
    // hypotheses with this call's actuals already bound: the range the call
    // feeds must be a defined interval of the callee's carrier here, not only
    // inside the callee's own entry proof.
    if let Some((floor, ceiling, _)) = &destination_range {
        let ExpressionNode::Range(range) = program.expression_table.expression(callee.range) else {
            return None;
        };
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

/// Bind every exact member projection inside `expression` that is rooted at a
/// `destination` record formal to the value its call actual installs: the
/// resolved chain re-resolved over the actual's own carrier. Members that do
/// not resolve -- or whose actual the coordinate cannot read -- stay unbound,
/// so an opaque endpoint still fails `normalize` rather than borrowing a
/// foreign record's lineage.
fn bind_destination_fields(
    program: &TypedTrees,
    destination: &State,
    source: &State,
    engine: &mut Engine<'_>,
    arguments: &[ExpressionHandle],
    expression: ExpressionHandle,
) -> Option<()> {
    let mut pending = vec![(expression, 0usize)];
    while let Some((expression, depth)) = pending.pop() {
        if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
            return None;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Member(member) => {
                if let Some(coordinate) =
                    fields::FieldCoordinate::resolve_projection(program, destination, expression)
                    && let Some(position) = program
                        .state_parameters(destination)
                        .iter()
                        .filter(|parameter| !parameter.is_self)
                        .position(|parameter| parameter.symbol == coordinate.parameter.symbol)
                    && let Some(actual) = coordinate.arrived(
                        program,
                        source,
                        engine,
                        arguments[position],
                        coordinate.borrowed,
                    )
                    && !engine.bind_strict_projection(expression, actual)
                {
                    return None;
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

/// Bind every authored `.len` spelling inside `expression` whose receiver is
/// an exact member chain of a `destination` record formal to the produced
/// length its call actual installs: the projected slice coordinate walked
/// over the actual's forward, borrow, or rebuilt literal. A `.len` whose
/// receiver resolves no projected coordinate -- or whose actual the
/// coordinate cannot read -- stays unbound, so an opaque endpoint still
/// fails `normalize` rather than naming a guessed length.
fn bind_destination_lengths(
    program: &TypedTrees,
    machine: &Machine,
    destination: &State,
    source: &State,
    engine: &mut Engine<'_>,
    arguments: &[ExpressionHandle],
    bindings: &[(SymbolHandle, String)],
    expression: ExpressionHandle,
) -> Option<()> {
    let mut pending = vec![(expression, 0usize)];
    while let Some((expression, depth)) = pending.pop() {
        if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
            return None;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Member(member) => {
                if member.case_variant.is_none()
                    && let Some(receiver) = crate::value_custody::places::collection_length_receiver(
                        program,
                        machine,
                        Some(destination),
                        expression,
                    )
                {
                    let actual = if let Some(coordinate) =
                        lengths::SliceCoordinate::resolve(program, destination, receiver)
                    {
                        program
                            .state_parameters(destination)
                            .iter()
                            .filter(|parameter| !parameter.is_self)
                            .position(|parameter| parameter.symbol == coordinate.parameter.symbol)
                            .and_then(|position| {
                                coordinate.arrived(
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
                            program
                                .state_parameters(destination)
                                .iter()
                                .filter(|parameter| !parameter.is_self)
                                .position(|parameter| parameter.symbol == formal.symbol)
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
                    if let Some(actual) = actual
                        && !engine.bind_strict_projection(expression, actual)
                    {
                        return None;
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
    machine: &Machine,
    state: &State,
    entry_parameters: &[SymbolHandle],
) -> Option<(Vec<StrictArithmeticSymbolBinding>, Vec<Comparison>)> {
    let root = program.machine_states(machine).first()?;
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
        let (identity, unsigned) = match bindings
            .iter()
            .find(|binding| binding.symbol == formal.symbol)
        {
            Some(binding) => {
                let StrictArithmeticBindingValue::Atom { identity, unsigned } = &binding.value
                else {
                    continue;
                };
                (identity.clone(), *unsigned)
            }
            // A record formal carries a bare integer entry role through the
            // unique natural leaf its declaration names; the role binds the
            // produced coordinate there rather than the record itself.
            None => match super::carried_natural_coordinate(program, root, formal, *role) {
                Some(coordinate) => (coordinate.identity.clone(), true),
                None => continue,
            },
        };
        if let Some(existing) = bindings.iter().find(|binding| binding.symbol == *role) {
            let StrictArithmeticBindingValue::Atom {
                identity: existing, ..
            } = &existing.value
            else {
                return None;
            };
            if *existing != identity {
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
            value: StrictArithmeticBindingValue::Atom { identity, unsigned },
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
    machine: &Machine,
    state: &State,
    entry_parameters: &[SymbolHandle],
) -> (Vec<(SymbolHandle, String)>, Vec<Comparison>) {
    let bindings = lengths::bindings(program, machine, state, None);
    let mut roles: Vec<(SymbolHandle, String)> = Vec::new();
    let mut equalities = Vec::new();
    let root = program.machine_states(machine).first();
    for (formal, role) in program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .zip(entry_parameters)
    {
        if !role.is_valid() {
            continue;
        }
        let identity = match bindings.iter().find(|(symbol, _)| *symbol == formal.symbol) {
            Some((_, identity)) => identity.clone(),
            // A record carrier holds the collection at its declaration's
            // unique slice leaf; the role binds the produced length there
            // only when the role's own entry formal is a slice.
            None => {
                let Some(root_formal) = root.and_then(|root| {
                    program
                        .state_parameters(root)
                        .iter()
                        .find(|formal| !formal.is_self && formal.symbol == *role)
                }) else {
                    continue;
                };
                if !lengths::is_slice(program, root_formal.type_reference) {
                    continue;
                }
                let Some(coordinate) = lengths::slice_leaf_coordinate(program, formal) else {
                    continue;
                };
                coordinate.identity.clone()
            }
        };
        if let Some((_, existing)) = roles.iter().find(|(symbol, _)| *symbol == *role) {
            if *existing != identity {
                equalities.push((
                    BinaryOperator::Equal,
                    Polynomial::atom(existing.clone()),
                    Polynomial::atom(identity.clone()),
                ));
            }
            continue;
        }
        roles.push((*role, identity));
    }
    (roles, equalities)
}

/// The scalar expressions whose member chains may carry a produced rank or
/// bound: the subject, a paired distance subject, and an `IncreasingTo`
/// view's authored limit.
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

/// The natural coordinates scalar subjects read through member chains: each
/// `record.field` subject resolves against the `authored` scope's formals,
/// then re-resolves onto `site`'s unique carrier of that role. `site` equals
/// `authored` at an entry judgment. A bare subject resolves no coordinate; a
/// resolved chain that cannot arrive fails the judgment rather than naming a
/// guessed carrier.
fn member_subject_coordinates<'program>(
    program: &'program TypedTrees,
    authored: &'program State,
    site: &'program State,
    entry_parameters: Option<&[SymbolHandle]>,
    subjects: &[ExpressionHandle],
) -> Option<field_coordinates::FieldCoordinates<'program>> {
    let mut coordinates = field_coordinates::FieldCoordinates::empty();
    for subject in subjects {
        if let Some(coordinate) =
            fields::FieldCoordinate::resolve_projection(program, authored, *subject)
        {
            let coordinate = match entry_parameters {
                Some(entries) => coordinate.at_arrival(
                    program,
                    RankingRangeState {
                        state: site,
                        entry_parameters: entries,
                    },
                    coordinate.parameter.symbol,
                )?,
                None => coordinate,
            };
            coordinates.include(coordinate);
            continue;
        }
        // A bare subject can still name a coordinate at this site: its entry
        // role may arrive packed inside a record carrier's unique natural
        // leaf, exactly as at a named-state edge.
        if let Some(entries) = entry_parameters
            && let Some(coordinate) =
                super::carried_subject_coordinate(program, authored, site, entries, *subject)
        {
            coordinates.include(coordinate);
        }
    }
    Some(coordinates)
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
        fields::FieldCoordinate::resolve(program, state, *subject, measure.symbol)?;
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
    // Authored subjects and endpoint expressions name entry parameters; the
    // member's own witness owns every internal arrival, so only the entry
    // binding is checked here.
    for subject in [member.subject, member.paired_subject]
        .into_iter()
        .filter(|subject| subject.is_valid())
    {
        if matches!(measure, RankingRangeMeasure::SliceLength(_)) {
            // The ranked subject is the exact slice-typed entry parameter or
            // a projected slice leaf of an entry record formal; its produced
            // length coordinate, not the collection value, carries the rank.
            if lengths::parameter(program, state, subject).is_none()
                && lengths::SliceCoordinate::resolve(program, state, subject).is_none()
            {
                return None;
            }
        } else {
            entry_scalar_parameter(program, state, subject)?;
        }
    }
    Some((state, measure))
}

/// A scalar view subject must arrive as the machine's own exact unsigned
/// entry input: a bare (possibly atomically wrapped) name bound to a
/// non-self, non-const `u8..=u64` formal, or an exact member chain rooted at
/// such a formal whose declared leaf is unsigned.
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
    if let ExpressionNode::Member(_) = program.expression_table.expression(subject) {
        // The projected leaf's declared type is the subject's carrier; every
        // step is an exact declared field of a non-self, non-const entry
        // formal's record.
        let reference = fields::projected_type(program, state, subject)?;
        return matches!(
            exact_integer_parameter(program, reference),
            Some(PrimitiveType::U8 | PrimitiveType::U16 | PrimitiveType::U32 | PrimitiveType::U64)
        )
        .then_some(());
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
