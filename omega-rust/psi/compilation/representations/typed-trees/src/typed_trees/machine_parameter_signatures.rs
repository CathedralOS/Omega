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
use crate::{machine, signature};
use symbols::SymbolHandle;

pub struct MachineParameterSignatureQueries<'program> {
    program: &'program TypedTrees,
    scope_parents: symbols::SymbolKeyMap<SymbolHandle, SymbolHandle>,
    templates: symbols::SymbolKeyMap<SymbolHandle, LifetimeTemplateCandidates>,
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
        MachineParameterSignatureQueries {
            program: self,
            scope_parents: Default::default(),
            templates,
        }
    }
}

impl<'program> MachineParameterSignatureQueries<'program> {
    /// Return the same first lexical owner and exact contract as the ordinary
    /// query. The owning parameter table supplies the same presence and
    /// retained-contract questions; the batch only shares ancestry work.
    pub fn get(
        &mut self,
        symbol: SymbolHandle,
    ) -> Option<(
        &'program machine::Machine,
        &'program signature::StateSignature,
    )> {
        let program = self.program;
        program.retained_static_machine_contract(symbol)?;
        let parents = &mut self.scope_parents;
        let templates = &self.templates;
        program.machines().iter().find_map(|machine| {
            program
                .machine_parameter_signature_in_with_scope_parents(machine, symbol, |owner| {
                    *parents.entry(owner).or_insert_with(|| {
                        let normalized = program.lifetime_declaration_owner_with_templates(
                            owner,
                            |owner| templates.get(&owner).copied().unwrap_or_default(),
                            |symbol| program.retained_static_machine_contract(symbol).is_some(),
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
