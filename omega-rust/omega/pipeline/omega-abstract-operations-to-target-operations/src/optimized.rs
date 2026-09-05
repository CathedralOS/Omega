use omega_optimization_run_to_abstract_operations::ValidatedOptimizedAbstractPlan;
use omega_psi_to_abstract_operations::AdmittedProviderInstallation;
use omega_target::NativeTarget;
use omega_target_operations::{TargetOperationPlan, TargetOperationPlanWithNativeCallbacks};
use std::sync::Arc;

use crate::{
    AbstractToTargetTranslationValidationReceipt, AdmittedBoundarySettlement,
    AdmittedIeeeFloatFmaSettlement, LoweringError,
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

/// One target lowering entrance for identity and selected abstract programs.
/// Native admissions are explicit inputs, not a reason to select a different
/// target producer. Translation coverage still names only reconstructed families.
pub fn lower_validated_abstract_to_target_operations(
    optimized: ValidatedOptimizedAbstractPlan,
    target: NativeTarget,
    settlements: &[AdmittedBoundarySettlement<'_>],
    installation: Option<AdmittedProviderInstallation>,
    ieee_float_fma: &[AdmittedIeeeFloatFmaSettlement<'_>],
    native_callbacks: &[crate::AdmittedNativeCallbackArgument],
) -> Result<ValidatedOptimizedTargetOperations, LoweringError> {
    let installed = installation
        .as_ref()
        .map(|value| value as &dyn omega_installation_evidence::ProviderInstallationEvidence);
    let program = crate::lower_to_target_operations_with_provider_executions_installation_ieee_float_fma_and_native_callbacks(
        optimized.plan(), target, settlements, installed, ieee_float_fma, native_callbacks,
    )?;
    let translation_validation =
        validate_abstract_to_target_translation_with_ieee_float_fma_settlements(
            optimized.plan(),
            target,
            &program.plan,
            ieee_float_fma,
        )?;
    Ok(ValidatedOptimizedTargetOperations {
        optimized,
        current_program: Arc::new(program),
        translation_validation,
        provider_installation: installation.map(Box::new),
    })
}

// Compatibility entrances delegate to the same authority-aware transform.
pub fn lower_optimized_to_target_operations(
    optimized: ValidatedOptimizedAbstractPlan,
    target: NativeTarget,
) -> Result<ValidatedOptimizedTargetOperations, LoweringError> {
    lower_validated_abstract_to_target_operations(optimized, target, &[], None, &[], &[])
}

pub fn lower_optimized_to_target_operations_with_ieee_float_fma_settlements(
    optimized: ValidatedOptimizedAbstractPlan,
    target: NativeTarget,
    settlements: &[AdmittedIeeeFloatFmaSettlement<'_>],
) -> Result<ValidatedOptimizedTargetOperations, LoweringError> {
    lower_validated_abstract_to_target_operations(optimized, target, &[], None, settlements, &[])
}

pub fn lower_optimized_to_target_operations_with_provider_executions(
    optimized: ValidatedOptimizedAbstractPlan,
    target: NativeTarget,
    settlements: &[AdmittedBoundarySettlement<'_>],
) -> Result<ValidatedOptimizedTargetOperations, LoweringError> {
    lower_validated_abstract_to_target_operations(optimized, target, settlements, None, &[], &[])
}

/// Retain the exact installation beside the target operations it authorized.
pub fn lower_optimized_to_target_operations_with_provider_executions_and_installation(
    optimized: ValidatedOptimizedAbstractPlan,
    target: NativeTarget,
    settlements: &[AdmittedBoundarySettlement<'_>],
    installation: AdmittedProviderInstallation,
) -> Result<ValidatedOptimizedTargetOperations, LoweringError> {
    lower_validated_abstract_to_target_operations(
        optimized,
        target,
        settlements,
        Some(installation),
        &[],
        &[],
    )
}
