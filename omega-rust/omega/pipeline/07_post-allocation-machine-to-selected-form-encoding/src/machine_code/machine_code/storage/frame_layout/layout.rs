use super::{NonAuthoritativeCalleeSaveSlotId, NonAuthoritativeCalleeSaveStorageIdentity};
use register_homes_to_post_allocation_machine::PostAllocationMachineIdentity;
use selected_instructions_to_selected_instructions::register_homes::AllocatedCalleeSavedRequirementIdentity;
use semantic_vocabulary::MachineId;
use target::NativeTarget;
use target_operations_to_selected_instructions::register_model::{
    FrameAbiPreservationConvention, PhysicalRegisterModelIdentity, RegisterViewId,
    TargetRegisterEnvironmentIdentity,
};

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
    pub id: target_operations_to_selected_instructions::LocalStorageSlotId,
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

/// One register restoration an unwind of the frame performs: the saved value
/// in the committed frame slot at `frame_offset_bytes` — measured upward from
/// the post-prologue stack pointer like every other frame coordinate — is
/// returned to `view`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameUnwindRestore {
    pub view: RegisterViewId,
    pub frame_offset_bytes: u64,
    pub size_bytes: u64,
}

/// The exact restoration roster an unwind of this frame performs, recorded
/// in the order the epilogue executes it: a saved link register is restored
/// first, then every preservation slot in reverse save order, so roster
/// order is strictly descending frame offset. `released_bytes` is the
/// committed extent the unwind returns to the caller's stack after the
/// restorations — always `frame_size_bytes - red_zone_resident_bytes`, since
/// resident bytes were never committed. `return_address` restates the
/// frame's return-address custody so the roster alone is a complete unwind
/// description: after the rows run and the release moves the stack pointer,
/// the continuation comes from the custody the frame recorded.
///
/// The roster is derived from the preservation storage plan and the
/// return-address custody decision, never from emitted bytes; replay
/// recovers the same roster independently and requires exact equality, so
/// the producer's record stays non-authoritative.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameUnwindPlan {
    pub restores: Vec<FrameUnwindRestore>,
    pub released_bytes: u64,
    pub return_address: ReturnAddressFrameCustody,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionTargetFrameLayout {
    pub machine: MachineId,
    pub contains_call: bool,
    pub stack_pointer: RegisterViewId,
    /// Stack-pointer alignment the ABI declares for a call boundary, resolved
    /// through the selected preservation convention — never a frame-local
    /// constant. A frame whose body executes calls commits an extent carrying
    /// the call-entry residue modulo this alignment, so the post-prologue
    /// stack pointer meets the declared boundary at every call site.
    pub pre_call_stack_alignment: u16,
    /// Addressed frame extent in bytes. Slot offsets are measured upward from
    /// the lowest addressed byte; the committed extent is
    /// `frame_size_bytes - red_zone_resident_bytes`.
    pub frame_size_bytes: u64,
    /// Bytes of the addressed extent that live below the unadjusted entry
    /// stack pointer, inside the ABI red zone. Either zero (a committed
    /// frame) or equal to `frame_size_bytes` (the whole extent is
    /// red-zone-resident and the prologue commits nothing). Nonzero only on
    /// leaf functions under conventions that guarantee a red zone.
    pub red_zone_resident_bytes: u64,
    /// The ABI's declared stack alignment in bytes — the same target-owned
    /// preservation-convention declaration `pre_call_stack_alignment` applies
    /// at call sites.
    pub abi_stack_alignment_bytes: u16,
    pub outgoing_abi_area: OutgoingAbiFrameArea,
    pub local_storage_slots: Vec<LocalStorageFrameSlot>,
    /// Activation-local slots whose stable addresses are materialized into
    /// program values, in canonical ascending order without duplicates. A
    /// `FrameAddress` materialization loans the slot's coordinate to whatever
    /// observes the pointer — call argument or result storage transported
    /// across the call boundary — so the coordinate must remain fixed for the
    /// whole activation under any policy that could move storage. Allocator
    /// spill slots are never loaned: the only addresses they gain are the
    /// private reload windows allocation inserts, consumed inside that same
    /// window. Every entry resolves to a `local_storage_slots` member; the
    /// validated layout is this roster's only authority.
    pub stable_address_loans: Vec<target_operations_to_selected_instructions::LocalStorageSlotId>,
    pub callee_save_slots: Vec<CalleeSaveFrameSlot>,
    pub return_address: ReturnAddressFrameCustody,
    pub stack_probe: StackProbePlan,
    /// The exact unwind roster for this frame: which views the epilogue
    /// restores from which committed slots, how many committed bytes the
    /// unwind releases, and where the continuation address is recovered.
    /// See `FrameUnwindPlan` for the ordering contract.
    pub unwind: FrameUnwindPlan,
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
