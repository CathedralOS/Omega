use build_providers::acceptance::{
    AcceptedSemanticBinding, AcceptedSemanticBindingRole, ServiceTerminalAuthorityPermission,
};
use diagnostics::Diagnostic;
use review_evidence::record::{
    PackagePolicyTerminalPermission, PackageReviewNominalIdentity, PackageReviewNominalOwner,
    PackageReviewToolchainSourceIdentity,
};
use semantic_vocabulary::{TerminalAuthorityDisposition, provider_identity::ServiceSchemaDigest};
use source_compiler::representations::source_state::{
    TypedTrees, trait_definition::TraitDefinition,
};
use std::collections::BTreeMap;

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

/// The receiving target checks both package-supplied permissions and the
/// physical operations implementing them. Only supplied permissions appear in
/// a package lock. Each one must have its exact authority on this target.
pub struct RealizedServicePermissions<'a> {
    rows: BTreeMap<(ServiceSchemaDigest, &'a str), &'a TerminalAuthorityDisposition>,
}

impl<'a> RealizedServicePermissions<'a> {
    pub fn new(
        rows: &'a [ServiceTerminalAuthorityPermission],
        identities: &'a terminal_psi::TerminalIdentityTable,
    ) -> Result<Self, &'static str> {
        let mut realized = BTreeMap::new();
        for row in rows {
            let requirement = identities
                .text(row.requirement_identity())
                .filter(|name| !name.is_empty())
                .ok_or("a realized permission has no requirement identity")?;
            if realized
                .insert((row.service_schema(), requirement), row.permitted())
                .is_some()
            {
                return Err("a realized requirement has duplicate receiver permissions");
            }
        }
        Ok(Self { rows: realized })
    }

    pub fn supplied_requirement<'binding>(
        &self,
        binding: &'binding AcceptedSemanticBinding,
        grant: &ServiceTerminalAuthorityPermission,
    ) -> Result<&'binding str, &'static str> {
        if grant.service_schema() != binding.normalized_schema_digest() {
            return Err("a receiver permission names a different service schema");
        }
        let requirement = binding
            .terminal_authority_permission_identities()
            .text(grant.requirement_identity())
            .filter(|name| !name.is_empty())
            .ok_or("a supplied receiver permission has no requirement identity")?;
        if self
            .rows
            .get(&(grant.service_schema(), requirement))
            .copied()
            != Some(grant.permitted())
        {
            return Err(
                "the selected target does not realize the supplied requirement's exact disposition",
            );
        }
        Ok(requirement)
    }
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
    let error = |reason: &str| vec![Diagnostic::error(reason)];
    let realized = RealizedServicePermissions::new(
        proposal.package_terminal_authority_permissions(),
        proposal.package_terminal_authority_permission_identities(),
    )
    .map_err(error)?;
    let source = declarations.checked_source();
    let nominal = |symbol| {
        reviewed_nominal_identity(
            source,
            symbol,
            source_admission::source_identity::toolchain_source_identity_digest,
        )
    };
    for binding in bindings
        .iter()
        .filter(|binding| !binding.terminal_authority_permissions().is_empty())
    {
        let service = reviewed_service_declaration(source, binding)?;
        let service_identity = nominal(service.symbol)?;
        let signatures = source.trait_machine_signatures(service);
        for grant in binding.terminal_authority_permissions() {
            let callable = realized
                .supplied_requirement(binding, grant)
                .map_err(error)?;
            let mut methods = signatures.iter().filter(|method| {
                source.trait_requirement_callable_identity_text(service, method) == callable
            });
            let (Some(method), None) = (methods.next(), methods.next()) else {
                return Err(vec![Diagnostic::error(format!(
                    "no unique checked method `{callable}` on `{}`",
                    binding.declaration_path()
                ))]);
            };
            rows.push(
                review_evidence::permission_acceptance::RuntimeServicePermission {
                    service: service_identity.clone().into(),
                    permission: PackagePolicyTerminalPermission {
                        requirement: nominal(method.symbol)?,
                        permitted: grant.permitted().clone(),
                    },
                },
            );
        }
    }
    accepted.compare_runtime_permissions(package, proposal.target_profile(), &rows, components)
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{
        PackageKeyIdentity, TerminalAuthorityClass, provider_identity::ProviderPlanDigest,
    };

    #[test]
    fn the_exit_implementation_cannot_satisfy_the_supplied_exit_permission() {
        let schema = ServiceSchemaDigest::from_digest([1; 32]);
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
        let exit = identities.intern("ProcessExit::exit_process");
        let primitive = identities.intern("ProcessExitPrimitive");
        let identities = identities.build();
        let termination = TerminalAuthorityDisposition::from_classes([
            TerminalAuthorityClass::ProcessTermination,
        ]);
        let grant = ServiceTerminalAuthorityPermission::new(schema, exit, termination.clone());
        let provider = AcceptedSemanticBinding::new(
            AcceptedSemanticBindingRole::ProcessExitExitProcessI32,
            PackageKeyIdentity::from_digest([2; 32]).unwrap(),
            "ProcessExit",
            schema,
            ProviderPlanDigest::from_digest([3; 32]),
        )
        .unwrap()
        .with_terminal_authority_permissions(vec![grant.clone()], &identities)
        .unwrap();
        let supplied = &provider.terminal_authority_permissions()[0];
        let implementation =
            ServiceTerminalAuthorityPermission::new(schema, primitive, termination.clone());
        let rows = [implementation.clone(), grant.clone()];
        let realized = RealizedServicePermissions::new(&rows, &identities).unwrap();
        assert_eq!(
            realized.supplied_requirement(&provider, supplied),
            Ok("ProcessExit::exit_process")
        );
        for substituted in [
            implementation,
            ServiceTerminalAuthorityPermission::new(
                ServiceSchemaDigest::from_digest([4; 32]),
                exit,
                termination,
            ),
            ServiceTerminalAuthorityPermission::new(
                schema,
                exit,
                TerminalAuthorityDisposition::from_classes([]),
            ),
        ] {
            let rows = [substituted];
            let realized = RealizedServicePermissions::new(&rows, &identities).unwrap();
            assert!(realized.supplied_requirement(&provider, supplied).is_err());
        }
        assert!(RealizedServicePermissions::new(&[grant.clone(), grant], &identities).is_err());
    }
}
