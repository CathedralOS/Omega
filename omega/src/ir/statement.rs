use crate::ir::expression::Expression;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    CommandCall(CommandCall),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandCall {
    pub receiver: String,
    pub command: String,
    pub arguments: Vec<Expression>,
}
