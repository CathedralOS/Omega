use crate::name::ProgramName;
use crate::signature::StateParameter;
use crate::statement::{Statement, StatementNode};
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub symbol: SymbolHandle,
    pub name: ProgramName,
    pub parameters: HandleSpan<StateParameter>,
    pub return_type: Option<crate::types::TypeReference>,
    pub statements: HandleSpan<Statement>,
    pub statement_nodes: HandleSpan<StatementNode>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: ProgramName::default(),
            parameters: HandleSpan::empty(),
            return_type: None,
            statements: HandleSpan::empty(),
            statement_nodes: HandleSpan::empty(),
        }
    }
}
