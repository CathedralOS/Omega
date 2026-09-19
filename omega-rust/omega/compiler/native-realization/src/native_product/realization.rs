use assembled_syntax_to_checked_compilation::CheckedCompilation;
use checked_compilation_to_terminal_artifact::ProgramEntryTerminalArtifact;
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
    terminal_authority_permission_policy: Option<&crate::TerminalAuthorityPermissionPolicy>,
) -> Result<(), Vec<Diagnostic>> {
    let permissions = checked
        .resolved_semantic_bindings()
        .flat_map(|binding| binding.terminal_authority_permissions());
    match terminal_authority_permission_policy {
        Some(policy) => crate::validate_package_terminal_authority_permissions(permissions, policy),
        None => crate::validate_package_terminal_authority_permission_custody(permissions),
    }
}

pub(super) fn realize(
    checked: &CheckedCompilation,
    admission: &super::admission::NativeCompilationAdmission,
    profile: &proof_admission::AdmissionProfile,
    terminal_authority_policy: crate::TerminalAuthorityPolicy,
    terminal_authority_permission_policy: Option<crate::TerminalAuthorityPermissionPolicy>,
    optimization_selections: &PostTerminalOptimizationSelections,
    prepared_terminal: ProgramEntryTerminalArtifact,
    prepared_input: &crate::PreparedNativeRealizationInput,
) -> Result<crate::NativeArtifact, Vec<Diagnostic>> {
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
    let demanded_intrinsics =
        provider_planning::compiler_intrinsics::demanded_boundary_identities(&terminal_module)?;
    let intrinsic_proposals =
        provider_planning::compiler_intrinsics::derive_selected_intrinsic_settlement_proposals(
            checked.selected_provider_plans().plans(),
            checked.selected_provider_provenance(),
            &demanded_intrinsics,
        )?;
    let selected_plans = checked.selected_provider_plans().plans();
    let compiler_builtins = intrinsic_proposals
        .iter()
        .map(|proposal| crate::NativeCompilerBuiltinSettlement {
            requirement_identity: &proposal.requirement_identity,
            provider_plan: &selected_plans[proposal.plan_index],
            execution: proposal.execution,
        })
        .collect::<Vec<_>>();
    let calling_plans = admission.program_entry.calling_plans().map(|plans| {
        (
            &plans.semantic_calling_application,
            &plans.physical_calling_application,
            &plans.storage_entry,
        )
    });
    let program_entry = crate::NativeProgramEntrySettlement::new(
        admission.program_entry.source_signature(),
        calling_plans,
        admission.program_entry.fused_service_establishments(),
    )
    .with_checked_entry(&checked_program_entry);
    let _validated_program_entry = crate::validate_native_program_entry_settlement(
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
            Some(build_evaluation::HostedApplicationIntent::Gui)
        )
        && code_signature_identifier.is_none()
    {
        return Err(vec![Diagnostic::error(
            "signed macOS GUI image emission requires the authored Build identifier",
        )]);
    }
    let request = crate::NativeRealizationRequest {
        checked_scope: Some(&checked_boundary_operator_scope),
        prepared_input: Some(prepared_input),
        target: admission.target,
        image_request: crate::ExecutableImageEmissionRequest::direct(checked.subsystem())
            .with_code_signature_identifier(code_signature_identifier),
        profile,
        terminal_authority_policy,
        terminal_authority_permission_policy,
        program_entry,
        optimization_selections,
        selected_provider_plans: checked.selected_provider_plans(),
        external_binding_rows: checked.external_binding_rows(),
        settlements: &[],
        compiler_builtins: &compiler_builtins,
        boundary_application_coverage: Some(&boundary_application_coverage),
        ieee_float_fma: &[],
        native_callbacks: &[],
        callback_thunks: &[],
    };
    crate::realize_native_artifact(artifact, request)
        .map_err(|error| error.into_parts().1)?
        .into_direct()
        .map_err(|_| {
            vec![Diagnostic::error(
                "direct native realization returned a different image kind",
            )]
        })
}

#[cfg(test)]
mod tests {
    use crate::validate_package_terminal_authority_permissions;
    use effects::{
        ServiceTerminalAuthorityPermission, TerminalAuthorityClass, TerminalAuthorityDisposition,
        provider_plan::{ProviderPlanDigest, ServiceSchemaDigest},
    };
    use package_compilation::{AcceptedSemanticBinding, AcceptedSemanticBindingRole};
    use semantic_vocabulary::PackageKeyIdentity;

    fn accepted_binding() -> AcceptedSemanticBinding {
        let schema = ServiceSchemaDigest::from_digest([41; 32]);
        AcceptedSemanticBinding::new(
            AcceptedSemanticBindingRole::ConsoleExitProcessI32,
            PackageKeyIdentity::from_digest([42; 32]).expect("nonzero package identity"),
            "Console",
            schema,
            ProviderPlanDigest::from_digest([43; 32]),
        )
        .expect("accepted Console binding")
        .with_terminal_authority_permissions(vec![ServiceTerminalAuthorityPermission::new(
            schema,
            "Console::exit_process#exact",
            TerminalAuthorityDisposition::from_classes([
                TerminalAuthorityClass::ProcessTermination,
            ]),
        )])
        .expect("exact permission")
    }

    fn policy(permitted: TerminalAuthorityDisposition) -> crate::TerminalAuthorityPermissionPolicy {
        crate::terminal_authority_permission_policy_with_rows(vec![
            crate::TerminalAuthorityPermissionPolicyRow::new(
                ServiceSchemaDigest::from_digest([41; 32]),
                "Console::exit_process#exact",
                permitted,
            ),
        ])
        .expect("valid receiving policy")
    }

    #[test]
    fn package_permission_must_match_receiving_policy_exactly() {
        let binding = accepted_binding();
        let exact = policy(TerminalAuthorityDisposition::from_classes([
            TerminalAuthorityClass::ProcessTermination,
        ]));
        assert!(
            validate_package_terminal_authority_permissions(
                binding.terminal_authority_permissions().iter(),
                &exact,
            )
            .is_ok()
        );

        let substituted = policy(TerminalAuthorityDisposition::from_classes([
            TerminalAuthorityClass::ProcessOutput,
        ]));
        let diagnostics = validate_package_terminal_authority_permissions(
            binding.terminal_authority_permissions().iter(),
            &substituted,
        )
        .expect_err("changed classes must reject");
        assert!(diagnostics[0].message.contains("substitutes"));

        let missing = crate::current_terminal_authority_permission_policy();
        let diagnostics = validate_package_terminal_authority_permissions(
            binding.terminal_authority_permissions().iter(),
            &missing,
        )
        .expect_err("missing exact row must reject");
        assert!(diagnostics[0].message.contains("omits"));
    }

    #[test]
    fn duplicate_permissions_across_resolved_bindings_reject() {
        let first = accepted_binding();
        let second = first.clone();
        let exact = policy(TerminalAuthorityDisposition::from_classes([
            TerminalAuthorityClass::ProcessTermination,
        ]));
        let diagnostics = validate_package_terminal_authority_permissions(
            [&first, &second]
                .into_iter()
                .flat_map(|binding| binding.terminal_authority_permissions()),
            &exact,
        )
        .expect_err("cross-binding duplicate must reject");
        assert!(diagnostics[0].message.contains("repeat"));
    }

    #[test]
    fn custody_scan_without_receiving_policy_keeps_package_rows_unclaimed() {
        let binding = accepted_binding();
        assert!(
            crate::validate_package_terminal_authority_permission_custody(
                binding.terminal_authority_permissions().iter(),
            )
            .is_ok(),
            "ordinary production with no receiving policy retains package custody",
        );

        // Custody without a receiving policy is not deny-all: the accepted
        // package row stands on its own and is not compared against any
        // receiver rows.
        let second = binding.clone();
        let diagnostics = crate::validate_package_terminal_authority_permission_custody(
            [&binding, &second]
                .into_iter()
                .flat_map(|binding| binding.terminal_authority_permissions()),
        )
        .expect_err("custody-only scan still rejects repeated coordinates");
        assert!(diagnostics[0].message.contains("repeat"));
    }
}
