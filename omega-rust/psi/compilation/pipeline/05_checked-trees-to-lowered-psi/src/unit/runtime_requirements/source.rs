//! Ordered scalar bodies reuse the invocation requirement package, but that
//! package is not source authority. Rejoin every authored fact and implicit
//! parameter range before emission. Read custody checks declaration identity;
//! value correspondence separately checks operators and literals. Neither alone
//! prevents a same-typed predicate substitution. Source order and duplicates are
//! checked before canonical publication; callers then owe each published slot
//! before importing guarantees, even if actual substitution makes slots equal.
//! The invocation namespace is shared with runtime requirement conversion: a
//! scalar return does not restrict which parameter observations its entry
//! contract may name. Keeping a second expression roster here rejected view
//! extents already supported by that converter.
use super::super::CheckedTrees;
use super::{CheckedBooleanExpression, LoweringError, unsupported};
use checked_trees::domain::ProofFact;
use checked_trees::signature::SignatureContractKind;

/// Rejoin membership rows at every runtime-contract publisher. Ordinary
/// expression requirements retain their existing source validators; this
/// walk counts their slots so a membership cannot substitute another clause.
pub(crate) fn validate_runtime_memberships(
    checked: &CheckedTrees,
    machine_symbol: symbols::SymbolHandle,
) -> Result<(), LoweringError> {
    let machine = checked
        .machine(machine_symbol)
        .ok_or(LoweringError::Unsupported(
            "runtime membership has no source machine",
        ))?;
    let state = checked
        .machine_states(machine)
        .first()
        .ok_or(LoweringError::Unsupported(
            "runtime membership has no source entry",
        ))?;
    let crash = &checked
        .facts
        .contract_plans
        .for_machine(machine_symbol)
        .ok_or(LoweringError::Unsupported(
            "runtime membership has no contract plan",
        ))?
        .crash;
    let mut retained = crash
        .structural_runtime_requirements()
        .ok_or(LoweringError::Unsupported(
            "runtime membership has no complete requirement package",
        ))?
        .iter();
    let mut subjects = crash.runtime_membership_subjects().iter();
    if checked
        .machine_contracts(machine)
        .iter()
        .chain(checked.state_contracts(state))
        .any(|contract| {
            contract.kind == SignatureContractKind::Requires && contract.binding.is_some()
        })
    {
        return unsupported("runtime entry requirement cannot erase an evidence binding");
    }
    let facts = checked
        .machine_contracts(machine)
        .iter()
        .chain(checked.state_contracts(state))
        .filter(|contract| contract.kind == SignatureContractKind::Requires)
        .flat_map(|contract| checked.proof_facts.span_or_empty(contract.facts));
    for (ordinal, fact) in facts.enumerate() {
        if validation::restates_structural_parameter_qualification(&checked.typed, state, fact) {
            continue;
        }
        match fact {
            ProofFact::Expression(_) => {
                retained.next().ok_or(LoweringError::Unsupported(
                    "runtime requirement lost an authored expression",
                ))?;
            }
            ProofFact::Membership(membership) => validate_membership_source(
                checked,
                membership,
                u32::try_from(ordinal).map_err(|_| {
                    LoweringError::Unsupported("runtime requirement fact ordinal overflow")
                })?,
                &mut subjects,
                &mut retained,
            )?,
            ProofFact::Proposition(_) => {
                return unsupported("runtime membership predicate needs its evidence owner");
            }
        }
    }
    if subjects.next().is_some() {
        return unsupported("runtime requirements retain an unauthored membership subject");
    }
    Ok(())
}

fn validate_membership_source<'clause>(
    checked: &CheckedTrees,
    membership: &checked_trees::domain::ProofMembershipFact,
    ordinal: u32,
    membership_subjects: &mut std::slice::Iter<
        'clause,
        checked_trees::CheckedRuntimeMembershipSubject,
    >,
    retained: &mut std::slice::Iter<'clause, CheckedBooleanExpression>,
) -> Result<(), LoweringError> {
    let witness = membership_subjects
        .next()
        .filter(|row| row.fact_ordinal == ordinal && row.membership == *membership)
        .ok_or(LoweringError::Unsupported(
            "scalar entry membership lost its exact subject row",
        ))?;
    let _ = validation::scalar_membership_carrier(&checked.typed, membership).ok_or(
        LoweringError::Unsupported(
            "scalar entry membership needs its declared theory evidence owner",
        ),
    )?;
    let predicates =
        validation::scalar_membership_predicates(&checked.typed, membership, &witness.subject)
            .ok_or(LoweringError::Unsupported(
                "scalar entry membership has no complete predicate projection",
            ))?;
    for _ in predicates {
        let actual = retained.next().ok_or(LoweringError::Unsupported(
            "scalar entry membership lost a declared predicate",
        ))?;
        super::validate_namespace(actual)?;
    }
    Ok(())
}
