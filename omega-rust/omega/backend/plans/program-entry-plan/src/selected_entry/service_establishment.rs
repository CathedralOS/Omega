//! Source-free receipt for a Fused service established in one selected
//! application-root occurrence.

use crate::ProgramEntrySourceSignatureIdentity;
use effects::provider_plan::{ProviderPlanDigest, ServiceSchemaDigest};

/// Exact semantic authority supplied while provisioning one selected
/// `ProgramEntry` receiver.
///
/// This is not a runtime publication receipt. Its type fixes the first rung to
/// Fused composition, while the retained identities prove which direct erased
/// field the generated entry bridge is responsible for establishing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramEntryFusedServiceEstablishment {
    source_signature_identity: ProgramEntrySourceSignatureIdentity,
    target_slot: target::ProgramEntrySlotDeclaration,
    receiver_type_identity: String,
    attachment_type_identity: String,
    field_identity: String,
    carrier_type_identity: String,
    carrier_base_identity: String,
    bound_domain_identity: String,
    requirement_identity: String,
    service_schema_digest: ServiceSchemaDigest,
    selected_provider_plan_digest: ProviderPlanDigest,
}

impl ProgramEntryFusedServiceEstablishment {
    /// Stable readable coordinate for the exact closed boundary schema. The
    /// name remains diagnostic while the digest prevents same-spelled schema
    /// substitution across packages or contract changes.
    pub fn requirement_identity_for_schema(
        schema: &effects::provider_plan::ServiceSchema,
    ) -> String {
        use std::fmt::Write as _;

        let mut identity = format!("{}#", schema.trait_name);
        for byte in schema.identity_digest().as_bytes() {
            let _ = write!(identity, "{byte:02x}");
        }
        identity
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_signature_identity: ProgramEntrySourceSignatureIdentity,
        target_slot: target::ProgramEntrySlotDeclaration,
        receiver_type_identity: String,
        attachment_type_identity: String,
        field_identity: String,
        carrier_type_identity: String,
        carrier_base_identity: String,
        bound_domain_identity: String,
        requirement_identity: String,
        service_schema_digest: ServiceSchemaDigest,
        selected_provider_plan_digest: ProviderPlanDigest,
    ) -> Result<Self, &'static str> {
        if target_slot != target_slot.owner.program_entry_slot() {
            return Err("Fused root establishment names a noncanonical ProgramEntry slot");
        }
        if receiver_type_identity.is_empty()
            || attachment_type_identity.is_empty()
            || field_identity.is_empty()
            || carrier_type_identity.is_empty()
            || carrier_base_identity.is_empty()
            || bound_domain_identity.is_empty()
            || requirement_identity.is_empty()
        {
            return Err("Fused root establishment contains an empty semantic identity");
        }
        Ok(Self {
            source_signature_identity,
            target_slot,
            receiver_type_identity,
            attachment_type_identity,
            field_identity,
            carrier_type_identity,
            carrier_base_identity,
            bound_domain_identity,
            requirement_identity,
            service_schema_digest,
            selected_provider_plan_digest,
        })
    }

    pub const fn source_signature_identity(&self) -> ProgramEntrySourceSignatureIdentity {
        self.source_signature_identity
    }

    pub const fn target_slot(&self) -> target::ProgramEntrySlotDeclaration {
        self.target_slot
    }

    pub fn receiver_type_identity(&self) -> &str {
        &self.receiver_type_identity
    }

    pub fn attachment_type_identity(&self) -> &str {
        &self.attachment_type_identity
    }

    pub fn field_identity(&self) -> &str {
        &self.field_identity
    }

    pub fn carrier_type_identity(&self) -> &str {
        &self.carrier_type_identity
    }

    pub fn carrier_base_identity(&self) -> &str {
        &self.carrier_base_identity
    }

    pub fn bound_domain_identity(&self) -> &str {
        &self.bound_domain_identity
    }

    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    pub const fn service_schema_digest(&self) -> ServiceSchemaDigest {
        self.service_schema_digest
    }

    pub const fn selected_provider_plan_digest(&self) -> ProviderPlanDigest {
        self.selected_provider_plan_digest
    }
}
