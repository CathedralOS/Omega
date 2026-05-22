use crate::name::ProgramName;
use crate::types::TypeReferenceHandle;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainDefinition {
    pub symbol: SymbolHandle,
    pub name: ProgramName,
    pub target_type: TypeReferenceHandle,
    pub facts: HandleSpan<DomainFact>,
    pub body_token_count: usize,
}

impl Default for DomainDefinition {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: ProgramName::default(),
            target_type: TypeReferenceHandle::invalid(),
            facts: HandleSpan::empty(),
            body_token_count: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainFact {
    Expression(crate::expression::ExpressionHandle),
    Membership(DomainMembershipFact),
}

impl Default for DomainFact {
    fn default() -> Self {
        Self::Expression(crate::expression::ExpressionHandle::invalid())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DomainMembershipFact {
    pub value: crate::expression::ExpressionHandle,
    pub domain: HandleSpan<ProgramName>,
    pub domain_symbol: SymbolHandle,
}

impl Default for DomainMembershipFact {
    fn default() -> Self {
        Self {
            value: crate::expression::ExpressionHandle::invalid(),
            domain: HandleSpan::empty(),
            domain_symbol: SymbolHandle::invalid(),
        }
    }
}
