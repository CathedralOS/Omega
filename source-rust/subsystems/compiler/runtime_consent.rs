use build_providers::acceptance::{AcceptedSemanticBinding, AcceptedSemanticBindingRole};
use diagnostics::Diagnostic;
use review_evidence::record::{
    PackagePolicyTerminalPermission, PackageReviewNominalIdentity, PackageReviewNominalOwner,
    PackageReviewToolchainSourceIdentity,
};
use source_compiler::representations::source_state::{
    TypedTrees, trait_definition::TraitDefinition,
};

/// Review and locked execution resolve the same Product declaration. Core declares
/// ProcessExit even when a dependency supplies its selected implementation.
pub fn reviewed_service_declaration<'a>(
    source: &'a TypedTrees,
    binding: &AcceptedSemanticBinding,
) -> Result<&'a TraitDefinition, Vec<Diagnostic>> {
    let path = binding.declaration_path();
    let mut services = source.traits().iter().filter(|service| {
        service.is_boundary
            && source.symbols.symbol_is_in_dependency_scope(
                service.symbol,
                source_provenance::DependencyScope::Product,
            )
            && source.symbols.display_path(service.symbol, "::") == path
            && if binding.role() == AcceptedSemanticBindingRole::ProcessExitExitProcessI32 {
                source.core_vocabulary(service.symbol)
                    == Some(language_core::core_vocabulary::CoreVocabulary::ProcessExitService)
            } else {
                source.symbols.symbol_package_identity(service.symbol) == Some(binding.package())
            }
    });
    match (services.next(), services.next()) {
        (Some(service), None) => Ok(service),
        _ => Err(vec![Diagnostic::error(format!(
            "no unique checked Product service declaration for `{path}`"
        ))]),
    }
}

/// The nominal coordinate recorded at review and re-derived from the locked
/// program. Package review supplies its prepared, source-local digest roster.
pub fn reviewed_nominal_identity(
    source: &TypedTrees,
    symbol: symbols::SymbolHandle,
    toolchain_digest: impl FnOnce(&source_admission::SourceFile) -> Result<[u8; 32], String>,
) -> Result<PackageReviewNominalIdentity, Vec<Diagnostic>> {
    let unavailable = |reason: &str| {
        vec![Diagnostic::error(format!(
            "cannot identify the accepted declaration: {reason}"
        ))]
    };
    let symbols = &source.symbols;
    if !symbols.symbols().nodes().is_valid(symbol) {
        return Err(unavailable("stale nominal declaration"));
    }
    let owner = if let Some(package) = symbols.symbol_package_identity(symbol) {
        PackageReviewNominalOwner::Package(package)
    } else {
        let file = symbols
            .symbol_provenance_source_span(symbol)
            .and_then(|span| symbols.source_file(span))
            .filter(|file| file.origin.is_toolchain())
            .ok_or_else(|| unavailable("declaration without package or toolchain custody"))?;
        PackageReviewNominalOwner::ToolchainSource(PackageReviewToolchainSourceIdentity {
            digest: toolchain_digest(file).map_err(|reason| unavailable(&reason))?,
        })
    };
    let path = source
        .authored_declaration_path(symbol)
        .map_err(|reason| unavailable(&reason))?;
    if path.is_empty() {
        return Err(unavailable("declaration without a stable path"));
    }
    Ok(PackageReviewNominalIdentity { owner, path })
}

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
    bindings: &[build_providers::acceptance::AcceptedSemanticBinding],
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
        let source = declarations.checked_source();
        let service = reviewed_service_declaration(source, owner)?;
        let signatures = source.trait_machine_signatures(service);
        let mut methods = signatures.iter().filter(|method| {
            source.trait_requirement_callable_identity_text(service, method) == callable
        });
        let (Some(method), None) = (methods.next(), methods.next()) else {
            return Err(vec![Diagnostic::error(format!(
                "no unique checked method `{callable}` on `{}`",
                owner.declaration_path()
            ))]);
        };
        let nominal = |symbol| {
            reviewed_nominal_identity(
                source,
                symbol,
                source_admission::source_identity::toolchain_source_identity_digest,
            )
        };
        rows.push(
            review_evidence::permission_acceptance::RuntimeServicePermission {
                service: nominal(service.symbol)?.into(),
                permission: PackagePolicyTerminalPermission {
                    requirement: nominal(method.symbol)?,
                    permitted: permission.permitted().clone(),
                },
            },
        );
    }
    accepted.compare_runtime_permissions(package, proposal.target_profile(), &rows, components)
}
