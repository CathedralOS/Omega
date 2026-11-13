// Contract-expression call evidence argument projection, verifying that each lane
// binds a visible `Requires` evidence term whose instantiated proposition matches
// the target callable's `Requires` parameter lane.

use super::names::portable_ordinal;
use crate::capture::SourceDeclarationInput;
use crate::capture::contracts::facts::ContractProjectionContext;
use crate::capture::semantics::declarations::nominal_identity;
use crate::record::{
    PackageReviewContractEvidenceArgument, PackageReviewContractEvidenceTerm,
    PackageReviewContractKind,
};
use checked_trees::ContractProofFactOwner;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

pub(crate) fn project_contract_call_evidence_arguments(
    _compilation: &SourceDeclarationInput<'_>,
    _context: &ContractProjectionContext<'_>,
    _fact: Option<arena::Handle<typed_trees::domain::ProofFact>>,
    _expression: typed_trees::expression::ExpressionHandle,
    _target_state_symbol: SymbolHandle,
    _authored_count: usize,
) -> Result<Vec<PackageReviewContractEvidenceArgument>, Vec<Diagnostic>> {
    Err(vec![diagnostics::Diagnostic::error(
        "no product route yet: package review requires lowered-form evidence",
    )])
}
