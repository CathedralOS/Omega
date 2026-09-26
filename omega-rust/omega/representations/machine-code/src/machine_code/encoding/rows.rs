//! Encoded instruction rows and their exact decoded footprints.

use crate::SelectedFormInternalMachineFixup;
use register_model::{RegisterUnitId, RegisterViewId};
use selected_instructions::{MachineAlternativeKey, MachineEncodedEffects, SelectedInstructionId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeferredControlEncodingReason {
    RequiresResolvedBranchLayout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedFormDecodedFootprint {
    pub register_reads: Vec<RegisterViewId>,
    pub register_writes: Vec<RegisterViewId>,
    pub implicit_defs: Vec<RegisterUnitId>,
    pub implicit_clobbers: Vec<RegisterUnitId>,
    pub encoded: MachineEncodedEffects,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedFormEncodingState {
    Encoded {
        bytes: Vec<u8>,
        footprint: Box<SelectedFormDecodedFootprint>,
    },
    DeferredControl {
        reason: DeferredControlEncodingReason,
    },
    UnresolvedInternalMachineCall {
        bytes: Vec<u8>,
        footprint: Box<SelectedFormDecodedFootprint>,
        fixup: SelectedFormInternalMachineFixup,
    },
    /// Normalized foreign call whose import field stays unresolved until
    /// object construction binds it to the declared import symbol. The fixup
    /// names the selected roster row, never a raw foreign coordinate.
    UnresolvedNormalizedForeignCall {
        bytes: Vec<u8>,
        footprint: Box<SelectedFormDecodedFootprint>,
        fixup: crate::SelectedFormNormalizedForeignCallFixup,
    },
}

/// Closed rule-neutral disposition consumed by generic encoding and layout.
/// Rule-local plans remain the authority; this value is only their exact row
/// projection under authenticated optimization custody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedFormMachineDisposition {
    RetainedV1,
    Aarch64ElidedCompareI64ZeroV1 {
        consumer: SelectedInstructionId,
    },
    Aarch64FusedBranchNonZeroToCbnzV1 {
        compare: SelectedInstructionId,
        source_read: physical_instructions::QualifiedPhysicalRead,
    },
    Aarch64ElidedSameViewCopyI64V1 {
        consumer: SelectedInstructionId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedFormEncodingRow {
    pub instruction: SelectedInstructionId,
    pub alternative: MachineAlternativeKey,
    pub machine_disposition: SelectedFormMachineDisposition,
    pub state: SelectedFormEncodingState,
    pub address: Option<ResolvedPhysicalAddress>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SelectedFormEncodingCounts {
    pub ordinary_encoded: u64,
    pub ordinary_deferred_control: u64,
    pub ordinary_encoded_call_templates: u64,
    pub ordinary_deferred_internal_control: u64,
    pub ordinary_internal_fixups: u64,
    /// Encoded normalized-foreign-call templates whose import fields stay
    /// unresolved until object construction binds them to declared import
    /// symbols. These are not internal control fixups.
    pub ordinary_normalized_foreign_import_fixups: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedPhysicalAddress {
    pub symbolic: physical_instructions::PhysicalAddressOperation,
    /// Signed byte displacement from the operand base. Negative values are
    /// admitted only for frame slots resident below the unadjusted entry
    /// stack pointer inside the ABI red zone.
    pub displacement: i64,
}
