use omega_machine_optimizer::PostAllocationMachineIdentity;
use omega_register_model::{
    PhysicalRegisterModelIdentity, RegisterViewId, TargetRegisterEnvironmentIdentity,
};
use omega_target::NativeTarget;
use psi_core::MachineId;

use crate::{
    AllocatedCalleeSavedFunctionKind, AllocatedCalleeSavedRequirementIdentity,
    FrameAbiPreservationConvention, NonAuthoritativeCalleeSaveSlotId,
    NonAuthoritativeCalleeSaveStorageIdentity,
};

pub use omega_machine_code::TargetFrameLayoutIdentity;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetFrameLayoutReceipt {
    identity: TargetFrameLayoutIdentity,
    post_allocation_machine: PostAllocationMachineIdentity,
    callee_saved_requirements: AllocatedCalleeSavedRequirementIdentity,
    callee_save_storage: NonAuthoritativeCalleeSaveStorageIdentity,
    target: NativeTarget,
    abi: FrameAbiPreservationConvention,
    policy: TargetFrameLayoutPolicy,
    function_count: usize,
    framed_function_count: usize,
    calling_function_count: usize,
    callee_save_slot_count: usize,
    saved_link_count: usize,
    max_frame_size_bytes: u64,
}

impl TargetFrameLayoutReceipt {
    pub const fn identity(self) -> TargetFrameLayoutIdentity {
        self.identity
    }
    pub const fn post_allocation_machine(self) -> PostAllocationMachineIdentity {
        self.post_allocation_machine
    }
    pub const fn callee_saved_requirements(self) -> AllocatedCalleeSavedRequirementIdentity {
        self.callee_saved_requirements
    }
    pub const fn callee_save_storage(self) -> NonAuthoritativeCalleeSaveStorageIdentity {
        self.callee_save_storage
    }
    pub const fn target(self) -> NativeTarget {
        self.target
    }
    pub const fn abi(self) -> FrameAbiPreservationConvention {
        self.abi
    }
    pub const fn policy(self) -> TargetFrameLayoutPolicy {
        self.policy
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn framed_function_count(self) -> usize {
        self.framed_function_count
    }
    pub const fn calling_function_count(self) -> usize {
        self.calling_function_count
    }
    pub const fn callee_save_slot_count(self) -> usize {
        self.callee_save_slot_count
    }
    pub const fn saved_link_count(self) -> usize {
        self.saved_link_count
    }
    pub const fn max_frame_size_bytes(self) -> u64 {
        self.max_frame_size_bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTargetFrameLayout {
    pub(crate) plan: TargetFrameLayoutPlan,
    pub(crate) receipt: TargetFrameLayoutReceipt,
}

impl ValidatedTargetFrameLayout {
    pub const fn plan(&self) -> &TargetFrameLayoutPlan {
        &self.plan
    }
    pub const fn receipt(&self) -> TargetFrameLayoutReceipt {
        self.receipt
    }
}

pub(super) fn seal(plan: &TargetFrameLayoutPlan) -> TargetFrameLayoutReceipt {
    TargetFrameLayoutReceipt {
        identity: super::target_frame_layout_identity(plan),
        post_allocation_machine: plan.post_allocation_machine,
        callee_saved_requirements: plan.callee_saved_requirements,
        callee_save_storage: plan.callee_save_storage,
        target: plan.target,
        abi: plan.abi,
        policy: plan.policy,
        function_count: plan.functions.len(),
        framed_function_count: plan
            .functions
            .iter()
            .filter(|row| row.frame_size_bytes != 0)
            .count(),
        calling_function_count: plan
            .functions
            .iter()
            .filter(|row| row.contains_call)
            .count(),
        callee_save_slot_count: plan
            .functions
            .iter()
            .map(|row| row.callee_save_slots.len())
            .sum(),
        saved_link_count: plan
            .functions
            .iter()
            .filter(|row| {
                matches!(
                    row.return_address,
                    ReturnAddressFrameCustody::SavedLinkRegister { .. }
                )
            })
            .count(),
        max_frame_size_bytes: plan
            .functions
            .iter()
            .map(|row| row.frame_size_bytes)
            .max()
            .unwrap_or(0),
    }
}
