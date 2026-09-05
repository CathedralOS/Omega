use crate::record::{
    PackageReviewNominalIdentity, PackageReviewNominalOwner, PackageReviewToolchainSourceIdentity,
};
use compiler::CheckedCompilation;
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
    compilation: &CheckedCompilation,
    symbol: SymbolHandle,
) -> Result<PackageReviewNominalOwner, Vec<Diagnostic>> {
    nominal_owner_from_symbols(&compilation.typed.symbols, symbol)
}

pub(crate) fn nominal_owner_from_symbols(
    symbols: &symbols::SymbolTable,
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
        source::SourceOrigin::Toolchain => Ok(PackageReviewNominalOwner::ToolchainSource(
            toolchain_source_identity(source_file)?,
        )),
        source::SourceOrigin::User => Ok(PackageReviewNominalOwner::Unresolved),
    }
}

pub(crate) fn toolchain_source_identity(
    source_file: &source::SourceFile,
) -> Result<PackageReviewToolchainSourceIdentity, Vec<Diagnostic>> {
    Ok(PackageReviewToolchainSourceIdentity {
        digest: package_compilation::toolchain_source_identity_digest(source_file)?,
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
