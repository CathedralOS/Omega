use crate::ir::expression::Expression;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    Assignment(Assignment),
    Call(Call),
    LocalData(LocalData),
    Transition(Transition),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assignment {
    pub target: Expression,
    pub value: Expression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalData {
    pub name: String,
    pub type_reference: crate::ir::types::TypeReference,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    pub receiver: Option<String>,
    pub target: String,
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
    Terminal,
}
