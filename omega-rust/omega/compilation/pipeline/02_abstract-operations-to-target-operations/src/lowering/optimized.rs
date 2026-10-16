use abstract_operations_to_abstract_operations::ValidatedOptimizedAbstractPlan;
use optimization_core::OptimizationExecutionPhase;
use std::sync::Arc;
use target::NativeTarget;
use target_operations::TargetOperationPlan;
use terminal_psi_to_abstract_operations::AdmittedProviderInstallation;

use crate::{AdmittedBoundarySettlement, LoweringError};

/// Target lowering with independently retained abstract and translation evidence.
/// The current program is shared immutable representation data. Borrowing or
/// retaining it does not grant the admission held by this private constructor.
#[derive(Debug)]
pub struct ValidatedOptimizedTargetOperations {
    pub(super) optimized: ValidatedOptimizedAbstractPlan,
    current_program: Arc<TargetOperationPlan>,
    pub(super) provider_installation: Option<Box<AdmittedProviderInstallation>>,
    borrowed_storage_layouts: Vec<runtime_abi::NativeBorrowedStorageLayout>,
}

impl ValidatedOptimizedTargetOperations {
    /// The target producer's source-free geometry for borrowed boundary inputs.
    pub fn borrowed_storage_layouts(&self) -> &[runtime_abi::NativeBorrowedStorageLayout] {
        &self.borrowed_storage_layouts
    }
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
) -> Result<ValidatedOptimizedTargetOperations, LoweringError> {
    // Translation is implemented; selected target-operation rewrites are not.
    let phase = OptimizationExecutionPhase::TargetOperations;
    if !optimized.selections().for_phase(phase).is_empty() {
        return Err(LoweringError::UnconsumedOptimizationPhase(phase));
    }
    let installed = installation
        .as_ref()
        .map(|value| value as &dyn installation_evidence::ProviderInstallationEvidence);
    let program = crate::lower_to_target_operations(
        optimized.plan(),
        crate::TargetLoweringRequest {
            verified_input: Some(optimized.verified_input()),
            target,
            settlements,
            installation: installed,
        },
    )?;
    // Semantic replay can establish that a candidate implements a boundary, but
    // cannot establish which candidate the caller selected. Join against the
    // independently supplied admitted installation before sealing this owner.
    crate::validation::selected_boundary_calls::validate(
        &program,
        installed,
        optimized.plan(),
        Some(optimized.verified_input()),
    )?;
    crate::validation::whole_plan::reject_raw_ieee_float_fma(optimized.plan())?;
    Ok(ValidatedOptimizedTargetOperations {
        borrowed_storage_layouts: super::borrowed_storage::capture(optimized.plan()),
        optimized,
        current_program: Arc::new(program),
        provider_installation: installation.map(Box::new),
    })
}

// Compatibility entrances delegate to the same authority-aware transform.
/// The admitted settlements an optimized target lowering consumes beyond the
/// validated plan and its target.
/// Callback authority belongs to the original verified input, never this
/// independently constructed request. A cohort from another artifact, entry,
/// provider selection or target cannot be substituted through this entrance.
///
/// ```compile_fail
/// fn inject(request: &mut abstract_operations_to_target_operations::OptimizedTargetLoweringRequest<'_>,
///     foreign: &terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput) {
///     request.native_callbacks = foreign.admitted_native_callbacks();
/// }
/// ```
pub struct OptimizedTargetLoweringRequest<'a> {
    pub target: NativeTarget,
    pub settlements: &'a [AdmittedBoundarySettlement<'a>],
    /// The exact installation retained beside the target operations it
    /// authorized.
    pub installation: Option<AdmittedProviderInstallation>,
}

impl OptimizedTargetLoweringRequest<'_> {
    /// A lowering with no admitted settlements.
    pub fn new(target: NativeTarget) -> Self {
        Self {
            target,
            settlements: &[],
            installation: None,
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
    } = request;
    lower_validated_abstract_to_target_operations(optimized, target, settlements, installation)
}
