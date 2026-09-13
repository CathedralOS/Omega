//! Native publication: validate Terminal custody, admit providers, emit, and replay.
//!
//! The request supplies realization authority separately from the portable program.
//! Entry and callback adapters call this same lifecycle; selected optimizations do
//! not choose a different publication route.

mod boundary_applications;
mod callback_custody;
mod diagnostics;
mod input;
mod model;
mod object;
mod optimization_stage;
mod optimized_fragment_projection;
mod output;
mod physical_stage;
mod program_entry;
pub(crate) mod providers;
mod target_stage;
mod terminal_authority_permission_policy;
mod terminal_authority_policy;
mod terminal_authority_review;

pub use callback_custody::{
    CallbackCustodyNativeRealizationError, RealizedNativeArtifactWithCallbackCustody,
    realize_native_artifact_with_callback_custody,
};
pub use input::{PreparedNativeRealizationInput, prepare_native_realization_input};
pub use model::{
    NativeBoundaryRealization, NativeCallbackThunkSettlement, NativeCompilerBuiltinSettlement,
    NativeProviderSettlement, NativeRealizationRequest, RequestedNativeArtifact,
    RequestedNativeArtifactError, SettledNativeArtifact,
};
pub use program_entry::realize_program_entry_native_artifact;
pub use terminal_authority_permission_policy::{
    MissingTerminalAuthorityPermission, TERMINAL_AUTHORITY_PERMISSION_POLICY_VERSION,
    TerminalAuthorityPermissionPolicy, TerminalAuthorityPermissionPolicyBuildError,
    TerminalAuthorityPermissionPolicyRow, current_terminal_authority_permission_policy,
    terminal_authority_permission_policy_with_rows,
};
pub use terminal_authority_policy::{
    COMPILER_INTRINSIC_TERMINAL_AUTHORITY_POLICY_VERSION, CompilerIntrinsicTerminalAuthorityPolicy,
    TERMINAL_AUTHORITY_POLICY_VERSION, TerminalAuthorityPolicy, TerminalAuthorityPolicyBuildError,
    TerminalAuthorityPolicyRow, UnclassifiedCompilerIntrinsicTerminalMechanism,
    UnclassifiedTerminalMechanism, conservative_syscall_terminal_mechanism,
    current_compiler_intrinsic_terminal_authority_policy, current_terminal_authority_policy,
    normalized_foreign_terminal_mechanism,
    normalized_foreign_terminal_mechanism_with_callback_materializations,
    terminal_authority_policy_with_rows,
};

use ::diagnostics::Diagnostic;

use self::{
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
    let provision_receiver = validate_executable_entry_receiver(
        input.plan(),
        input.context().module(),
        &artifact,
        request,
    )?;
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
        provision_receiver.then_some(request.program_entry),
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
    terminal: &terminal_psi::TerminalModule,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    request: &NativeRealizationRequest<'_>,
) -> Result<bool, Vec<Diagnostic>> {
    // Settlement retains the entry declaration, not an installed receiver.
    // Every route through realize_image emits an executable image; callable
    // lowering and explicit semantic wrappers retain their own boundaries.
    let has_self_parameter = |parameters: &[terminal_psi::StructuralParameterDeclaration]| {
        parameters.iter().any(|parameter| parameter.is_self)
    };
    let has_receiver = plan.functions.iter().any(|function| {
        function.machine == plan.entry && has_self_parameter(&function.structural_parameters)
    });
    // The verified Terminal entry is the receiver-mode frontier the abstract
    // lowering replays. An unused `&mut self` receiver erases entirely inside
    // checked production, so a provisioned source entry legitimately reaches
    // this stage without a self parameter; the plan cannot gain or lose the
    // marker that verification retained.
    let terminal_has_receiver = terminal.machines.iter().any(|machine| {
        machine.id == terminal.entry && has_self_parameter(&machine.structural_parameters)
    });
    if has_receiver != terminal_has_receiver {
        return Err(realization_error(
            "ProgramEntry receiver provisioning",
            "lowered entry does not preserve the source-selected receiver mode",
        ));
    }
    let source_provisions_receiver = matches!(
        request.program_entry.source().receiver(),
        program_entry_plan::ProgramEntrySourceReceiverSignature::ProvisionedMutable { .. }
    );
    if !has_receiver {
        // A provisioned receiver whose self place erased before realization
        // keeps its mode through the retained entry attachment: that attached
        // type is the exact record the root bridge provisions and lends.
        // Losing it leaves the bridge nothing to provision, which is mode
        // loss rather than erasure.
        let retains_receiver_type = plan
            .functions
            .iter()
            .any(|function| function.machine == plan.entry && function.attachment.is_some());
        if source_provisions_receiver && !retains_receiver_type {
            return Err(realization_error(
                "ProgramEntry receiver provisioning",
                "lowered entry does not preserve the source-selected receiver mode",
            ));
        }
        return Ok(false);
    }
    if !source_provisions_receiver {
        // A free source entry cannot acquire a receiver through lowering.
        return Err(realization_error(
            "ProgramEntry receiver provisioning",
            "lowered entry does not preserve the source-selected receiver mode",
        ));
    }
    if request.target != target::NativeTarget::macos_arm64() {
        return Err(realization_error(
            "ProgramEntry receiver provisioning",
            "the executable entry retains a self parameter, but no root-backed bridge constructs and lends its receiver; source-entry settlement alone does not provision receiver storage",
        ));
    }
    let checked_entry = request.program_entry.checked_entry.ok_or_else(|| {
        realization_error(
            "ProgramEntry receiver provisioning",
            "hosted receiver requires exact checked initialization and cleanup custody",
        )
    })?;
    let settled = crate::validate_native_program_entry_settlement(
        artifact,
        checked_entry,
        request.program_entry,
        request.target,
    )
    .map_err(|error| realization_error("ProgramEntry receiver custody", error))?;
    if settled.checked_entry().receiver_eligibility().is_none() {
        return Err(realization_error(
            "ProgramEntry receiver provisioning",
            "hosted receiver requires a checked ZII-valid value with no executable nominal cleanup",
        ));
    }
    if !request.native_callbacks.is_empty() || !request.callback_thunks.is_empty() {
        return Err(realization_error(
            "ProgramEntry receiver provisioning",
            "hosted private-stack entry does not yet admit callback occupancy",
        ));
    }
    // This only permits physical construction to begin. The object binder and
    // final image replay must still prove the disjoint backing, exact entry
    // pointer, stack switch, and normal-return continuation before publication.
    Ok(true)
}

#[cfg(test)]
mod tests;
