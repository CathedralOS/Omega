//! Validated Terminal artifact to native-artifact realization lifecycle.

use omega_native_artifact::NativeArtifact;
use psi_diagnostics::Diagnostic;

use super::{
    boundary_applications::retain_boundary_application_coverage,
    diagnostics::realization_error,
    input::lower_realization_input,
    machine_code::emit_realization_machine_code,
    output::assemble_native_artifact,
    providers::{admit_native_providers, AdmittedNativeProviders},
    NativeRealizationRequest,
};

pub(super) fn realize(
    artifact: psi_terminal_codec::CanonicalTerminalArtifact,
    checked_scope: Option<&psi_checked_trees_to_terminal::CheckedBoundaryOperatorApplicationScope>,
    request: NativeRealizationRequest<'_>,
) -> Result<NativeArtifact, Vec<Diagnostic>> {
    request
        .program_entry
        .validate_for_target(request.target)
        .map_err(|error| realization_error("ProgramEntry custody", error))?;
    artifact
        .validate()
        .map_err(|error| realization_error("canonical artifact replay", error))?;
    let boundary_application_coverage = retain_boundary_application_coverage(
        &artifact,
        checked_scope,
        request.boundary_application_coverage,
    )?;
    let semantic_bytes = artifact.semantic_bytes();
    let proof_bytes = artifact.proof_bytes();
    let terminal_artifact_identity = *artifact.manifest().identity().as_bytes();
    let input = lower_realization_input(semantic_bytes, proof_bytes, &request)?;
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
        &request,
    )?;
    let physical_evidence_scope = input.physical_evidence_scope(checked_scope);
    let machine_code = emit_realization_machine_code(input, installation, &settlements, &request)?;
    assemble_native_artifact(
        artifact,
        &machine_code,
        executions,
        terminal_authority_policy_identity,
        terminal_authority_permission_policy_identity,
        terminal_authority_closure_review,
        boundary_application_coverage,
        physical_evidence_scope,
        &request,
    )
}
