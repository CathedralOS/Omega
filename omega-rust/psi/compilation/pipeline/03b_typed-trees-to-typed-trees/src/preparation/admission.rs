//! Exact retained preparation inputs share one immutable arena owner.
//!
//! Keep every invocation roster in its original order and retain full payloads.
//! Static application handles name their original frozen issuer: equal graph
//! contents under a different allocation cannot supply that original receipt.
//! This mirrors demanded application materialization without changing ordinary
//! semantic equality of the underlying demand vocabulary. These inputs justify
//! reuse of preparation; they supply no declaration or provider authority.

use arena::{Arena, HandleSpan};

use super::PreparationRequest;

/// One request's complete owned admission inputs for exact preparation reuse.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparationAdmission {
    admits_pending_inputs: bool,
    asm_authority: Option<validation::AsmAuthorityAdmission>,
    rows: Arena<PreparationAdmissionRow>,
    opaque_property_receipts: HandleSpan<PreparationAdmissionRow>,
    opaque_representation_selections: HandleSpan<PreparationAdmissionRow>,
    wire_compatibility_demands: HandleSpan<PreparationAdmissionRow>,
    static_application_demands: HandleSpan<PreparationAdmissionRow>,
    assumed_binding_contract_demands: HandleSpan<PreparationAdmissionRow>,
}

#[derive(Debug, Clone, Default)]
enum PreparationAdmissionRow {
    #[default]
    Absent,
    OpaquePropertyReceipt(validation::OpaqueDataPropertyReceipt),
    OpaqueRepresentationSelection(representation_selections::OpaqueRepresentationSelection),
    WireCompatibilityDemand(language_semantics::wire::WireCompatibilityDemand),
    StaticApplicationDemand(typed_trees::static_applications::StaticApplicationDemand),
    AssumedBindingContractDemand(typed_trees::static_applications::AssumedBindingContractDemand),
}

impl PartialEq for PreparationAdmissionRow {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Absent, Self::Absent) => true,
            (Self::OpaquePropertyReceipt(left), Self::OpaquePropertyReceipt(right)) => {
                left == right
            }
            (
                Self::OpaqueRepresentationSelection(left),
                Self::OpaqueRepresentationSelection(right),
            ) => left == right,
            (Self::WireCompatibilityDemand(left), Self::WireCompatibilityDemand(right)) => {
                left == right
            }
            (Self::StaticApplicationDemand(left), Self::StaticApplicationDemand(right)) => {
                left.declaration == right.declaration
                    && left.application == right.application
                    && std::sync::Arc::ptr_eq(&left.owner, &right.owner)
            }
            (
                Self::AssumedBindingContractDemand(left),
                Self::AssumedBindingContractDemand(right),
            ) => left == right,
            _ => false,
        }
    }
}

impl PreparationRequest<'_> {
    /// Retain every request input and its original owner in one admission.
    pub fn admission(&self) -> PreparationAdmission {
        let mut rows = Arena::new();
        let opaque_property_receipts = rows.insert_many(
            self.opaque_property_receipts
                .iter()
                .cloned()
                .map(PreparationAdmissionRow::OpaquePropertyReceipt),
        );
        let opaque_representation_selections = rows.insert_many(
            self.opaque_representation_selections
                .iter()
                .cloned()
                .map(PreparationAdmissionRow::OpaqueRepresentationSelection),
        );
        let wire_compatibility_demands = rows.insert_many(
            self.wire_compatibility_demands
                .iter()
                .cloned()
                .map(PreparationAdmissionRow::WireCompatibilityDemand),
        );
        let static_application_demands = rows.insert_many(
            self.static_application_demands
                .iter()
                .cloned()
                .map(PreparationAdmissionRow::StaticApplicationDemand),
        );
        let assumed_binding_contract_demands = rows.insert_many(
            self.assumed_binding_contract_demands
                .iter()
                .cloned()
                .map(PreparationAdmissionRow::AssumedBindingContractDemand),
        );
        PreparationAdmission {
            admits_pending_inputs: self.admits_pending_inputs,
            asm_authority: self.asm_authority,
            rows,
            opaque_property_receipts,
            opaque_representation_selections,
            wire_compatibility_demands,
            static_application_demands,
            assumed_binding_contract_demands,
        }
    }
}
