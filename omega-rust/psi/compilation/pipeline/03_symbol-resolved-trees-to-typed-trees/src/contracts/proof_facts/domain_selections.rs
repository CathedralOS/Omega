//! Resolve contract memberships in their declaration's parameter telescope.
//! Carrier selection binds identity only; ordinary contract proof still owns
//! establishment. Valid selections and ambiguous visible candidates stay intact.
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::AuthoredDeclarationSelectionLateBinding;
use symbols::{JoinedPath, SymbolHandle};
use typed_trees::{
    TypedTrees,
    signature::{SignatureContract, StateParameter},
};

pub(crate) fn normalize(
    source: &symbol_resolved_trees::SymbolResolvedTrees,
    program: &mut TypedTrees,
) -> Result<(), Diagnostic> {
    let mut updates = Vec::new();
    let mut collect = |owner: SymbolHandle,
                       parameters: &[StateParameter],
                       contracts: &[SignatureContract]|
     -> Result<(), Diagnostic> {
        for contract in contracts {
            for handle in contract.facts.handles() {
                let typed_trees::domain::ProofFact::Membership(membership) =
                    program.proof_facts.get(handle)
                else {
                    continue;
                };
                if membership.domain_symbol.is_valid() {
                    continue;
                }
                let Some(carrier) = validation::parameter_expression_result_type_reference(
                    program,
                    owner,
                    parameters,
                    membership.value,
                ) else {
                    continue;
                };
                let carrier = program.type_reference_table.peeled_type_reference(carrier);
                let Some(occurrence) = membership.authored_domain_selection else {
                    continue;
                };
                let selection = program
                    .authored_declaration_selections()
                    .get(occurrence)
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "contract membership lost its authored declaration occurrence",
                        )
                    })?;
                let name = program
                    .domain_path_members
                    .span_or_empty(membership.domain)
                    .joined_path();
                let candidates =
                    crate::type_reference::domain_constraints::select_domain_candidates(
                        source,
                        program,
                        carrier,
                        &name,
                        Some(selection.source_span()),
                        SymbolHandle::invalid(),
                    );
                let [domain] = candidates.as_slice() else {
                    continue;
                };
                // Transparent aliases require atom expansion, owned by the
                // existing proof-fact lowering. Do not substitute an alias
                // identity for that conjunction during declaration selection.
                if domain.alias.is_some() {
                    continue;
                }
                super::check_membership_argument_count(
                    program,
                    domain,
                    membership.domain_arguments.len(),
                )?;
                updates.push((handle, occurrence, domain.symbol));
            }
        }
        Ok(())
    };
    for machine in program.machines() {
        if let Some(entry) = program.machine_states(machine).first() {
            collect(
                machine.symbol,
                program.state_parameters(entry),
                program.machine_contracts(machine),
            )?;
        }
        for state in program.machine_states(machine) {
            collect(
                state.symbol,
                program.state_parameters(state),
                program.state_contracts(state),
            )?;
        }
    }
    for definition in program.traits() {
        for signature in program.trait_machine_signatures(definition) {
            collect(
                signature.symbol,
                program.state_signature_parameters(signature),
                program.state_signature_contracts(signature),
            )?;
        }
    }
    for operator in program.operators() {
        collect(
            operator.symbol,
            program.operator_parameters(operator),
            program.operator_contracts(operator),
        )?;
    }
    let mut selections = program.authored_declaration_selections().clone();
    for (position, (_, occurrence, symbol)) in updates.iter().enumerate() {
        if let Some((_, _, previous)) = updates[..position]
            .iter()
            .find(|(_, prior, _)| prior == occurrence)
        {
            if previous != symbol {
                return Err(Diagnostic::error(
                    "one contract membership occurrence selects conflicting declarations",
                ));
            }
            continue;
        }
        selections
            .finalize_late_bound(
                *occurrence,
                AuthoredDeclarationSelectionLateBinding::CheckedDomainMembership,
                *symbol,
            )
            .map_err(|error| {
                Diagnostic::error(format!(
                    "failed to finalize contract membership declaration custody: {error:?}"
                ))
            })?;
    }
    program.retain_authored_declaration_selections(selections);
    for (handle, _, symbol) in updates {
        if let typed_trees::domain::ProofFact::Membership(membership) =
            program.proof_facts.get_mut(handle)
        {
            membership.domain_symbol = symbol;
        }
    }
    Ok(())
}
