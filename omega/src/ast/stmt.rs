#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    CommandCall(CommandCall),
    Transition(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandCall {
    pub receiver: String,
    pub command: String,
    pub args: Vec<crate::ast::expression::Expression>,
}
