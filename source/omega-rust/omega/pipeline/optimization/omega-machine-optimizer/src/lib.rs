#![forbid(unsafe_code)]

//! Independently reconstructed machine-effect facts for the clean selected CFG.
//!
//! This crate owns no rewrite schedule, physical-home assignment, encoder, or
//! publication authority. Its sealed inputs are already validated selected
//! plans, and its output is a sidecar over those immutable instructions.

mod aarch64_cbnz_codec;
mod aarch64_cbnz_compute;
mod aarch64_cbnz_identity;
mod aarch64_cbnz_model;
mod aarch64_cbnz_validate;
mod aarch64_movn_codec;
mod aarch64_movn_compute;
mod aarch64_movn_identity;
mod aarch64_movn_model;
mod aarch64_movn_validate;
mod alternative_codec;
mod alternative_compute;
mod alternative_identity;
mod alternative_model;
mod alternative_validate;
mod effect_codec;
mod effect_compute;
mod effect_identity;
mod effect_model;
mod effect_validate;

pub use aarch64_cbnz_codec::Aarch64CbnzFusionDecodeError;
pub use aarch64_cbnz_identity::aarch64_cbnz_fusion_identity;
pub use aarch64_cbnz_model::*;
pub use aarch64_cbnz_validate::validate_aarch64_cbnz_fusion;
pub use aarch64_movn_codec::Aarch64MovnMaterializationDecodeError;
pub use aarch64_movn_identity::aarch64_movn_materialization_identity;
pub use aarch64_movn_model::*;
pub use aarch64_movn_validate::validate_aarch64_movn_materialization;
pub use alternative_codec::PostAllocationMachineDecodeError;
pub use alternative_identity::post_allocation_machine_identity;
pub use alternative_model::*;
pub use alternative_validate::validate_post_allocation_machine_plan;
pub use effect_codec::PreAllocationMachineEffectDecodeError;
pub use effect_identity::pre_allocation_machine_effect_identity;
pub use effect_model::*;
pub use effect_validate::validate_pre_allocation_machine_effects;

use omega_regalloc::ValidatedSelectedAnalysis;
use omega_register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile,
};
use omega_selected_instructions::ValidatedMachineEffectCatalog;

/// Compute and independently reconstruct the complete pre-allocation effect
/// sidecar. This grants no transformation, home, emission, or publication
/// authority.
#[allow(clippy::too_many_arguments)]
pub fn analyze_pre_allocation_machine_effects<S: ValidatedSelectedAnalysis>(
    selected: &S,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    catalog: &ValidatedMachineEffectCatalog,
) -> Result<ValidatedPreAllocationMachineEffects, MachineEffectError> {
    let plan = effect_compute::compute_terminal_pre_allocation_machine_effects(
        selected,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        catalog,
    )?;
    validate_pre_allocation_machine_effects(
        selected,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        catalog,
        plan,
    )
}

/// Join one validated selected CFG, its pre-allocation machine effects, and
/// its independently validated physical homes. The result chooses no
/// performance policy: target applicability must identify exactly one legal
/// alternative for every instruction.
#[allow(clippy::too_many_arguments)]
pub fn analyze_post_allocation_machine_plan<S: omega_regalloc::ValidatedSelectedAnalysis>(
    selected: &S,
    effects: &ValidatedPreAllocationMachineEffects,
    ranges: &omega_regalloc::ValidatedLiveRanges,
    legality: &omega_regalloc::ValidatedAllocationLegality,
    homes: &omega_regalloc::ValidatedRegisterHomes,
    manifest: &omega_regalloc::ValidatedPostAllocationOptimizationManifest,
    register_environment: omega_register_model::TargetRegisterEnvironmentIdentity,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
    constraints: &omega_register_model::ValidatedRegisterConstraintCatalog,
) -> Result<ValidatedPostAllocationMachinePlan, PostAllocationMachineError> {
    let plan = alternative_compute::compute_terminal_post_allocation_machine_plan(
        selected,
        effects,
        ranges,
        legality,
        homes,
        manifest,
        register_environment,
        physical,
        constraints,
    )?;
    validate_post_allocation_machine_plan(
        selected,
        effects,
        ranges,
        legality,
        homes,
        manifest,
        register_environment,
        physical,
        constraints,
        plan,
    )
}

/// Apply the exact named AArch64 `CMP Xn, #0; B.NE` to `CBNZ Xn` symbolic
/// transformation. The result carries no branch displacement or bytes.
pub fn optimize_aarch64_compare_i64_zero_branch_nonzero_to_cbnz<
    S: omega_regalloc::ValidatedSelectedAnalysis,
>(
    selected: &S,
    liveness: &omega_regalloc::ValidatedLiveness,
    source: &ValidatedPostAllocationMachinePlan,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
    budget: omega_optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedAarch64CbnzFusion, Aarch64CbnzFusionError> {
    let plan = aarch64_cbnz_compute::compute(selected, liveness, source, physical, budget)?;
    validate_aarch64_cbnz_fusion(selected, liveness, source, physical, plan)
}

/// Select the exact shortest MOVN-seeded AArch64 symbolic sequence for each
/// post-allocation i64 materialization, but only when it strictly reduces the
/// declared zero-seeded instruction count. This owns no encoded bytes.
pub fn optimize_aarch64_materialize_i64_with_shortest_movn_seed<
    S: omega_regalloc::ValidatedSelectedAnalysis,
>(
    selected: &S,
    source: &ValidatedPostAllocationMachinePlan,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
    budget: omega_optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedAarch64MovnMaterialization, Aarch64MovnMaterializationError> {
    let plan = aarch64_movn_compute::compute(selected, source, physical, budget)?;
    validate_aarch64_movn_materialization(selected, source, physical, plan)
}
