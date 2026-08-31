//! Canonical Microsoft-x64 frame, action sequence, and symbolic call slot.

use omega_calling_conventions::{CallingPolicy, MachineRegister};

use crate::{ProgramEntrySourceExtentFieldRole, ProgramStorageEntryRootRole};

use super::model::{
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
pub(super) const CALL_STEP_INDEX: usize = 8;

pub(super) fn expected_steps(fingerprint: u64) -> [OptimizedProgramStorageSemanticWrapperStep; 11] {
    use OptimizedProgramStorageSemanticWrapperStep::*;
    [
        EnterFunction,
        ReserveOutgoingStackFrame {
            byte_count: OUTGOING_FRAME_BYTE_COUNT,
        },
        copy(ProgramStorageEntryRootRole::Image, 0, ProgramEntrySourceExtentFieldRole::Base, MachineRegister::X86Rcx, 0, 32),
        copy(ProgramStorageEntryRootRole::Image, 0, ProgramEntrySourceExtentFieldRole::Length, MachineRegister::X86Rcx, 8, 40),
        copy(ProgramStorageEntryRootRole::InitialStorage, 1, ProgramEntrySourceExtentFieldRole::Base, MachineRegister::X86Rdx, 0, 48),
        copy(ProgramStorageEntryRootRole::InitialStorage, 1, ProgramEntrySourceExtentFieldRole::Length, MachineRegister::X86Rdx, 8, 56),
        bind(ProgramStorageEntryRootRole::Image, 0, MachineRegister::X86Rcx, 32),
        bind(ProgramStorageEntryRootRole::InitialStorage, 1, MachineRegister::X86Rdx, 48),
        CallPrivateTerminalContinuation {
            calling_policy: CallingPolicy::MicrosoftX64,
            semantic_calling_plan_report_fingerprint: fingerprint,
            disposition: OptimizedProgramStorageSemanticWrapperContinuationDisposition::PrivateTerminalSymbolRequiredV1,
        },
        ReleaseOutgoingStackFrame {
            byte_count: OUTGOING_FRAME_BYTE_COUNT,
        },
        ReturnUnit,
    ]
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

pub(super) fn expected_relocation() -> OptimizedProgramStorageSemanticWrapperRelocationRequirement {
    OptimizedProgramStorageSemanticWrapperRelocationRequirement {
        call_step_index: CALL_STEP_INDEX,
        byte_width: 4,
        addend: 0,
        kind: OptimizedProgramStorageSemanticWrapperRelocationKind::X86Relative32PrivateContinuationV1,
        continuation: OptimizedProgramStorageSemanticWrapperContinuationDisposition::PrivateTerminalSymbolRequiredV1,
    }
}
