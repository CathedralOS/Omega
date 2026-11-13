//! Legacy evidence reconstruction is unavailable after source-checker removal.

use crate::capture::SourceDeclarationInput;
use crate::capture::contracts::facts::ContractProjectionContext;
use crate::record::PackageReviewCrashRoute;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::signature::StateSignature;

pub(crate) fn project(
    _compilation: &SourceDeclarationInput<'_>,
    _owner: SymbolHandle,
    _signature: &StateSignature,
    _context: &ContractProjectionContext<'_>,
    _binders: &[(SymbolHandle, String)],
) -> Result<Vec<PackageReviewCrashRoute>, Vec<Diagnostic>> {
    Err(vec![diagnostics::Diagnostic::error(
        "no product route yet: package-review static signature crash evidence",
    )])
}
