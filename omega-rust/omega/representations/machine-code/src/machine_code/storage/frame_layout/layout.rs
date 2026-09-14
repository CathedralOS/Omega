use super::{NonAuthoritativeCalleeSaveSlotId, NonAuthoritativeCalleeSaveStorageIdentity};
use physical_instructions::PostAllocationMachineIdentity;
use register_homes::AllocatedCalleeSavedRequirementIdentity;
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
pub struct OutgoingAbiFrameArea {
    /// Reserved interval [0, byte_size) from the post-prologue stack pointer.
    /// This is caller ABI storage, never allocator spill or preservation storage.
    pub byte_size: u64,
    pub shadow_bytes: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalStorageFrameSlot {
    pub id: selected_instructions::LocalStorageSlotId,
    pub frame_offset_bytes: u64,
    pub size_bytes: u32,
    pub alignment_bytes: u16,
}

/// Ordered stack-commit touches the prologue performs while growing the
/// frame. A stack whose backing is established lazily (guard-page growth)
/// cannot let one stack-pointer move skip a whole commit granule: each newly
/// entered granule is touched in descending address order before any frame
/// content is stored. `touches == 0` records that the frame fits inside one
/// granule and commits without probing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackProbePlan {
    /// Stack-commit granule the target guarantees, in bytes. Probing commits
    /// a frame one granule at a time; a finer granule is always safe.
    pub interval_bytes: u64,
    /// Granule touches the prologue performs, one per committed chunk, each
    /// at the new stack-pointer position after that chunk is moved.
    pub touches: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionTargetFrameLayout {
    pub machine: MachineId,
    pub contains_call: bool,
    pub stack_pointer: RegisterViewId,
    pub pre_call_stack_alignment: u16,
    pub frame_size_bytes: u64,
    pub abi_stack_alignment_bytes: u16,
    pub outgoing_abi_area: OutgoingAbiFrameArea,
    pub local_storage_slots: Vec<LocalStorageFrameSlot>,
    pub callee_save_slots: Vec<CalleeSaveFrameSlot>,
    pub return_address: ReturnAddressFrameCustody,
    pub stack_probe: StackProbePlan,
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
