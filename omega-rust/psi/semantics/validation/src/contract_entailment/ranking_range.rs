//! Exact, transient rank-range obligations use the ordinary arithmetic engine.
//! The checked ranking owner selects the view and supplies one actual edge.

use super::*;
use typed_trees::state::State;
use typed_trees::types::{PrimitiveType, TypeConstraintNode, TypeReferenceNode};

mod calls;
mod field_coordinates;
mod fields;
mod lengths;
mod meanings;
mod projections;
mod requirements;
mod state_aliases;

pub use requirements::{
    arithmetic_entry_requirement_is_covered, prove_arithmetic_call_requirement,
};

pub(crate) use calls::{
    RankingRangeCallEdge, RankingRangeCallMember, RankingRangeCallProgress,
    mixed_call_endpoints_are_pinned, prove_ranking_range_call, prove_ranking_range_call_entry,
};

#[cfg(test)]
mod tests;

/// The scalar rank produced by an independently selected ranking view.
#[derive(Clone, Copy)]
pub enum RankingRangeMeasure {
    Single(ExpressionHandle),
    SliceLength(ExpressionHandle),
    Field {
        subject: ExpressionHandle,
        field: symbols::SymbolHandle,
    },
    Distance {
        lower: ExpressionHandle,
        upper: ExpressionHandle,
    },
    IncreasingTo {
        subject: ExpressionHandle,
        limit: ExpressionHandle,
    },
}

/// Edge premises for a complete checked state graph. EntryInvariant must be
/// checked on every edge; InitialEntry is the non-reentered-root exception to
/// RankInvariant. Entry facts are never automatically renewed at reentry.
#[derive(Clone, Copy)]
pub enum RankingRangePremises {
    RankInvariant,
    EntryInvariant,
    /// Entry facts can support the first transfer without surviving it, but
    /// only when no local transition can return to this root state.
    InitialEntry,
}

/// Prove current and next rank membership, with invocation-fixed endpoints.
/// This query neither mutates source/evidence nor admits an unknown judgment.
/// The caller must provide an exact root self-edge and its live guard facts;
/// arbitrary state-to-root substitutions are deliberately not inferred here.
pub fn prove_ranking_range_edge(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    range: ExpressionHandle,
    measure: RankingRangeMeasure,
    premises: RankingRangePremises,
    guards: &[(ExpressionHandle, bool)],
    evaluated_prefix: &[ExpressionHandle],
    arguments: &[ExpressionHandle],
) -> Option<RankingRangeEdgeProof> {
    prove_edge(
        program,
        machine,
        state,
        range,
        measure,
        premises,
        guards,
        evaluated_prefix,
        Some(arguments),
        EdgeContext::Root,
    )
}

/// An exact state telescope over the entry witness. The parameter list follows
/// non-self formal order and names the entry symbol represented by each slot.
/// Entry parameters may be absent or repeated. Required scalar copies carry an
/// equality invariant checked at every arrival; ancestry alone is not equality.
/// An invalid handle marks a slot with no entry role: a payload computed from
/// several auxiliary inputs. It binds nothing, so no template premise can reach
/// it, and a required symbol folded into it is rejected as a missing premise.
#[derive(Clone, Copy)]
pub struct RankingRangeState<'program> {
    pub state: &'program State,
    pub entry_parameters: &'program [symbols::SymbolHandle],
}

/// Check one state transition under independently established source and target
/// telescopes. Each source uses the selected graph-wide invariant, including
/// reentered roots. Every destination formal receives its exact actual in
/// one simultaneous substitution; graph ownership decides whether strict
/// decrease is additionally required for this edge.
pub fn prove_ranking_range_transition(
    program: &TypedTrees,
    machine: &Machine,
    range: ExpressionHandle,
    measure: RankingRangeMeasure,
    premises: RankingRangePremises,
    source: RankingRangeState<'_>,
    destination: RankingRangeState<'_>,
    guards: &[(ExpressionHandle, bool)],
    evaluated_prefix: &[ExpressionHandle],
    arguments: &[ExpressionHandle],
) -> Option<RankingRangeEdgeProof> {
    prove_edge(
        program,
        machine,
        source.state,
        range,
        measure,
        premises,
        guards,
        evaluated_prefix,
        Some(arguments),
        EdgeContext::Transition {
            source_parameters: source.entry_parameters,
            destination,
        },
    )
}

enum EdgeContext<'program> {
    Root,
    Transition {
        source_parameters: &'program [symbols::SymbolHandle],
        destination: RankingRangeState<'program>,
    },
}

/// Separate results prevent a range-membership proof from authorizing descent.
#[derive(Clone, Copy)]
pub struct RankingRangeEdgeProof {
    pub membership_and_pinning: bool,
    pub strictly_decreases: bool,
}

/// Establish the produced rank at entry, including an acyclic invocation.
/// No backedge guard or actual argument can strengthen this obligation.
pub fn prove_ranking_range_entry(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    range: ExpressionHandle,
    measure: RankingRangeMeasure,
) -> bool {
    prove_edge(
        program,
        machine,
        state,
        range,
        measure,
        RankingRangePremises::EntryInvariant,
        &[],
        &[],
        None,
        EdgeContext::Root,
    )
    .is_some_and(|proof| proof.membership_and_pinning)
}

fn prove_edge(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    range: ExpressionHandle,
    measure: RankingRangeMeasure,
    premises: RankingRangePremises,
    guards: &[(ExpressionHandle, bool)],
    evaluated_prefix: &[ExpressionHandle],
    arguments: Option<&[ExpressionHandle]>,
    context: EdgeContext<'_>,
) -> Option<RankingRangeEdgeProof> {
    let states = program.machine_states(machine);
    let root = states.first()?;
    if !states
        .iter()
        .any(|candidate| candidate.symbol == state.symbol)
        || (matches!(context, EdgeContext::Root) && root.symbol != state.symbol)
    {
        return None;
    }
    if matches!(premises, RankingRangePremises::InitialEntry)
        && (state.symbol != root.symbol || root_has_incoming_transition(program, machine, root))
    {
        return None;
    }
    let (entry_parameters, destination) = match &context {
        EdgeContext::Root => (None, None),
        EdgeContext::Transition {
            source_parameters,
            destination,
        } => {
            validate_mapping(program, machine, state, source_parameters)?;
            validate_mapping(
                program,
                machine,
                destination.state,
                destination.entry_parameters,
            )?;
            (Some(*source_parameters), Some(*destination))
        }
    };
    let mut field_rank = match measure {
        RankingRangeMeasure::Field { subject, field } => {
            // Scalar telescope compatibility says nothing about records.
            // Rebind the selected field only after proving its unique owned
            // nominal arrival; template spellings cannot supply that proof.
            let coordinate = fields::FieldCoordinate::resolve(program, root, subject, field)?;
            let coordinate = match entry_parameters {
                Some(entries) => coordinate.at_arrival(
                    program,
                    RankingRangeState {
                        state,
                        entry_parameters: entries,
                    },
                    coordinate.parameter.symbol,
                )?,
                None => coordinate,
            };
            Some(field_coordinates::FieldCoordinates::new(coordinate))
        }
        _ => None,
    };
    let ExpressionNode::Range(range) = program.expression_table.expression(range) else {
        return None;
    };
    if field_rank.is_some() {
        fields::endpoints_formed(program, machine, root, range)?;
    }
    let admit_template = |expression| meanings::builtin(program, machine, root, expression, 0);
    admit_template(range.start)?;
    admit_template(range.end)?;
    match measure {
        RankingRangeMeasure::Single(subject)
        | RankingRangeMeasure::SliceLength(subject)
        | RankingRangeMeasure::Field { subject, .. } => {
            admit_template(subject)?;
        }
        RankingRangeMeasure::Distance { lower, upper }
        | RankingRangeMeasure::IncreasingTo {
            subject: lower,
            limit: upper,
        } => {
            admit_template(lower)?;
            admit_template(upper)?;
        }
    }
    let admit = |expression| meanings::builtin(program, machine, state, expression, 0);
    for argument in arguments.unwrap_or_default() {
        admit(*argument)?;
    }
    for (guard, _) in guards {
        admit(*guard)?;
    }
    // Prefix guards/initializers may have executed even when they establish no
    // surviving hypothesis. Check their meaning without assuming their truth.
    for expression in evaluated_prefix {
        admit(*expression)?;
    }
    let parameters = program.state_parameters(state);
    // Local-state induction still has no mutable-parameter arrival evidence.
    // Call components own a separate exact prefix-preservation judgment;
    // shared meaning and symbol binding do not establish either guarantee.
    if parameters.iter().any(|parameter| {
        !parameter.is_self
            && parameter.is_mutable
            && exact_integer_parameter(program, parameter.type_reference).is_some()
    }) {
        return None;
    }
    if arguments.is_some_and(|arguments| {
        program
            .state_parameters(destination.map_or(state, |destination| destination.state))
            .iter()
            .filter(|parameter| !parameter.is_self)
            .count()
            != arguments.len()
    }) {
        return None;
    }
    let required_symbols =
        state_aliases::required_symbols(program, machine, range, measure, premises)?;
    let mut bindings = integer_bindings(program, state)?;
    let mut alias_comparisons = Vec::new();
    if let Some(entry_parameters) = entry_parameters {
        for (parameter, entry_symbol) in parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .zip(entry_parameters)
        {
            if !entry_symbol.is_valid()
                || (entry_parameters
                    .iter()
                    .filter(|candidate| *candidate == entry_symbol)
                    .take(2)
                    .count()
                    != 1
                    && !required_symbols.contains(entry_symbol))
            {
                // The two current slots remain independent. Do not choose a
                // copy or infer equality from their shared entry ancestry.
                continue;
            }
            let Some(binding) = bindings
                .iter()
                .find(|binding| binding.symbol == parameter.symbol)
            else {
                // An unrelated payload contributes no arithmetic fact. The
                // strict engine cannot normalize either omitted symbol, even
                // inside an expression that would otherwise cancel to zero.
                continue;
            };
            let value = binding.value.clone();
            if let Some(existing) = bindings
                .iter()
                .find(|binding| binding.symbol == *entry_symbol)
            {
                let (
                    StrictArithmeticBindingValue::Atom {
                        identity: existing, ..
                    },
                    StrictArithmeticBindingValue::Atom {
                        identity: current, ..
                    },
                ) = (&existing.value, &value)
                else {
                    return None;
                };
                if existing != current {
                    alias_comparisons.push((
                        BinaryOperator::Equal,
                        Polynomial::atom(existing.clone()),
                        Polynomial::atom(current.clone()),
                    ));
                }
                continue;
            }
            bindings.push(StrictArithmeticSymbolBinding {
                symbol: *entry_symbol,
                value,
            });
        }
    }
    // Equality is a graph invariant only with complete source and destination
    // coverage. An omitted auxiliary input cannot silently drop its copies'
    // arrival obligations and become an equality premise in the next state.
    if required_symbols.iter().any(|symbol| {
        !bindings.iter().any(|binding| binding.symbol == *symbol)
            || destination.is_some_and(|destination| !destination.entry_parameters.contains(symbol))
    }) {
        return None;
    }
    let mut engine = Engine::strict_with_symbol_bindings(program, machine, &bindings);
    if !engine.strict_symbol_bindings_are_valid() {
        return None;
    }
    let length_bindings = lengths::bindings(program, state, entry_parameters);
    if !length_bindings.is_empty() || field_rank.is_some() {
        let expressions = projections::expressions(
            program,
            machine,
            root,
            range,
            measure,
            premises,
            arguments,
            guards,
            evaluated_prefix,
        );
        lengths::install(
            program,
            machine,
            state,
            root,
            &length_bindings,
            &mut engine,
            &expressions,
        )?;
        if let Some(field) = &mut field_rank {
            field.install(
                program,
                state,
                root,
                entry_parameters,
                &mut engine,
                &expressions,
            )?;
        }
    }
    let auxiliary =
        if arguments.is_none() || !matches!(premises, RankingRangePremises::RankInvariant) {
            entry_comparisons(program, machine, root, &mut engine, &bindings)?
        } else {
            Vec::new()
        };
    let mut comparisons = auxiliary.clone();
    if let Some(field) = &field_rank {
        comparisons.extend(field.comparisons(program));
    }
    comparisons.extend(alias_comparisons);
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

    let floor = engine.normalize(range.start)?;
    let ceiling = engine.normalize(range.end)?;
    let rank = match measure {
        RankingRangeMeasure::Single(subject) => engine.normalize(subject)?,
        RankingRangeMeasure::Field { .. } => field_rank.as_ref()?.value()?,
        RankingRangeMeasure::SliceLength(subject) => {
            let parameter = lengths::parameter(program, root, subject)?;
            let (_, identity) = length_bindings
                .iter()
                .find(|(symbol, _)| *symbol == parameter.symbol)?;
            Polynomial::atom(identity.clone())
        }
        RankingRangeMeasure::Distance { lower, upper }
        | RankingRangeMeasure::IncreasingTo {
            subject: lower,
            limit: upper,
        } => engine.normalize(upper)?.sub(&engine.normalize(lower)?),
    };
    if arguments.is_some() {
        // Every edge, including a root edge, consumes the established rank
        // invariant. Entry-only premises never silently reappear at reentry.
        comparisons.extend([
            (
                BinaryOperator::GreaterOrEqual,
                rank.clone(),
                Polynomial::constant(BigInt::from_i64(0)),
            ),
            (BinaryOperator::GreaterOrEqual, rank.clone(), floor.clone()),
            (
                if range.end_inclusive {
                    BinaryOperator::LessOrEqual
                } else {
                    BinaryOperator::Less
                },
                rank.clone(),
                ceiling.clone(),
            ),
        ]);
    }
    if !engine.install_hypotheses(comparisons) {
        return None;
    }
    // For distance views raw subtraction represents the produced natural rank
    // only on this proved branch. The caller retains the separate clamped
    // interval tier for entries where subject <= limit is not established.
    let entry_membership = {
        let prove = |difference: Polynomial, minimum: i64| {
            engine.prove_at_least(&engine.substituted(&difference), &BigInt::from_i64(minimum))
        };
        prove(rank.clone(), 0)
            && prove(rank.sub(&floor), 0)
            && prove(ceiling.sub(&rank), i64::from(!range.end_inclusive))
    };
    let Some(arguments) = arguments else {
        return Some(RankingRangeEdgeProof {
            membership_and_pinning: entry_membership || engine.requires_unsatisfiable,
            strictly_decreases: false,
        });
    };
    let mut substitutions = BTreeMap::new();
    // Actual arguments retain the complete non-self formal ordinal. Filtering
    // numeric bindings must not shift a payload slot onto the next rank input.
    let target_parameters =
        program.state_parameters(destination.map_or(state, |destination| destination.state));
    for (position, (parameter, argument)) in target_parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .zip(arguments)
        .enumerate()
    {
        let source_symbol = destination.map_or(parameter.symbol, |destination| {
            destination.entry_parameters[position]
        });
        if let Some(field) = &field_rank
            && field.substitute(
                program,
                state,
                entry_parameters,
                destination,
                &mut engine,
                source_symbol,
                *argument,
                &mut substitutions,
            )?
        {
            continue;
        }
        if !source_symbol.is_valid()
            || destination.is_some_and(|destination| {
                destination
                    .entry_parameters
                    .iter()
                    .filter(|candidate| **candidate == source_symbol)
                    .take(2)
                    .count()
                    != 1
                    && !required_symbols.contains(&source_symbol)
            })
        {
            // No first/last-wins substitution for duplicated destinations.
            // apply_argument_map rejects an omitted atom if the proof uses it.
            continue;
        }
        if let Some((_, identity)) = length_bindings
            .iter()
            .find(|(symbol, _)| *symbol == source_symbol)
        {
            if !lengths::is_slice(program, parameter.type_reference) {
                return None;
            }
            substitutions.insert(
                identity.clone(),
                lengths::actual(
                    program,
                    machine,
                    state,
                    *argument,
                    &length_bindings,
                    &mut engine,
                )?,
            );
            continue;
        }
        let Some(binding) = bindings
            .iter()
            .find(|binding| binding.symbol == source_symbol)
        else {
            continue;
        };
        let StrictArithmeticBindingValue::Atom { identity, .. } = &binding.value else {
            return None;
        };
        let actual = engine.normalize(*argument)?;
        if let Some(existing) = substitutions.get(identity) {
            // Source copies remain independent atoms. Their established
            // equality may prove this arrival, but destination copies never
            // become hypotheses for their own equality obligation.
            if !engine.requires_unsatisfiable
                && !comparison_proven(&engine, BinaryOperator::Equal, existing, &actual)
            {
                return None;
            }
        } else {
            substitutions.insert(identity.clone(), actual);
        }
    }
    let next_rank = inductive_judgment::apply_argument_map(&rank, &substitutions)?;
    let next_floor = inductive_judgment::apply_argument_map(&floor, &substitutions)?;
    let next_ceiling = inductive_judgment::apply_argument_map(&ceiling, &substitutions)?;
    let pinned_view_bound = match measure {
        RankingRangeMeasure::IncreasingTo { limit, .. } => {
            let bound = engine.normalize(limit)?;
            let next = inductive_judgment::apply_argument_map(&bound, &substitutions)?;
            Some((bound, next))
        }
        RankingRangeMeasure::Single(_)
        | RankingRangeMeasure::SliceLength(_)
        | RankingRangeMeasure::Field { .. }
        | RankingRangeMeasure::Distance { .. } => None,
    };
    for (operator, left, right) in auxiliary
        .iter()
        .filter(|_| matches!(premises, RankingRangePremises::EntryInvariant))
    {
        let next_left = inductive_judgment::apply_argument_map(left, &substitutions)?;
        let next_right = inductive_judgment::apply_argument_map(right, &substitutions)?;
        if !engine.requires_unsatisfiable
            && !comparison_proven(&engine, *operator, &next_left, &next_right)
        {
            return None;
        }
    }
    // Even unreachable arrivals retain exact rank-input coverage. Vacuity
    // discharges comparisons, not missing or ambiguous substitutions.
    if engine.requires_unsatisfiable {
        return Some(RankingRangeEdgeProof {
            membership_and_pinning: true,
            strictly_decreases: true,
        });
    }
    let prove = |difference: Polynomial, minimum: i64| {
        engine.prove_at_least(&engine.substituted(&difference), &BigInt::from_i64(minimum))
    };
    if let Some((bound, next)) = pinned_view_bound
        && (!prove(bound.sub(&next), 0) || !prove(next.sub(&bound), 0))
    {
        return None;
    }
    // Prove equality to the old endpoints, not merely membership in a newly
    // moved interval. Simultaneous substitution cannot telescope n -> n - 1.
    if !prove(next_floor.sub(&floor), 0)
        || !prove(floor.sub(&next_floor), 0)
        || !prove(next_ceiling.sub(&ceiling), 0)
        || !prove(ceiling.sub(&next_ceiling), 0)
    {
        return None;
    }
    let membership_and_pinning = entry_membership
        && prove(next_rank.clone(), 0)
        && prove(next_rank.sub(&floor), 0)
        && prove(ceiling.sub(&next_rank), i64::from(!range.end_inclusive));
    // This same edge judgment owns strict decrease and natural-rank formation;
    // callers need not fall back to a second syntactic `n > 0` recognizer.
    let strictly_decreases = prove(rank.sub(&next_rank), 1) && prove(next_rank.clone(), 0);
    Some(RankingRangeEdgeProof {
        membership_and_pinning,
        strictly_decreases,
    })
}

/// Read entry facts in the root template, even when their current aliases
/// belong to a named state. Nothing here derives a fact from a state spelling.
fn entry_comparisons(
    program: &TypedTrees,
    machine: &Machine,
    root: &State,
    engine: &mut Engine<'_>,
    bindings: &[StrictArithmeticSymbolBinding],
) -> Option<Vec<Comparison>> {
    let admit = |expression| meanings::builtin(program, machine, root, expression, 0);
    let mut comparisons = Vec::new();
    for contract in program.machine_contracts(machine) {
        if contract.kind != SignatureContractKind::Requires {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            // Membership and proposition facts remain outside this arithmetic
            // projection. They are neither assumed nor claimed re-established.
            if let ProofFact::Expression(expression) = fact {
                admit(*expression)?;
                collect_guard(engine, *expression, true, &mut comparisons, 0)?;
            }
        }
    }
    for parameter in program
        .state_parameters(root)
        .iter()
        .filter(|parameter| !parameter.is_self)
    {
        if exact_integer_parameter(program, parameter.type_reference).is_none() {
            continue;
        }
        let mut reference = parameter.type_reference;
        while let TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = program.type_reference_table.type_reference(reference)
        {
            for constraint in program.type_reference_table.constraints(*constraints) {
                let TypeConstraintNode::Range { minimum, maximum } = constraint else {
                    continue;
                };
                // Missing or ambiguous aliases cannot contribute an auxiliary
                // invariant, even when another premise makes the edge dead.
                let binding = bindings
                    .iter()
                    .find(|binding| binding.symbol == parameter.symbol)?;
                let StrictArithmeticBindingValue::Atom { identity, .. } = &binding.value else {
                    return None;
                };
                admit(*minimum)?;
                admit(*maximum)?;
                comparisons.push((
                    BinaryOperator::GreaterOrEqual,
                    Polynomial::atom(identity.clone()),
                    engine.normalize(*minimum)?,
                ));
                comparisons.push((
                    BinaryOperator::LessOrEqual,
                    Polynomial::atom(identity.clone()),
                    engine.normalize(*maximum)?,
                ));
            }
            reference = *base_type;
        }
    }
    Some(comparisons)
}

fn comparison_proven(
    engine: &Engine<'_>,
    operator: BinaryOperator,
    left: &Polynomial,
    right: &Polynomial,
) -> bool {
    let prove = |difference: Polynomial, minimum: i64| {
        engine.prove_at_least(&engine.substituted(&difference), &BigInt::from_i64(minimum))
    };
    match operator {
        BinaryOperator::Less => prove(right.sub(left), 1),
        BinaryOperator::LessOrEqual => prove(right.sub(left), 0),
        BinaryOperator::Greater => prove(left.sub(right), 1),
        BinaryOperator::GreaterOrEqual => prove(left.sub(right), 0),
        BinaryOperator::Equal => prove(left.sub(right), 0) && prove(right.sub(left), 0),
        BinaryOperator::NotEqual => prove(left.sub(right), 1) || prove(right.sub(left), 1),
        _ => false,
    }
}

/// Initial-entry precision is safe only when no local edge can revisit the
/// root. Check exact primary and continuation targets, including `self`.
fn root_has_incoming_transition(program: &TypedTrees, machine: &Machine, root: &State) -> bool {
    use typed_trees::statement::{StatementNode, TransitionTargetNode};

    program.machine_states(machine).iter().any(|state| {
        program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .any(|statement| {
                let StatementNode::Transition(transition) = statement else {
                    return false;
                };
                [transition.target, transition.continuation]
                    .into_iter()
                    .any(|target| {
                        if !target.is_valid() {
                            return false;
                        }
                        match program.statement_table.transition_target(target) {
                            TransitionTargetNode::Named { path, .. } => {
                                path.symbol == root.symbol || path.symbol == machine.symbol
                            }
                            TransitionTargetNode::SelfTarget => state.symbol == root.symbol,
                            _ => false,
                        }
                    })
            })
    })
}

fn validate_mapping(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    entry_parameters: &[symbols::SymbolHandle],
) -> Option<()> {
    let states = program.machine_states(machine);
    let root = states.first()?;
    if !states
        .iter()
        .any(|candidate| candidate.symbol == state.symbol)
    {
        return None;
    }
    let root_parameters = program.state_parameters(root);
    let parameters = program.state_parameters(state);
    if entry_parameters.len()
        != parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .count()
    {
        return None;
    }
    for (parameter, entry_symbol) in parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .zip(entry_parameters)
    {
        if !parameter.symbol.is_valid()
            || parameter.is_mutable
            || parameter.is_const
            || parameters
                .iter()
                .filter(|candidate| candidate.symbol == parameter.symbol)
                .take(2)
                .count()
                != 1
            || (state.symbol != root.symbol
                && root_parameters
                    .iter()
                    .any(|entry| entry.symbol == parameter.symbol))
            || (state.symbol == root.symbol && parameter.symbol != *entry_symbol)
        {
            return None;
        }
        if !entry_symbol.is_valid() {
            // No entry role: the slot's own typed binding still serves guards,
            // but no root spelling and no entry constraint can reach it.
            continue;
        }
        let entry = root_parameters
            .iter()
            .find(|entry| !entry.is_self && entry.symbol == *entry_symbol)?;
        if entry.is_mutable
            || entry.is_const
            || exact_integer_parameter(program, entry.type_reference)
                != exact_integer_parameter(program, parameter.type_reference)
        {
            return None;
        }
    }
    // Two absent integer projections establish no payload type compatibility.
    // Ordinary typed arrivals own that check; this query leaves both payload
    // symbols unbound and can prove only the independently numeric rank.
    Some(())
}

fn integer_bindings(
    program: &TypedTrees,
    state: &State,
) -> Option<Vec<StrictArithmeticSymbolBinding>> {
    let mut bindings = Vec::new();
    for parameter in program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
    {
        let Some(primitive) = exact_integer_parameter(program, parameter.type_reference) else {
            // Unrelated payloads are never promoted to numeric facts.
            continue;
        };
        if !parameter.symbol.is_valid() {
            return None;
        }
        bindings.push(StrictArithmeticSymbolBinding {
            symbol: parameter.symbol,
            value: StrictArithmeticBindingValue::Atom {
                identity: format!("\0ranking:{:?}", parameter.symbol),
                unsigned: matches!(
                    primitive,
                    PrimitiveType::U8
                        | PrimitiveType::U16
                        | PrimitiveType::U32
                        | PrimitiveType::U64
                ),
            },
        });
    }
    Some(bindings)
}

fn exact_integer_parameter(
    program: &TypedTrees,
    mut reference: typed_trees::types::TypeReferenceHandle,
) -> Option<PrimitiveType> {
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(reference)
    {
        if program
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .any(|constraint| !matches!(constraint, TypeConstraintNode::Range { .. }))
        {
            return None;
        }
        reference = *base_type;
    }
    let primitive = crate::recasts::exact_primitive_type(program, reference)?;
    matches!(
        primitive,
        PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64
            | PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
    )
    .then_some(primitive)
}

type Comparison = (BinaryOperator, Polynomial, Polynomial);

/// Project already meaning-checked hypotheses into integer comparisons.
/// Unreadable Boolean facts contribute nothing; they cannot strengthen a rank
/// proof, but need not prevent an independently proven forwarding edge.
fn collect_guard(
    engine: &mut Engine<'_>,
    expression: ExpressionHandle,
    holds: bool,
    comparisons: &mut Vec<Comparison>,
    depth: usize,
) -> Option<()> {
    if depth >= 128 {
        return None;
    }
    match engine.program.expression_table.expression(expression) {
        ExpressionNode::Unary(unary)
            if unary.operator == typed_trees::expression::UnaryOperator::LogicalNot =>
        {
            collect_guard(engine, unary.operand, !holds, comparisons, depth + 1)
        }
        ExpressionNode::Atomic(atomic) => {
            collect_guard(engine, atomic.value, holds, comparisons, depth + 1)
        }
        ExpressionNode::Binary(binary)
            if (binary.operator == BinaryOperator::And && holds)
                || (binary.operator == BinaryOperator::Or && !holds) =>
        {
            collect_guard(engine, binary.left, holds, comparisons, depth + 1)?;
            collect_guard(engine, binary.right, holds, comparisons, depth + 1)
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) =>
        {
            for (condition, boolean) in [(binary.left, binary.right), (binary.right, binary.left)] {
                if let ExpressionNode::Boolean(polarity) =
                    engine.program.expression_table.expression(boolean)
                {
                    return collect_guard(
                        engine,
                        condition,
                        holds == (*polarity == (binary.operator == BinaryOperator::Equal)),
                        comparisons,
                        depth + 1,
                    );
                }
            }
            if let Some(comparison) =
                inductive_judgment::guard_arm_comparison(engine, expression, holds)
            {
                comparisons.push(comparison);
            }
            Some(())
        }
        _ => {
            if let Some(comparison) =
                inductive_judgment::guard_arm_comparison(engine, expression, holds)
            {
                comparisons.push(comparison);
            }
            Some(())
        }
    }
}
