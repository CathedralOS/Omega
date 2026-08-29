use crate::model::{
    PackageReviewNominalIdentity, PackageReviewNominalOwner, PackageReviewToolchainSourceIdentity,
};
use omega_compiler::CheckedCompilation;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

pub(crate) fn reviewed_package_owns(
    identity: &PackageReviewNominalIdentity,
    package: PackageKeyIdentity,
) -> Result<bool, Vec<Diagnostic>> {
    match identity.owner {
        PackageReviewNominalOwner::Package(owner) => Ok(owner == package),
        PackageReviewNominalOwner::ToolchainSource(_) => Ok(false),
        PackageReviewNominalOwner::Unresolved => Err(vec![Diagnostic::error(format!(
            "reviewed public declaration `{}` has no managed package owner",
            identity.path
        ))]),
    }
}

pub(crate) fn nominal_identity(
    compilation: &CheckedCompilation,
    symbol: SymbolHandle,
) -> Result<PackageReviewNominalIdentity, Vec<Diagnostic>> {
    let owner = nominal_owner(compilation, symbol)?;
    let path = compilation.typed.symbols.display_path(symbol, "::");
    if path.is_empty() {
        return Err(vec![Diagnostic::error(
            "package review encountered a symbol without a stable declaration path",
        )]);
    }
    Ok(PackageReviewNominalIdentity { owner, path })
}

pub(crate) fn trait_requirement_identity(
    compilation: &CheckedCompilation,
    owner: &psi_typed_trees::trait_definition::TraitDefinition,
    requirement: &psi_typed_trees::signature::StateSignature,
) -> Result<PackageReviewNominalIdentity, Vec<Diagnostic>> {
    let owner_identity = nominal_identity(compilation, owner.symbol)?;
    let requirement_owner = nominal_owner(compilation, requirement.symbol)?;
    if owner_identity.owner != requirement_owner {
        return Err(vec![Diagnostic::error(format!(
            "package review trait `{}` and requirement `{}` have mismatched exact ownership",
            owner.name, requirement.name
        ))]);
    }
    Ok(PackageReviewNominalIdentity {
        owner: requirement_owner,
        path: compilation
            .normalized_trait_requirement_overload_identity(owner, requirement)
            .identity(),
    })
}

pub(crate) fn trait_requirement_identity_from_symbols(
    compilation: &CheckedCompilation,
    trait_symbol: SymbolHandle,
    requirement_symbol: SymbolHandle,
    context: &str,
) -> Result<PackageReviewNominalIdentity, Vec<Diagnostic>> {
    let owners = compilation
        .traits()
        .iter()
        .filter(|candidate| candidate.symbol == trait_symbol)
        .collect::<Vec<_>>();
    let [owner] = owners.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "{context} resolves its declaring trait to {} declarations; expected exactly one",
            owners.len()
        ))]);
    };
    let requirements = compilation
        .trait_machine_signatures(owner)
        .iter()
        .filter(|candidate| candidate.symbol == requirement_symbol)
        .collect::<Vec<_>>();
    let [requirement] = requirements.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "{context} resolves its requirement to {} overload declarations under its exact trait; expected exactly one",
            requirements.len()
        ))]);
    };
    trait_requirement_identity(compilation, owner, requirement)
}

pub(crate) fn provider_requirement_identity(
    compilation: &CheckedCompilation,
    schema: omega_provider_planning::plans::ProviderSchemaDeclaration,
    requirement_symbol: SymbolHandle,
) -> Result<PackageReviewNominalIdentity, Vec<Diagnostic>> {
    match schema {
        omega_provider_planning::plans::ProviderSchemaDeclaration::BoundaryTrait(trait_symbol) => {
            trait_requirement_identity_from_symbols(
                compilation,
                trait_symbol,
                requirement_symbol,
                "selected provider row",
            )
        }
        omega_provider_planning::plans::ProviderSchemaDeclaration::BoundaryOperator(_) => {
            let operators = compilation.operators().iter().chain(
                compilation
                    .domain_definitions()
                    .iter()
                    .flat_map(|domain| compilation.domain_operators(domain)),
            );
            let matches = operators
                .filter(|candidate| candidate.symbol == requirement_symbol && candidate.is_boundary)
                .collect::<Vec<_>>();
            let [operator] = matches.as_slice() else {
                return Err(vec![Diagnostic::error(format!(
                    "selected provider row resolves its boundary operator requirement to {} declarations; expected exactly one",
                    matches.len()
                ))]);
            };
            let nominal = nominal_identity(compilation, requirement_symbol)?;
            Ok(PackageReviewNominalIdentity {
                owner: nominal.owner,
                path: psi_typed_trees::operator::boundary_operator_requirement_identity(
                    &compilation.typed,
                    operator,
                ),
            })
        }
    }
}

pub(crate) fn nominal_owner(
    compilation: &CheckedCompilation,
    symbol: SymbolHandle,
) -> Result<PackageReviewNominalOwner, Vec<Diagnostic>> {
    nominal_owner_from_symbols(&compilation.typed.symbols, symbol)
}

pub(crate) fn nominal_owner_from_symbols(
    symbols: &psi_symbols::SymbolTable,
    symbol: SymbolHandle,
) -> Result<PackageReviewNominalOwner, Vec<Diagnostic>> {
    if let Some(package) = symbols.symbol_package_identity(symbol) {
        return Ok(PackageReviewNominalOwner::Package(package));
    }
    let Some(source_file) = symbols
        .symbol_provenance_source_span(symbol)
        .and_then(|span| symbols.source_file(span))
    else {
        return Ok(PackageReviewNominalOwner::Unresolved);
    };
    match source_file.origin {
        psi_source::SourceOrigin::Toolchain => Ok(PackageReviewNominalOwner::ToolchainSource(
            toolchain_source_identity(source_file)?,
        )),
        psi_source::SourceOrigin::User => Ok(PackageReviewNominalOwner::Unresolved),
    }
}

pub(crate) fn toolchain_source_identity(
    source_file: &psi_source::SourceFile,
) -> Result<PackageReviewToolchainSourceIdentity, Vec<Diagnostic>> {
    Ok(PackageReviewToolchainSourceIdentity {
        digest: omega_package_compilation::toolchain_source_identity_digest(source_file)?,
    })
}

pub(crate) fn is_canonical_virtual_toolchain_path(path: &std::path::Path) -> bool {
    let mut components = path.components();
    let Some(std::path::Component::Normal(component)) = components.next() else {
        return false;
    };
    if components.next().is_some() {
        return false;
    }
    component.to_str().is_some_and(|component| {
        component.len() >= 3 && component.starts_with('<') && component.ends_with('>')
    })
}
