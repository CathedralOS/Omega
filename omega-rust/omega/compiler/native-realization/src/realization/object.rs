//! One abstract-to-object sequence, independent of optimization selection.

use crate::realization::diagnostics::realization_error;
use crate::realization::model::{NativeRealizationCoreRequest, NativeRealizationInput};
use crate::realization::optimization_stage::lower_realization_optimization_stage;
use crate::realization::optimized_fragment_projection::{
    OptimizedFragmentPublicationRequest, emit_optimized_fragments,
};
use crate::realization::physical_stage::lower_realization_physical_stage;
use crate::realization::target_stage::lower_realization_target_stage;
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
    request: &NativeRealizationCoreRequest<'_>,
) -> Result<EmittedRealizationObject, Vec<Diagnostic>> {
    if !request.callback_thunks.is_empty() || !request.native_callbacks.is_empty() {
        return Err(realization_error(
            "native instruction selection",
            "callback ABI transport is not implemented in the common instruction pipeline",
        ));
    }
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
        physical.physical,
        OptimizedFragmentPublicationRequest {
            boundary_application_coverage,
            optimized_plan: &physical.optimized_plan,
            terminal: physical.terminal,
            validation: physical.validation,
            final_unit: physical.final_unit,
        },
    )?;
    Ok(EmittedRealizationObject {
        object,
        physical_evidence_scope,
    })
}
