use crate::{
    CheckFacts, ContractOperatorUseFact, FlowCallFact, FlowExitFact, FlowStateFact,
    FlowStatementFact,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateOperationAcceptanceKind {
    Statement,
    Call,
    Exit,
    Operator,
}

#[derive(Debug, Clone, Copy)]
pub struct StateAcceptance<'facts> {
    pub(crate) facts: &'facts CheckFacts,
    pub(crate) state: &'facts FlowStateFact,
}

#[derive(Debug, Clone, Copy)]
pub struct StatementAcceptance<'facts> {
    pub(crate) facts: &'facts CheckFacts,
    pub(crate) state: &'facts FlowStateFact,
    pub(crate) statement: &'facts FlowStatementFact,
}

#[derive(Debug, Clone, Copy)]
pub struct CallAcceptance<'facts> {
    pub(crate) facts: &'facts CheckFacts,
    pub(crate) call: &'facts FlowCallFact,
}

#[derive(Debug, Clone, Copy)]
pub struct ExitAcceptance<'facts> {
    pub(crate) facts: &'facts CheckFacts,
    pub(crate) exit: &'facts FlowExitFact,
}

#[derive(Debug, Clone, Copy)]
pub struct OperatorAcceptance<'facts> {
    pub(crate) facts: &'facts CheckFacts,
    pub(crate) operator_use: &'facts ContractOperatorUseFact,
}

#[derive(Debug, Clone, Copy)]
pub enum StateOperationAcceptance<'facts> {
    Statement(StatementAcceptance<'facts>),
    Call(CallAcceptance<'facts>),
    Exit(ExitAcceptance<'facts>),
    Operator(OperatorAcceptance<'facts>),
}
