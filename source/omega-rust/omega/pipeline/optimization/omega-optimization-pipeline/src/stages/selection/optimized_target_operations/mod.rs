//! Optimized abstract-plan to target-operation lowering stage.
//!
//! Exact source routes descend into `lowering`; the retained owning carrier
//! descends into `model`. This entrance owns every lowering-to-custody join,
//! including retention of an admitted provider installation when present.

mod lowering;
mod model;

pub use model::*;

use omega_abstract_operations_to_target_operations::{
    AdmittedBoundarySettlement, LoweringError, validate_abstract_to_target_translation,
};
use omega_optimization_run_to_abstract_operations::ValidatedOptimizedAbstractPlan;
use omega_psi_to_abstract_operations::AdmittedProviderInstallation;
use omega_target::NativeTarget;

pub fn lower_optimized_to_target_operations(
    optimized: ValidatedOptimizedAbstractPlan,
    target: NativeTarget,
) -> Result<ValidatedOptimizedTargetOperations, LoweringError> {
    let target_operations = lowering::lower_optimized_plan(&optimized, target)?;
    let translation_validation =
        validate_abstract_to_target_translation(optimized.plan(), target, &target_operations)?;
    Ok(ValidatedOptimizedTargetOperations {
        optimized,
        target_operations,
        translation_validation,
        provider_installation: None,
    })
}

pub fn lower_optimized_to_target_operations_with_provider_executions(
    optimized: ValidatedOptimizedAbstractPlan,
    target: NativeTarget,
    settlements: &[AdmittedBoundarySettlement<'_>],
) -> Result<ValidatedOptimizedTargetOperations, LoweringError> {
    let target_operations =
        lowering::lower_optimized_plan_with_provider_executions(&optimized, target, settlements)?;
    let translation_validation =
        validate_abstract_to_target_translation(optimized.plan(), target, &target_operations)?;
    Ok(ValidatedOptimizedTargetOperations {
        optimized,
        target_operations,
        translation_validation,
        provider_installation: None,
    })
}

/// Lower with one exact checked-provider installation while retaining that
/// opaque admission beside the target projection it authorized. Remaining
/// bodyless boundaries may still be supplied as external executions; target
/// lowering rejects overlap with installed boundaries.
pub fn lower_optimized_to_target_operations_with_provider_executions_and_installation(
    optimized: ValidatedOptimizedAbstractPlan,
    target: NativeTarget,
    settlements: &[AdmittedBoundarySettlement<'_>],
    installation: AdmittedProviderInstallation,
) -> Result<ValidatedOptimizedTargetOperations, LoweringError> {
    let target_operations = lowering::lower_optimized_plan_with_provider_installation(
        &optimized,
        target,
        settlements,
        &installation,
    )?;
    let translation_validation =
        validate_abstract_to_target_translation(optimized.plan(), target, &target_operations)?;
    Ok(ValidatedOptimizedTargetOperations {
        optimized,
        target_operations,
        translation_validation,
        provider_installation: Some(Box::new(installation)),
    })
}
