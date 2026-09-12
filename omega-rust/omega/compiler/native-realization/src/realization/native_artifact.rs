//! Validated Terminal artifact to native-artifact realization lifecycle.

use diagnostics::Diagnostic;

use super::{
    NativeRealizationRequest, RequestedNativeArtifact, RequestedNativeArtifactError,
    boundary_applications::retain_boundary_application_coverage,
    diagnostics::realization_error,
    input::lower_realization_input,
    object::emit_realization_object,
    output::assemble_requested_native_artifact,
    providers::{AdmittedNativeProviders, admit_native_providers},
};

/// Realize one Terminal artifact using explicit image, custody, and reuse inputs.
/// Failure returns the exact image request; no product is silently substituted.
pub fn realize_native_artifact(
    artifact: terminal_codec::CanonicalTerminalArtifact,
    request: NativeRealizationRequest<'_>,
) -> Result<RequestedNativeArtifact, RequestedNativeArtifactError> {
    realize_image(artifact, &request).map_err(|diagnostics| RequestedNativeArtifactError {
        image_request: request.image_request,
        diagnostics,
    })
}

fn realize_image(
    artifact: terminal_codec::CanonicalTerminalArtifact,
    request: &NativeRealizationRequest<'_>,
) -> Result<RequestedNativeArtifact, Vec<Diagnostic>> {
    if let Some(scope) = request.checked_scope {
        scope
            .validate_for_artifact(&artifact)
            .map_err(|error| realization_error("checked boundary-operator scope", error))?;
    }
    request
        .program_entry
        .validate_for_target(request.target)
        .map_err(|error| realization_error("ProgramEntry custody", error))?;
    artifact
        .validate()
        .map_err(|error| realization_error("canonical artifact replay", error))?;
    crate::entry_settlement::validate_fused_program_entry_establishments(
        &artifact,
        request.program_entry,
        request.selected_provider_plans,
    )
    .map_err(|error| realization_error("Fused ProgramEntry establishment", error))?;
    let boundary_application_coverage = retain_boundary_application_coverage(
        &artifact,
        request.checked_scope,
        request.boundary_application_coverage,
    )?;
    let semantic_bytes = artifact.semantic_bytes();
    let proof_bytes = artifact.proof_bytes();
    let terminal_artifact_identity = *artifact.manifest().identity().as_bytes();
    let input = match request.prepared_input {
        Some(prepared) => prepared.reopen(&artifact, request)?,
        None => lower_realization_input(semantic_bytes, proof_bytes, request.profile)?,
    };
    validate_executable_entry_receiver(input.plan())?;
    let AdmittedNativeProviders {
        settlements,
        executions,
        terminal_authority_policy_identity,
        terminal_authority_permission_policy_identity,
        terminal_authority_closure_review,
        installation,
    } = admit_native_providers(
        &input,
        semantic_bytes,
        proof_bytes,
        terminal_artifact_identity,
        request,
    )?;
    let emitted = emit_realization_object(
        input,
        installation,
        &settlements,
        boundary_application_coverage.as_ref(),
        request,
    )?;
    assemble_requested_native_artifact(
        artifact,
        emitted.object,
        executions,
        terminal_authority_policy_identity,
        terminal_authority_permission_policy_identity,
        terminal_authority_closure_review,
        boundary_application_coverage,
        emitted.physical_evidence_scope,
        request.image_request.clone(),
        request,
    )
}

fn validate_executable_entry_receiver(
    plan: &abstract_operations::AbstractOperationPlan,
) -> Result<(), Vec<Diagnostic>> {
    // Settlement retains the entry declaration, not an installed receiver.
    // Every route through realize_image emits an executable image; callable
    // lowering and explicit semantic wrappers retain their own boundaries.
    if plan.functions.iter().any(|function| {
        function.machine == plan.entry
            && function
                .structural_parameters
                .iter()
                .any(|parameter| parameter.is_self)
    }) {
        return Err(realization_error(
            "ProgramEntry receiver provisioning",
            "the executable entry retains a self parameter, but no root-backed bridge constructs and lends its receiver; source-entry settlement alone does not provision receiver storage",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
