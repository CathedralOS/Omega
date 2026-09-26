//! One abstract-to-object sequence, independent of optimization selection.

use crate::compiler::native::native_realization::callback_thunks::lower_callback_thunks;
use crate::compiler::native::native_realization::optimization_stage::lower_realization_optimization_stage;
use crate::compiler::native::native_realization::optimized_fragment_projection::{
    OptimizedFragmentPublicationRequest, emit_optimized_fragments,
};
use crate::compiler::native::native_realization::physical_stage::lower_realization_physical_stage;
use crate::compiler::native::native_realization::realization_request::{
    NativeRealizationInput, NativeRealizationRequest,
};
use crate::compiler::native::native_realization::target_stage::lower_realization_target_stage;
use abstract_operations_to_target_operations::AdmittedBoundarySettlement;
use diagnostics::Diagnostic;
use resolved_layout_to_resolved_layout::boundary_applications::TerminalBoundaryApplicationCoverage;
use resolved_layout_to_resolved_layout::native_artifact::NativePhysicalEvidenceScope;
use terminal_psi_to_abstract_operations::AdmittedProviderInstallation;

pub(crate) struct EmittedRealizationObject {
    pub(crate) object: resolved_layout_to_resolved_layout::image_emission::ObjectArtifact,
    pub(crate) physical_evidence_scope: NativePhysicalEvidenceScope,
    /// The validated semantic-entry object custody when the admitted entry is
    /// a source-authored semantic shell (the UEFI receiver route). Retained
    /// for the settlement's caller; absent on every ordinary fragment object.
    pub(crate) semantic_wrapper_object: Option<
        crate::compiler::native::StagedValidatedOptimizedProgramStorageSemanticWrapperObject,
    >,
}

pub(crate) fn emit_realization_object(
    input: NativeRealizationInput,
    provider_installation: Option<AdmittedProviderInstallation>,
    settlements: &[AdmittedBoundarySettlement<'_>],
    boundary_application_coverage: Option<&TerminalBoundaryApplicationCoverage>,
    hosted_receiver: Option<&crate::compiler::native::ValidatedNativeProgramEntrySettlement>,
    terminal: &terminal_codec::CanonicalTerminalArtifact,
    request: &NativeRealizationRequest<'_>,
) -> Result<EmittedRealizationObject, Vec<Diagnostic>> {
    let abstract_stage = lower_realization_optimization_stage(input, request)?;
    // Thunks are independent canonical artifacts: materialize them once the
    // request-level callback/selection custody check has passed, before the
    // shared instruction pipeline runs the program plan.
    let private_functions = lower_callback_thunks(request)?;
    let target_stage = lower_realization_target_stage(
        abstract_stage,
        provider_installation,
        settlements,
        request,
    )?;
    let physical = lower_realization_physical_stage(target_stage, request)?;
    let (object, physical_evidence_scope, semantic_wrapper_object) = emit_optimized_fragments(
        physical,
        OptimizedFragmentPublicationRequest {
            terminal: Some(terminal),
            boundary_application_coverage,
            hosted_receiver,
            private_functions: &private_functions,
        },
    )?;
    Ok(EmittedRealizationObject {
        object,
        physical_evidence_scope,
        semantic_wrapper_object,
    })
}
