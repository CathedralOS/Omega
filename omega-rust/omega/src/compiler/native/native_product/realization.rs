use crate::compiler::checked::CheckedCompilation;
use crate::compiler::terminal::ProgramEntryTerminalArtifact;
use diagnostics::Diagnostic;
use optimization_core::PostTerminalOptimizationSelections;

/// Rejoin the checked compilation's package-approved terminal permissions.
///
/// The package-permission custody scan always runs. The receiver join runs
/// only when a receiving permission policy was explicitly supplied; `None`
/// means this production makes no receiver-admission claim and never
/// fabricates receiver rows from accepted package evidence.
pub(super) fn validate_terminal_authority_permissions(
    checked: &CheckedCompilation,
    terminal_authority_permission_policy: Option<
        &crate::compiler::native::TerminalAuthorityPermissionPolicy,
    >,
) -> Result<(), Vec<Diagnostic>> {
    let permissions = checked
        .resolved_semantic_bindings()
        .flat_map(|binding| binding.terminal_authority_permissions());
    match terminal_authority_permission_policy {
        Some(policy) => crate::compiler::native::validate_package_terminal_authority_permissions(
            permissions,
            policy,
        ),
        None => crate::compiler::native::validate_package_terminal_authority_permission_custody(
            permissions,
        ),
    }
}

pub(super) fn realize(
    checked: &CheckedCompilation,
    admission: &super::admission::NativeCompilationAdmission,
    profile: &proof_admission::AdmissionProfile,
    terminal_authority_policy: crate::compiler::native::TerminalAuthorityPolicy,
    terminal_authority_permission_policy: Option<
        crate::compiler::native::TerminalAuthorityPermissionPolicy,
    >,
    optimization_selections: &PostTerminalOptimizationSelections,
    prepared_terminal: ProgramEntryTerminalArtifact,
    prepared_input: &crate::compiler::native::PreparedNativeRealizationInput,
) -> Result<crate::compiler::native::NativeArtifact, Vec<Diagnostic>> {
    let (
        artifact,
        checked_program_entry,
        checked_boundary_operator_scope,
        boundary_application_coverage,
    ) = prepared_terminal.into_parts();
    let terminal_module = terminal_codec::decode_module(artifact.semantic_bytes()).map_err(
        |error| {
            vec![Diagnostic::error(format!(
                "native-artifact intrinsic settlement could not replay canonical Terminal semantics: {error}"
            ))]
        },
    )?;
    // The retained authored exclusion rows resolve into the canonical union
    // against this artifact's own module — the same trait-identity join the
    // product-admission checker applies — so a requested physical absence
    // reaches mechanism adjudication even without a receiver permission
    // policy.
    let behavior_exclusions = crate::build_evaluation::authored_behavior_exclusion_set_in(
        checked.behavior_exclusions(),
        &terminal_module,
        &|symbol: symbols::SymbolHandle| {
            checked
                .typed
                .traits()
                .iter()
                .find(|definition| definition.symbol == symbol && definition.is_boundary)
                .map(|definition| definition.name.as_str().to_owned())
        },
    );
    let demanded_intrinsics =
        crate::provider_planning::compiler_intrinsics::demanded_boundary_identities(
            &terminal_module,
        )?;
    let intrinsic_proposals =
        crate::provider_planning::compiler_intrinsics::derive_selected_intrinsic_settlement_proposals(
            checked.selected_provider_plans().plans(),
            checked.selected_provider_provenance(),
            &demanded_intrinsics,
        )?;
    let selected_plans = checked.selected_provider_plans().plans();
    // A `via` leaf that evaluated to an import has no installed provider
    // behind it; its settlement is minted from the selected plan itself.
    let minted_imports =
        crate::compiler::native::native_realization::source_evaluated_imports::mint_source_evaluated_imports(
            selected_plans,
            &demanded_intrinsics,
        )?;
    let import_settlements = minted_imports
        .iter()
        .map(|minted| crate::compiler::native::NativeProviderSettlement {
            provider_execution: &minted.execution,
            provider_plan: minted.plan,
            realization: crate::compiler::native::NativeBoundaryRealization::NormalizedForeignCall(
                &minted.same_stack,
            ),
        })
        .collect::<Vec<_>>();
    let compiler_builtins = intrinsic_proposals
        .iter()
        .map(
            |proposal| crate::compiler::native::NativeCompilerBuiltinSettlement {
                requirement_identity: &proposal.requirement_identity,
                provider_plan: &selected_plans[proposal.plan_index],
                execution: proposal.execution,
            },
        )
        .collect::<Vec<_>>();
    let calling_plans = admission.program_entry.calling_plans().map(|plans| {
        (
            &plans.semantic_calling_application,
            &plans.physical_calling_application,
            &plans.storage_entry,
        )
    });
    let program_entry = crate::compiler::native::NativeProgramEntrySettlement::new(
        admission.program_entry.source_signature(),
        calling_plans,
        admission.program_entry.fused_service_establishments(),
    )
    .with_checked_entry(&checked_program_entry);
    let _validated_program_entry =
        crate::compiler::native::validate_native_program_entry_settlement(
            &artifact,
            &checked_program_entry,
            program_entry,
            admission.target,
        )
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "native-artifact checked ProgramEntry settlement failed: {error}"
            ))]
        })?;
    // The authored Build.identifier supplies the Mach-O CodeDirectory signing
    // identity (wiki/spec/build/macos_application.md). Signed macOS GUI image
    // emission requires it: absence is an early realization configuration
    // error, not a source-semantic rejection. Console Mach-O and non-Mach-O
    // output keep the executable-leaf ad-hoc label fallback.
    let code_signature_identifier = checked
        .application_identifier()
        .map(|identifier| identifier.as_str().to_owned());
    if admission.target.object_format == target::ObjectFormat::MachO
        && matches!(
            checked.application_intent(),
            Some(crate::build_evaluation::HostedApplicationIntent::Gui)
        )
        && code_signature_identifier.is_none()
    {
        return Err(vec![Diagnostic::error(
            "signed macOS GUI image emission requires the authored Build identifier",
        )]);
    }
    let request = crate::compiler::native::NativeRealizationRequest {
        checked_scope: Some(&checked_boundary_operator_scope),
        prepared_input: Some(prepared_input),
        target: admission.target,
        image_request: crate::compiler::native::ExecutableImageEmissionRequest::direct(
            checked.subsystem(),
        )
        .with_code_signature_identifier(code_signature_identifier),
        profile,
        terminal_authority_policy,
        terminal_authority_permission_policy,
        program_entry,
        optimization_selections,
        selected_provider_plans: checked.selected_provider_plans(),
        external_binding_rows: checked.external_binding_rows(),
        settlements: &import_settlements,
        compiler_builtins: &compiler_builtins,
        boundary_application_coverage: Some(&boundary_application_coverage),
        ieee_float_fma: &[],
        native_callbacks: &[],
        callback_thunks: &[],
        behavior_exclusions: &behavior_exclusions,
    };
    crate::compiler::native::realize_native_artifact(artifact, request)
        .map_err(|error| error.into_parts().1)?
        .into_direct()
        .map_err(|_| {
            vec![Diagnostic::error(
                "direct native realization returned a different image kind",
            )]
        })
}
