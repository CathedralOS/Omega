//! A result inferred by the checker is bound to the scalar each normal exit returns.
use semantic_vocabulary::{EdgeId, MachineId, ObligationId, ValueId};

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvocationPrivateResult {
    pub machine: MachineId,
    pub ordinal: u32,
    pub clause: crate::ContractClause,
    pub returns: Vec<(EdgeId, ValueId, ObligationId)>,
}

impl InvocationPrivateResult {
    pub(super) fn matches(&self, machine: &crate::TerminalMachine, ordinal: usize) -> bool {
        machine.ensures.get(ordinal) == Some(&self.clause)
            && !self.returns.is_empty()
            && machine
                .blocks
                .iter()
                .filter_map(|block| match block.terminator {
                    crate::Terminator::Return { edge, value, .. } => Some((edge, value)),
                    _ => None,
                })
                .eq(self.returns.iter().map(|&(edge, value, _)| (edge, value)))
    }
}
