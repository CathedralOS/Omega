#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    Assignment(Assignment),
    CommandCall(CommandCall),
    Transition(Transition),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assignment {
    pub target: Vec<String>,
    pub value: crate::ast::expression::Expression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandCall {
    pub receiver: Option<String>,
    pub command: String,
    pub arguments: Vec<crate::ast::expression::Expression>,
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
