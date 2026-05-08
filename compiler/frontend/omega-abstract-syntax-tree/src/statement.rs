use crate::identifier::{Identifier, IdentifierPath};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    Assignment(Assignment),
    Call(Call),
    Expression(crate::expression::Expression),
    LocalData(LocalData),
    Transition(Transition),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assignment {
    pub target: crate::expression::Expression,
    pub value: crate::expression::Expression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalData {
    pub name: Identifier,
    pub type_reference: crate::types::TypeReference,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    pub receiver: Option<Identifier>,
    pub target: Identifier,
    pub arguments: Vec<crate::expression::Expression>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transition {
    pub target: TransitionTarget,
    pub continuation: Option<TransitionTarget>,
    pub guard: TransitionGuard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionGuard {
    Always,
    When(crate::expression::Expression),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionTarget {
    Named {
        path: IdentifierPath,
        arguments: Vec<crate::expression::Expression>,
    },
    SelfTarget,
    Terminal,
}
