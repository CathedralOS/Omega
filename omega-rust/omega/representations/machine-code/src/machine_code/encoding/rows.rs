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
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SelectedFormEncodingCounts {
    pub ordinary_encoded: u64,
    pub ordinary_deferred_control: u64,
    pub ordinary_encoded_call_templates: u64,
    pub ordinary_deferred_internal_control: u64,
    pub ordinary_internal_fixups: u64,
    pub structural_encoded_call_templates: u64,
    pub structural_encoded_returns: u64,
    pub structural_deferred_internal_control: u64,
    pub structural_internal_fixups: u64,
}
