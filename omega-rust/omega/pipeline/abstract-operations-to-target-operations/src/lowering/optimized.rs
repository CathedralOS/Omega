use abstract_operations_to_abstract_operations::ValidatedOptimizedAbstractPlan;
use std::sync::Arc;
use target::NativeTarget;
use target_operations::TargetOperationPlan;
use terminal_psi_to_abstract_operations::AdmittedProviderInstallation;

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
    current_program: Arc<TargetOperationPlan>,
    pub(super) translation_validation: AbstractToTargetTranslationValidationReceipt,
    pub(super) provider_installation: Option<Box<AdmittedProviderInstallation>>,
}

impl ValidatedOptimizedTargetOperations {
    pub const fn optimized(&self) -> &ValidatedOptimizedAbstractPlan {
        &self.optimized
    }

    pub fn target(&self) -> NativeTarget {
        self.current_program.target
    }

    pub fn target_operations(&self) -> &TargetOperationPlan {
        &self.current_program
    }

    /// The original current program, not a snapshot recovered from replay inputs.
    /// This owner exposes raw data only; it cannot reconstruct this admission.
    pub fn shared_program(&self) -> Arc<TargetOperationPlan> {
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
fn lower_validated_abstract_to_target_operations(
    optimized: ValidatedOptimizedAbstractPlan,
    target: NativeTarget,
    settlements: &[AdmittedBoundarySettlement<'_>],
    installation: Option<AdmittedProviderInstallation>,
    ieee_float_fma: &[AdmittedIeeeFloatFmaSettlement<'_>],
    native_callbacks: &[crate::AdmittedNativeCallbackArgument],
) -> Result<ValidatedOptimizedTargetOperations, LoweringError> {
    let installed = installation
        .as_ref()
        .map(|value| value as &dyn installation_evidence::ProviderInstallationEvidence);
    let program = crate::lower_to_target_operations_and_native_callbacks(
        optimized.plan(),
        crate::TargetLoweringRequest {
            target,
            settlements,
            installation: installed,
            ieee_float_fma,
        },
        native_callbacks,
    )?;
    // Semantic replay can establish that a candidate implements a boundary, but
    // cannot establish which candidate the caller selected. Join against the
    // independently supplied admitted installation before sealing this owner.
    crate::validation::installed_calls::validate(&program, installed)?;
    let translation_validation =
        validate_abstract_to_target_translation_with_ieee_float_fma_settlements(
            optimized.plan(),
            target,
            &program,
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
/// The admitted settlements an optimized target lowering consumes beyond the
/// validated plan and its target.
pub struct OptimizedTargetLoweringRequest<'a> {
    pub target: NativeTarget,
    pub settlements: &'a [AdmittedBoundarySettlement<'a>],
    /// The exact installation retained beside the target operations it
    /// authorized.
    pub installation: Option<AdmittedProviderInstallation>,
    pub ieee_float_fma: &'a [AdmittedIeeeFloatFmaSettlement<'a>],
    pub native_callbacks: &'a [crate::AdmittedNativeCallbackArgument],
}

impl OptimizedTargetLoweringRequest<'_> {
    /// A lowering with no admitted settlements.
    pub fn new(target: NativeTarget) -> Self {
        Self {
            target,
            settlements: &[],
            installation: None,
            ieee_float_fma: &[],
            native_callbacks: &[],
        }
    }
}

/// One target lowering entrance for identity and selected abstract programs.
/// Native admissions are explicit request data, not a reason to select a
/// different target producer.
pub fn lower_optimized_to_target_operations(
    optimized: ValidatedOptimizedAbstractPlan,
    request: OptimizedTargetLoweringRequest<'_>,
) -> Result<ValidatedOptimizedTargetOperations, LoweringError> {
    let OptimizedTargetLoweringRequest {
        target,
        settlements,
        installation,
        ieee_float_fma,
        native_callbacks,
    } = request;
    lower_validated_abstract_to_target_operations(
        optimized,
        target,
        settlements,
        installation,
        ieee_float_fma,
        native_callbacks,
    )
}
