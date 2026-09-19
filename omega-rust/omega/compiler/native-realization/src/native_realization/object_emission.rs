//! One abstract-to-object sequence, independent of optimization selection.

use crate::native_realization::optimization_stage::lower_realization_optimization_stage;
use crate::native_realization::optimized_fragment_projection::{
    OptimizedFragmentPublicationRequest, emit_optimized_fragments,
};
use crate::native_realization::physical_stage::lower_realization_physical_stage;
use crate::native_realization::realization_diagnostics::realization_error;
use crate::native_realization::realization_request::{
    NativeRealizationInput, NativeRealizationRequest,
};
use crate::native_realization::target_stage::lower_realization_target_stage;
use abstract_operations_to_target_operations::AdmittedBoundarySettlement;
use boundary_applications::TerminalBoundaryApplicationCoverage;
use diagnostics::Diagnostic;
use native_artifact::NativePhysicalEvidenceScope;
use terminal_psi_to_abstract_operations::AdmittedProviderInstallation;

pub(crate) struct EmittedRealizationObject {
    pub(crate) object: image_emission::ObjectArtifact,
    pub(crate) physical_evidence_scope: NativePhysicalEvidenceScope,
}

pub(crate) fn emit_realization_object(
    input: NativeRealizationInput,
    provider_installation: Option<AdmittedProviderInstallation>,
    settlements: &[AdmittedBoundarySettlement<'_>],
    boundary_application_coverage: Option<&TerminalBoundaryApplicationCoverage>,
    hosted_receiver: Option<&crate::ValidatedNativeProgramEntrySettlement>,
    request: &NativeRealizationRequest<'_>,
) -> Result<EmittedRealizationObject, Vec<Diagnostic>> {
    if !request.ieee_float_fma.is_empty() {
        return Err(realization_error(
            "native instruction selection",
            "FMA provider transport is not implemented in the common instruction pipeline",
        ));
    }
    let abstract_stage = lower_realization_optimization_stage(input, request)?;
    let target_stage = lower_realization_target_stage(
        abstract_stage,
        provider_installation,
        settlements,
        request,
    )?;
    let physical = lower_realization_physical_stage(target_stage, request)?;
    let (object, physical_evidence_scope) = emit_optimized_fragments(
        physical,
        OptimizedFragmentPublicationRequest {
            boundary_application_coverage,
            hosted_receiver,
        },
    )?;
    Ok(EmittedRealizationObject {
        object,
        physical_evidence_scope,
    })
}
