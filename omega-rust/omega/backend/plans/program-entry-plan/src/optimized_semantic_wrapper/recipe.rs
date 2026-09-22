//! Canonical Microsoft-x64 frame, action sequence, and symbolic call slot.

use calling_conventions::{CallingPolicy, MachineRegister};

use crate::{ProgramEntrySourceExtentFieldRole, ProgramStorageEntryRootRole};

use super::{
    OptimizedProgramStorageSemanticReceiverStorage,
    OptimizedProgramStorageSemanticWrapperContinuationDisposition,
    OptimizedProgramStorageSemanticWrapperRelocationKind,
    OptimizedProgramStorageSemanticWrapperRelocationRequirement,
    OptimizedProgramStorageSemanticWrapperStep,
};

pub(super) const SHADOW_BYTE_COUNT: u32 = 32;
pub(super) const OUTGOING_FRAME_BYTE_COUNT: u32 = 72;
pub(super) const PRE_CALL_STACK_ALIGNMENT: u16 = 16;
pub(super) const EXTENT_BYTE_COUNT: u16 = 16;
pub(super) const EXTENT_ALIGNMENT: u16 = 8;

/// The provisioned receiver slot follows both extent copies, so the shared
/// copy coordinates stay fixed across both source shapes. Its zeroed region
/// is widened to the next 16-byte multiple: the outgoing frame must keep
/// `sub rsp, N` with `N ≡ 8 (mod 16)` for the call site to stay 16-aligned.
pub(super) const RECEIVER_SLOT_BYTE_OFFSET: u32 = 64;
pub(super) const RECEIVER_FRAME_TAIL_PAD: u32 = 8;

/// The checked referent layout the caller must supply for a receiver-carrying
/// source signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizedProgramStorageSemanticReceiverLayout {
    pub(super) byte_count: u32,
    pub(super) alignment: u32,
}

impl OptimizedProgramStorageSemanticReceiverLayout {
    pub const fn new(byte_count: u32, alignment: u32) -> Self {
        Self {
            byte_count,
            alignment,
        }
    }

    pub const fn byte_count(&self) -> u32 {
        self.byte_count
    }

    pub const fn alignment(&self) -> u32 {
        self.alignment
    }

    /// Zeroed slot width: the referent rounded up to a 16-byte multiple so
    /// every provisioning store is one aligned word and the call-site frame
    /// alignment is unchanged by receiver size.
    pub(super) fn slot_byte_count(self) -> Option<u32> {
        let count = self.byte_count.max(1);
        count.checked_add(15).map(|slot| slot & !15)
    }
}

pub(super) fn expected_steps(
    report_fingerprint: u64,
    receiver: Option<OptimizedProgramStorageSemanticReceiverStorage>,
) -> Vec<OptimizedProgramStorageSemanticWrapperStep> {
    use OptimizedProgramStorageSemanticWrapperStep::*;
    let frame_byte_count = receiver_frame_byte_count(receiver);
    let mut steps = vec![
        EnterFunction,
        ReserveOutgoingStackFrame {
            byte_count: frame_byte_count,
        },
        copy(
            ProgramStorageEntryRootRole::Image,
            0,
            ProgramEntrySourceExtentFieldRole::Base,
            MachineRegister::X86Rcx,
            0,
            32,
        ),
        copy(
            ProgramStorageEntryRootRole::Image,
            0,
            ProgramEntrySourceExtentFieldRole::Length,
            MachineRegister::X86Rcx,
            8,
            40,
        ),
        copy(
            ProgramStorageEntryRootRole::InitialStorage,
            1,
            ProgramEntrySourceExtentFieldRole::Base,
            MachineRegister::X86Rdx,
            0,
            48,
        ),
        copy(
            ProgramStorageEntryRootRole::InitialStorage,
            1,
            ProgramEntrySourceExtentFieldRole::Length,
            MachineRegister::X86Rdx,
            8,
            56,
        ),
    ];
    if let Some(receiver) = receiver {
        steps.push(ProvisionReceiverMutableStorage {
            outgoing_stack_byte_offset: receiver.outgoing_stack_byte_offset,
            slot_byte_count: receiver.slot_byte_count,
        });
        // The continuation is the checked machine `Boot::launch`-shaped entry:
        // self precedes both visible roots under Microsoft-x64 ordering.
        steps.push(BindOutgoingReceiverAddress {
            register: MachineRegister::X86Rcx,
            outgoing_stack_byte_offset: receiver.outgoing_stack_byte_offset,
            byte_count: receiver.byte_count,
            alignment: receiver.alignment,
        });
        steps.push(bind(
            ProgramStorageEntryRootRole::Image,
            0,
            MachineRegister::X86Rdx,
            32,
        ));
        steps.push(bind(
            ProgramStorageEntryRootRole::InitialStorage,
            1,
            MachineRegister::X86R8,
            48,
        ));
    } else {
        steps.push(bind(
            ProgramStorageEntryRootRole::Image,
            0,
            MachineRegister::X86Rcx,
            32,
        ));
        steps.push(bind(
            ProgramStorageEntryRootRole::InitialStorage,
            1,
            MachineRegister::X86Rdx,
            48,
        ));
    }
    steps.push(CallPrivateTerminalContinuation {
        calling_policy: CallingPolicy::MicrosoftX64,
        semantic_calling_plan_report_fingerprint: report_fingerprint,
        disposition:
            OptimizedProgramStorageSemanticWrapperContinuationDisposition::PrivateTerminalSymbolRequiredV1,
    });
    steps.push(ReleaseOutgoingStackFrame {
        byte_count: frame_byte_count,
    });
    steps.push(ReturnUnit);
    steps
}

fn receiver_frame_byte_count(
    receiver: Option<OptimizedProgramStorageSemanticReceiverStorage>,
) -> u32 {
    receiver.map_or(OUTGOING_FRAME_BYTE_COUNT, |receiver| {
        receiver.outgoing_stack_byte_offset + receiver.slot_byte_count + RECEIVER_FRAME_TAIL_PAD
    })
}

pub(super) fn copy(
    role: ProgramStorageEntryRootRole,
    parameter_index: usize,
    field: ProgramEntrySourceExtentFieldRole,
    source_register: MachineRegister,
    source_byte_offset: u16,
    outgoing_stack_byte_offset: u32,
) -> OptimizedProgramStorageSemanticWrapperStep {
    OptimizedProgramStorageSemanticWrapperStep::CopyIncomingIndirectExtentWord {
        role,
        parameter_index,
        field,
        source_register,
        source_byte_offset,
        outgoing_stack_byte_offset,
    }
}

fn bind(
    role: ProgramStorageEntryRootRole,
    parameter_index: usize,
    register: MachineRegister,
    outgoing_stack_byte_offset: u32,
) -> OptimizedProgramStorageSemanticWrapperStep {
    OptimizedProgramStorageSemanticWrapperStep::BindOutgoingExtentCopyAddress {
        role,
        parameter_index,
        register,
        outgoing_stack_byte_offset,
        byte_count: EXTENT_BYTE_COUNT,
        alignment: EXTENT_ALIGNMENT,
    }
}

pub(super) fn expected_relocation(
    call_step_index: usize,
) -> OptimizedProgramStorageSemanticWrapperRelocationRequirement {
    OptimizedProgramStorageSemanticWrapperRelocationRequirement {
        call_step_index,
        byte_width: 4,
        addend: 0,
        kind: OptimizedProgramStorageSemanticWrapperRelocationKind::X86Relative32PrivateContinuationV1,
        continuation: OptimizedProgramStorageSemanticWrapperContinuationDisposition::PrivateTerminalSymbolRequiredV1,
    }
}
