//! Optimizer module role: executable entrance. Address-free semantic ProgramStorage wrapper planning.
//!
//! This entrance coordinates the canonical recipe with an independent replay:
//! construction cannot publish a wrapper plan until the retained source,
//! frame, action sequence, and symbolic relocation validate together.

mod recipe;
mod validation;

#[cfg(test)]
mod tests;

pub use recipe::OptimizedProgramStorageSemanticReceiverLayout;

use crate::{
    OptimizedProgramStoragePhysicalEntryDisposition, OptimizedProgramStorageSemanticEntryContract,
    ProgramStorageEntryDiagnostic,
};
use crate::{
    ProgramEntrySourceExtentFieldRole, ProgramEntrySourceSignatureIdentity,
    ProgramStorageEntryRootRole,
};
use calling_conventions::{CallingPolicy, MachineRegister};

/// Construct the pure semantic wrapper recipe without selecting a Terminal
/// call target or emitting bytes. `receiver` supplies the referent layout the
/// emitted semantic child derived from its checked `self` parameter exactly
/// when the contract's source signature provisions a mutable receiver.
pub fn plan_optimized_program_storage_semantic_wrapper(
    source: OptimizedProgramStorageSemanticEntryContract,
    receiver: Option<OptimizedProgramStorageSemanticReceiverLayout>,
) -> Result<OptimizedProgramStorageSemanticWrapperPlan, ProgramStorageEntryDiagnostic> {
    validation::validate_contract_surface(&source)?;
    let receiver = validation::receiver_storage(receiver, &source)?;
    let source_signature_identity = source.source_signature_identity();
    let fingerprint = source.semantic_calling_plan_report_fingerprint();
    let frame_byte_count = receiver.map_or(recipe::OUTGOING_FRAME_BYTE_COUNT, |receiver| {
        receiver.outgoing_stack_byte_offset()
            + receiver.slot_byte_count()
            + recipe::RECEIVER_FRAME_TAIL_PAD
    });
    let steps = recipe::expected_steps(fingerprint, receiver);
    let call_step_index = steps
        .iter()
        .position(|step| {
            matches!(
                step,
                OptimizedProgramStorageSemanticWrapperStep::CallPrivateTerminalContinuation { .. }
            )
        })
        .expect("the canonical recipe always emits one continuation call");
    let plan = OptimizedProgramStorageSemanticWrapperPlan {
        source,
        source_signature_identity,
        shadow_byte_count: recipe::SHADOW_BYTE_COUNT,
        outgoing_frame_byte_count: frame_byte_count,
        outgoing_release_byte_count: frame_byte_count,
        pre_call_stack_alignment: recipe::PRE_CALL_STACK_ALIGNMENT,
        receiver,
        steps,
        relocation: recipe::expected_relocation(call_step_index),
        encoding_disposition:
            OptimizedProgramStorageSemanticWrapperEncodingDisposition::TargetEncodingRequiredV1,
        physical_disposition: OptimizedProgramStoragePhysicalEntryDisposition::PlannedNotInvokedV1,
    };
    validate_optimized_program_storage_semantic_wrapper(&plan)?;
    Ok(plan)
}

/// Independently replay the retained contract, frame geometry, action order,
/// call slot, and symbolic relocation requirement.
pub fn validate_optimized_program_storage_semantic_wrapper(
    plan: &OptimizedProgramStorageSemanticWrapperPlan,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    validation::validate(plan)
}

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
    call_step_index: usize,
    byte_width: u8,
    addend: i64,
    kind: OptimizedProgramStorageSemanticWrapperRelocationKind,
    continuation: OptimizedProgramStorageSemanticWrapperContinuationDisposition,
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

/// One compiler-provisioned receiver residence in the outgoing frame.
/// `byte_count`/`alignment` are the checked referent layout supplied by the
/// emitted semantic child; the slot is the 16-aligned zeroed region the
/// wrapper provisions before binding its address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizedProgramStorageSemanticReceiverStorage {
    byte_count: u32,
    alignment: u32,
    slot_byte_count: u32,
    outgoing_stack_byte_offset: u32,
}

impl OptimizedProgramStorageSemanticReceiverStorage {
    pub const fn byte_count(&self) -> u32 {
        self.byte_count
    }

    pub const fn alignment(&self) -> u32 {
        self.alignment
    }

    pub const fn slot_byte_count(&self) -> u32 {
        self.slot_byte_count
    }

    pub const fn outgoing_stack_byte_offset(&self) -> u32 {
        self.outgoing_stack_byte_offset
    }
}

/// One compiler-owned action in the exact semantic wrapper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedProgramStorageSemanticWrapperStep {
    EnterFunction,
    ReserveOutgoingStackFrame {
        byte_count: u32,
    },
    /// Zero-fill the frame region backing the provisioned receiver. The
    /// incoming boundary plan never carries a receiver, so this residence is
    /// the only storage the continuation's `&mut self` argument may name.
    ProvisionReceiverMutableStorage {
        outgoing_stack_byte_offset: u32,
        slot_byte_count: u32,
    },
    /// Bind the provisioned receiver's address into the continuation's first
    /// argument register. The continuation is the checked machine, whose
    /// receiver parameter precedes its two visible Extent roots.
    BindOutgoingReceiverAddress {
        register: MachineRegister,
        outgoing_stack_byte_offset: u32,
        byte_count: u32,
        alignment: u32,
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
    source: OptimizedProgramStorageSemanticEntryContract,
    source_signature_identity: ProgramEntrySourceSignatureIdentity,
    shadow_byte_count: u32,
    outgoing_frame_byte_count: u32,
    outgoing_release_byte_count: u32,
    pre_call_stack_alignment: u16,
    /// Present exactly when the selected source signature provisions a
    /// mutable receiver; the supplied layout was derived from the emitted
    /// child's checked self parameter.
    receiver: Option<OptimizedProgramStorageSemanticReceiverStorage>,
    steps: Vec<OptimizedProgramStorageSemanticWrapperStep>,
    relocation: OptimizedProgramStorageSemanticWrapperRelocationRequirement,
    encoding_disposition: OptimizedProgramStorageSemanticWrapperEncodingDisposition,
    physical_disposition: OptimizedProgramStoragePhysicalEntryDisposition,
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

    pub const fn receiver(&self) -> Option<OptimizedProgramStorageSemanticReceiverStorage> {
        self.receiver
    }

    pub fn steps(&self) -> &[OptimizedProgramStorageSemanticWrapperStep] {
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
