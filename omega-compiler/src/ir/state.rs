use crate::ir::signature::StateParameter;
use crate::ir::statement::Statement;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub name: String,
    pub parameters: Vec<StateParameter>,
    pub return_type: Option<crate::ir::types::TypeReference>,
    pub statements: Vec<Statement>,
}
