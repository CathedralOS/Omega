#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    CommandCall(CommandCall),
    Transition(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandCall {
    pub receiver: String,
    pub command: String,
    pub arguments: Vec<crate::ast::expression::Expression>,
}
