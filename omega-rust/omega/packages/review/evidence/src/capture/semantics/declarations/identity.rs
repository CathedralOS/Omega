use super::ownership::nominal_owner_from_symbols;
use crate::capture::PackageReviewInput;
use crate::record::PackageReviewNominalIdentity;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

pub(crate) fn nominal_identity(
    compilation: &PackageReviewInput<'_>,
    symbol: SymbolHandle,
) -> Result<PackageReviewNominalIdentity, Vec<Diagnostic>> {
    nominal_identity_from_symbols(
        &compilation.typed.symbols,
        symbol,
        compilation.custody.exact_toolchain_sources(),
    )
}

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
