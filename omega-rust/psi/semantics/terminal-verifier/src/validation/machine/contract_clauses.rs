//! The requires, ensures and outcome-specific ensures clauses of one
//! machine's contract.

use super::super::{
    BTreeSet, ContractClauseKind, IdRegistry, ModuleError, PropositionContext, TerminalMachine,
    TerminalModule, contracts, crash, insert_unique,
};
use semantic_vocabulary::{PlaceId, ValueId};

/// Validates the contract's clauses: each requires, ensures and
/// outcome-specific ensures proposition has the clause's shape, is well
/// formed in the machine's context, stays within the clause's value scope,
/// observes structural cases soundly, and (for ensures) registers its
/// obligation; ensures obligations must be in increasing order.
pub(super) fn validate_contract_clauses(
    module: &TerminalModule,
    machine: &TerminalMachine,
    registry: &mut IdRegistry,
    context: &PropositionContext,
    contract_receiver: Option<PlaceId>,
    requires_values: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let mut ensures_values = requires_values.clone();
    if let Some(result) = machine.result.scalar() {
        ensures_values.insert(result.id);
    }
    for proposition in &machine.contract.requires {
        contracts::validate_contract_clause_kind(
            proposition,
            machine.contract.id,
            ContractClauseKind::Requires,
        )?;
        context
            .validate(proposition)
            .map_err(ModuleError::MalformedProposition)?;
        // Nominal cleanup sites validate their exact Boolean-field requirements
        // against the action's receiver type. That proof-only receiver is not an
        // executable parameter; ordinary contracts use the parameter scope here.
        if contract_receiver.is_none() {
            contracts::validate_contract_scope(
                module,
                machine,
                proposition,
                requires_values,
                machine.contract.id,
                ContractClauseKind::Requires,
            )?;
        }
        crash::validate_structural_case_memberships(module, machine, proposition)?;
    }
    for clause in &machine.contract.ensures {
        insert_unique(
            &mut registry.obligations,
            clause.obligation,
            ModuleError::DuplicateObligation,
        )?;
        contracts::validate_contract_clause_kind(
            &clause.proposition,
            machine.contract.id,
            ContractClauseKind::Ensures,
        )?;
        context
            .validate(&clause.proposition)
            .map_err(ModuleError::MalformedProposition)?;
        contracts::validate_contract_scope(
            module,
            machine,
            &clause.proposition,
            &ensures_values,
            machine.contract.id,
            ContractClauseKind::Ensures,
        )?;
        crash::validate_structural_case_memberships(module, machine, &clause.proposition)?;
    }
    for row in &machine.contract.outcome_specific_ensures {
        insert_unique(
            &mut registry.obligations,
            row.obligation,
            ModuleError::DuplicateObligation,
        )?;
        contracts::validate_contract_clause_kind(
            &row.proposition,
            machine.contract.id,
            ContractClauseKind::Ensures,
        )?;
        context
            .validate(&row.proposition)
            .map_err(ModuleError::MalformedProposition)?;
        contracts::validate_contract_scope(
            module,
            machine,
            &row.proposition,
            &ensures_values,
            machine.contract.id,
            ContractClauseKind::Ensures,
        )?;
        crash::validate_structural_case_memberships(module, machine, &row.proposition)?;
        super::super::evidence::validate_outcome_guard(module, machine, row.guard)?;
    }
    if machine
        .contract
        .ensures
        .windows(2)
        .any(|pair| pair[0].obligation >= pair[1].obligation)
    {
        return Err(ModuleError::NonCanonicalContractEnsures(
            machine.contract.id,
        ));
    }
    Ok(())
}
