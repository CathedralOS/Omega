//! Checked-source `ProgramEntry` join receipts retained beside a published
//! Terminal artifact.
//!
//! These are durable companion schemas: the producer derives them from the
//! exact checked machine and canonical Terminal module, and later Omega
//! settlement independently rejoins them against retained authority. Merely
//! constructing or carrying a receipt grants no target, calling convention,
//! runtime root, image, installation, or publication authority.

use semantic_vocabulary::{MachineId, PlaceId, StructuralTypeId};

use crate::TerminalPsiIdentity;
use crate::terminal_module::TerminalIdentityIndex;

/// Checked projection of the source receiver into the Terminal entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedProgramEntryReceiverProjection {
    Retained {
        terminal_self: PlaceId,
        source_position: u32,
    },
    Erased {
        source_position: u32,
    },
}

/// Whether the receiver's nominal cleanup must occupy its hosted extent.
///
/// An erased receiver occupies no hosted extent at all, so source cleanup can
/// never acquire ledger residence there. A retained receiver's BSS partition
/// remains occupied for the whole hosted activation; nominal cleanup on that
/// receiver must therefore occupy the same hosted installation ledger extent
/// and stay tracked through it until actual completion retires it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedProgramEntryReceiverCleanup {
    /// The receiver carries no nominal cleanup obligation.
    None,
    /// Nominal cleanup must occupy the receiver's hosted installation ledger
    /// extent: the extent stays tracked through the ledger's aggregate
    /// accounting until completion retires the occupancy.
    OccupiesHostedExtent,
}

/// An exact source Bound-service field requiring separate root establishment.
/// This correspondence carries no selected-plan or installation authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedProgramEntryFusedServiceField {
    field_identity: TerminalIdentityIndex,
    carrier_type_identity: TerminalIdentityIndex,
    field_path: Vec<TerminalIdentityIndex>,
}

impl CheckedProgramEntryFusedServiceField {
    /// `field_path` is the complete route from the receiver root through each
    /// enclosing record field down to the service field itself, so its last
    /// segment is always `field_identity`. A direct receiver field carries a
    /// single segment.
    pub fn new(
        field_identity: TerminalIdentityIndex,
        carrier_type_identity: TerminalIdentityIndex,
        field_path: Vec<TerminalIdentityIndex>,
    ) -> Self {
        Self {
            field_identity,
            carrier_type_identity,
            field_path,
        }
    }

    pub fn field_identity(&self) -> TerminalIdentityIndex {
        self.field_identity
    }

    pub fn carrier_type_identity(&self) -> TerminalIdentityIndex {
        self.carrier_type_identity
    }

    /// The receiver-root-to-field route through enclosing record fields,
    /// ending at the service field's own identity.
    pub fn field_path(&self) -> &[TerminalIdentityIndex] {
        &self.field_path
    }
}

/// An erased receiver-tree field whose term a unique accessible nullary
/// constructor determines (wiki/spec/proofs/contracts.md, Explicit erased
/// bindings). Provisioning supplies that term exactly as construction elaborates an
/// omitted erased initializer: the field has no storage and needs no
/// establishment. This row is the positive Psi-side witness that the erased
/// field is determined evidence rather than a Bound service whose
/// establishment row went missing; it carries no establishment authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedProgramEntryDeterminedEvidenceField {
    carrier_type_identity: TerminalIdentityIndex,
    field_path: Vec<TerminalIdentityIndex>,
}

impl CheckedProgramEntryDeterminedEvidenceField {
    /// `field_path` is the complete route from the receiver root through each
    /// enclosing record field or mixed-shape common field down to the erased
    /// field itself, spelled as for [`CheckedProgramEntryFusedServiceField`].
    pub fn new(
        carrier_type_identity: TerminalIdentityIndex,
        field_path: Vec<TerminalIdentityIndex>,
    ) -> Self {
        Self {
            carrier_type_identity,
            field_path,
        }
    }

    pub fn carrier_type_identity(&self) -> TerminalIdentityIndex {
        self.carrier_type_identity
    }

    pub fn field_path(&self) -> &[TerminalIdentityIndex] {
        &self.field_path
    }
}

/// Checked-source receiver eligibility for zero establishment and no-code
/// owned receiver disposal, bound to the exact Terminal attachment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedProgramEntryReceiverEligibility {
    source_receiver_type_identity: TerminalIdentityIndex,
    owned_receiver_type_identity: TerminalIdentityIndex,
    projection: CheckedProgramEntryReceiverProjection,
    terminal_receiver_type: StructuralTypeId,
    fused_service_fields: Vec<CheckedProgramEntryFusedServiceField>,
    determined_evidence_fields: Vec<CheckedProgramEntryDeterminedEvidenceField>,
    cleanup: CheckedProgramEntryReceiverCleanup,
}

impl CheckedProgramEntryReceiverEligibility {
    pub fn new(
        source_receiver_type_identity: TerminalIdentityIndex,
        owned_receiver_type_identity: TerminalIdentityIndex,
        projection: CheckedProgramEntryReceiverProjection,
        terminal_receiver_type: StructuralTypeId,
        fused_service_fields: Vec<CheckedProgramEntryFusedServiceField>,
        cleanup: CheckedProgramEntryReceiverCleanup,
    ) -> Self {
        Self {
            source_receiver_type_identity,
            owned_receiver_type_identity,
            projection,
            terminal_receiver_type,
            fused_service_fields,
            determined_evidence_fields: Vec::new(),
            cleanup,
        }
    }

    /// Attach the receiver tree's determined erased evidence rows.
    pub fn with_determined_evidence_fields(
        mut self,
        determined_evidence_fields: Vec<CheckedProgramEntryDeterminedEvidenceField>,
    ) -> Self {
        self.determined_evidence_fields = determined_evidence_fields;
        self
    }

    pub fn source_receiver_type_identity(&self) -> TerminalIdentityIndex {
        self.source_receiver_type_identity
    }

    pub fn owned_receiver_type_identity(&self) -> TerminalIdentityIndex {
        self.owned_receiver_type_identity
    }

    pub const fn projection(&self) -> CheckedProgramEntryReceiverProjection {
        self.projection
    }

    pub const fn terminal_receiver_type(&self) -> StructuralTypeId {
        self.terminal_receiver_type
    }

    pub fn fused_service_fields(&self) -> &[CheckedProgramEntryFusedServiceField] {
        &self.fused_service_fields
    }

    pub fn determined_evidence_fields(&self) -> &[CheckedProgramEntryDeterminedEvidenceField] {
        &self.determined_evidence_fields
    }

    /// Whether this receiver's nominal cleanup must occupy its hosted
    /// installation ledger extent through completion.
    pub const fn cleanup(&self) -> CheckedProgramEntryReceiverCleanup {
        self.cleanup
    }
}

/// Durable source-to-Terminal join for one checked `ProgramEntry`.
///
/// The source-signature identity is computed by the build-owned declaration
/// checker and supplied as opaque digest bytes. The remaining fields are
/// reconstructed by the producer from the exact checked machine and the
/// canonical Terminal module. This receipt owns no target, calling convention,
/// runtime roots, image, installation, or publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedProgramEntryTerminalReceipt {
    source_signature_identity: [u8; 32],
    source_machine_name: String,
    declaration: crate::NominalDeclarationIdentity,
    terminal_psi_identity: TerminalPsiIdentity,
    terminal_entry: MachineId,
    receiver_eligibility: Option<CheckedProgramEntryReceiverEligibility>,
}

impl CheckedProgramEntryTerminalReceipt {
    pub fn new(
        source_signature_identity: [u8; 32],
        source_machine_name: String,
        declaration: crate::NominalDeclarationIdentity,
        terminal_psi_identity: TerminalPsiIdentity,
        terminal_entry: MachineId,
        receiver_eligibility: Option<CheckedProgramEntryReceiverEligibility>,
    ) -> Self {
        Self {
            source_signature_identity,
            source_machine_name,
            declaration,
            terminal_psi_identity,
            terminal_entry,
            receiver_eligibility,
        }
    }

    /// Checked-source eligibility for zero establishment and no-code owned
    /// receiver disposal. Absence does not reject generic Terminal production.
    pub const fn receiver_eligibility(&self) -> Option<&CheckedProgramEntryReceiverEligibility> {
        self.receiver_eligibility.as_ref()
    }

    pub const fn source_signature_identity(&self) -> [u8; 32] {
        self.source_signature_identity
    }

    pub fn source_machine_name(&self) -> &str {
        &self.source_machine_name
    }

    /// Exact package/toolchain-owned declaration, independent of source arenas.
    /// Display names cannot distinguish same-named declarations across packages.
    pub const fn declaration(&self) -> &crate::NominalDeclarationIdentity {
        &self.declaration
    }

    pub const fn terminal_psi_identity(&self) -> TerminalPsiIdentity {
        self.terminal_psi_identity
    }

    pub const fn terminal_entry(&self) -> MachineId {
        self.terminal_entry
    }
}
