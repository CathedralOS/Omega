use calling_conventions::{CallingPolicy, MachineRegister};

use crate::{
    OptimizedProgramStoragePhysicalEntryDisposition, OptimizedProgramStorageSemanticEntryContract,
    ProgramEntrySourceExtentFieldRole, ProgramEntrySourceSignatureIdentity,
    ProgramStorageEntryRootRole,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedProgramStorageSemanticWrapperContinuationDisposition {
    PrivateTerminalSymbolRequiredV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedProgramStorageSemanticWrapperRelocationKind {
    X86Relative32PrivateContinuationV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedProgramStorageSemanticWrapperEncodingDisposition {
    /// A target realization must select, emit, and independently replay one
    /// concrete encoding before any byte or object custody exists.
    TargetEncodingRequiredV1,
}

/// One symbolic relocation requirement. The downstream object join must bind
/// its target to the exact private Terminal entry symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizedProgramStorageSemanticWrapperRelocationRequirement {
    pub(super) call_step_index: usize,
    pub(super) byte_width: u8,
    pub(super) addend: i64,
    pub(super) kind: OptimizedProgramStorageSemanticWrapperRelocationKind,
    pub(super) continuation: OptimizedProgramStorageSemanticWrapperContinuationDisposition,
}

impl OptimizedProgramStorageSemanticWrapperRelocationRequirement {
    pub const fn call_step_index(&self) -> usize {
        self.call_step_index
    }

    pub const fn byte_width(&self) -> u8 {
        self.byte_width
    }

    pub const fn addend(&self) -> i64 {
        self.addend
    }

    pub const fn kind(&self) -> OptimizedProgramStorageSemanticWrapperRelocationKind {
        self.kind
    }

    pub const fn continuation(
        &self,
    ) -> OptimizedProgramStorageSemanticWrapperContinuationDisposition {
        self.continuation
    }
}

/// One compiler-owned action in the exact receiver-free semantic wrapper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedProgramStorageSemanticWrapperStep {
    EnterFunction,
    ReserveOutgoingStackFrame {
        byte_count: u32,
    },
    CopyIncomingIndirectExtentWord {
        role: ProgramStorageEntryRootRole,
        parameter_index: usize,
        field: ProgramEntrySourceExtentFieldRole,
        source_register: MachineRegister,
        source_byte_offset: u16,
        outgoing_stack_byte_offset: u32,
    },
    BindOutgoingExtentCopyAddress {
        role: ProgramStorageEntryRootRole,
        parameter_index: usize,
        register: MachineRegister,
        outgoing_stack_byte_offset: u32,
        byte_count: u16,
        alignment: u16,
    },
    CallPrivateTerminalContinuation {
        calling_policy: CallingPolicy,
        semantic_calling_plan_report_fingerprint: u64,
        disposition: OptimizedProgramStorageSemanticWrapperContinuationDisposition,
    },
    ReleaseOutgoingStackFrame {
        byte_count: u32,
    },
    ReturnUnit,
}

/// Exact address-free recipe for the clean semantic ProgramStorage wrapper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedProgramStorageSemanticWrapperPlan {
    pub(super) source: OptimizedProgramStorageSemanticEntryContract,
    pub(super) source_signature_identity: ProgramEntrySourceSignatureIdentity,
    pub(super) shadow_byte_count: u32,
    pub(super) outgoing_frame_byte_count: u32,
    pub(super) outgoing_release_byte_count: u32,
    pub(super) pre_call_stack_alignment: u16,
    pub(super) steps: [OptimizedProgramStorageSemanticWrapperStep; 11],
    pub(super) relocation: OptimizedProgramStorageSemanticWrapperRelocationRequirement,
    pub(super) encoding_disposition: OptimizedProgramStorageSemanticWrapperEncodingDisposition,
    pub(super) physical_disposition: OptimizedProgramStoragePhysicalEntryDisposition,
}

impl OptimizedProgramStorageSemanticWrapperPlan {
    pub const fn source(&self) -> &OptimizedProgramStorageSemanticEntryContract {
        &self.source
    }

    pub const fn source_signature_identity(&self) -> ProgramEntrySourceSignatureIdentity {
        self.source_signature_identity
    }

    pub const fn shadow_byte_count(&self) -> u32 {
        self.shadow_byte_count
    }

    pub const fn outgoing_frame_byte_count(&self) -> u32 {
        self.outgoing_frame_byte_count
    }

    pub const fn outgoing_release_byte_count(&self) -> u32 {
        self.outgoing_release_byte_count
    }

    pub const fn pre_call_stack_alignment(&self) -> u16 {
        self.pre_call_stack_alignment
    }

    pub const fn steps(&self) -> &[OptimizedProgramStorageSemanticWrapperStep; 11] {
        &self.steps
    }

    pub const fn relocation(&self) -> &OptimizedProgramStorageSemanticWrapperRelocationRequirement {
        &self.relocation
    }

    pub const fn encoding_disposition(
        &self,
    ) -> OptimizedProgramStorageSemanticWrapperEncodingDisposition {
        self.encoding_disposition
    }

    pub const fn physical_disposition(&self) -> OptimizedProgramStoragePhysicalEntryDisposition {
        self.physical_disposition
    }
}
