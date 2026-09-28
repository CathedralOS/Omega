//! Native re-entry for a retained Terminal product with source-evaluated imports.
//!
//! Provider admission owns the exact-plan join. Callers supply only independently
//! admitted execution and same-stack evidence; they cannot select a provider
//! plan, locator, target, compiler builtin, or checked-scope receipt.

pub use abstract_operations_to_target_operations::provider_admission::SourceEvaluatedImportSettlement;
use abstract_operations_to_target_operations::provider_admission::prepare_source_evaluated_imports;
use diagnostics::Diagnostic;

/// Explicit policy, image custody, and admitted imports for native re-entry.
///
/// Policies do not establish package admission. Package orchestration must bind
/// opaque accepted evidence to the retained production subject and derive the
/// exact accepted-package policy before constructing this request.
///
/// `terminal_authority_permission_policy` is the receiver-admission axis and
/// stays optional: `None` means this production makes no receiver-admission
/// claim — accepted package rows are never silently projected into receiver
/// permissions, and the emitted artifact cannot satisfy an explicit admission
/// replay. `Some` (including an explicit empty policy) rejoins every accepted
/// row and adjudicates every closure leaf against it.
pub struct RetainedNativeRealizationRequest<'request> {
    pub profile: &'request proof_admission::AdmissionProfile,
    pub optimization_selections: &'request optimization_core::PostTerminalOptimizationSelections,
    pub terminal_authority_policy: crate::native::TerminalAuthorityPolicy,
    pub accepted_package_terminal_authority_permission_policy:
        crate::native::TerminalAuthorityPermissionPolicy,
    pub terminal_authority_permission_policy:
        Option<crate::native::TerminalAuthorityPermissionPolicy>,
    pub image_request: crate::native::ExecutableImageEmissionRequest,
    pub imports: &'request [SourceEvaluatedImportSettlement<'request>],
}

/// Consume a retained Terminal product into the requested non-installing native
/// carrier. Rejection returns the complete image input, including its interpreter.
///
/// Package permissions must exactly match the retained proposal. A supplied
/// receiving policy may add unrelated rows, but cannot omit or alter accepted
/// rows; when no receiving policy is supplied this production makes no
/// receiver-admission claim at all. Imports are independently rejoined to
/// their exact retained provider plans.
pub fn realize_retained_native_artifact(
    retained: crate::report::RetainedTerminalArtifact,
    request: RetainedNativeRealizationRequest<'_>,
) -> Result<
    crate::native::RequestedNativeArtifact,
    (
        crate::native::ExecutableImageEmissionRequest,
        Vec<Diagnostic>,
    ),
> {
    let RetainedNativeRealizationRequest {
        profile,
        optimization_selections,
        terminal_authority_policy,
        accepted_package_terminal_authority_permission_policy,
        terminal_authority_permission_policy,
        image_request,
        imports,
    } = request;
    let recoverable_image_request = image_request.clone();
    let result = (|| {
        retained
            .validate()
            .map_err(|message| diagnostic("retained Terminal product", message))?;
        let (artifact, callback_placements, proposal) = retained.into_parts();
        let proposal = proposal.ok_or_else(|| {
            diagnostic(
                "retained Terminal product",
                "source-evaluated import realization requires one native proposal",
            )
        })?;
        proposal
            .validate_for_artifact(&artifact)
            .map_err(|message| diagnostic("Terminal native proposal", message))?;
        // The image request is bound to the retained artifact's build
        // metadata: a caller cannot substitute a different loader word or
        // signing identity than the ones the build evaluated. An omitted
        // request identifier inherits the retained authored identity, so the
        // metadata follows the artifact across invocations
        // (wiki/spec/build/macos_application.md).
        let mut image_request = image_request;
        if let crate::native::ExecutableImageEmissionRequest::Direct {
            subsystem,
            code_signature_identifier,
        } = &mut image_request
        {
            if *subsystem != proposal.subsystem() {
                return Err(diagnostic(
                    "Terminal native proposal",
                    "image request subsystem differs from the retained build subsystem",
                ));
            }
            if let (Some(retained), Some(requested)) = (
                proposal.application_identifier(),
                code_signature_identifier.as_deref(),
            ) && requested != retained.as_str()
            {
                return Err(diagnostic(
                    "Terminal native proposal",
                    "image request code signature identifier differs from the retained build identifier",
                ));
            }
            if let Some(retained) = proposal.application_identifier()
                && code_signature_identifier.is_none()
            {
                *code_signature_identifier = Some(retained.as_str().to_owned());
            }
        }
        // Signed macOS GUI image emission requires the authored identifier;
        // absence is an early realization configuration error, not a
        // source-semantic rejection.
        if proposal.native_target().object_format == target::ObjectFormat::MachO
            && matches!(
                proposal.application_intent(),
                Some(build_evaluation::HostedApplicationIntent::Gui)
            )
            && image_request.code_signature_identifier().is_none()
        {
            return Err(diagnostic(
                "Terminal native proposal",
                "signed macOS GUI image emission requires the retained build identifier",
            ));
        }
        // Proposal validation rejoins each IEEE comparison to its exact selected
        // intrinsic and checked arm/expression occurrence. The ordinary graph
        // lowering preserves that operation identity into the D29 physical child;
        // bitwise IEEE ordering needs no independent foreign-provider settlement.
        if proposal.post_terminal_optimizations().selections() != optimization_selections {
            return Err(diagnostic(
                "Terminal native proposal",
                "receiving lowerer selections differ from the exact post-Terminal build proposal",
            ));
        }
        crate::native::validate_retained_package_terminal_authority_permissions(
            proposal.package_terminal_authority_permissions(),
            &accepted_package_terminal_authority_permission_policy,
        )?;
        // The receiver-admission join runs only under an explicitly supplied
        // receiving policy. `None` leaves the accepted package rows as package
        // evidence only; they are never silently projected into receiver
        // permissions.
        if let Some(receiving_policy) = &terminal_authority_permission_policy {
            crate::native::validate_package_terminal_authority_permissions(
                accepted_package_terminal_authority_permission_policy
                    .rows()
                    .iter(),
                receiving_policy,
            )?;
        }
        let native_callbacks =
            abstract_operations_to_target_operations::provider_admission::admit_native_callback_arguments(
                &callback_placements,
                proposal.callback_occurrences(),
            )?;
        let callback_thunks = backend_plan::admitted_native_callback_thunks(
            &callback_placements,
            proposal.callback_occurrences(),
        )
        .map_err(|error| diagnostic("Terminal callback thunk custody", error))?;
        debug_assert_eq!(callback_placements.len(), native_callbacks.len());
        debug_assert_eq!(callback_placements.len(), callback_thunks.len());

        let module = terminal_codec::decode_module(artifact.semantic_bytes()).map_err(|error| {
            diagnostic(
                "Terminal native proposal",
                format!("canonical semantics could not be decoded: {error}"),
            )
        })?;
        let demanded =
            provider_planning::compiler_intrinsics::demanded_boundary_identities(&module)?;
        let prepared_imports = prepare_source_evaluated_imports(
            proposal.selected_provider_plans().plans(),
            &demanded,
            imports,
        )?;
        let native_settlements = prepared_imports.settlements();

        let selected_plans = proposal.selected_provider_plans().plans();
        let compiler_builtins = proposal
            .compiler_builtins()
            .iter()
            .map(|builtin| {
                let provider_plan = selected_plans
                    .get(builtin.provider_plan_index())
                    .ok_or_else(|| {
                        diagnostic(
                            "Terminal native proposal",
                            format!(
                                "compiler builtin `{}` names an absent selected provider plan",
                                builtin.requirement_identity()
                            ),
                        )
                    })?;
                Ok(crate::native::NativeCompilerBuiltinSettlement {
                    requirement_identity: builtin.requirement_identity(),
                    provider_plan,
                    execution: builtin.execution(),
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        let calling_plans = proposal.program_entry().calling_plans().map(|plans| {
            (
                &plans.semantic_calling_application,
                &plans.physical_calling_application,
                &plans.storage_entry,
            )
        });
        // The retained proposal carries the checked ProgramEntry receipt its
        // own production derived; attaching it here supplies the independently
        // admitted checked initialization/cleanup authority the entry
        // settlement requires once the checked frontend is gone.
        let program_entry = crate::native::NativeProgramEntrySettlement::new(
            proposal.program_entry().source_signature(),
            calling_plans,
            proposal.program_entry().fused_service_establishments(),
        )
        .with_checked_entry(proposal.checked_program_entry());
        crate::native::realize_native_artifact(
            artifact,
            crate::native::NativeRealizationRequest {
                checked_scope: Some(proposal.checked_boundary_operator_scope()),
                prepared_input: None,
                target: proposal.native_target(),
                image_request,
                profile,
                terminal_authority_policy,
                terminal_authority_permission_policy,
                program_entry,
                optimization_selections,
                selected_provider_plans: proposal.selected_provider_plans(),
                external_binding_rows: proposal.external_binding_rows(),
                settlements: &native_settlements,
                compiler_builtins: &compiler_builtins,
                boundary_application_coverage: Some(proposal.boundary_application_coverage()),
                native_callbacks: &native_callbacks,
                callback_thunks: &callback_thunks,
                behavior_exclusions: proposal.behavior_exclusions(),
            },
        )
        .map_err(|error| error.into_parts().1)
    })();
    result.map_err(|diagnostics| (recoverable_image_request, diagnostics))
}

fn diagnostic(context: &str, message: impl std::fmt::Display) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!("{context}: {message}"))]
}
