use crate::name::DiagnosticName;
use crate::types::TypeReference;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DomainDefinition {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    pub target_type: TypeReference,
    pub facts: HandleSpan<ProofFact>,
    pub operators: HandleSpan<crate::operator::OperatorDefinition>,
    pub body_token_count: usize,
    /// STR4 checked plans, slice 1: the normalized semantic identity from
    /// the program's SemanticDomainTable (populated ONCE at
    /// syntax->resolved, copied downstream; NULL only pre-lowering).
    pub semantic_id: omega_core::semantics::SemanticDomainId,
    /// DOM1/STR2: the normalized predicate/semantic role pair. Populated once
    /// at syntax->resolved and copied downstream; consumers must not infer a
    /// role from the presence of facts or operators.
    pub facets: omega_core::semantics::DomainFacets,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofFact {
    Expression(crate::expression::ExpressionHandle),
    Membership(ProofMembershipFact),
}

impl Default for ProofFact {
    fn default() -> Self {
        Self::Expression(crate::expression::ExpressionHandle::invalid())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProofMembershipFact {
    pub value: crate::expression::ExpressionHandle,
    pub domain: HandleSpan<DiagnosticName>,
    pub domain_symbol: SymbolHandle,
}

impl Default for ProofMembershipFact {
    fn default() -> Self {
        Self {
            value: crate::expression::ExpressionHandle::invalid(),
            domain: HandleSpan::empty(),
            domain_symbol: SymbolHandle::invalid(),
        }
    }
}
