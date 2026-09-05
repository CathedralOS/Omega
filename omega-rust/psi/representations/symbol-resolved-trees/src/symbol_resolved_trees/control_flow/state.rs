use crate::name::DiagnosticName;
use crate::signature::{SignatureContract, StateParameter};
use crate::statement::{Statement, StatementNode};
use arena::HandleSpan;
use std::ops::{Deref, DerefMut};
use symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct State {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    pub storage: StateStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateStorage {
    pub parameters: HandleSpan<StateParameter>,
    pub return_type: Option<crate::types::TypeReference>,
    pub contracts: HandleSpan<SignatureContract>,
    pub statements: HandleSpan<Statement>,
    pub statement_nodes: HandleSpan<StatementNode>,
}

impl Deref for State {
    type Target = StateStorage;

    fn deref(&self) -> &Self::Target {
        &self.storage
    }
}

impl DerefMut for State {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.storage
    }
}
