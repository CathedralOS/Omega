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
    /// evidence outputs lower through the evidence contract plan, and
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
    validate_guarantee_sources(checked, machine, state, GuaranteeScope::ScalarResult)
}

/// Validate the post-state guarantees a Unit or structural result publishes.
pub(crate) fn validate_post_state_guarantees(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    state: &checked_trees::state::State,
) -> Result<(), LoweringError> {
    validate_guarantee_sources(checked, machine, state, GuaranteeScope::PostState)
}

fn validate_guarantee_sources(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    state: &checked_trees::state::State,
    scope: GuaranteeScope,
) -> Result<(), LoweringError> {
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
        let Some([ProofFact::Expression(expression)]) = checked.proof_facts.span(source.facts)
        else {
            return unsupported("scalar guarantee has no exact authored predicate");
        };
        let predicate = match retained.next() {
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
        namespace::validate(&predicate)?;
        let scalar = CheckedScalarExpression::Boolean(Box::new(predicate));
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
    if retained.next().is_some() {
        return unsupported("scalar completion contains an unauthored guarantee");
    }
    Ok(())
}
