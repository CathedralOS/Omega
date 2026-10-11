//! Source-free receipt for a Fused service established in one selected
//! application-root occurrence.

use crate::ProgramEntrySignatureIdentity;
use effects::provider_plan::{ProviderPlanDigest, ServiceSchemaDigest};

/// Exact semantic authority supplied while provisioning one selected
/// `ProgramEntry` receiver.
///
/// This is not a runtime publication receipt. Its type fixes the first rung to
/// Fused composition, while the retained identities prove which erased field —
/// a direct receiver field or one nested through intermediate record fields —
/// the generated entry bridge is responsible for establishing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramEntryFusedServiceEstablishment {
    source_signature_identity: ProgramEntrySignatureIdentity,
    target_slot: target::ProgramEntrySlotDeclaration,
    receiver_type_identity: terminal_psi::TerminalIdentityIndex,
    attachment_type_identity: terminal_psi::TerminalIdentityIndex,
    field_identity: terminal_psi::TerminalIdentityIndex,
    field_path: Vec<terminal_psi::TerminalIdentityIndex>,
    carrier_type_identity: terminal_psi::TerminalIdentityIndex,
    carrier_base_identity: terminal_psi::TerminalIdentityIndex,
    requirement_identity: terminal_psi::TerminalIdentityIndex,
    service_schema_digest: ServiceSchemaDigest,
    selected_provider_plan_digest: ProviderPlanDigest,
    receiver_application_index: usize,
}

impl ProgramEntryFusedServiceEstablishment {
    /// Stable readable coordinate for the exact closed boundary schema. The
    /// name remains diagnostic while the digest prevents same-spelled schema
    /// substitution across packages or contract changes.
    pub fn requirement_identity_for_schema(
        schema: &effects::provider_plan::ServiceSchema,
    ) -> String {
        use std::fmt::Write as _;

        let mut identity = format!("{}#", schema.identity_text(schema.trait_name));
        for byte in schema.identity_digest().as_bytes() {
            let _ = write!(identity, "{byte:02x}");
        }
        identity
    }

    /// `field_path` is the receiver-root-to-field route through enclosing
    /// record fields; its last segment must be `field_identity`. A direct
    /// receiver field carries a single segment.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_signature_identity: ProgramEntrySignatureIdentity,
        target_slot: target::ProgramEntrySlotDeclaration,
        receiver_type_identity: terminal_psi::TerminalIdentityIndex,
        attachment_type_identity: String,
        field_identity: String,
        field_path: Vec<String>,
        carrier_type_identity: String,
        carrier_base_identity: String,
        requirement_identity: String,
        service_schema_digest: ServiceSchemaDigest,
        selected_provider_plan_digest: ProviderPlanDigest,
        receiver_application_index: usize,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    ) -> Result<Self, &'static str> {
        if target_slot != target_slot.owner.program_entry_slot() {
            return Err("Fused root establishment names a noncanonical ProgramEntry slot");
        }
        if receiver_type_identity.is_absent()
            || attachment_type_identity.is_empty()
            || field_identity.is_empty()
            || field_path.iter().any(|segment| segment.is_empty())
            || carrier_type_identity.is_empty()
            || carrier_base_identity.is_empty()
            || requirement_identity.is_empty()
        {
            return Err("Fused root establishment contains an empty semantic identity");
        }
        if field_path.is_empty() || field_path.last() != Some(&field_identity) {
            return Err("Fused root establishment field path must end at the field identity");
        }
        Ok(Self {
            source_signature_identity,
            target_slot,
            receiver_type_identity,
            attachment_type_identity: identities.intern(&attachment_type_identity),
            field_identity: identities.intern(&field_identity),
            field_path: field_path
                .iter()
                .map(|segment| identities.intern(segment))
                .collect(),
            carrier_type_identity: identities.intern(&carrier_type_identity),
            carrier_base_identity: identities.intern(&carrier_base_identity),
            requirement_identity: identities.intern(&requirement_identity),
            service_schema_digest,
            selected_provider_plan_digest,
            receiver_application_index,
        })
    }

    pub const fn source_signature_identity(&self) -> ProgramEntrySignatureIdentity {
        self.source_signature_identity
    }

    pub const fn target_slot(&self) -> target::ProgramEntrySlotDeclaration {
        self.target_slot
    }

    pub const fn receiver_type_identity(&self) -> terminal_psi::TerminalIdentityIndex {
        self.receiver_type_identity
    }

    pub const fn attachment_type_identity(&self) -> terminal_psi::TerminalIdentityIndex {
        self.attachment_type_identity
    }

    pub const fn field_identity(&self) -> terminal_psi::TerminalIdentityIndex {
        self.field_identity
    }

    /// The receiver-root-to-field route through enclosing record fields,
    /// ending at this row's `field_identity`.
    pub fn field_path(&self) -> &[terminal_psi::TerminalIdentityIndex] {
        &self.field_path
    }

    pub const fn carrier_type_identity(&self) -> terminal_psi::TerminalIdentityIndex {
        self.carrier_type_identity
    }

    pub const fn carrier_base_identity(&self) -> terminal_psi::TerminalIdentityIndex {
        self.carrier_base_identity
    }

    /// The synthesized `trait#digest` requirement this row satisfies.
    pub const fn requirement_identity(&self) -> terminal_psi::TerminalIdentityIndex {
        self.requirement_identity
    }

    pub const fn service_schema_digest(&self) -> ServiceSchemaDigest {
        self.service_schema_digest
    }

    pub const fn selected_provider_plan_digest(&self) -> ProviderPlanDigest {
        self.selected_provider_plan_digest
    }

    /// Exact view in the digest-bound whole plan, not an independently selected
    /// ancestor plan. Its roster limits the slots provisioned for this field.
    pub const fn receiver_application_index(&self) -> usize {
        self.receiver_application_index
    }
}
