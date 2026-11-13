// Exact authored crash guards in policy-only nested static signatures.

use crate::capture::SourceDeclarationInput;
use crate::capture::contracts::facts::ContractProjectionContext;
use crate::capture::semantics::facts::exactly_one;
use crate::record::{
    PackagePolicyCrashGuard, PackageReviewCrashRoute, PackageReviewCrashRouteGuard,
};
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
        "no product route yet: package review requires lowered-form evidence",
    )])
}
