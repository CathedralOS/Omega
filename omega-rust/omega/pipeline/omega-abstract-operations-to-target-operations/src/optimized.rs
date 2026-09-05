use omega_optimization_run_to_abstract_operations::ValidatedOptimizedAbstractPlan;
use omega_psi_to_abstract_operations::AdmittedProviderInstallation;
use omega_target::NativeTarget;
use omega_target_operations::{TargetOperationPlan, TargetOperationPlanWithNativeCallbacks};
use std::sync::Arc;

use crate::{
    AbstractToTargetTranslationValidationReceipt, AdmittedBoundarySettlement,
    AdmittedIeeeFloatFmaSettlement, LoweringError, lower_to_target_operations,
    lower_to_target_operations_with_provider_executions,
    lower_to_target_operations_with_provider_executions_and_installation,
    lower_to_target_operations_with_provider_executions_installation_and_ieee_float_fma,
    validate_abstract_to_target_translation,
    validate_abstract_to_target_translation_with_ieee_float_fma_settlements,
};

/// Target lowering with independently retained abstract and translation evidence.
/// The current program is shared immutable representation data. Borrowing or
/// retaining it does not grant the admission held by this private constructor.
#[derive(Debug)]
pub struct ValidatedOptimizedTargetOperations {
    pub(super) optimized: ValidatedOptimizedAbstractPlan,
    current_program: Arc<TargetOperationPlanWithNativeCallbacks>,
    pub(super) translation_validation: AbstractToTargetTranslationValidationReceipt,
    pub(super) provider_installation: Option<Box<AdmittedProviderInstallation>>,
}

impl ValidatedOptimizedTargetOperations {
    pub const fn optimized(&self) -> &ValidatedOptimizedAbstractPlan {
        &self.optimized
    }

    pub fn target(&self) -> NativeTarget {
        self.current_program.plan.target
    }

    pub fn target_operations(&self) -> &TargetOperationPlan {
        &self.current_program.plan
    }

    /// The original current program, not a snapshot recovered from replay inputs.
    /// This owner exposes raw data only; it cannot reconstruct this admission.
    pub fn shared_program(&self) -> Arc<TargetOperationPlanWithNativeCallbacks> {
        Arc::clone(&self.current_program)
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

fn current_program(plan: TargetOperationPlan) -> Arc<TargetOperationPlanWithNativeCallbacks> {
    Arc::new(TargetOperationPlanWithNativeCallbacks {
        plan,
        native_callback_arguments: Vec::new(),
    })
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
        current_program: current_program(target_operations),
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
        current_program: current_program(target_operations),
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
        current_program: current_program(target_operations),
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
        current_program: current_program(target_operations),
        translation_validation,
        provider_installation: Some(Box::new(installation)),
    })
}
