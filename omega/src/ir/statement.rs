use crate::ir::expression::Expression;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    CommandCall(CommandCall),
    Transition(Transition),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandCall {
    pub receiver: String,
    pub command: String,
    pub arguments: Vec<Expression>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transition {
    pub target: TransitionTarget,
    pub continuation: Option<TransitionTarget>,
    pub condition: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionTarget {
    Named(Vec<String>),
    SelfTarget,
    ReturnToCaller,
}
