use crate::name::Identifier;
use crate::signature::StateParameter;
use crate::statement::StatementNode;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub parameters: HandleSpan<StateParameter>,
    pub return_type: crate::types::TypeReferenceHandle,
    pub statement_nodes: HandleSpan<StatementNode>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            parameters: HandleSpan::empty(),
            return_type: crate::types::TypeReferenceHandle::invalid(),
            statement_nodes: HandleSpan::empty(),
        }
    }
}
