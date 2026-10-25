//! Canonical Microsoft-x64 frame, action sequence, and symbolic call slot.

use calling_conventions::{CallingPolicy, MachineRegister};

use crate::{ProgramEntryExtentFieldRole, ProgramStorageEntryRootRole};

use super::{
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

pub(super) fn expected_steps(
    report_fingerprint: u64,
) -> Vec<OptimizedProgramStorageSemanticWrapperStep> {
    use OptimizedProgramStorageSemanticWrapperStep::*;
    let frame_byte_count = OUTGOING_FRAME_BYTE_COUNT;
    let mut steps = vec![
        EnterFunction,
        ReserveOutgoingStackFrame {
            byte_count: frame_byte_count,
        },
        copy(
            ProgramStorageEntryRootRole::Image,
            0,
            ProgramEntryExtentFieldRole::Base,
            MachineRegister::X86Rcx,
            0,
            32,
        ),
        copy(
            ProgramStorageEntryRootRole::Image,
            0,
            ProgramEntryExtentFieldRole::Length,
            MachineRegister::X86Rcx,
            8,
            40,
        ),
        copy(
            ProgramStorageEntryRootRole::InitialStorage,
            1,
            ProgramEntryExtentFieldRole::Base,
            MachineRegister::X86Rdx,
            0,
            48,
        ),
        copy(
            ProgramStorageEntryRootRole::InitialStorage,
            1,
            ProgramEntryExtentFieldRole::Length,
            MachineRegister::X86Rdx,
            8,
            56,
        ),
    ];
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

pub(super) fn copy(
    role: ProgramStorageEntryRootRole,
    parameter_index: usize,
    field: ProgramEntryExtentFieldRole,
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
