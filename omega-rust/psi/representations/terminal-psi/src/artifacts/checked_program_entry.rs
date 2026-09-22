//! Checked-source `ProgramEntry` join receipts retained beside a published
//! Terminal artifact.
//!
//! These are durable companion schemas: the producer derives them from the
//! exact checked machine and canonical Terminal module, and later Omega
//! settlement independently rejoins them against retained authority. Merely
//! constructing or carrying a receipt grants no target, calling convention,
//! runtime root, image, installation, or publication authority.

use semantic_vocabulary::{MachineId, PlaceId, StructuralTypeId};
use symbols::SymbolHandle;

use crate::TerminalPsiIdentity;

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
    field_identity: String,
    carrier_type_identity: String,
}

impl CheckedProgramEntryFusedServiceField {
    pub fn new(field_identity: String, carrier_type_identity: String) -> Self {
        Self {
            field_identity,
            carrier_type_identity,
        }
    }

    pub fn field_identity(&self) -> &str {
        &self.field_identity
    }

    pub fn carrier_type_identity(&self) -> &str {
        &self.carrier_type_identity
    }
}

/// Checked-source receiver eligibility for zero establishment and no-code
/// owned receiver disposal, bound to the exact Terminal attachment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedProgramEntryReceiverEligibility {
    source_receiver_type_identity: String,
    owned_receiver_type_identity: String,
    projection: CheckedProgramEntryReceiverProjection,
    terminal_receiver_type: StructuralTypeId,
    fused_service_fields: Vec<CheckedProgramEntryFusedServiceField>,
    cleanup: CheckedProgramEntryReceiverCleanup,
}

impl CheckedProgramEntryReceiverEligibility {
    pub fn new(
        source_receiver_type_identity: String,
        owned_receiver_type_identity: String,
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
            cleanup,
        }
    }

    pub fn source_receiver_type_identity(&self) -> &str {
        &self.source_receiver_type_identity
    }

    pub fn owned_receiver_type_identity(&self) -> &str {
        &self.owned_receiver_type_identity
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
    source_machine_symbol: SymbolHandle,
    terminal_psi_identity: TerminalPsiIdentity,
    terminal_entry: MachineId,
    receiver_eligibility: Option<CheckedProgramEntryReceiverEligibility>,
}

impl CheckedProgramEntryTerminalReceipt {
    pub fn new(
        source_signature_identity: [u8; 32],
        source_machine_name: String,
        source_machine_symbol: SymbolHandle,
        terminal_psi_identity: TerminalPsiIdentity,
        terminal_entry: MachineId,
        receiver_eligibility: Option<CheckedProgramEntryReceiverEligibility>,
    ) -> Self {
        Self {
            source_signature_identity,
            source_machine_name,
            source_machine_symbol,
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

    /// The exact checked machine this Terminal entry was produced from.
    /// Settlement compares this symbol, not the display name: a qualified name
    /// cannot rejoin the selected identity when another package declares a
    /// same-named machine.
    pub const fn source_machine_symbol(&self) -> SymbolHandle {
        self.source_machine_symbol
    }

    pub const fn terminal_psi_identity(&self) -> TerminalPsiIdentity {
        self.terminal_psi_identity
    }

    pub const fn terminal_entry(&self) -> MachineId {
        self.terminal_entry
    }
}
