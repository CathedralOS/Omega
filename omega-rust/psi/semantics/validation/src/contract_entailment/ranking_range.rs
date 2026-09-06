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
        guards,
        evaluated_prefix,
        Some(arguments),
    )
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
    prove_edge(program, machine, state, range, measure, &[], &[], None)
        .is_some_and(|proof| proof.membership_and_pinning)
}

fn prove_edge(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    range: ExpressionHandle,
    measure: RankingRangeMeasure,
    guards: &[(ExpressionHandle, bool)],
    evaluated_prefix: &[ExpressionHandle],
    arguments: Option<&[ExpressionHandle]>,
) -> Option<RankingRangeEdgeProof> {
    if program.machine_states(machine).first()?.symbol != state.symbol {
        return None;
    }
    let ExpressionNode::Range(range) = program.expression_table.expression(range) else {
        return None;
    };
    let admit = |expression| meanings::builtin(program, machine, state, expression, 0);
    admit(range.start)?;
    admit(range.end)?;
    match measure {
        RankingRangeMeasure::Single(subject) => {
            admit(subject)?;
        }
        RankingRangeMeasure::Distance { lower, upper }
        | RankingRangeMeasure::IncreasingTo {
            subject: lower,
            limit: upper,
        } => {
            admit(lower)?;
            admit(upper)?;
        }
    }
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
        parameters
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
    let mut engine = Engine::strict_with_symbol_bindings(program, machine, &bindings);
    if !engine.strict_symbol_bindings_are_valid() {
        return None;
    }
    let mut comparisons = Vec::new();
    for contract in program.machine_contracts(machine) {
        if contract.kind != SignatureContractKind::Requires {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            // Unread hypotheses cannot strengthen this positive-only query.
            if let ProofFact::Expression(expression) = fact {
                admit(*expression)?;
                collect_guard(&mut engine, *expression, true, &mut comparisons, 0)?;
            }
        }
    }
    // Declared ranges are enforced at arrivals. Read the exact endpoints, not
    // their display text or the ordinary engine's name-keyed range shortcut.
    for binding in &bindings {
        let parameter = parameters
            .iter()
            .find(|parameter| parameter.symbol == binding.symbol)?;
        let StrictArithmeticBindingValue::Atom { identity, .. } = &binding.value else {
            return None;
        };
        let mut reference = parameter.type_reference;
        while let TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = program.type_reference_table.type_reference(reference)
        {
            for constraint in program.type_reference_table.constraints(*constraints) {
                if let TypeConstraintNode::Range { minimum, maximum } = constraint {
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
            }
            reference = *base_type;
        }
    }
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
    if !engine.install_hypotheses(comparisons) {
        return None;
    }
    if engine.requires_unsatisfiable {
        return Some(RankingRangeEdgeProof {
            membership_and_pinning: true,
            strictly_decreases: true,
        });
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
            membership_and_pinning: entry_membership,
            strictly_decreases: false,
        });
    };
    let mut substitutions = BTreeMap::new();
    // Actual arguments retain the complete non-self formal ordinal. Filtering
    // numeric bindings must not shift a payload slot onto the next rank input.
    for (parameter, argument) in parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .zip(arguments)
    {
        let Some(binding) = bindings
            .iter()
            .find(|binding| binding.symbol == parameter.symbol)
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
