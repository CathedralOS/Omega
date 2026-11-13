//! Declaration names are joined through the retained symbol table and exact
//! package or toolchain source ownership. They grant no checking authority.

use crate::record::{
    PackageReviewNominalIdentity, PackageReviewNominalOwner, PackageReviewToolchainSourceIdentity,
};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

pub(crate) fn nominal_identity_from_symbols(
    symbols: &symbols::SymbolTable,
    symbol: SymbolHandle,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
) -> Result<PackageReviewNominalIdentity, Vec<Diagnostic>> {
    let owner = nominal_owner_from_symbols(symbols, symbol, exact_toolchain_sources)?;
    let path = symbols.display_path(symbol, "::");
    if path.is_empty() {
        return Err(vec![Diagnostic::error(
            "package review encountered a symbol without a stable declaration path",
        )]);
    }
    Ok(PackageReviewNominalIdentity { owner, path })
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
        source::SourceOrigin::Toolchain | source::SourceOrigin::ToolchainContract => {
            Ok(PackageReviewNominalOwner::ToolchainSource(
                toolchain_source_identity(source_file, exact_toolchain_sources)?,
            ))
        }
        source::SourceOrigin::User => Ok(PackageReviewNominalOwner::Unresolved),
    }
}

pub(crate) fn toolchain_source_identity(
    source_file: &source::SourceFile,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
) -> Result<PackageReviewToolchainSourceIdentity, Vec<Diagnostic>> {
    if !source_file.origin.is_toolchain() {
        return Err(vec![Diagnostic::error(format!(
            "toolchain source identity requested for non-toolchain source `{}`",
            source_file.path.display(),
        ))]);
    }
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
    components.next().is_none()
        && component.to_str().is_some_and(|component| {
            component.len() >= 3 && component.starts_with('<') && component.ends_with('>')
        })
}
