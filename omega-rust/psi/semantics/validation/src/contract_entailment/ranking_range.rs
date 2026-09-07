//! Exact, transient rank-range obligations use the ordinary arithmetic engine.
//! The checked ranking owner selects the view and supplies one actual edge.

use super::*;
use typed_trees::state::State;
use typed_trees::types::{PrimitiveType, TypeConstraintNode, TypeReferenceNode};

mod meanings;

#[cfg(test)]
mod tests;

/// The scalar rank produced by an independently selected ranking view.
#[derive(Clone, Copy)]
pub enum RankingRangeMeasure {
    Single(ExpressionHandle),
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
/// Entry parameters may be absent or repeated; neither supplies an arithmetic
/// alias. Every input needed by the rank and its bounds must remain unambiguous.
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
    let ExpressionNode::Range(range) = program.expression_table.expression(range) else {
        return None;
    };
    let admit_template = |expression| meanings::builtin(program, machine, root, expression, 0);
    admit_template(range.start)?;
    admit_template(range.end)?;
    match measure {
        RankingRangeMeasure::Single(subject) => {
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
    let mut bindings = Vec::new();
    for parameter in parameters.iter().filter(|parameter| !parameter.is_self) {
        let Some(primitive) = exact_integer_parameter(program, parameter.type_reference) else {
            // Unrelated bool/record payloads need not be polynomials. Any use
            // of one in the numeric obligation remains unresolved below.
            continue;
        };
        if !parameter.symbol.is_valid() || parameter.is_mutable {
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
    if let Some(entry_parameters) = entry_parameters {
        for (parameter, entry_symbol) in parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .zip(entry_parameters)
        {
            if entry_parameters
                .iter()
                .filter(|candidate| *candidate == entry_symbol)
                .take(2)
                .count()
                != 1
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
            bindings.push(StrictArithmeticSymbolBinding {
                symbol: *entry_symbol,
                value,
            });
        }
    }
    let mut engine = Engine::strict_with_symbol_bindings(program, machine, &bindings);
    if !engine.strict_symbol_bindings_are_valid() {
        return None;
    }
    let auxiliary =
        if arguments.is_none() || !matches!(premises, RankingRangePremises::RankInvariant) {
            entry_comparisons(program, machine, root, &mut engine, &bindings)?
        } else {
            Vec::new()
        };
    let mut comparisons = auxiliary.clone();
    for &(guard, holds) in guards {
        collect_guard(&mut engine, guard, holds, &mut comparisons, 0)?;
    }

    let floor = engine.normalize(range.start)?;
    let ceiling = engine.normalize(range.end)?;
    let rank = match measure {
        RankingRangeMeasure::Single(subject) => engine.normalize(subject)?,
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
        if destination.is_some_and(|destination| {
            destination
                .entry_parameters
                .iter()
                .filter(|candidate| **candidate == source_symbol)
                .take(2)
                .count()
                != 1
        }) {
            // No first/last-wins substitution for duplicated destinations.
            // apply_argument_map rejects an omitted atom if the proof uses it.
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
        substitutions.insert(identity.clone(), engine.normalize(*argument)?);
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
        RankingRangeMeasure::Single(_) | RankingRangeMeasure::Distance { .. } => None,
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
        let entry = root_parameters
            .iter()
            .find(|entry| !entry.is_self && entry.symbol == *entry_symbol)?;
        if !entry.symbol.is_valid()
            || !parameter.symbol.is_valid()
            || entry.is_mutable
            || entry.is_const
            || parameter.is_mutable
            || parameter.is_const
            || (state.symbol == root.symbol && parameter.symbol != *entry_symbol)
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
            if binary.operator == BinaryOperator::Equal
                && matches!(
                    engine.program.expression_table.expression(binary.right),
                    ExpressionNode::Boolean(_)
                ) =>
        {
            let ExpressionNode::Boolean(polarity) =
                engine.program.expression_table.expression(binary.right)
            else {
                return None;
            };
            collect_guard(
                engine,
                binary.left,
                *polarity == holds,
                comparisons,
                depth + 1,
            )
        }
        _ => {
            comparisons.push(inductive_judgment::guard_arm_comparison(
                engine, expression, holds,
            )?);
            Some(())
        }
    }
}
