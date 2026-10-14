//! Exact source occurrences licensed by the checked quotient correspondence.
//! Runtime planning consumes these rows while the source request stays sealed.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedQuotientOperation {
    pub request: typed_trees::expression::ExpressionHandle,
    pub public_machine: symbols::SymbolHandle,
    pub public_state: symbols::SymbolHandle,
    pub representative_state: symbols::SymbolHandle,
    pub correspondence:
        language_semantics::quotient_correspondence::CanonicalQuotientCorrespondence,
    /// Entry-state symbols of the selected resultless theorem machines, in
    /// the request's role order; one per `correspondence.theorem_evidence`
    /// row. Lowering emits the theorem roster from here.
    pub theorems: Box<[symbols::SymbolHandle]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedQuotientFormation {
    pub data_symbol: symbols::SymbolHandle,
    pub carrier: typed_trees::types::TypeReferenceHandle,
    pub relation_symbol: symbols::SymbolHandle,
}
