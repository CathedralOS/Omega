/*
We compute nominal declaration identities and evaluated-binding producer
callable identities.

Custody is the sole owner of the retained evaluated-binding identity table, so
an absent row yields an empty string rather than re-deriving from the receipt's
ordinal.
*/

use super::ownership::nominal_owner_from_symbols;
use crate::capture::{PackageReviewInput, SourceDeclarationInput};
use crate::record::PackageReviewNominalIdentity;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

pub(crate) fn nominal_identity(
    compilation: &SourceDeclarationInput<'_>,
    symbol: SymbolHandle,
) -> Result<PackageReviewNominalIdentity, Vec<Diagnostic>> {
    nominal_identity_from_symbols(
        &compilation.typed.symbols,
        symbol,
        compilation.exact_toolchain_sources,
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
