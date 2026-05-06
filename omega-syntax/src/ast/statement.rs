#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    Assignment(Assignment),
    Call(Call),
    LocalData(LocalData),
    Transition(Transition),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assignment {
    pub target: crate::ast::expression::Expression,
    pub value: crate::ast::expression::Expression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalData {
    pub name: String,
    pub type_reference: crate::ast::types::TypeReference,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    pub receiver: Option<String>,
    pub target: String,
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
    Terminal,
}
