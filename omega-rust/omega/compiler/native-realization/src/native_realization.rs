//! Native publication: validate Terminal custody, admit providers, emit, and replay.
//!
//! The request supplies realization authority separately from the portable program.
//! Entry and callback adapters call this same lifecycle; selected optimizations do
//! not choose a different publication route.

mod artifact_assembly;
mod behavior_exclusions;
mod boundary_applications;
mod callback_custody;
mod input_preparation;
mod object_emission;
mod optimization_stage;
mod optimized_fragment_projection;
mod physical_stage;
mod program_entry;
pub(crate) mod providers;
mod realization_diagnostics;
mod realization_request;
mod target_stage;
mod terminal_authority_permission_policy;
pub mod terminal_authority_permissions;
mod terminal_authority_policy;
mod terminal_authority_review;

pub use callback_custody::{
    CallbackCustodyNativeRealizationError, RealizedNativeArtifactWithCallbackCustody,
    realize_native_artifact_with_callback_custody,
};
pub use input_preparation::{PreparedNativeRealizationInput, prepare_native_realization_input};
pub use program_entry::realize_program_entry_native_artifact;
pub use realization_request::{
    NativeBoundaryRealization, NativeCallbackThunkSettlement, NativeCompilerBuiltinSettlement,
    NativeProviderSettlement, NativeRealizationRequest, RequestedNativeArtifact,
    RequestedNativeArtifactError, SettledNativeArtifact,
};
pub use terminal_authority_permission_policy::{
    MissingTerminalAuthorityPermission, TERMINAL_AUTHORITY_PERMISSION_POLICY_VERSION,
    TerminalAuthorityPermissionPolicy, TerminalAuthorityPermissionPolicyBuildError,
    TerminalAuthorityPermissionPolicyRow, current_terminal_authority_permission_policy,
    terminal_authority_permission_policy_with_rows,
};
pub use terminal_authority_policy::{
    COMPILER_INTRINSIC_TERMINAL_AUTHORITY_POLICY_VERSION, CompilerIntrinsicTerminalAuthorityPolicy,
    FilesystemCohortDisposition, FilesystemOrdinaryReleaseContract,
    TERMINAL_AUTHORITY_POLICY_VERSION, TerminalAuthorityPolicy, TerminalAuthorityPolicyBuildError,
    TerminalAuthorityPolicyRow, UnclassifiedCompilerIntrinsicTerminalMechanism,
    UnclassifiedTerminalMechanism, UnsettledFilesystemRequirement,
    conservative_syscall_terminal_mechanism, current_compiler_intrinsic_terminal_authority_policy,
    current_terminal_authority_policy, filesystem_host_permission_row,
    filesystem_host_permission_rows, filesystem_mechanism_row,
    filesystem_ordinary_release_contract, filesystem_release_bound_mechanism,
    filesystem_release_mechanism_row, normalized_foreign_terminal_mechanism,
    normalized_foreign_terminal_mechanism_with_callback_materializations,
    settled_filesystem_cohort, terminal_authority_policy_with_rows,
};

use diagnostics::Diagnostic;

use self::{
    artifact_assembly::assemble_requested_native_artifact,
    boundary_applications::retain_boundary_application_coverage,
    input_preparation::lower_realization_input,
    object_emission::emit_realization_object,
    providers::{AdmittedNativeProviders, admit_native_providers},
    realization_diagnostics::realization_error,
};

/// Realize one Terminal artifact using explicit image, custody, and reuse inputs.
/// Failure returns the exact image request; no product is silently substituted.
pub fn realize_native_artifact(
    artifact: terminal_codec::CanonicalTerminalArtifact,
    request: NativeRealizationRequest<'_>,
) -> Result<RequestedNativeArtifact, RequestedNativeArtifactError> {
    realize_image(
        artifact,
        &request,
        &build_evaluation::BehaviorExclusions::default(),
    )
    .map_err(|diagnostics| RequestedNativeArtifactError {
        image_request: request.image_request,
        diagnostics,
    })
}

fn realize_image(
    artifact: terminal_codec::CanonicalTerminalArtifact,
    request: &NativeRealizationRequest<'_>,
    behavior_exclusions: &build_evaluation::BehaviorExclusions,
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
    let receiver_settlement = validate_executable_entry_receiver(
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
    // A requested physical-authority exclusion is a demand on the admitted
    // mechanism closure, not a receiver permission: it is adjudicated against
    // the review's exercised dispositions whether or not the request carries
    // a permission policy, and a violation publishes no product.
    behavior_exclusions::admit_behavior_exclusion_closure(
        behavior_exclusions,
        &terminal_authority_closure_review,
    )?;
    let emitted = emit_realization_object(
        input,
        installation,
        &settlements,
        boundary_application_coverage.as_ref(),
        receiver_settlement.as_ref(),
        request,
    )?;
    validate_emitted_receiver_binding(&emitted.object, receiver_settlement.as_ref())?;
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
) -> Result<Option<crate::ValidatedNativeProgramEntrySettlement>, Vec<Diagnostic>> {
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
    if !source_provisions_receiver {
        if has_receiver {
            // A free source entry cannot acquire a receiver through lowering.
            return Err(realization_error(
                "ProgramEntry receiver provisioning",
                "lowered entry does not preserve the source-selected receiver mode",
            ));
        }
        return Ok(None);
    }
    let lowered_entry = plan
        .functions
        .iter()
        .find(|function| function.machine == plan.entry);
    let terminal_entry = terminal
        .machines
        .iter()
        .find(|machine| machine.id == terminal.entry);
    let lowered_attachment = lowered_entry.and_then(|function| function.attachment);
    let terminal_attachment = terminal_entry.and_then(|machine| machine.attachment);
    if !has_receiver {
        // Erasure removes the borrow parameter, not the source owner's
        // initialization and cleanup obligations. The attachment still binds
        // that owner, and checked eligibility below must justify eliding it.
        if lowered_attachment.is_none() || lowered_attachment != terminal_attachment {
            return Err(realization_error(
                "ProgramEntry receiver provisioning",
                "lowered entry does not preserve the source-selected receiver mode",
            ));
        }
    } else {
        // A retained borrow is the receiver this realization physically
        // provisions: the bridge sizes and lends the storage the lowered
        // declaration spells out, and no later stage rejoins a drifted
        // receiver to the checked Terminal receiver. The owner attachment and
        // the exact self declaration must survive lowering unchanged.
        let receiver_preserved = match (lowered_entry, terminal_entry) {
            (Some(function), Some(machine)) => function
                .structural_parameters
                .iter()
                .filter(|parameter| parameter.is_self)
                .eq(machine
                    .structural_parameters
                    .iter()
                    .filter(|parameter| parameter.is_self)),
            _ => false,
        };
        if lowered_attachment != terminal_attachment || !receiver_preserved {
            return Err(realization_error(
                "ProgramEntry receiver provisioning",
                "lowered entry does not preserve the source-selected receiver identity",
            ));
        }
    }
    let supported_receiver_bridge = request.target == target::NativeTarget::macos_arm64()
        || request.target == target::NativeTarget::linux_x64()
        || request.target == target::NativeTarget::linux_arm64()
        || request.target == target::NativeTarget::windows_x64();
    if has_receiver && !supported_receiver_bridge {
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
    if !has_receiver {
        // Source ZII/no-code disposal and the exact erased projection have
        // been replayed. No physical receiver argument or storage is needed;
        // Fused establishment is still checked independently before this call.
        return Ok(None);
    }
    if !request.native_callbacks.is_empty() || !request.callback_thunks.is_empty() {
        return Err(realization_error(
            "ProgramEntry receiver provisioning",
            "hosted private-stack entry does not yet admit callback occupancy",
        ));
    }
    // This only permits physical construction to begin. The validated
    // settlement is the demand the object binder must satisfy exactly, and the
    // final image replay must still prove the disjoint backing, exact entry
    // pointer, stack switch, and normal-return continuation before publication.
    Ok(Some(settled))
}

/// Admission is demand, not construction. The emitted object must carry a
/// hosted receiver binding iff one was admitted, and that binding must be the
/// validated settlement's exact source signature and physical contract — a
/// missing binding is bypassed provisioning, and a substituted one binds a
/// different continuation, receiver, or arrival contract than the checked
/// source selected.
fn validate_emitted_receiver_binding(
    object: &image_emission::ObjectArtifact,
    admitted: Option<&crate::ValidatedNativeProgramEntrySettlement>,
) -> Result<(), Vec<Diagnostic>> {
    let binding = object.hosted_receiver_binding();
    let Some(settlement) = admitted else {
        return if binding.is_none() {
            Ok(())
        } else {
            Err(realization_error(
                "ProgramEntry receiver provisioning",
                "emitted object carries a hosted receiver binding that admission never granted",
            ))
        };
    };
    let Some(binding) = binding else {
        return Err(realization_error(
            "ProgramEntry receiver provisioning",
            "admitted receiver provisioning did not reach the emitted object",
        ));
    };
    let admitted_contract = settlement
        .storage_entry()
        .and_then(|storage| storage.physical_contract());
    if binding.source() != settlement.source()
        || admitted_contract != Some(binding.physical_contract())
    {
        return Err(realization_error(
            "ProgramEntry receiver provisioning",
            "emitted hosted receiver binding does not carry the admitted source signature and physical contract",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
