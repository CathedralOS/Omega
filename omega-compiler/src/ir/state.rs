use crate::ir::statement::Statement;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub name: String,
    pub statements: Vec<Statement>,
}
