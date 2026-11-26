use diagnostics::Diagnostic;

/// Project scoped, verified supplier premises into the same rows for review and execution.
pub(super) fn component_assumptions<'a>(
    components: impl IntoIterator<
        Item = (
            semantic_vocabulary::PackageKeyIdentity,
            &'a component_description::VerifiedComponent,
        ),
    >,
) -> Result<Vec<review_evidence::record::PackagePolicyComponentAssumption>, Vec<Diagnostic>> {
    let mut rows = Vec::new();
    for (component, verified) in components {
        for (assumption, bound_rows) in verified
            .assumption_rows()
            .map_err(|reason| vec![Diagnostic::error(reason)])?
        {
            rows.push(review_evidence::record::PackagePolicyComponentAssumption {
                component,
                assumption,
                bound_rows,
            });
        }
    }
    rows.sort();
    Ok(rows)
}

pub(super) fn compare(
    accepted: &review_evidence::permission_acceptance::TerminalPermissionAcceptance,
    package: Option<semantic_vocabulary::PackageKeyIdentity>,
    program: &native_compiler::NativeProgram,
    components: &[review_evidence::record::PackagePolicyComponentAssumption],
    bindings: &[provider_acceptance::AcceptedSemanticBinding],
    declarations: &source_compiler::RuntimePermissionDeclarations,
) -> Result<(), Vec<Diagnostic>> {
    let package = package.ok_or_else(|| {
        vec![Diagnostic::error(
            "locked runtime permission comparison requires the source Product's package identity",
        )]
    })?;
    let proposal = program.native_realization_proposal();
    // The locked source and target were checked before this Build ran. Its root grants
    // still have to name providers selected by this execution of the Build.
    let grants = declarations.root_provider_grants(package)?;
    super::selected_provider_grants(&proposal.providers().selection, grants)
        .map_err(|error| vec![error])?;
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
    accepted.compare_runtime_permissions(package, proposal.target_profile(), &rows, components)
}
