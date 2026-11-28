/*
Target stack-frame layouts, callee-save storage plans, encoded frame protocols,
applied prologue/epilogue spans, and content identities.

We define the immutable frame geometry (`TargetFrameLayoutPlan`), abstract
callee-save preservation storage (`NonAuthoritativeCalleeSaveStoragePlan`),
packed prologue/epilogue byte arenas (`TargetFrameProtocolEncodingPlan`), and
applied function fragment frame records (`FunctionFragmentFrameApplication`).
All records remain non-authoritative representation data whose invariants and
unwind rosters (`FrameUnwindPlan`) are independently replayed by the backend.
*/

use calling_conventions::frame_unwind::FrameProtocolStep;
use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use post_allocation_machine::PostAllocationMachinePlan;
use register_homes::{AllocatedCalleeSavedRequirements, AllocatedCalleeSavedUnitRequirement};
use register_model::{
    FrameAbiPreservationConvention, PhysicalRegisterModelIdentity,
    PreservationStorageCatalogIdentity, PreservationStorageGroupId, RegisterUnitId, RegisterViewId,
    TargetRegisterEnvironmentIdentity,
};
use selected_instructions::{SelectedBlockId, SelectedInstructionId};
use semantic_vocabulary::{EdgeId, MachineId};
use std::sync::Arc;
use target::NativeTarget;

use crate::FunctionFragmentEmissionPlan;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NonAuthoritativeCalleeSaveStoragePolicy {
    CanonicalTargetPreservationGroupsV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NonAuthoritativeCalleeSaveSlotId(pub u16);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonAuthoritativeCalleeSaveSlot {
    pub id: NonAuthoritativeCalleeSaveSlotId,
    pub storage_group: PreservationStorageGroupId,
    pub storage_view: RegisterViewId,
    pub preserved_units: Vec<RegisterUnitId>,
    pub modified_units: Vec<AllocatedCalleeSavedUnitRequirement>,
    pub abstract_offset_bytes: u64, // Relative only to this artifact's abstract callee-save area origin
    pub size_bytes: u64,
    pub alignment_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionNonAuthoritativeCalleeSaveStorage {
    pub machine: MachineId,
    pub abstract_area_bytes: u64,
    pub abstract_area_alignment: u64,
    pub slots: Vec<NonAuthoritativeCalleeSaveSlot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonAuthoritativeCalleeSaveStoragePlan {
    pub callee_saved_requirements: AllocatedCalleeSavedRequirements,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub physical_register_model: PhysicalRegisterModelIdentity,
    pub preservation_storage_catalog: PreservationStorageCatalogIdentity,
    pub target: NativeTarget,
    pub abi: FrameAbiPreservationConvention,
    pub callee_saved_units: Vec<RegisterUnitId>,
    pub policy: NonAuthoritativeCalleeSaveStoragePolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionNonAuthoritativeCalleeSaveStorage>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetFrameLayoutPolicy {
    CanonicalOrdinaryCallFrameV1,
    CanonicalSavedReturnAddressFrameV1, // Every AArch64 function gives incoming link an exact frame slot whether or not it calls
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReturnAddressFrameCustody {
    CallerActivationStack {
        post_prologue_offset_bytes: u64, // Measured from post-prologue RSP
        size_bytes: u16,
    },
    LiveLinkRegister {
        view: RegisterViewId,
    },
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
    pub byte_size: u64, // Reserved interval [0, byte_size) from post-prologue stack pointer
    pub shadow_bytes: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalStorageFrameSlot {
    pub id: selected_instructions::LocalStorageSlotId,
    pub frame_offset_bytes: u64, // Start of reserved interval; alignment above ABI rounds absolute address upward inside interval
    pub size_bytes: u32,
    pub alignment_bytes: u16,
}

impl LocalStorageFrameSlot {
    pub fn reserved_size_bytes(&self, abi_alignment_bytes: u16) -> u64 {
        u64::from(self.size_bytes)
            + u64::from(self.alignment_bytes.saturating_sub(abi_alignment_bytes))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackProbePlan {
    pub interval_bytes: u64, // Target-guaranteed stack-commit granule in bytes
    pub touches: u32,        // Granule touches performed in descending address order
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameUnwindRestore {
    pub view: RegisterViewId,
    pub frame_offset_bytes: u64,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameUnwindPlan {
    pub restores: Vec<FrameUnwindRestore>, // Ordered in descending frame offset (saved link register first, then preservation slots in reverse save order)
    pub released_bytes: u64, // Committed extent returned to caller stack (`frame_size_bytes - red_zone_resident_bytes`)
    pub return_address: ReturnAddressFrameCustody,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionTargetFrameLayout {
    pub machine: MachineId,
    pub contains_call: bool,
    pub stack_pointer: RegisterViewId,
    pub pre_call_stack_alignment: u16, // ABI stack-pointer alignment at call boundaries
    pub frame_size_bytes: u64, // Addressed frame extent in bytes measured upward from lowest addressed byte
    pub red_zone_resident_bytes: u64, // Bytes living below unadjusted entry stack pointer inside ABI red zone
    pub abi_stack_alignment_bytes: u16,
    pub outgoing_abi_area: OutgoingAbiFrameArea,
    pub local_storage_slots: Vec<LocalStorageFrameSlot>,
    pub stable_address_loans: Vec<selected_instructions::LocalStorageSlotId>, // Canonical ascending local slots whose addresses are materialized into program values
    pub callee_save_slots: Vec<CalleeSaveFrameSlot>,
    pub return_address: ReturnAddressFrameCustody,
    pub stack_probe: StackProbePlan,
    pub unwind: FrameUnwindPlan,
}

impl FunctionTargetFrameLayout {
    pub fn local_storage_slot(
        &self,
        id: selected_instructions::LocalStorageSlotId,
    ) -> Option<&LocalStorageFrameSlot> {
        self.local_storage_slots.iter().find(|slot| slot.id == id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetFrameLayoutPlan {
    pub post_allocation_machine: Arc<PostAllocationMachinePlan>,
    pub callee_saved_requirements: AllocatedCalleeSavedRequirements,
    pub callee_save_storage: Arc<NonAuthoritativeCalleeSaveStoragePlan>,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub physical_register_model: PhysicalRegisterModelIdentity,
    pub target: NativeTarget,
    pub abi: FrameAbiPreservationConvention,
    pub policy: TargetFrameLayoutPolicy,
    pub functions: Vec<FunctionTargetFrameLayout>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetFrameProtocolEncodingPolicy {
    CanonicalFixedFrameV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameProtocolByteSpan {
    pub offset: u32,
    pub length: u32,
}

impl FrameProtocolByteSpan {
    pub fn bytes(self, arena: &[u8]) -> Option<&[u8]> {
        let start = usize::try_from(self.offset).ok()?;
        let end = start.checked_add(usize::try_from(self.length).ok()?)?;
        arena.get(start..end)
    }
}

/// A function's prologue or epilogue steps inside the plan's step arena.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameProtocolStepSpan {
    pub offset: u32,
    pub length: u32,
}

impl FrameProtocolStepSpan {
    pub fn steps(self, arena: &[FrameProtocolStep]) -> Option<&[FrameProtocolStep]> {
        let start = usize::try_from(self.offset).ok()?;
        let end = start.checked_add(usize::try_from(self.length).ok()?)?;
        arena.get(start..end)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionTargetFrameProtocolEncoding {
    pub machine: MachineId,
    pub prologue: FrameProtocolByteSpan,
    pub epilogue: FrameProtocolByteSpan,
    pub prologue_steps: FrameProtocolStepSpan,
    pub epilogue_steps: FrameProtocolStepSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetFrameProtocolEncodingPlan {
    pub frame_layout: std::sync::Arc<TargetFrameLayoutPlan>,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub physical_register_model: PhysicalRegisterModelIdentity,
    pub target: NativeTarget,
    pub policy: TargetFrameProtocolEncodingPolicy,
    pub functions: Vec<FunctionTargetFrameProtocolEncoding>,
    pub bytes: Vec<u8>, // Packed arena for every function's prologue and epilogue
    pub steps: Vec<FrameProtocolStep>, // Packed arena of each instruction's unwind effect, in byte order
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionAppliedFrameEpilogue {
    pub block: SelectedBlockId,
    pub return_instruction: SelectedInstructionId,
    pub psi_return_edge: EdgeId,
    pub function_offset: u64,
    pub byte_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionAppliedFrameProtocol {
    pub machine: MachineId,
    pub prologue_function_offset: u64,
    pub prologue_byte_count: u64,
    pub epilogues: Vec<FunctionAppliedFrameEpilogue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragmentFrameApplication {
    pub functions: Vec<FunctionAppliedFrameProtocol>,
    pub fragments: FunctionFragmentEmissionPlan,
}
