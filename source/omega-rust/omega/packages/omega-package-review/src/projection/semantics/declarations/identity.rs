use crate::evidence::PackageReviewNominalIdentity;
use crate::projection::semantics::declarations::nominal_owner;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

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
