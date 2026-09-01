use omega_optimization_core::OptimizationSelections;
use psi_diagnostics::Diagnostic;
use std::collections::BTreeSet;

pub(super) fn realize(
    checked: &crate::pipeline::CheckedCompilation,
    admission: super::admission::NativeOptimizationAdmission<'_>,
    profile: &psi_proof_admission::AdmissionProfile,
    terminal_authority_permission_policy:
        omega_terminal_psi_to_native_artifact::TerminalAuthorityPermissionPolicy,
    optimization_selections: &OptimizationSelections,
) -> Result<omega_terminal_psi_to_native_artifact::NativeArtifact, Vec<Diagnostic>> {
    validate_resolved_package_terminal_authority_permissions(
        checked.resolved_semantic_bindings(),
        &terminal_authority_permission_policy,
    )?;
    let entry_machine = admission.program_entry.machine_name().to_owned();
    let produced =
        psi_checked_trees_to_terminal::produce_terminal_artifact_with_checked_boundary_operator_scope(
            checked,
            &entry_machine,
        ).map_err(
            |error| {
                vec![Diagnostic::error(format!(
                    "native-artifact Terminal production failed: {error}"
                ))]
            },
        )?;
    let (artifact, checked_boundary_operator_scope, selected_ieee_float_fma_occurrences) =
        produced.into_parts();
    if !selected_ieee_float_fma_occurrences.is_empty() {
        return Err(vec![Diagnostic::error(
            "optimized direct native realization does not yet consume retained IEEE-FMA occurrence custody",
        )]);
    }
    let terminal_module = psi_terminal_codec::decode_module(artifact.semantic_bytes()).map_err(
        |error| {
            vec![Diagnostic::error(format!(
                "native-artifact intrinsic settlement could not replay canonical Terminal semantics: {error}"
            ))]
        },
    )?;
    let boundary_application_coverage =
        crate::compiler::terminal_product::project_terminal_boundary_application_coverage(
            checked,
            &artifact,
            &checked_boundary_operator_scope,
        )?;
    let demanded_intrinsics =
        crate::compiler::intrinsic_settlements::demanded_boundary_identities(&terminal_module)?;
    let intrinsic_proposals =
        crate::compiler::intrinsic_settlements::derive_compiler_intrinsic_settlement_proposals(
            checked,
            &demanded_intrinsics,
        )?;
    let selected_plans = checked.selected_provider_plans().plans();
    let compiler_builtins = intrinsic_proposals
        .iter()
        .map(
            |proposal| omega_terminal_psi_to_native_artifact::NativeCompilerBuiltinSettlement {
                requirement_identity: &proposal.requirement_identity,
                provider_plan: &selected_plans[proposal.plan_index],
                execution: proposal.execution,
            },
        )
        .collect::<Vec<_>>();
    let calling_plans = admission
        .program_entry
        .calling_plans()
        .map(|plans| (&plans.semantic_boundary_entry_plan, &plans.storage_entry));
    let program_entry = omega_terminal_psi_to_native_artifact::NativeProgramEntrySettlement::new(
        admission.program_entry.source_signature(),
        calling_plans,
    );
    omega_terminal_psi_to_native_artifact::realize_native_artifact_with_checked_boundary_operator_scope(
        artifact,
        &checked_boundary_operator_scope,
        omega_terminal_psi_to_native_artifact::NativeRealizationRequest {
            target: admission.target,
            subsystem: checked.subsystem(),
            profile,
            terminal_authority_policy:
                omega_terminal_psi_to_native_artifact::current_compiler_intrinsic_terminal_authority_policy(),
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
        },
    )
}

/// Rejoin consumer-approved package permission rows to the independently
/// supplied receiving policy before native realization. The receiving policy
/// may contain rows for other artifacts, but it may neither omit nor alter a
/// row already accepted for this exact package compilation.
fn validate_resolved_package_terminal_authority_permissions<'a>(
    bindings: impl Iterator<Item = &'a omega_package_compilation::AcceptedSemanticBinding>,
    policy: &omega_terminal_psi_to_native_artifact::TerminalAuthorityPermissionPolicy,
) -> Result<(), Vec<Diagnostic>> {
    let mut seen = BTreeSet::new();
    let mut diagnostics = Vec::new();
    for binding in bindings {
        for permission in binding.terminal_authority_permissions() {
            let coordinate = (
                permission.service_schema(),
                permission.requirement_identity().to_owned(),
            );
            if !seen.insert(coordinate) {
                diagnostics.push(Diagnostic::error(format!(
                    "resolved semantic bindings repeat terminal-authority permission `{}` for one exact service schema",
                    permission.requirement_identity(),
                )));
                continue;
            }
            match policy.permission_for(
                permission.service_schema(),
                permission.requirement_identity(),
            ) {
                Ok(permitted) if &permitted == permission.permitted() => {}
                Ok(_) => diagnostics.push(Diagnostic::error(format!(
                    "receiving terminal-authority policy substitutes the accepted permission for `{}`",
                    permission.requirement_identity(),
                ))),
                Err(_) => diagnostics.push(Diagnostic::error(format!(
                    "receiving terminal-authority policy omits the accepted permission for `{}`",
                    permission.requirement_identity(),
                ))),
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use super::validate_resolved_package_terminal_authority_permissions;
    use omega_effects::{
        ServiceTerminalAuthorityPermission, TerminalAuthorityClass, TerminalAuthorityDisposition,
        provider_plan::{ProviderPlanDigest, ServiceSchemaDigest},
    };
    use omega_package_compilation::{AcceptedSemanticBinding, AcceptedSemanticBindingRole};
    use psi_core::PackageKeyIdentity;

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

    fn policy(
        permitted: TerminalAuthorityDisposition,
    ) -> omega_terminal_psi_to_native_artifact::TerminalAuthorityPermissionPolicy {
        omega_terminal_psi_to_native_artifact::terminal_authority_permission_policy_with_rows(vec![
            omega_terminal_psi_to_native_artifact::TerminalAuthorityPermissionPolicyRow::new(
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
            validate_resolved_package_terminal_authority_permissions(
                std::iter::once(&binding),
                &exact,
            )
            .is_ok()
        );

        let substituted = policy(TerminalAuthorityDisposition::from_classes([
            TerminalAuthorityClass::ProcessOutput,
        ]));
        let diagnostics = validate_resolved_package_terminal_authority_permissions(
            std::iter::once(&binding),
            &substituted,
        )
        .expect_err("changed classes must reject");
        assert!(diagnostics[0].message.contains("substitutes"));

        let missing =
            omega_terminal_psi_to_native_artifact::current_terminal_authority_permission_policy();
        let diagnostics = validate_resolved_package_terminal_authority_permissions(
            std::iter::once(&binding),
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
        let diagnostics = validate_resolved_package_terminal_authority_permissions(
            [&first, &second].into_iter(),
            &exact,
        )
        .expect_err("cross-binding duplicate must reject");
        assert!(diagnostics[0].message.contains("repeat"));
    }
}
