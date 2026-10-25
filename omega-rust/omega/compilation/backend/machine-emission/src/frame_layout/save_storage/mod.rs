//! Optimizer module role: executable entrance. Non-authoritative callee-save storage.
//!
//! This boundary maps validated modified ABI-preserved units through the
//! target-owned storage catalog, assigns canonical abstract area-relative
//! slots, and admits the result only after independent keyed replay. It grants
//! no frame, save/restore instruction, memory, unwind, or publication authority.

mod compute;
mod custody;
mod error;
mod identity;
mod replay;
mod validation;
mod work;

pub(crate) use error::NonAuthoritativeCalleeSaveStorageError;
pub(crate) use identity::non_authoritative_callee_save_storage_identity;
pub(crate) use validation::validate_non_authoritative_callee_save_storage;

use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

pub use machine_code::{
    FunctionNonAuthoritativeCalleeSaveStorage, NonAuthoritativeCalleeSaveSlot,
    NonAuthoritativeCalleeSaveSlotId, NonAuthoritativeCalleeSaveStorageIdentity,
    NonAuthoritativeCalleeSaveStoragePlan, NonAuthoritativeCalleeSaveStoragePolicy,
};
use register_environment::{
    FrameAbiPreservationConvention, ValidatedTargetRegisterEnvironment, selected_abi_preservation,
    selected_preservation_storage_catalog,
};
use register_model::{
    PhysicalRegisterModelIdentity, PreservationStorageCatalogIdentity,
    TargetRegisterEnvironmentIdentity, ValidatedPreservationStorageCatalog,
};
use selected_instructions_to_register_homes::{
    AllocatedCalleeSavedRequirementIdentity, AllocatedCalleeSavedRequirementPlan,
    AllocatedCalleeSavedUnitRequirement, FunctionAllocatedCalleeSavedRequirements,
    ValidatedAllocatedCalleeSavedRequirements, encode_callee_saved_modification_witness_identity,
};
use target::NativeTarget;

pub(crate) fn stage_non_authoritative_callee_save_storage(
    source: &ValidatedAllocatedCalleeSavedRequirements,
    environment: &ValidatedTargetRegisterEnvironment,
    policy: NonAuthoritativeCalleeSaveStoragePolicy,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedNonAuthoritativeCalleeSaveStorage, NonAuthoritativeCalleeSaveStorageError> {
    let plan = admit_plan(
        source,
        environment,
        policy,
        budget,
        compute::derive_functions,
    )?;
    validate_non_authoritative_callee_save_storage(source, environment, plan)
}

/// Admit one callee-save storage plan under the single bounded-claim policy:
/// the policy, the target's preservation catalog, the work-extent ceiling and
/// the plan row. `functions` is the one input the forward derivation and the
/// keyed reconstruction admit through different owners, so it is the caller's
/// own function roster.
pub(super) fn admit_plan(
    source: &ValidatedAllocatedCalleeSavedRequirements,
    environment: &ValidatedTargetRegisterEnvironment,
    policy: NonAuthoritativeCalleeSaveStoragePolicy,
    budget: OptimizationWorkBudget,
    functions: impl FnOnce(
        &AllocatedCalleeSavedRequirementPlan,
        &ValidatedPreservationStorageCatalog,
    ) -> Result<
        Vec<FunctionNonAuthoritativeCalleeSaveStorage>,
        NonAuthoritativeCalleeSaveStorageError,
    >,
) -> Result<NonAuthoritativeCalleeSaveStoragePlan, NonAuthoritativeCalleeSaveStorageError> {
    if policy != NonAuthoritativeCalleeSaveStoragePolicy::CanonicalTargetPreservationGroupsV1 {
        return Err(NonAuthoritativeCalleeSaveStorageError::UnsupportedPolicy);
    }
    let source_plan = source.plan();
    if source_plan.register_environment != environment.identity()
        || source_plan.physical_register_model != environment.physical().identity()
        || source_plan.target != environment.target()
    {
        return Err(NonAuthoritativeCalleeSaveStorageError::RootMismatch);
    }
    let preservation = selected_abi_preservation(environment)
        .map_err(|_| NonAuthoritativeCalleeSaveStorageError::UnsupportedTargetCatalog)?;
    let catalog = selected_preservation_storage_catalog(environment)
        .map_err(|_| NonAuthoritativeCalleeSaveStorageError::UnsupportedTargetCatalog)?;
    if source_plan.abi != preservation.kind
        || source_plan.callee_saved_units != preservation.convention.callee_saved
    {
        return Err(NonAuthoritativeCalleeSaveStorageError::RootMismatch);
    }
    let functions = functions(source_plan, &catalog)?;
    let usage = work::usage(source_plan, &catalog, &functions)?;
    // The derivation reads each requirement once: its ceilings grow with the
    // program's requirements rather than capping the program's size.
    let ceiling = budget.for_extent(work::extent(source_plan)?);
    if !usage.within(ceiling) {
        return Err(NonAuthoritativeCalleeSaveStorageError::BudgetExceeded {
            required: usage,
            budget: ceiling,
        });
    }
    Ok(NonAuthoritativeCalleeSaveStoragePlan {
        callee_saved_requirements: source.receipt().identity(),
        register_environment: environment.identity(),
        physical_register_model: environment.physical().identity(),
        preservation_storage_catalog: catalog.identity(),
        target: environment.target(),
        abi: source_plan.abi,
        callee_saved_units: source_plan.callee_saved_units.clone(),
        policy,
        budget,
        usage,
        functions,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NonAuthoritativeCalleeSaveStorageReceipt {
    pub(in crate::frame_layout::save_storage) identity: NonAuthoritativeCalleeSaveStorageIdentity,
    pub(in crate::frame_layout::save_storage) callee_saved_requirements:
        AllocatedCalleeSavedRequirementIdentity,
    pub(in crate::frame_layout::save_storage) register_environment:
        TargetRegisterEnvironmentIdentity,
    pub(in crate::frame_layout::save_storage) physical_register_model:
        PhysicalRegisterModelIdentity,
    pub(in crate::frame_layout::save_storage) preservation_storage_catalog:
        PreservationStorageCatalogIdentity,
    pub(in crate::frame_layout::save_storage) target: NativeTarget,
    pub(in crate::frame_layout::save_storage) abi: FrameAbiPreservationConvention,
    pub(in crate::frame_layout::save_storage) policy: NonAuthoritativeCalleeSaveStoragePolicy,
    pub(in crate::frame_layout::save_storage) usage: OptimizationWorkUsage,
    pub(in crate::frame_layout::save_storage) function_count: usize,
    pub(in crate::frame_layout::save_storage) modified_function_count: usize,
    pub(in crate::frame_layout::save_storage) slot_count: usize,
    pub(in crate::frame_layout::save_storage) modified_unit_count: usize,
    pub(in crate::frame_layout::save_storage) witness_count: usize,
    pub(in crate::frame_layout::save_storage) max_abstract_area_bytes: u64,
    pub(in crate::frame_layout::save_storage) max_abstract_area_alignment: u64,
}

impl NonAuthoritativeCalleeSaveStorageReceipt {
    pub const fn identity(self) -> NonAuthoritativeCalleeSaveStorageIdentity {
        self.identity
    }
    pub const fn target(self) -> NativeTarget {
        self.target
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedNonAuthoritativeCalleeSaveStorage {
    pub(in crate::frame_layout::save_storage) plan: NonAuthoritativeCalleeSaveStoragePlan,
    pub(in crate::frame_layout::save_storage) receipt: NonAuthoritativeCalleeSaveStorageReceipt,
}

impl ValidatedNonAuthoritativeCalleeSaveStorage {
    pub const fn plan(&self) -> &NonAuthoritativeCalleeSaveStoragePlan {
        &self.plan
    }
    pub const fn receipt(&self) -> NonAuthoritativeCalleeSaveStorageReceipt {
        self.receipt
    }
}
