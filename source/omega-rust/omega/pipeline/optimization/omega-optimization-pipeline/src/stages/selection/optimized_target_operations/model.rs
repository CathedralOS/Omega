use omega_optimization_run_to_abstract_operations::ValidatedOptimizedAbstractPlan;
use omega_psi_to_abstract_operations::AdmittedProviderInstallation;
use omega_target::NativeTarget;
use omega_target_operations::TargetOperationPlan;

/// Target lowering paired with the complete optimized abstract custody that
/// authorized it. This realization carrier joins two existing pipeline
/// stages; no consuming accessor can detach either side of that join.
#[derive(Debug)]
pub struct ValidatedOptimizedTargetOperations {
    pub(super) optimized: ValidatedOptimizedAbstractPlan,
    pub(super) target_operations: TargetOperationPlan,
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

    /// Borrow the exact provider installation retained beside any target
    /// operations it authorized. It cannot detach from the joined custody.
    pub fn provider_installation(&self) -> Option<&AdmittedProviderInstallation> {
        self.provider_installation.as_deref()
    }
}
