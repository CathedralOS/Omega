//! Rewrite references within one machine, never unrelated arena residents.

use crate::TypedTrees;
use crate::domain::ProofFact;
use crate::machine::Machine;
use crate::signature::SignatureContract;
use arena::HandleSpan;
use symbols::SymbolHandle;

impl TypedTrees {
    /// Retarget a machine's body, contract and `via` references to replacement states.
    /// The declaration's own symbols and other machines remain unchanged.
    /// Callers supply exact state identities, not name-based substitutions.
    pub fn retarget_machine_calls(
        &mut self,
        machine: &Machine,
        replacements: &[(SymbolHandle, SymbolHandle)],
    ) {
        if replacements.is_empty() {
            return;
        }
        self.retarget_contract_calls(machine.contracts, replacements);
        let via_roots = self
            .machine_trait_conformances(machine)
            .iter()
            .map(|conformance| conformance.via_expression)
            .collect::<Vec<_>>();
        for root in via_roots {
            self.expression_table.remap_symbols_in(root, replacements);
        }
        for owned in self.machine_owned_data(machine).to_vec() {
            self.expression_table
                .remap_symbols_in(owned.initial_value, replacements);
        }
        for state in self.machine_states(machine).to_vec() {
            self.retarget_contract_calls(state.contracts, replacements);
            let tables = &mut self.tables;
            tables.statement_table.remap_symbols_in(
                state.statement_nodes,
                &mut tables.expression_table,
                &mut tables.type_reference_table,
                replacements,
            );
        }
    }

    fn retarget_contract_calls(
        &mut self,
        contracts: HandleSpan<SignatureContract>,
        replacements: &[(SymbolHandle, SymbolHandle)],
    ) {
        let roots = self
            .signature_contracts
            .span_or_empty(contracts)
            .iter()
            .flat_map(|contract| self.proof_facts.span_or_empty(contract.facts))
            .flat_map(|fact| match fact {
                ProofFact::Expression(expression) => std::slice::from_ref(expression),
                ProofFact::Membership(membership) => std::slice::from_ref(&membership.value),
                ProofFact::Proposition(application) => self
                    .expression_table
                    .expression_handles(application.arguments),
            })
            .copied()
            .collect::<Vec<_>>();
        for root in roots {
            self.expression_table.remap_symbols_in(root, replacements);
        }
    }
}
