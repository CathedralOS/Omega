use diagnostics::Diagnostic;

/// This program supplies no independent components or imports. Its receiver still
/// compares the complete consent inventory, so a stale component decision also fails.
pub(super) fn compare(
    accepted: &review_evidence::permission_acceptance::TerminalPermissionAcceptance,
    package: Option<semantic_vocabulary::PackageKeyIdentity>,
    program: &native_compiler::NativeProgram,
    bindings: &[provider_acceptance::AcceptedSemanticBinding],
    declarations: &source_compiler::RuntimePermissionDeclarations,
) -> Result<(), Vec<Diagnostic>> {
    let package = package.ok_or_else(|| {
        vec![Diagnostic::error(
            "locked runtime permission comparison requires the source Product's package identity",
        )]
    })?;
    let proposal = program.native_realization_proposal()?;
    if !proposal.providers().selection.assumed_bindings().is_empty() {
        return Err(vec![Diagnostic::error(
            "the selected provider's assumed binding lacks its canonical Build review request",
        )]);
    }
    let mut rows = Vec::new();
    let identities = proposal.package_terminal_authority_permission_identities();
    for permission in proposal.package_terminal_authority_permissions() {
        let callable = identities
            .text(permission.requirement_identity())
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "selected terminal permission lost its checked requirement identity",
                )]
            })?;
        let mut owners = bindings.iter().filter(|binding| {
            binding.normalized_schema_digest() == permission.service_schema()
                && binding.terminal_authority_permissions().iter().any(|row| {
                    binding
                        .terminal_authority_permission_identities()
                        .text(row.requirement_identity())
                        == Some(callable)
                        && row.permitted() == permission.permitted()
                })
        });
        let owner = match (owners.next(), owners.next()) {
            (Some(owner), None) => owner,
            // Stage 01 also classifies pure compiler primitives. These synthesized
            // rows grant no authority and have no package permission to review.
            (None, None) if permission.permitted().classes().is_empty() => continue,
            _ => {
                return Err(vec![Diagnostic::error(format!(
                    "selected terminal permission `{callable}` has no unique package service declaration to compare with the lock",
                ))]);
            }
        };
        rows.push(
            review_evidence::permission_acceptance::RuntimeServicePermission {
                owner: owner.package(),
                service_path: owner.declaration_path().to_owned(),
                requirement_path: declarations.requirement_path(
                    owner.package(),
                    owner.declaration_path(),
                    callable,
                )?,
                permitted: permission.permitted().clone(),
            },
        );
    }
    accepted.compare_service_permissions(package, proposal.target_profile(), &rows)
}
