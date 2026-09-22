#![forbid(unsafe_code)]

//! Optimizer module role: executable entrance. Target-owned ordinary frame geometry.
//!
//! This backend calculation joins the post-allocation machine plan to validated
//! preservation storage and chooses exact stack-frame coordinates. The result
//! is independently replayed. It does not claim that prologue, epilogue,
//! unwind, probing, or memory-access instructions have been emitted.
//! Durable plans live in `machine_code::storage::frame_layout`; only the
//! independently validated wrappers and implementation state live here.

mod call_site;
mod compute;
mod error;
mod replay;
mod save_storage;
mod spill_requirements;
mod stack_commit;
mod unwind;
mod validation;

pub use error::*;
pub use machine_code::target_frame_layout_identity;
pub use save_storage::*;
pub use spill_requirements::*;
pub use validation::validate_target_frame_layout;

pub(crate) use unwind::{FrameContinuationCustody, frame_unwind_policy};

pub use machine_code::{
    CalleeSaveFrameSlot, FrameUnwindPlan, FrameUnwindRestore, FunctionTargetFrameLayout,
    ReturnAddressFrameCustody, StackProbePlan, TargetFrameLayoutIdentity, TargetFrameLayoutPlan,
    TargetFrameLayoutPolicy,
};
use physical_instructions::PostAllocationMachineIdentity;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;
use selected_instructions_to_register_homes::{
    AllocatedCalleeSavedRequirementIdentity, ValidatedAllocatedCalleeSavedRequirements,
};
use target::NativeTarget;

pub fn stage_target_frame_layout(
    machine: &StagedOptimizedPostAllocationMachinePlan,
    requirements: &ValidatedAllocatedCalleeSavedRequirements,
    storage: &ValidatedNonAuthoritativeCalleeSaveStorage,
    environment: &ValidatedTargetRegisterEnvironment,
    policy: TargetFrameLayoutPolicy,
) -> Result<ValidatedTargetFrameLayout, TargetFrameLayoutError> {
    let plan = compute::derive(machine, requirements, storage, environment, policy)?;
    validate_target_frame_layout(machine, requirements, storage, environment, plan)
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
    probed_function_count: usize,
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
    pub const fn probed_function_count(self) -> usize {
        self.probed_function_count
    }
    pub const fn max_frame_size_bytes(self) -> u64 {
        self.max_frame_size_bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTargetFrameLayout {
    pub(in crate::frame_layout) plan: std::sync::Arc<TargetFrameLayoutPlan>,
    pub(in crate::frame_layout) receipt: TargetFrameLayoutReceipt,
}

impl ValidatedTargetFrameLayout {
    pub fn plan(&self) -> &TargetFrameLayoutPlan {
        &self.plan
    }
    pub fn shared_plan(&self) -> std::sync::Arc<TargetFrameLayoutPlan> {
        std::sync::Arc::clone(&self.plan)
    }
    pub const fn receipt(&self) -> TargetFrameLayoutReceipt {
        self.receipt
    }
}

fn seal(plan: &TargetFrameLayoutPlan) -> TargetFrameLayoutReceipt {
    TargetFrameLayoutReceipt {
        identity: self::target_frame_layout_identity(plan),
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
        probed_function_count: plan
            .functions
            .iter()
            .filter(|row| row.stack_probe.touches != 0)
            .count(),
        max_frame_size_bytes: plan
            .functions
            .iter()
            .map(|row| row.frame_size_bytes)
            .max()
            .unwrap_or(0),
    }
}
