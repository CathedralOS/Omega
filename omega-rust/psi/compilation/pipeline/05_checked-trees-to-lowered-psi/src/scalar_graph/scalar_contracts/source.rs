//! Guarantees retain authored clause identity before entering the shared scalar
//! predicate lowering. Read replay binds immutable formals and this contract's
//! reserved result; value correspondence independently checks operators/literals.
//! Successful conversion is not proof: emitted return edges must establish each
//! guarantee, and a crash supplies neither a result nor its postconditions.
use super::super::{CheckedIntegerComparisonKind, CheckedTrees, PrimitiveType};
use super::{
    CheckedBooleanExpression, CheckedScalarExpression, ClosedScalarContractValue, LoweringError,
    namespace, unsupported,
};
use checked_trees::domain::ProofFact;
use checked_trees::signature::SignatureContractKind;

/// Which normal-exit guarantees a lowering route publishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GuaranteeScope {
    /// A scalar result publishes every authored guarantee through this
    /// contract, so a clause another owner would carry is refused rather
    /// than erased.
    ScalarResult,
    /// A Unit or structural result publishes only the unconditional
    /// machine-level predicates the closed scalar contract retains. Named
    /// evidence outputs lower through the evidence contract plan, domain
    /// memberships and propositions through their qualification owners, and
    /// outcome-specific or state guarantees keep their own owners; this
    /// publication validates exactly the clauses it publishes.
    PostState,
}

/// Validate a scalar result's guarantees against their authored clauses.
pub(crate) fn validate_guarantees(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    state: &checked_trees::state::State,
) -> Result<(), LoweringError> {
    validate_guarantee_sources(checked, machine, state, GuaranteeScope::ScalarResult).map(drop)
}

/// Validate the post-state guarantees a Unit or structural result publishes
/// and return exactly those closed rows, in authored order.
pub(crate) fn validate_post_state_guarantees(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    state: &checked_trees::state::State,
) -> Result<Vec<Option<ClosedScalarContractValue>>, LoweringError> {
    validate_guarantee_sources(checked, machine, state, GuaranteeScope::PostState)
}

fn validate_guarantee_sources(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    state: &checked_trees::state::State,
    scope: GuaranteeScope,
) -> Result<Vec<Option<ClosedScalarContractValue>>, LoweringError> {
    let contract = checked
        .facts
        .contract_plans
        .for_machine(machine.symbol)
        .ok_or(LoweringError::Unsupported(
            "scalar completion has no checked contract",
        ))?;
    if scope == GuaranteeScope::ScalarResult
        && (contract.closed_scalar_values.has_outcome_specific_clauses()
            || checked
                .state_contracts(state)
                .iter()
                .any(|clause| clause.kind == SignatureContractKind::Ensures))
    {
        return unsupported(
            "scalar completion needs its state or outcome-specific guarantee owner",
        );
    }
    let mut retained = contract.closed_scalar_values.ensures().iter();
    let mut published = Vec::new();
    for source in checked
        .machine_contracts(machine)
        .iter()
        .filter(|source| source.kind == SignatureContractKind::Ensures)
    {
        if source.binding.is_some() {
            // The closed scalar contract omits named witness lanes, so a
            // post-state publication skips them without consuming a row.
            if scope == GuaranteeScope::PostState {
                continue;
            }
            return unsupported("scalar guarantee cannot erase an evidence binding");
        }
        let facts = checked.proof_facts.span(source.facts);
        // A membership or proposition fact has no scalar predicate reading,
        // so the closed contract keeps the whole clause as an unlowered row.
        // A post-state publication leaves that row to its qualification
        // owner; a scalar result still refuses to erase it below.
        if scope == GuaranteeScope::PostState
            && facts.is_some_and(|facts| {
                facts
                    .iter()
                    .any(|fact| !matches!(fact, ProofFact::Expression(_)))
            })
        {
            if !matches!(retained.next(), Some(None)) {
                return unsupported("post-state membership guarantee has a scalar row");
            }
            continue;
        }
        // One keyword may list several facts; the checked row is their
        // conjunction, and each fact replays against its own conjunct.
        let Some(expressions) = facts.filter(|facts| !facts.is_empty()).and_then(|facts| {
            facts
                .iter()
                .map(|fact| match fact {
                    ProofFact::Expression(expression) => Some(*expression),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()
        }) else {
            return unsupported("scalar guarantee has no exact authored predicate");
        };
        let row = retained.next();
        let predicate = match row {
            Some(Some(ClosedScalarContractValue::Predicate(predicate))) => predicate.clone(),
            // Closed legacy rows encode reflexive literal equality, not the
            // truth of the literal itself. Reconstruct that exact meaning for
            // source replay before the shared emitter normalizes it to Truth.
            Some(Some(ClosedScalarContractValue::Boolean(value))) => {
                CheckedBooleanExpression::Equal {
                    left: Box::new(CheckedBooleanExpression::Constant(*value)),
                    right: Box::new(CheckedBooleanExpression::Constant(*value)),
                }
            }
            Some(Some(ClosedScalarContractValue::Integer(literal))) => {
                CheckedBooleanExpression::IntegerComparison {
                    kind: CheckedIntegerComparisonKind::Equal,
                    left: Box::new(CheckedScalarExpression::IntegerLiteral {
                        literal: literal.clone(),
                    }),
                    right: Box::new(CheckedScalarExpression::IntegerLiteral {
                        literal: literal.clone(),
                    }),
                }
            }
            _ => return unsupported("scalar guarantee lost its checked predicate"),
        };
        validate_authored_predicate(checked, state, &expressions, &predicate)?;
        published.push(row.cloned().flatten());
    }
    if retained.next().is_some() {
        return unsupported("scalar completion contains an unauthored guarantee");
    }
    Ok(published)
}

/// Boundary publication may retain scalar consequences beside contracts owned
/// by other lanes. Validate each retained predicate against its authored clause;
/// mathematical equality consequences are separately rejoined to their sealed
/// source application by the common checking owner.
pub(crate) fn validate_boundary_guarantees(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
) -> Result<(), LoweringError> {
    let contract = checked
        .facts
        .contract_plans
        .for_machine(machine.symbol)
        .ok_or(LoweringError::Unsupported(
            "boundary has no checked contract",
        ))?;
    let [state] = checked.machine_states(machine) else {
        return unsupported("boundary scalar guarantees have no exact entry state");
    };
    let mut retained = contract.closed_scalar_values.ensures().iter();
    for source in checked
        .machine_contracts(machine)
        .iter()
        .filter(|source| source.kind == SignatureContractKind::Ensures && source.binding.is_none())
    {
        let row = retained.next().ok_or(LoweringError::Unsupported(
            "boundary scalar guarantee lost its source clause",
        ))?;
        match row {
            Some(ClosedScalarContractValue::Predicate(predicate)) => {
                let expressions = checked
                    .proof_facts
                    .span_or_empty(source.facts)
                    .iter()
                    .map(|fact| match fact {
                        ProofFact::Expression(expression) => Some(*expression),
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>()
                    .filter(|expressions| !expressions.is_empty())
                    .ok_or(LoweringError::Unsupported(
                        "boundary predicate has no authored expression",
                    ))?;
                validate_authored_predicate(checked, state, &expressions, predicate)?;
            }
            Some(ClosedScalarContractValue::FloatMeaningEquality { expression, .. }) => {
                if !matches!(checked.proof_facts.span_or_empty(source.facts),
                    [ProofFact::Expression(authored)] if authored == expression)
                {
                    return unsupported(
                        "boundary meaning guarantee changed its authored expression",
                    );
                }
            }
            _ => {}
        }
    }
    if retained.next().is_some() {
        return unsupported("boundary contains an unauthored scalar guarantee");
    }
    Ok(())
}

fn validate_authored_predicate(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    expressions: &[checked_trees::expression::ExpressionHandle],
    predicate: &CheckedBooleanExpression,
) -> Result<(), LoweringError> {
    namespace::validate(&predicate)?;
    for (expression, conjunct) in expressions
        .iter()
        .zip(authored_conjuncts(&predicate, expressions.len())?)
    {
        let scalar = CheckedScalarExpression::Boolean(Box::new(conjunct.clone()));
        crate::expression_preparation::source_custody::validate_normal_result_read_expression(
            checked,
            state.symbol,
            *expression,
            &scalar,
        )?;
        crate::expression_preparation::source_custody::value_correspondence::validate(
            checked,
            state.symbol,
            0,
            *expression,
            PrimitiveType::Bool,
            &checked_trees::CheckedCallScalarArgument::Pure(scalar),
        )?;
    }
    Ok(())
}

/// Split one clause row back into its authored facts. The checked producer
/// joins a clause's facts left-associated, as `a && b && c` reads, so the
/// last fact is the outermost right conjunct; peeling from the right keeps a
/// first fact that is itself a conjunction whole. A row with fewer
/// conjunctions than facts lost an authored fact and rejects.
fn authored_conjuncts(
    predicate: &CheckedBooleanExpression,
    facts: usize,
) -> Result<Vec<&CheckedBooleanExpression>, LoweringError> {
    let mut conjuncts = Vec::with_capacity(facts);
    let mut rest = predicate;
    for _ in 1..facts {
        let CheckedBooleanExpression::And { left, right } = rest else {
            return unsupported("scalar guarantee lost one of its authored facts");
        };
        conjuncts.push(right.as_ref());
        rest = left;
    }
    conjuncts.push(rest);
    conjuncts.reverse();
    Ok(conjuncts)
}

#[cfg(test)]
mod tests {
    use super::{CheckedBooleanExpression, authored_conjuncts};

    fn fact(position: usize) -> CheckedBooleanExpression {
        CheckedBooleanExpression::Parameter { position }
    }

    fn and(
        left: CheckedBooleanExpression,
        right: CheckedBooleanExpression,
    ) -> CheckedBooleanExpression {
        CheckedBooleanExpression::And {
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    #[test]
    fn each_listed_fact_rejoins_its_own_conjunct() {
        let joined = and(and(fact(0), fact(1)), fact(2));
        assert_eq!(
            authored_conjuncts(&joined, 3).unwrap(),
            [&fact(0), &fact(1), &fact(2)]
        );
        // Two listed facts: the first is itself the conjunction `a && b`.
        assert_eq!(
            authored_conjuncts(&joined, 2).unwrap(),
            [&and(fact(0), fact(1)), &fact(2)]
        );
        assert_eq!(authored_conjuncts(&joined, 1).unwrap(), [&joined]);
    }

    #[test]
    fn a_row_missing_a_listed_fact_rejects() {
        assert!(authored_conjuncts(&and(fact(0), fact(1)), 3).is_err());
        assert!(authored_conjuncts(&fact(0), 2).is_err());
    }
}
