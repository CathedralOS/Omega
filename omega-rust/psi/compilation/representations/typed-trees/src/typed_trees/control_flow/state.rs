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

/// The state keyed `symbol`, when declared. States are keyed by symbol rather
/// than name, so transition and evidence lookups resolve identity directly.
pub fn state_by_symbol(states: &[State], symbol: SymbolHandle) -> Option<&State> {
    states.iter().find(|state| state.symbol == symbol)
}
