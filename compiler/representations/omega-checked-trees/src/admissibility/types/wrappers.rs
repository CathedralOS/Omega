use crate::{CheckFacts, FlowCallFact, FlowExitFact, FlowStateFact, FlowStatementFact};

#[derive(Debug, Clone, Copy)]
pub struct StateAcceptance<'facts> {
    pub(crate) facts: &'facts CheckFacts,
    pub(crate) state: &'facts FlowStateFact,
}

#[derive(Debug, Clone, Copy)]
pub struct StatementAcceptance<'facts> {
    pub(crate) facts: &'facts CheckFacts,
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
