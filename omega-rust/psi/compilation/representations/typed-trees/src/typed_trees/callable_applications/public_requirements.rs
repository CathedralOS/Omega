//! Complete contract payloads for one original public call application.
//!
//! These rows retain source declaration/formal symbols for dependency and
//! call-frame identity. Their fresh fact/type graphs belong to the exact
//! application; they are never executable headers or provider candidates.

use super::{AppliedCallableOwner, AppliedCallableSignature};
use crate::{TypedTrees, signature, static_applications};
use arena::HandleSpan;
use symbols::SymbolHandle;

impl TypedTrees {
    pub fn retain_public_requirement_contract(
        &mut self,
        application: static_applications::StaticMachineApplicationHandle,
        signature: signature::StateSignature,
        contract_fact_copies: HandleSpan<signature::SignatureContractFactCopy>,
    ) -> Result<(), &'static str> {
        if !self
            .expression_table
            .static_machine_application_is_valid(application)
        {
            return Err("public contract lost its original application");
        }
        let original = self
            .expression_table
            .static_machine_application(application);
        if original.registration_operation != signature.symbol
            || original.binding_declaration != signature.symbol
            || self.trait_machine_signature(signature.symbol).is_none()
        {
            return Err("public contract lost its original requirement declaration");
        }
        let previous = if original.public_contract.is_valid() {
            original.public_contract
        } else {
            // Mutation retires the read handle, not this occurrence's storage.
            // Reuse its row after rederivation; copies have different handles.
            self.applied_callable_signatures.iter().find_map(|(handle, prepared)| {
                matches!(&prepared.owner, AppliedCallableOwner::PublicRequirement { application: retained, .. } if *retained == application).then_some(handle)
            }).unwrap_or_default()
        };
        let prepared = AppliedCallableSignature {
            owner: AppliedCallableOwner::PublicRequirement {
                application,
                declaration: signature.symbol,
                contract_fact_copies,
            },
            signature,
        };
        let handle = if self.applied_callable_signatures.is_valid(previous) {
            *self.applied_callable_signatures.get_mut(previous) = prepared;
            previous
        } else {
            self.applied_callable_signatures.insert(prepared)
        };
        self.expression_table
            .static_machine_application_mut(application)
            .public_contract = handle;
        Ok(())
    }

    /// The call selects its original application before reading this view.
    /// A missing/invalidated view cannot be reconstructed from equal tuples.
    pub fn public_requirement_contract(
        &self,
        application: static_applications::StaticMachineApplicationHandle,
        declaration: SymbolHandle,
    ) -> Option<&signature::StateSignature> {
        if !self
            .expression_table
            .static_machine_application_is_valid(application)
        {
            return None;
        }
        let original = self
            .expression_table
            .static_machine_application(application);
        if !self
            .applied_callable_signatures
            .is_valid(original.public_contract)
        {
            return None;
        }
        let prepared = self
            .applied_callable_signatures
            .get(original.public_contract);
        let AppliedCallableOwner::PublicRequirement {
            application: retained,
            declaration: retained_declaration,
            ..
        } = &prepared.owner
        else {
            return None;
        };
        (*retained == application
            && *retained_declaration == declaration
            && original.registration_operation == declaration
            && original.binding_declaration == declaration
            && prepared.signature.symbol == declaration)
            .then_some(&prepared.signature)
    }

    pub(crate) fn public_requirement_contract_copies(
        &self,
        signature: &signature::StateSignature,
    ) -> Option<(
        SymbolHandle,
        HandleSpan<signature::SignatureContractFactCopy>,
    )> {
        self.applied_callable_signatures
            .iter()
            .find_map(|(_, prepared)| {
                if !std::ptr::eq(&prepared.signature, signature) {
                    return None;
                }
                let AppliedCallableOwner::PublicRequirement {
                    application,
                    declaration,
                    contract_fact_copies,
                } = &prepared.owner
                else {
                    return None;
                };
                let selected = self.public_requirement_contract(*application, *declaration)?;
                std::ptr::eq(selected, signature).then_some((*declaration, *contract_fact_copies))
            })
    }

    /// Active logical contract graphs selected by their issuing occurrences.
    /// These views retain authored declaration symbols, so declaration lookup
    /// cannot enumerate them. Retired or replaced application rows contribute
    /// no graph to source/result occurrence ownership.
    pub fn public_requirement_contracts(&self) -> impl Iterator<Item = &signature::StateSignature> {
        self.applied_callable_signatures
            .iter()
            .filter_map(|(_, prepared)| {
                let AppliedCallableOwner::PublicRequirement {
                    application,
                    declaration,
                    ..
                } = &prepared.owner
                else {
                    return None;
                };
                let selected = self.public_requirement_contract(*application, *declaration)?;
                std::ptr::eq(selected, &prepared.signature).then_some(selected)
            })
    }

    /// A closed public header retains its own formal/fact copies. This join
    /// authenticates its original call tuple independently, so erased inferred
    /// callbacks still participate in source/proof dependency closure.
    pub fn selected_public_requirement_application(
        &self,
        application: static_applications::StaticMachineApplicationHandle,
        target: SymbolHandle,
    ) -> Option<&static_applications::StaticMachineApplication> {
        if !self
            .expression_table
            .static_machine_application_is_valid(application)
        {
            return None;
        }
        let original = self
            .expression_table
            .static_machine_application(application);
        if self
            .public_requirement_contract(application, target)
            .is_some()
        {
            return Some(original);
        }
        let selected = self.selected_public_trait_requirement_signature(target, application)?;
        if !selected.application.is_valid() {
            return None;
        }
        (original.registration_operation == selected.declaration.symbol
            && original.binding_declaration == selected.declaration.symbol)
            .then_some(original)
    }
}
