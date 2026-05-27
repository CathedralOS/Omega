use crate::name::Identifier;
use crate::types::TypeReferenceHandle;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainDefinition {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub target_type: TypeReferenceHandle,
    pub facts: HandleSpan<ProofFact>,
    pub operators: HandleSpan<crate::operator::OperatorDefinition>,
    pub body_token_count: usize,
}

impl Default for DomainDefinition {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            target_type: TypeReferenceHandle::invalid(),
            facts: HandleSpan::empty(),
            operators: HandleSpan::empty(),
            body_token_count: 0,
        }
    }
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
    pub domain: HandleSpan<Identifier>,
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
