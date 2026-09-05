use super::{NonAuthoritativeCalleeSaveSlotId, NonAuthoritativeCalleeSaveStorageIdentity};
use physical_instructions::PostAllocationMachineIdentity;
use register_homes::{AllocatedCalleeSavedFunctionKind, AllocatedCalleeSavedRequirementIdentity};
use register_model::{
    FrameAbiPreservationConvention, PhysicalRegisterModelIdentity, RegisterViewId,
    TargetRegisterEnvironmentIdentity,
};
use semantic_vocabulary::MachineId;
use target::NativeTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetFrameLayoutPolicy {
    CanonicalOrdinaryCallFrameV1,
    /// Every AArch64 function gives the incoming link an exact frame slot,
    /// whether or not it calls. Unit functions realized by the ordinary
    /// machine emitter carry that slot unconditionally, and the object
    /// boundary requires it of every AArch64 Unit function, so an optimized
    /// Unit route must agree with them rather than take the leaf exemption.
    CanonicalSavedReturnAddressFrameV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReturnAddressFrameCustody {
    /// AMD64 entry custody: `call` placed the return address in the caller's
    /// activation record. The offset is measured from post-prologue RSP.
    CallerActivationStack {
        post_prologue_offset_bytes: u64,
        size_bytes: u16,
    },
    /// A leaf AArch64 function keeps the incoming link in the architectural
    /// link-register view.
    LiveLinkRegister { view: RegisterViewId },
    /// A non-leaf AArch64 function gives the incoming link an exact frame slot.
    SavedLinkRegister {
        view: RegisterViewId,
        frame_offset_bytes: u64,
        size_bytes: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalleeSaveFrameSlot {
    pub abstract_slot: NonAuthoritativeCalleeSaveSlotId,
    pub storage_view: RegisterViewId,
    pub frame_offset_bytes: u64,
    pub size_bytes: u64,
    pub alignment_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionTargetFrameLayout {
    pub machine: MachineId,
    pub kind: AllocatedCalleeSavedFunctionKind,
    pub contains_call: bool,
    pub stack_pointer: RegisterViewId,
    pub pre_call_stack_alignment: u16,
    pub frame_size_bytes: u64,
    pub abi_stack_alignment_bytes: u16,
    pub callee_save_slots: Vec<CalleeSaveFrameSlot>,
    pub return_address: ReturnAddressFrameCustody,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetFrameLayoutPlan {
    pub post_allocation_machine: PostAllocationMachineIdentity,
    pub callee_saved_requirements: AllocatedCalleeSavedRequirementIdentity,
    pub callee_save_storage: NonAuthoritativeCalleeSaveStorageIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub physical_register_model: PhysicalRegisterModelIdentity,
    pub target: NativeTarget,
    pub abi: FrameAbiPreservationConvention,
    pub policy: TargetFrameLayoutPolicy,
    pub functions: Vec<FunctionTargetFrameLayout>,
}
