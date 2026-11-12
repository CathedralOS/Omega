/*
We resolve nominal declaration ownership and source-instance membership against
checked source custody.

Toolchain source identities use the same checked source snapshot and sorted
digest roster as review type identity: `SourceId` is only a private join
coordinate, while canonical review bytes retain the compiler-computed digest
without rehashing each declaration. The digest includes the consumed source role,
so closed contract provenance remains distinct from ambient core provenance.
Current Product declaration membership is
checked against immutable custody so a supplied selection mask cannot hide a
live checked declaration, and Product review roots exclude Build declarations.
*/

use super::identity::nominal_identity;
use crate::capture::SourceDeclarationInput;
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

pub(crate) fn owned_identity(
    compilation: &SourceDeclarationInput<'_>,
    package: PackageKeyIdentity,
    symbol: SymbolHandle,
) -> Result<Option<PackageReviewNominalIdentity>, Vec<Diagnostic>> {
    if !is_product_scope_instance(compilation, symbol)? {
        return Ok(None);
    }
    let identity = nominal_identity(compilation, symbol)?;
    Ok(reviewed_package_owns(&identity, package)?.then_some(identity))
}

pub(crate) fn nominal_owner(
    compilation: &SourceDeclarationInput<'_>,
    symbol: SymbolHandle,
) -> Result<PackageReviewNominalOwner, Vec<Diagnostic>> {
    nominal_owner_from_symbols(
        &compilation.typed.symbols,
        symbol,
        compilation.exact_toolchain_sources,
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
    if components.next().is_some() {
        return false;
    }
    component.to_str().is_some_and(|component| {
        component.len() >= 3 && component.starts_with('<') && component.ends_with('>')
    })
}

pub(crate) fn is_checked_source_instance(
    compilation: &SourceDeclarationInput<'_>,
    symbol: SymbolHandle,
) -> Result<bool, Vec<Diagnostic>> {
    let symbols = &compilation.typed.symbols;
    let original = &compilation.original_program.typed.symbols;
    if !symbols.symbols().nodes().is_valid(symbol) || !original.symbols().nodes().is_valid(symbol) {
        return Err(vec![Diagnostic::error(
            "reviewed declaration has no exact checked symbol coordinate",
        )]);
    }
    let membership = |symbols: &symbols::SymbolTable| {
        let scope = symbols
            .symbol_provenance_source_span(symbol)
            .and_then(|span| symbols.source_file(span))
            .map(|file| file.dependency_scope);
        (symbols.symbol_source_is_selected(symbol), scope)
    };
    let selected = membership(symbols);
    if selected != membership(original) {
        return Err(vec![Diagnostic::error(format!(
            "reviewed declaration `{}` source membership differs from checked source custody",
            symbols.display_path(symbol, "::"),
        ))]);
    }
    Ok(selected.0)
}

pub(crate) fn is_product_scope_instance(
    compilation: &SourceDeclarationInput<'_>,
    symbol: SymbolHandle,
) -> Result<bool, Vec<Diagnostic>> {
    Ok(is_checked_source_instance(compilation, symbol)?
        && compilation
            .typed
            .symbols
            .symbol_provenance_source_span(symbol)
            .and_then(|span| compilation.typed.symbols.source_file(span))
            .is_none_or(|file| file.dependency_scope != source::DependencyScope::Build))
}
