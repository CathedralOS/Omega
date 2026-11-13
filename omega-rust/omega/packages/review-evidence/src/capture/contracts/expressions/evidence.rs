//! Legacy evidence reconstruction is unavailable after source-checker removal.

use crate::capture::SourceDeclarationInput;
use crate::capture::contracts::facts::ContractProjectionContext;
use crate::record::PackageReviewContractEvidenceArgument;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

pub(crate) fn project_contract_call_evidence_arguments(
    _compilation: &SourceDeclarationInput<'_>,
    _context: &ContractProjectionContext<'_>,
    _fact: Option<arena::Handle<typed_trees::domain::ProofFact>>,
    _expression: typed_trees::expression::ExpressionHandle,
    _target_state_symbol: SymbolHandle,
    authored_count: usize,
) -> Result<Vec<PackageReviewContractEvidenceArgument>, Vec<Diagnostic>> {
    if authored_count == 0 {
        return Ok(Vec::new());
    }
    Err(vec![diagnostics::Diagnostic::error(
        "no product route yet: contract-call evidence publication",
    )])
}
