//! Exact, transient rank-range obligations use the ordinary arithmetic engine.
//! The checked ranking owner selects the view and supplies one actual edge.
use super::{
    BTreeMap, BigInt, BinaryOperator, Engine, ExpressionHandle, ExpressionNode, Machine,
    Polynomial, ProofFact, SignatureContractKind, StrictArithmeticBindingValue,
    StrictArithmeticSymbolBinding, TypedTrees, inductive_judgment,
};
use typed_trees::signature::StateParameter;
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
mod telescope;

pub use requirements::{
    arithmetic_entry_requirement_is_covered, prove_arithmetic_call_requirement,
};
pub(crate) use telescope::positive_step_amount;
pub use telescope::{discover_state_entry_mappings, discover_state_entry_mappings_preferring};
mod identity_views;
pub use identity_views::{
    ComputationBodyShape, DeclaredIdentityView, DeclaredScalarView, MeasureBodyShape,
    ProjectionStep, ScalarViewComputation, computation_body_shape, declared_identity_view,
    declared_scalar_view, find_declared_measure, identity_subject_matches, measure_body_shape,
    measure_constraints_cover_subject, unwrap_constraint_shells,
};

pub(crate) use calls::{
    RankingRangeCallEdge, RankingRangeCallMember, RankingRangeCallProgress, RankingRangeCallSite,
    call_member_premise_symbols, mixed_call_endpoints_are_pinned, prove_ranking_range_call,
    prove_ranking_range_call_entry,
};

#[cfg(test)]
mod tests;

/// The scalar rank produced by an independently selected ranking view.
#[derive(Clone, Copy)]
pub enum RankingRangeMeasure {
    Single(ExpressionHandle),
    /// A declared scalar view's computed rank: `body` with the measure's
    /// `parameter` bound to `subject` (validation's `declared_scalar_view`
    /// admits the body). The rank must also form inside `carrier`; the
    /// judgment proves that upper bound from the same hypotheses that prove
    /// membership, so an unbounded subject cannot claim `{ value * 2 }`.
    Computed {
        subject: ExpressionHandle,
        parameter: symbols::SymbolHandle,
        body: ExpressionHandle,
        carrier: symbols::BuiltinTypeAtom,
    },
    SliceLength(ExpressionHandle),
    /// A declared field view's rank: the `u64` field the measure `measure`
    /// projects from `subject`'s record, through the measure body's exact
    /// nested path and, for a borrowed subject, through its reference. The
    /// judgment re-resolves that chain against the subject's declaration; the
    /// checked owner's selection is not trusted as the coordinate.
    Field {
        subject: ExpressionHandle,
        measure: symbols::SymbolHandle,
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
/// `evaluated_prefix` must come from a premise-preserving prefix: live
/// write-frame evidence that no earlier statement writes the path of any
/// premise carrier (`ranking_range_premise_symbols`), so each such mutable
/// parameter still denotes its arrival value at the transition.
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
/// decrease is additionally required for this edge. `evaluated_prefix` must
/// come from a premise-preserving prefix: live write-frame evidence that no
/// earlier statement writes the path of any slot carrying a premise role
/// (`ranking_range_premise_symbols` through the telescope), so each such
/// mutable parameter still denotes its arrival value at the transition.
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

/// The entry symbols whose current copies the edge judgment holds equal at
/// every arrival: each integer root parameter named by the produced-rank
/// expression, the pinned endpoints, or the active entry premises. Mapping
/// discovery uses this set to decide whether several slots claiming one entry
/// keep their duplicated equality obligation or resolve to a bare forward.
pub fn ranking_range_required_symbols(
    program: &TypedTrees,
    machine: &Machine,
    range: ExpressionHandle,
    measure: RankingRangeMeasure,
    premises: RankingRangePremises,
) -> Option<Vec<symbols::SymbolHandle>> {
    let ExpressionNode::Range(range) = program.expression_table.expression(range) else {
        return None;
    };
    state_aliases::required_symbols(program, machine, range, measure, premises)
}

/// The root formals, in every carrier, whose arrival value some range judgment
/// on this witness may read: the produced-rank subjects, the range endpoints,
/// every requires fact, and every range-constrained integer formal. A prefix
/// store whose complete write frame is disjoint from each path carrying one of
/// these roles preserves the entry-relative ranking; a store into any of them
/// invalidates it, whatever value it stores. An invalid `range` means the
/// witness authored no range and contributes no endpoint.
pub fn ranking_range_premise_symbols(
    program: &TypedTrees,
    machine: &Machine,
    range: ExpressionHandle,
    measure: RankingRangeMeasure,
) -> Option<Vec<symbols::SymbolHandle>> {
    let range = if range.is_valid() {
        let ExpressionNode::Range(range) = program.expression_table.expression(range) else {
            return None;
        };
        Some(range)
    } else {
        None
    };
    state_aliases::premise_symbols(program, machine, range, measure, true)
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

/// The natural coordinates builtin scalar subjects read through member
/// chains: each `record.field` subject resolves against its carrier formal's
/// own declaration, then re-resolves onto the unique formal carrying that
/// role at this state. A bare subject names no coordinate; an authored chain
/// that resolves but cannot arrive -- a duplicated or role-less record slot --
/// fails the edge rather than borrowing a foreign record's lineage.
fn subject_field_coordinates<'program>(
    program: &'program TypedTrees,
    root: &'program State,
    state: &'program State,
    entry_parameters: Option<&[symbols::SymbolHandle]>,
    subjects: &[ExpressionHandle],
) -> Option<field_coordinates::FieldCoordinates<'program>> {
    let mut coordinates = field_coordinates::FieldCoordinates::empty();
    for subject in subjects {
        if let Some(coordinate) =
            fields::FieldCoordinate::resolve_projection(program, root, *subject)
        {
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
            coordinates.include(coordinate);
            continue;
        }
        // A bare subject can still name a coordinate at this state: its entry
        // role may arrive packed inside a record carrier, which holds the
        // subject's value at the record's unique natural leaf.
        if let Some(entries) = entry_parameters
            && let Some(coordinate) =
                carried_subject_coordinate(program, root, state, entries, *subject)
        {
            coordinates.include(coordinate);
        }
    }
    Some(coordinates)
}

/// The natural coordinate a bare scalar `subject` takes at `state` when a
/// record formal claims its entry role: the record's unique owned-path
/// natural leaf, of the subject formal's exact width. A subject spelled as a
/// member chain, an unclaimed or contested role, and a record without one
/// matching leaf all keep no coordinate.
fn carried_subject_coordinate<'program>(
    program: &'program TypedTrees,
    root: &'program State,
    state: &'program State,
    entry_parameters: &[symbols::SymbolHandle],
    subject: ExpressionHandle,
) -> Option<fields::FieldCoordinate<'program>> {
    let formal = fields::parameter(program, root, subject)?;
    let carrier = fields::unique_entry_carrier(program, state, entry_parameters, formal.symbol)?;
    carried_natural_coordinate(program, root, carrier, formal.symbol)
}

/// The natural coordinate `parameter`'s record assigns to `entry`'s role:
/// the unique owned-path natural leaf whose exact width matches the entry
/// formal's own carrier. A bare slot, a record with no such leaf or two, and
/// a width the typed arrival could never carry all keep no coordinate.
fn carried_natural_coordinate<'program>(
    program: &'program TypedTrees,
    root: &'program State,
    parameter: &'program StateParameter,
    entry: symbols::SymbolHandle,
) -> Option<fields::FieldCoordinate<'program>> {
    let entry_formal = program
        .state_parameters(root)
        .iter()
        .find(|formal| !formal.is_self && formal.symbol == entry)?;
    if entry_formal.is_const {
        return None;
    }
    let width = exact_integer_parameter(program, entry_formal.type_reference)?;
    let coordinate = fields::integer_leaf_coordinate(program, parameter)?;
    (exact_integer_parameter(program, coordinate.field.type_reference) == Some(width))
        .then_some(coordinate)
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
        RankingRangeMeasure::Field { subject, measure } => {
            // Scalar telescope compatibility says nothing about records.
            // Rebind the selected field only after proving its unique
            // nominal arrival; template spellings cannot supply that proof.
            let coordinate = fields::FieldCoordinate::resolve(program, root, subject, measure)?;
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
        // A declared scalar view may read projected storage: the subject's
        // member chain resolves to the exact `u64` coordinate the view body
        // consumes, so `computed_rank` and arrival substitution share the
        // atom a field view would use. A bare subject keeps the bare-name
        // path -- its symbol binding or an unbound member spelling decides
        // normalization -- while the installed set still binds the authored
        // endpoint and premise projections the same edge judgment reads.
        RankingRangeMeasure::Computed { subject, .. } => Some(subject_field_coordinates(
            program,
            root,
            state,
            entry_parameters,
            &[subject],
        )?),
        // A builtin scalar rank may read projected storage directly:
        // `bag.count` names the exact unsigned coordinate rooted at its
        // carrier formal, of whatever unsigned width the declaration gives
        // the leaf. Bare subjects resolve to no coordinate and keep the
        // symbol-binding path; an unresolvable chain stays an unbound
        // spelling and fails normalization below.
        RankingRangeMeasure::Single(subject) => Some(subject_field_coordinates(
            program,
            root,
            state,
            entry_parameters,
            &[subject],
        )?),
        RankingRangeMeasure::IncreasingTo { subject, limit } => Some(subject_field_coordinates(
            program,
            root,
            state,
            entry_parameters,
            &[subject, limit],
        )?),
        RankingRangeMeasure::Distance { lower, upper } => Some(subject_field_coordinates(
            program,
            root,
            state,
            entry_parameters,
            &[lower, upper],
        )?),
        // A produced slice length is a length coordinate, never a field
        // atom, so the subject names no natural coordinate here. The range
        // endpoints and the rest of the read surface still spell authored
        // member projections (`limits.cap`): the installed set binds exactly
        // those projections so entry membership and endpoint pinning read
        // real atoms instead of failing to normalize.
        RankingRangeMeasure::SliceLength(subject) => Some(subject_field_coordinates(
            program,
            root,
            state,
            entry_parameters,
            &[subject],
        )?),
    };
    // A slice over projected storage produces its length from the member
    // chain's exact leaf: `record.field` names one slice coordinate whose
    // produced length the range reads. Bare slice formals keep the
    // `length_bindings` coordinate below.
    let mut slice_rank = match measure {
        RankingRangeMeasure::SliceLength(subject) => {
            match lengths::SliceCoordinate::resolve(program, root, subject) {
                Some(coordinate) => {
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
                    Some(lengths::SliceCoordinates::new(coordinate))
                }
                // A bare slice formal can name a coordinate at this state
                // when its entry role arrives packed inside a record carrier:
                // the record holds the collection at its unique slice leaf.
                None => match (entry_parameters, lengths::parameter(program, root, subject)) {
                    (Some(entries), Some(formal)) => {
                        fields::unique_entry_carrier(program, state, entries, formal.symbol)
                            .and_then(|carrier| lengths::slice_leaf_coordinate(program, carrier))
                            .map(lengths::SliceCoordinates::new)
                    }
                    _ => None,
                },
            }
        }
        _ => None,
    };
    let ExpressionNode::Range(range) = program.expression_table.expression(range) else {
        return None;
    };
    // An endpoint that is not statically formed still owes a carrier-landing
    // proof under this edge's installed hypotheses; it is deferred, not
    // rejected, until that engine exists below. Every measure owes it: the
    // range reads the produced rank, whatever view produced it.
    let mut deferred_endpoints = Vec::new();
    for endpoint in [range.start, range.end] {
        if !fields::endpoint_statically_formed(program, machine, root, endpoint) {
            deferred_endpoints.push(endpoint);
        }
    }
    let admit_template = |expression| meanings::builtin(program, machine, root, expression, 0);
    admit_template(range.start)?;
    admit_template(range.end)?;
    match measure {
        RankingRangeMeasure::Single(subject)
        | RankingRangeMeasure::Computed { subject, .. }
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
    // A mutable parameter's live value equals its arrival value only while no
    // intervening write touches its path. The evaluated-prefix callers prove
    // that per edge with complete write frames disjoint from every premise
    // carrier's path before this judgment runs; the entry query has no prefix
    // at all. A formal outside the premise set may have been written: nothing
    // here assumes its arrival value, and the actual it feeds is read live.
    // Mutability is then a storage capability, not a value distinction, and an
    // integer atom names the same live value it would for an immutable formal.
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
            let value = match bindings
                .iter()
                .find(|binding| binding.symbol == parameter.symbol)
            {
                Some(binding) => binding.value.clone(),
                // A record formal carries a bare integer entry role through
                // the unique natural leaf its declaration names: the role
                // binds the produced coordinate there, not the record itself.
                None => match carried_natural_coordinate(program, root, parameter, *entry_symbol) {
                    Some(coordinate) => StrictArithmeticBindingValue::Atom {
                        identity: coordinate.identity.clone(),
                        unsigned: true,
                    },
                    // An unrelated payload contributes no arithmetic fact. The
                    // strict engine cannot normalize either omitted symbol,
                    // even inside an expression that would otherwise cancel
                    // to zero.
                    None => continue,
                },
            };
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
    let length_bindings = lengths::bindings(program, machine, state, entry_parameters);
    if !length_bindings.is_empty() || field_rank.is_some() || slice_rank.is_some() {
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
        if let Some(slice) = &mut slice_rank {
            slice.install(
                program,
                machine,
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
    if let Some(slice) = &slice_rank {
        // A produced length is a natural coordinate: the projected slice's
        // `>= 0` fact is exactly what the bare-formal length bindings add.
        comparisons.extend(slice.comparisons());
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
        RankingRangeMeasure::Computed {
            subject,
            parameter,
            body,
            ..
        } => computed_rank(program, machine, &mut engine, subject, parameter, body)?,
        RankingRangeMeasure::Field { .. } => field_rank.as_ref()?.value()?,
        RankingRangeMeasure::SliceLength(subject) => match &slice_rank {
            // A slice reached through a member chain reads its projected
            // coordinate; a bare slice formal reads its parameter binding.
            Some(slice) => slice.value()?,
            None => {
                let parameter = lengths::parameter(program, root, subject)?;
                let (_, identity) = length_bindings
                    .iter()
                    .find(|(symbol, _)| *symbol == parameter.symbol)?;
                Polynomial::atom(identity.clone())
            }
        },
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
    // A computed endpoint that declaration bounds alone could not place still
    // owes its carrier landing under this edge's hypotheses: a requires or
    // constrained-parameter fact that bounds a leaf reaches the produced
    // polynomial where the declaration-interval owner saw only the leaves'
    // store ranges. Dead edges stay vacuous; a live edge that cannot land its
    // endpoint has no defined range to read.
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
                machine,
                root,
                *endpoint,
                polynomial,
            ) {
                return None;
            }
        }
    }
    // For distance views raw subtraction represents the produced natural rank
    // only on this proved branch. The caller retains the separate clamped
    // interval tier for entries where subject <= limit is not established.
    // A computed rank is a value of its carrier only while the body forms
    // there; the same hypotheses that bound the subject must bound the rank.
    let carrier_maximum = match measure {
        RankingRangeMeasure::Computed { carrier, .. } => Some(carrier_maximum(carrier)?),
        _ => None,
    };
    let entry_membership = {
        let prove = |difference: Polynomial, minimum: i64| {
            engine.prove_at_least(&engine.substituted(&difference), &BigInt::from_i64(minimum))
        };
        prove(rank.clone(), 0)
            && prove(rank.sub(&floor), 0)
            && prove(ceiling.sub(&rank), i64::from(!range.end_inclusive))
            && carrier_maximum
                .as_ref()
                .is_none_or(|maximum| prove(maximum.sub(&rank), 0))
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
        // One carrier can owe both kinds of produced coordinate: a record
        // slot may claim a scalar endpoint leaf and the ranked collection's
        // slice leaf at once. Each substituter owns only its own atoms, so a
        // field match must not skip the slice coordinate's demanded actual.
        // The scalar fallback below applies only when neither coordinate
        // kind claims this slot's role.
        let mut coordinate_matched = false;
        if let Some(field) = &field_rank {
            coordinate_matched |= field.substitute(
                program,
                state,
                entry_parameters,
                destination,
                parameter,
                &required_symbols,
                &mut engine,
                source_symbol,
                *argument,
                &mut substitutions,
            )?;
        }
        if let Some(slice) = &slice_rank {
            coordinate_matched |= slice.substitute(
                program,
                machine,
                state,
                entry_parameters,
                destination,
                &mut engine,
                source_symbol,
                *argument,
                &length_bindings,
                &mut substitutions,
            )?;
        }
        if coordinate_matched {
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
            let actual = if lengths::is_slice(program, parameter.type_reference) {
                lengths::actual(
                    program,
                    machine,
                    state,
                    *argument,
                    &length_bindings,
                    &mut engine,
                )?
            } else {
                // The destination slot is a record carrier: the produced
                // length arrives at the record's unique slice leaf, read off
                // the actual exactly as a projected subject does.
                let coordinate = lengths::slice_leaf_coordinate(program, parameter)?;
                coordinate.arrived(
                    program,
                    machine,
                    state,
                    &mut engine,
                    *argument,
                    coordinate.borrowed,
                    &length_bindings,
                )?
            };
            substitutions.insert(identity.clone(), actual);
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
        let actual = match fields::integer_leaf_coordinate(program, parameter) {
            // The destination slot is a record carrier: the role's produced
            // value is the leaf its declaration names, read off the actual.
            Some(coordinate) => {
                coordinate.actual(program, state, &mut engine, *argument, coordinate.borrowed)?
            }
            None => engine.normalize(*argument)?,
        };
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
    // Remainder/quotient atoms embed their operand's display, so the map
    // cannot reach them by name: each minted term whose operand substitutes
    // completely re-mints under the transported operand. An operand leaf the
    // loop above did not cover keeps its atom unmapped and fails closed here.
    engine.extend_argument_map_over_opaque_terms(&mut substitutions);
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
        | RankingRangeMeasure::Computed { .. }
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
        && prove(ceiling.sub(&next_rank), i64::from(!range.end_inclusive))
        && carrier_maximum
            .as_ref()
            .is_none_or(|maximum| prove(maximum.sub(&next_rank), 0));
    // This same edge judgment owns strict decrease and natural-rank formation;
    // callers need not fall back to a second syntactic `n > 0` recognizer.
    let strictly_decreases = prove(rank.sub(&next_rank), 1) && prove(next_rank.clone(), 0);
    Some(RankingRangeEdgeProof {
        membership_and_pinning,
        strictly_decreases,
    })
}

/// The produced rank of a declared computation view: the measure body,
/// normalized once over a private parameter atom, with that atom replaced by
/// the subject's polynomial in `engine`'s namespace. Substituting instead of
/// binding the parameter symbol lets two members that select one measure --
/// and therefore share its parameter symbol -- each produce their own rank
/// in one engine, and lets destination substitution of the subject reach the
/// rank through the subject's atoms.
pub(super) fn computed_rank(
    program: &TypedTrees,
    machine: &Machine,
    engine: &mut Engine<'_>,
    subject: ExpressionHandle,
    parameter: symbols::SymbolHandle,
    body: ExpressionHandle,
) -> Option<Polynomial> {
    const PARAMETER_ATOM: &str = "\0ranking:view:parameter";
    let subject = engine.normalize(subject)?;
    let mut template = Engine::strict_with_symbol_bindings(
        program,
        machine,
        &[StrictArithmeticSymbolBinding {
            symbol: parameter,
            value: StrictArithmeticBindingValue::Atom {
                identity: PARAMETER_ATOM.to_owned(),
                unsigned: true,
            },
        }],
    );
    if !template.strict_symbol_bindings_are_valid() {
        return None;
    }
    let body = template.normalize(body)?;
    inductive_judgment::apply_argument_map(
        &body,
        &BTreeMap::from([(PARAMETER_ATOM.to_owned(), subject)]),
    )
}

/// The greatest value of an unsigned carrier, as the formation ceiling of a
/// computed rank.
pub(super) fn carrier_maximum(carrier: symbols::BuiltinTypeAtom) -> Option<Polynomial> {
    let maximum = match carrier {
        symbols::BuiltinTypeAtom::U8 => u64::from(u8::MAX),
        symbols::BuiltinTypeAtom::U16 => u64::from(u16::MAX),
        symbols::BuiltinTypeAtom::U32 => u64::from(u32::MAX),
        symbols::BuiltinTypeAtom::U64 => u64::MAX,
        _ => return None,
    };
    Some(Polynomial::constant(BigInt::from_u64(maximum)))
}

/// The representable range of an exact integer primitive as BigInt endpoints.
/// The shared interval engine cannot hold `u64::MAX` or `i64::MIN`, so a
/// flow-dependent endpoint formation proof bounds the produced polynomial with
/// this exact carrier range instead of a signed-window approximation.
pub(super) fn integer_carrier_bounds(primitive: PrimitiveType) -> Option<(BigInt, BigInt)> {
    let (signed, bits): (bool, u32) = match primitive {
        PrimitiveType::I8 => (true, 8),
        PrimitiveType::I16 => (true, 16),
        PrimitiveType::I32 => (true, 32),
        PrimitiveType::I64 => (true, 64),
        PrimitiveType::U8 => (false, 8),
        PrimitiveType::U16 => (false, 16),
        PrimitiveType::U32 => (false, 32),
        PrimitiveType::U64 => (false, 64),
        _ => return None,
    };
    let (minimum, maximum): (i128, i128) = if signed {
        (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1)
    } else {
        (0, (1i128 << bits) - 1)
    };
    Some((BigInt::from_i128(minimum), BigInt::from_i128(maximum)))
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
    let mut comparisons = contract_comparisons(program, machine, root, engine)?;
    comparisons.extend(parameter_comparisons(
        program, machine, root, engine, bindings,
    )?);
    Some(comparisons)
}

/// Requires-fact hypotheses at the entry state. A subordinate call site must
/// not substitute them: only the member's own entry-invariant proof can
/// re-establish a requires clause after an internal arrival.
fn contract_comparisons(
    program: &TypedTrees,
    machine: &Machine,
    root: &State,
    engine: &mut Engine<'_>,
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
    Some(comparisons)
}

/// Constrained-type hypotheses for a state's own formals. They hold on every
/// arrival and remain valid at any call site inside that state.
fn parameter_comparisons(
    program: &TypedTrees,
    machine: &Machine,
    root: &State,
    engine: &mut Engine<'_>,
    bindings: &[StrictArithmeticSymbolBinding],
) -> Option<Vec<Comparison>> {
    let admit = |expression| meanings::builtin(program, machine, root, expression, 0);
    let mut comparisons = Vec::new();
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
                let TypeConstraintNode::Range {
                    minimum,
                    maximum,
                    end_inclusive,
                } = constraint
                else {
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
                let maximum = engine.normalize(*maximum)?;
                let maximum = if *end_inclusive {
                    maximum
                } else {
                    maximum.sub(&Polynomial::constant(BigInt::from_i64(1)))
                };
                comparisons.push((
                    BinaryOperator::GreaterOrEqual,
                    Polynomial::atom(identity.clone()),
                    engine.normalize(*minimum)?,
                ));
                comparisons.push((
                    BinaryOperator::LessOrEqual,
                    Polynomial::atom(identity.clone()),
                    maximum,
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
        // A record slot has no integer carrier of its own: it may still carry
        // a bare integer entry role when its declaration holds that role's
        // value at one unique natural leaf of the entry's exact width.
        if entry.is_const
            || (exact_integer_parameter(program, entry.type_reference)
                != exact_integer_parameter(program, parameter.type_reference)
                && carried_natural_coordinate(program, root, parameter, *entry_symbol).is_none())
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
    let primitive = crate::value_custody::recasts::exact_primitive_type(program, reference)?;
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
