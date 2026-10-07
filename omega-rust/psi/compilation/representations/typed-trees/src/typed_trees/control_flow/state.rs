use crate::name::Identifier;
use crate::signature::SignatureContract;
use crate::signature::StateParameter;
use crate::statement::StatementNode;
use arena::HandleSpan;
use symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct State {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub parameters: HandleSpan<StateParameter>,
    pub return_type: crate::types::TypeReferenceHandle,
    pub contracts: HandleSpan<SignatureContract>,
    pub statement_nodes: HandleSpan<StatementNode>,
}
