//! Preserve scalar requirements separately from structural qualification claims.

use super::*;
use checked_trees::domain::ProofFact;
use checked_trees::signature::SignatureContractKind;

pub(crate) fn checked_requirements(
    checked: &CheckedTrees,
    plan: &CheckedBoundaryScalarReturnMachinePlan,
) -> Result<Vec<Option<ClosedScalarContractValue>>, LoweringError> {
    let (machine, state) = crate::scalar_source_custody::authored_state(checked, plan.state)?;
    let contract = checked
        .facts
        .contract_plans
        .for_machine(plan.machine)
        .ok_or(LoweringError::Unsupported(
            "boundary scalar return lacks its checked contract",
        ))?;
    let mut clauses = contract.closed_scalar_values.requires().iter();
    let mut requirements = Vec::new();
    // The checked contract retains one row per unbound authored machine clause,
    // followed by implicit parameter ranges. Only source-owned structural
    // memberships may lack a scalar predicate: their qualifications/claims are
    // carried by the separate structural signature, never by scalar call slots.
    for source in checked
        .machine_contracts(machine)
        .iter()
        .filter(|source| source.kind == SignatureContractKind::Requires && source.binding.is_none())
    {
        let clause = clauses.next().ok_or(LoweringError::Unsupported(
            "boundary scalar contract lost an authored requirement",
        ))?;
        let facts = checked.proof_facts.span_or_empty(source.facts);
        if !plan.structural_parameters.is_empty()
            && !facts.is_empty()
            && facts
                .iter()
                .all(|fact| matches!(fact, ProofFact::Membership(_)))
        {
            for fact in facts {
                memberships::validate(checked, plan, fact)?;
            }
            if clause.is_some() {
                return unsupported("structural membership acquired an unrelated scalar predicate");
            }
            continue;
        }
        if clause.is_none() {
            return unsupported("boundary scalar requirement has no checked predicate");
        }
        requirements.push(clause.clone());
    }
    let mut range_count = 0;
    for parameter in checked.state_parameters(state) {
        let mut reference = parameter.type_reference;
        loop {
            match checked.type_reference_table.type_reference(reference) {
                checked_trees::types::TypeReferenceNode::Reference { referee, .. } => {
                    reference = *referee
                }
                checked_trees::types::TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    range_count += checked
                        .type_reference_table
                        .constraints(*constraints)
                        .iter()
                        .filter(|constraint| {
                            matches!(
                                constraint,
                                checked_trees::types::TypeConstraintNode::Range { .. }
                            )
                        })
                        .count();
                    reference = *base_type;
                }
                _ => break,
            }
        }
    }
    if clauses.len() != range_count {
        return unsupported("boundary scalar contract lost its exact parameter range roster");
    }
    for clause in clauses {
        if clause.is_none() {
            return unsupported("boundary scalar parameter range has no checked predicate");
        }
        requirements.push(clause.clone());
    }
    Ok(requirements)
}
