use crate::capture::PackageReviewInput;
use crate::record::{
    PackageReviewNominalIdentity, PackageReviewNominalOwner, PackageReviewToolchainSourceIdentity,
};
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use symbols::SymbolHandle;

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

pub(crate) fn nominal_owner(
    compilation: &PackageReviewInput<'_>,
    symbol: SymbolHandle,
) -> Result<PackageReviewNominalOwner, Vec<Diagnostic>> {
    nominal_owner_from_symbols(
        &compilation.typed.symbols,
        symbol,
        compilation.custody.exact_toolchain_sources(),
    )
}

pub(crate) fn nominal_owner_from_symbols(
    symbols: &symbols::SymbolTable,
    symbol: SymbolHandle,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
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
        source::SourceOrigin::Toolchain => Ok(PackageReviewNominalOwner::ToolchainSource(
            toolchain_source_identity(source_file, exact_toolchain_sources)?,
        )),
        source::SourceOrigin::User => Ok(PackageReviewNominalOwner::Unresolved),
    }
}

pub(crate) fn toolchain_source_identity(
    source_file: &source::SourceFile,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
) -> Result<PackageReviewToolchainSourceIdentity, Vec<Diagnostic>> {
    if source_file.origin != source::SourceOrigin::Toolchain {
        return Err(vec![Diagnostic::error(format!(
            "toolchain source identity requested for non-toolchain source `{}`",
            source_file.path.display(),
        ))]);
    }
    // This is the same checked source snapshot and sorted digest roster used
    // by review type identity. SourceId is only its private join coordinate;
    // canonical review bytes retain the compiler-computed digest, not the ID.
    // Rehashing each declaration would repeatedly process the same file bytes.
    let position = exact_toolchain_sources
        .binary_search_by_key(&source_file.source_id.0, |(source_id, _)| source_id.0)
        .map_err(|_| {
            vec![Diagnostic::error(format!(
                "reviewed toolchain source `{}` has no exact checked source identity",
                source_file.path.display(),
            ))]
        })?;
    Ok(PackageReviewToolchainSourceIdentity {
        digest: exact_toolchain_sources[position].1,
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

/// Whether the symbol's declaration lives on the product dependency scope.
/// A source imported on both the product and build scopes is checked once
/// per scope; review projections name authored declarations, so they see
/// only the product instance. Source-free and toolchain declarations keep
/// their existing behavior.
pub(crate) fn is_product_scope_instance(
    symbols: &symbols::SymbolTable,
    symbol: SymbolHandle,
) -> bool {
    symbols
        .symbol_provenance_source_span(symbol)
        .and_then(|span| symbols.source_file(span))
        .is_none_or(|file| file.dependency_scope != source::DependencyScope::Build)
}
