//! Batch exact machine-parameter signature queries over one immutable program.
//!
//! Operational inference asks this question for many symbols. Repeating its
//! enclosing-scope walk re-scanned all specialization/capture rosters for the
//! same parents. This borrowed batch indexes every input template occurrence,
//! retaining ambiguity even for equal duplicates, and remembers each original
//! parent's answer, including invalid/ambiguous ancestry; it grants no new scope or
//! contract. Dropping the batch before mutation makes freshness structural.
use super::TypedTrees;
use super::declaration_tables::LifetimeTemplateCandidates;
use crate::{data, machine, signature};
use symbols::SymbolHandle;

pub struct MachineParameterSignatureQueries<'program> {
    program: &'program TypedTrees,
    scope_parents: symbols::SymbolKeyMap<SymbolHandle, SymbolHandle>,
    templates: symbols::SymbolKeyMap<SymbolHandle, LifetimeTemplateCandidates>,
    contracts: symbols::SymbolKeyMap<SymbolHandle, Vec<arena::Handle<data::TypeParameter>>>,
    retained_contracts: symbols::SymbolKeyMap<SymbolHandle, bool>,
}

impl TypedTrees {
    pub fn machine_parameter_signature_queries(&self) -> MachineParameterSignatureQueries<'_> {
        let mut templates: symbols::SymbolKeyMap<SymbolHandle, LifetimeTemplateCandidates> =
            Default::default();
        for (instance, template) in self
            .machine_specializations
            .iter()
            .map(|application| (application.instance, application.template))
            .chain(
                self.captured_machine_applications
                    .iter()
                    .map(|(_, application)| (application.instance, application.template)),
            )
        {
            templates.entry(instance).or_default().include(template);
        }
        // Retain all occurrences in arena order. A nominal/identity contract
        // may have no view while a later occurrence does, so the first row
        // alone cannot answer the original first-valid-view question.
        let mut contracts: symbols::SymbolKeyMap<SymbolHandle, Vec<_>> = Default::default();
        for (handle, parameter) in self.data_type_parameters.iter() {
            if matches!(parameter.kind, data::TypeParameterKind::Machine { .. }) {
                contracts.entry(parameter.symbol).or_default().push(handle);
            }
        }
        MachineParameterSignatureQueries {
            program: self,
            scope_parents: Default::default(),
            templates,
            contracts,
            retained_contracts: Default::default(),
        }
    }
}

impl<'program> MachineParameterSignatureQueries<'program> {
    /// Return the same first lexical owner and exact contract as the ordinary
    /// query. The batch-local roster is only a presence filter over every
    /// Machine-kind arena occurrence; it never selects a contract or owner.
    pub fn get(
        &mut self,
        symbol: SymbolHandle,
    ) -> Option<(
        &'program machine::Machine,
        &'program signature::StateSignature,
    )> {
        let program = self.program;
        program.data_type_parameter_by_symbol(symbol)?;
        if !self.contracts.contains_key(&symbol) {
            return None;
        }
        let parents = &mut self.scope_parents;
        let templates = &self.templates;
        let contracts = &self.contracts;
        let retained = &mut self.retained_contracts;
        program.machines().iter().find_map(|machine| {
            program
                .machine_parameter_signature_in_with_scope_parents(machine, symbol, |owner| {
                    *parents.entry(owner).or_insert_with(|| {
                        let normalized = program.lifetime_declaration_owner_with_templates(
                            owner,
                            |owner| templates.get(&owner).copied().unwrap_or_default(),
                            |symbol| {
                                *retained.entry(symbol).or_insert_with(|| {
                                    symbol.is_valid()
                                        && contracts.get(&symbol).is_some_and(|rows| {
                                            rows.iter().any(|handle| {
                                                let data::TypeParameterKind::Machine { contract } =
                                                    &program.data_type_parameters.get(*handle).kind
                                                else {
                                                    unreachable!()
                                                };
                                                program
                                                    .machine_parameter_contract_view(contract)
                                                    .is_some()
                                            })
                                        })
                                })
                            },
                        );
                        program.declaration_scope_parent_for_lifetime_owner(normalized)
                    })
                })
                .map(|signature| (machine, signature))
        })
    }
}

#[cfg(test)]
mod tests;
