use crate::typed_trees::name::Identifier;
use crate::typed_trees::signature::SignatureContract;
use crate::typed_trees::signature::StateParameter;
use crate::typed_trees::statement::StatementNode;
use arena::HandleSpan;
use symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub parameters: HandleSpan<StateParameter>,
    pub return_type: crate::typed_trees::types::TypeReferenceHandle,
    pub contracts: HandleSpan<SignatureContract>,
    pub statement_nodes: HandleSpan<StatementNode>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            parameters: HandleSpan::empty(),
            return_type: crate::typed_trees::types::TypeReferenceHandle::invalid(),
            contracts: HandleSpan::empty(),
            statement_nodes: HandleSpan::empty(),
        }
    }
}
