//! A retained original public fact selects its exact applied occurrence.
//!
//! The full-header producer records this association while copying each
//! declaration-owned clause. Consumers use the applied formal/type graph;
//! equal source text, guessed ordinals, and implementation facts supply none.

use super::{SignatureContractKind, StateSignature};
use crate::TypedTrees;
use crate::domain::ProofFact;
use crate::expression::ExpressionHandle;

impl TypedTrees {
    /// Locate a declared obligation in this public header, without assuming
    /// it. Direct facts retain their handle. Copied facts require the original
    /// declaration and exact producing correspondence attached to this instance.
    /// The zero expression handle denotes unavailable copy custody.
    pub fn applied_requirement_contract_expression(
        &self,
        signature: &StateSignature,
        kind: &SignatureContractKind,
        declared_fact: arena::Handle<ProofFact>,
        declared: ExpressionHandle,
    ) -> ExpressionHandle {
        if self.state_signature_contracts(signature)
            .iter()
            .filter(|contract| &contract.kind == kind)
            .flat_map(|contract| contract.facts.handles())
            .any(|fact| (!declared_fact.is_valid() || fact == declared_fact)
                && matches!(self.proof_facts.get(fact), ProofFact::Expression(expression) if *expression == declared))
        {
            return declared;
        }
        // A caller lacking an exact original fact may only name an already
        // owned expression above; equal expressions cannot elect a source row.
        if !declared_fact.is_valid() {
            return ExpressionHandle::invalid();
        }
        let (source_signature, copies) = if let Some(copies) =
            self.public_requirement_contract_copies(signature)
        {
            copies
        } else if let Some(slot) = self.dynamic_slot_by_symbol(signature.symbol) {
            let Some(applied) = self.dynamic_slot_requirement(slot) else {
                return ExpressionHandle::invalid();
            };
            if applied.symbol != signature.symbol {
                return ExpressionHandle::invalid();
            }
            (slot.requirement, slot.contract_fact_copies)
        } else {
            let mut instances =
                self.instantiated_requirement_signatures
                    .iter()
                    .filter(|(_, row)| {
                        self.trait_machine_signatures.get(row.signature).symbol == signature.symbol
                    });
            let Some((_, instance)) = instances.next() else {
                return ExpressionHandle::invalid();
            };
            if instances.next().is_some() {
                return ExpressionHandle::invalid();
            }
            (instance.requirement_template, instance.contract_fact_copies)
        };
        let mut found = ExpressionHandle::invalid();
        let Some(copies) = self.signature_contract_fact_copies.span(copies) else {
            return found;
        };
        for copy in copies {
            if copy.declared_signature != source_signature
                || copy.applied_signature != signature.symbol
                || &copy.kind != kind
                || copy.declared != declared_fact
            {
                continue;
            }
            let ProofFact::Expression(source) = self.proof_facts.get(copy.declared) else {
                continue;
            };
            if *source != declared {
                continue;
            }
            let ProofFact::Expression(applied) = self.proof_facts.get(copy.applied) else {
                return ExpressionHandle::invalid();
            };
            if found.is_valid() && found != *applied {
                return ExpressionHandle::invalid();
            }
            found = *applied;
        }
        found
    }
}
