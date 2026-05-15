use crate::name::DiagnosticName;
use crate::signature::StateParameter;
use crate::statement::{Statement, StatementNode};
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    pub storage: StateStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateStorage {
    pub parameters: HandleSpan<StateParameter>,
    pub return_type: Option<crate::types::TypeReference>,
    pub statements: Vec<Statement>,
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
