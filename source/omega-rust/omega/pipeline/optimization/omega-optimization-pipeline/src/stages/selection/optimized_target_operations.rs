use omega_abstract_operations_to_target_operations::{
    AdmittedBoundarySettlement, LoweringError, lower_to_target_operations,
    lower_to_target_operations_with_provider_executions,
    lower_to_target_operations_with_provider_executions_and_installation,
};
use omega_optimization_run_to_abstract_operations::ValidatedOptimizedAbstractPlan;
use omega_psi_to_abstract_operations::AdmittedProviderInstallation;
use omega_target::NativeTarget;
use omega_target_operations::TargetOperationPlan;

/// Target lowering paired with the complete optimized abstract custody that
/// authorized it. This realization carrier joins two existing pipeline
/// stages; no consuming accessor can detach either side of that join.
#[derive(Debug)]
pub struct ValidatedOptimizedTargetOperations {
    optimized: ValidatedOptimizedAbstractPlan,
    target_operations: TargetOperationPlan,
    provider_installation: Option<Box<AdmittedProviderInstallation>>,
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

    /// Borrow the exact provider installation retained beside any target
    /// operations it authorized. It cannot detach from the joined custody.
    pub fn provider_installation(&self) -> Option<&AdmittedProviderInstallation> {
        self.provider_installation.as_deref()
    }
}

pub fn lower_optimized_to_target_operations(
    optimized: ValidatedOptimizedAbstractPlan,
    target: NativeTarget,
) -> Result<ValidatedOptimizedTargetOperations, LoweringError> {
    let target_operations = lower_to_target_operations(optimized.plan(), target)?;
    Ok(ValidatedOptimizedTargetOperations {
        optimized,
        target_operations,
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
    Ok(ValidatedOptimizedTargetOperations {
        optimized,
        target_operations,
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
    Ok(ValidatedOptimizedTargetOperations {
        optimized,
        target_operations,
        provider_installation: Some(Box::new(installation)),
    })
}
