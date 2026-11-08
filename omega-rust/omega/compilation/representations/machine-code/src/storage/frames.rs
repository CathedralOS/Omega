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

use optimization_core::{
    FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
    OptimizationWorkBudget, OptimizationWorkUsage,
};
use physical_instructions::PostAllocationMachineIdentity;
use register_homes::{
    AllocatedCalleeSavedRequirementIdentity, AllocatedCalleeSavedUnitRequirement,
};
use register_model::{
    FrameAbiPreservationConvention, PhysicalRegisterModelIdentity,
    PreservationStorageCatalogIdentity, PreservationStorageGroupId, RegisterUnitId, RegisterViewId,
    TargetRegisterEnvironmentIdentity,
};
use selected_instructions::{SelectedBlockId, SelectedInstructionId};
use semantic_vocabulary::{EdgeId, MachineId};
use target::NativeTarget;

use crate::FunctionFragmentEmissionPlan;
use crate::identity_encoding::encode_nested_identity;

register_homes::identity_newtype! { pub struct NonAuthoritativeCalleeSaveStorageIdentity([u8; 32]); }
register_homes::identity_newtype! { pub struct TargetFrameLayoutIdentity([u8; 32]); }
register_homes::identity_newtype! { pub struct TargetFrameProtocolEncodingIdentity([u8; 32]); }
register_homes::identity_newtype! { pub struct FunctionFragmentFrameApplicationIdentity([u8; 32]); }

const FRAME_APPLICATION_SCHEMA: &[u8] = b"omega.function-fragment-frame-application.v3";

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
    pub callee_saved_requirements: AllocatedCalleeSavedRequirementIdentity,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionTargetFrameProtocolEncoding {
    pub machine: MachineId,
    pub prologue: FrameProtocolByteSpan,
    pub epilogue: FrameProtocolByteSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetFrameProtocolEncodingPlan {
    pub frame_layout: TargetFrameLayoutIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub physical_register_model: PhysicalRegisterModelIdentity,
    pub target: NativeTarget,
    pub policy: TargetFrameProtocolEncodingPolicy,
    pub functions: Vec<FunctionTargetFrameProtocolEncoding>,
    pub bytes: Vec<u8>, // Packed arena for every function's prologue and epilogue
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
    pub identity: FunctionFragmentFrameApplicationIdentity,
    pub source_fragment_manifest: FunctionFragmentEmissionManifestIdentity,
    pub source_fragments: FunctionFragmentEmissionIdentity,
    pub frame_protocol: TargetFrameProtocolEncodingIdentity,
    pub functions: Vec<FunctionAppliedFrameProtocol>,
    pub fragments: FunctionFragmentEmissionPlan,
}

impl FunctionFragmentFrameApplication {
    pub fn recomputed_identity(&self) -> FunctionFragmentFrameApplicationIdentity {
        function_fragment_frame_application_identity(self)
    }
}

pub fn target_frame_layout_identity(plan: &TargetFrameLayoutPlan) -> TargetFrameLayoutIdentity {
    let mut hasher =
        crate::digest_encoder::DomainDigestEncoder::new(b"omega.target-frame-layout.v10");
    hasher.raw(&plan.post_allocation_machine.bytes());
    hasher.raw(&plan.callee_saved_requirements.bytes());
    hasher.raw(&plan.callee_save_storage.bytes());
    hasher.raw(&plan.register_environment.bytes());
    hasher.raw(&plan.physical_register_model.bytes());
    hasher.raw(&plan.target.identity_bytes());
    hasher.byte(plan.abi.abi_tag());
    hasher.byte(match plan.policy {
        TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1 => 0,
        TargetFrameLayoutPolicy::CanonicalSavedReturnAddressFrameV1 => 1,
    });
    hasher.len(plan.functions.len());
    for function in &plan.functions {
        hasher.raw(&function.machine.get().to_le_bytes());
        hasher.byte(u8::from(function.contains_call));
        hasher.raw(&function.stack_pointer.0.to_le_bytes());
        hasher.raw(&function.pre_call_stack_alignment.to_le_bytes());
        hasher.raw(&function.frame_size_bytes.to_le_bytes());
        hasher.raw(&function.red_zone_resident_bytes.to_le_bytes());
        hasher.raw(&function.abi_stack_alignment_bytes.to_le_bytes());
        hasher.raw(&function.outgoing_abi_area.byte_size.to_le_bytes());
        hasher.raw(&function.outgoing_abi_area.shadow_bytes.to_le_bytes());
        hasher.len(function.local_storage_slots.len());
        for slot in &function.local_storage_slots {
            encode_nested_identity(&mut hasher, |out| slot.id.encode_identity(out));
            hasher.raw(&slot.frame_offset_bytes.to_le_bytes());
            hasher.raw(&slot.size_bytes.to_le_bytes());
            hasher.raw(&slot.alignment_bytes.to_le_bytes());
        }
        hasher.len(function.stable_address_loans.len());
        for slot in &function.stable_address_loans {
            encode_nested_identity(&mut hasher, |out| slot.encode_identity(out));
        }
        hasher.len(function.callee_save_slots.len());
        for slot in &function.callee_save_slots {
            hasher.raw(&slot.abstract_slot.0.to_le_bytes());
            hasher.raw(&slot.storage_view.0.to_le_bytes());
            hasher.raw(&slot.frame_offset_bytes.to_le_bytes());
            hasher.raw(&slot.size_bytes.to_le_bytes());
            hasher.raw(&slot.alignment_bytes.to_le_bytes());
        }
        encode_return_address(&mut hasher, function.return_address);
        hasher.raw(&function.stack_probe.interval_bytes.to_le_bytes());
        hasher.raw(&function.stack_probe.touches.to_le_bytes());
        hasher.len(function.unwind.restores.len());
        for restore in &function.unwind.restores {
            hasher.raw(&restore.view.0.to_le_bytes());
            hasher.raw(&restore.frame_offset_bytes.to_le_bytes());
            hasher.raw(&restore.size_bytes.to_le_bytes());
        }
        hasher.raw(&function.unwind.released_bytes.to_le_bytes());
        encode_return_address(&mut hasher, function.unwind.return_address);
    }
    TargetFrameLayoutIdentity::from_bytes(hasher.finish())
}

fn encode_return_address(
    hasher: &mut crate::digest_encoder::DomainDigestEncoder,
    custody: ReturnAddressFrameCustody,
) {
    match custody {
        ReturnAddressFrameCustody::CallerActivationStack {
            post_prologue_offset_bytes,
            size_bytes,
        } => {
            hasher.byte(0);
            hasher.raw(&post_prologue_offset_bytes.to_le_bytes());
            hasher.raw(&size_bytes.to_le_bytes());
        }
        ReturnAddressFrameCustody::LiveLinkRegister { view } => {
            hasher.byte(1);
            hasher.raw(&view.0.to_le_bytes());
        }
        ReturnAddressFrameCustody::SavedLinkRegister {
            view,
            frame_offset_bytes,
            size_bytes,
        } => {
            hasher.byte(2);
            hasher.raw(&view.0.to_le_bytes());
            hasher.raw(&frame_offset_bytes.to_le_bytes());
            hasher.raw(&size_bytes.to_le_bytes());
        }
    }
}

pub fn target_frame_protocol_encoding_identity(
    plan: &TargetFrameProtocolEncodingPlan,
) -> TargetFrameProtocolEncodingIdentity {
    let mut hasher =
        crate::digest_encoder::DomainDigestEncoder::new(b"omega.target-frame-protocol-encoding.v1");
    hasher.raw(&plan.frame_layout.bytes());
    hasher.raw(&plan.register_environment.bytes());
    hasher.raw(&plan.physical_register_model.bytes());
    hasher.raw(&plan.target.identity_bytes());
    hasher.byte(match plan.policy {
        TargetFrameProtocolEncodingPolicy::CanonicalFixedFrameV1 => 0,
    });
    hasher.u64(plan.functions.len() as u64);
    for function in &plan.functions {
        hasher.raw(&function.machine.get().to_le_bytes());
        hasher.raw(&function.prologue.offset.to_le_bytes());
        hasher.raw(&function.prologue.length.to_le_bytes());
        hasher.raw(&function.epilogue.offset.to_le_bytes());
        hasher.raw(&function.epilogue.length.to_le_bytes());
    }
    hasher.u64(plan.bytes.len() as u64);
    hasher.raw(&plan.bytes);
    TargetFrameProtocolEncodingIdentity::from_bytes(hasher.finish())
}

pub fn function_fragment_frame_application_identity(
    application: &FunctionFragmentFrameApplication,
) -> FunctionFragmentFrameApplicationIdentity {
    let mut hasher = crate::digest_encoder::DomainDigestEncoder::new(FRAME_APPLICATION_SCHEMA);
    hasher.raw(&application.source_fragment_manifest.bytes());
    hasher.raw(&application.source_fragments.bytes());
    hasher.raw(&application.frame_protocol.bytes());
    hasher.u64(application.functions.len() as u64);
    for row in &application.functions {
        hasher.raw(&row.machine.get().to_le_bytes());
        hasher.raw(&row.prologue_function_offset.to_le_bytes());
        hasher.raw(&row.prologue_byte_count.to_le_bytes());
        hasher.u64(row.epilogues.len() as u64);
        for epilogue in &row.epilogues {
            hasher.raw(&epilogue.block.0.to_le_bytes());
            hasher.raw(&epilogue.return_instruction.0.to_le_bytes());
            hasher.raw(&epilogue.psi_return_edge.get().to_le_bytes());
            hasher.raw(&epilogue.function_offset.to_le_bytes());
            hasher.raw(&epilogue.byte_count.to_le_bytes());
        }
    }
    hasher.raw(&application.fragments.identity.bytes());
    FunctionFragmentFrameApplicationIdentity::from_bytes(hasher.finish())
}
