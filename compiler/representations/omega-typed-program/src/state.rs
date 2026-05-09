use crate::name::ProgramName;
use crate::signature::StateParameter;
use crate::statement::Statement;
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub symbol: SymbolHandle,
    pub name: ProgramName,
    pub parameters: Vec<StateParameter>,
    pub return_type: Option<crate::types::TypeReference>,
    pub statements: Vec<Statement>,
}
