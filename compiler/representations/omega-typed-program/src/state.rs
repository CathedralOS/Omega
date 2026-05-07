use crate::signature::StateParameter;
use crate::statement::Statement;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub name: String,
    pub parameters: Vec<StateParameter>,
    pub return_type: Option<crate::types::TypeReference>,
    pub statements: Vec<Statement>,
}
