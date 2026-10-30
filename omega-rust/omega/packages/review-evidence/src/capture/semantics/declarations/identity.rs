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
        compilation.custody.sources().exact_toolchain_sources(),
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

/// The reviewed producer's exact callable identity text. Custody is the only
/// owner of the retained identity table, so an absent row is empty, never a
/// re-derivation from the receipt's ordinal.
pub(crate) fn producer_callable_identity(
    compilation: &PackageReviewInput<'_>,
    receipt: &effects::provider_plan::EvaluatedBindingReceipt,
) -> String {
    compilation
        .custody
        .providers()
        .evaluated_via_bindings
        .identities()
        .text(receipt.producer_callable_identity())
        .unwrap_or_default()
        .to_owned()
}
