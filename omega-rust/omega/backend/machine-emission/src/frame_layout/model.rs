use physical_instructions::PostAllocationMachineIdentity;
use target::NativeTarget;

use crate::frame_layout::{
    AllocatedCalleeSavedRequirementIdentity, FrameAbiPreservationConvention,
    NonAuthoritativeCalleeSaveStorageIdentity,
};

pub use machine_code::TargetFrameLayoutIdentity;

pub use machine_code::{
    CalleeSaveFrameSlot, FunctionTargetFrameLayout, ReturnAddressFrameCustody,
    TargetFrameLayoutPlan, TargetFrameLayoutPolicy,
};

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
    pub(in crate::frame_layout) plan: TargetFrameLayoutPlan,
    pub(in crate::frame_layout) receipt: TargetFrameLayoutReceipt,
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
