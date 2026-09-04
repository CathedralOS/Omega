use omega_optimization_run_to_abstract_operations::ValidatedOptimizedAbstractPlan;
use omega_psi_to_abstract_operations::AdmittedProviderInstallation;
use omega_target::NativeTarget;
use omega_target_operations::TargetOperationPlan;

use crate::{
    AbstractToTargetTranslationValidationReceipt, AdmittedBoundarySettlement,
    AdmittedIeeeFloatFmaSettlement, LoweringError, lower_to_target_operations,
    lower_to_target_operations_with_provider_executions,
    lower_to_target_operations_with_provider_executions_and_installation,
    lower_to_target_operations_with_provider_executions_installation_and_ieee_float_fma,
    validate_abstract_to_target_translation,
    validate_abstract_to_target_translation_with_ieee_float_fma_settlements,
};

/// Target lowering paired with the complete optimized abstract custody that
/// authorized it. This realization carrier joins two existing pipeline
/// stages; no consuming accessor can detach either side of that join.
#[derive(Debug)]
pub struct ValidatedOptimizedTargetOperations {
    pub(super) optimized: ValidatedOptimizedAbstractPlan,
    pub(super) target_operations: TargetOperationPlan,
    pub(super) translation_validation: AbstractToTargetTranslationValidationReceipt,
    pub(super) provider_installation: Option<Box<AdmittedProviderInstallation>>,
}

impl ValidatedOptimizedTargetOperations {
    pub const fn optimized(&self) -> &ValidatedOptimizedAbstractPlan {
        &self.optimized
    }

    pub const fn target(&self) -> NativeTarget {
        self.target_operations.target
    }

    pub const fn target_operations(&self) -> &TargetOperationPlan {
        &self.target_operations
    }

    pub const fn translation_validation(&self) -> &AbstractToTargetTranslationValidationReceipt {
        &self.translation_validation
    }

    /// Borrow the exact provider installation retained beside any target
    /// operations it authorized. It cannot detach from the joined custody.
    pub fn provider_installation(&self) -> Option<&AdmittedProviderInstallation> {
        self.provider_installation.as_deref()
    }
}

/// Lower one validated optimized abstract plan while retaining the complete
/// upstream custody beside the target-operation result.
pub fn lower_optimized_to_target_operations(
    optimized: ValidatedOptimizedAbstractPlan,
    target: NativeTarget,
) -> Result<ValidatedOptimizedTargetOperations, LoweringError> {
    let target_operations = lower_to_target_operations(optimized.plan(), target)?;
    let translation_validation =
        validate_abstract_to_target_translation(optimized.plan(), target, &target_operations)?;
    Ok(ValidatedOptimizedTargetOperations {
        optimized,
        target_operations,
        translation_validation,
        provider_installation: None,
    })
}

pub fn lower_optimized_to_target_operations_with_ieee_float_fma_settlements(
    optimized: ValidatedOptimizedAbstractPlan,
    target: NativeTarget,
    settlements: &[AdmittedIeeeFloatFmaSettlement<'_>],
) -> Result<ValidatedOptimizedTargetOperations, LoweringError> {
    let target_operations =
        lower_to_target_operations_with_provider_executions_installation_and_ieee_float_fma(
            optimized.plan(),
            target,
            &[],
            None,
            settlements,
        )?;
    let translation_validation =
        validate_abstract_to_target_translation_with_ieee_float_fma_settlements(
            optimized.plan(),
            target,
            &target_operations,
            settlements,
        )?;
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
        lower_to_target_operations_with_provider_executions(optimized.plan(), target, settlements)?;
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
    let target_operations = lower_to_target_operations_with_provider_executions_and_installation(
        optimized.plan(),
        target,
        settlements,
        Some(&installation),
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
