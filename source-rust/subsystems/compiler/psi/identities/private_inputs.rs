//! Facts inferred from private callers belong to the invocation whose calls proved them.
use semantic_vocabulary::{InvocationSite, MachineId, ObligationId, Proposition};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InvocationPrivateInputs {
    clauses: Vec<(MachineId, u32, Proposition)>,
    calls: Vec<(MachineId, InvocationSite, MachineId, u32, ObligationId)>,
}

impl InvocationPrivateInputs {
    #[doc(hidden)]
    pub fn from_checked(
        mut clauses: Vec<(MachineId, u32, Proposition)>,
        mut calls: Vec<(MachineId, InvocationSite, MachineId, u32, ObligationId)>,
    ) -> Result<Self, &'static str> {
        clauses.sort_unstable_by_key(|row| (row.0, row.1));
        if clauses
            .windows(2)
            .any(|pair| (pair[0].0, pair[0].1) == (pair[1].0, pair[1].1))
        {
            return Err("a private input has competing checked origins");
        }
        calls.sort_unstable();
        if calls.windows(2).any(|pair| {
            let (caller, site, _, clause, _) = pair[0];
            let (next, at, _, ordinal, _) = pair[1];
            caller == next && site == at && clause == ordinal
        }) {
            return Err("a private call input was recorded twice");
        }
        Ok(Self { clauses, calls })
    }

    pub(super) fn is_empty(&self) -> bool {
        self.clauses.is_empty() && self.calls.is_empty()
    }

    pub(super) fn clause(&self, callee: MachineId, clause: usize) -> Option<&Proposition> {
        let ordinal = u32::try_from(clause).ok()?;
        let index = self
            .clauses
            .binary_search_by_key(&(callee, ordinal), |row| (row.0, row.1))
            .ok()?;
        Some(&self.clauses[index].2)
    }

    pub(super) fn proved(
        &self,
        caller: MachineId,
        site: InvocationSite,
        callee: MachineId,
        clause: usize,
        obligation: ObligationId,
    ) -> bool {
        u32::try_from(clause).is_ok_and(|ordinal| {
            self.calls
                .binary_search(&(caller, site, callee, ordinal, obligation))
                .is_ok()
        })
    }
}
