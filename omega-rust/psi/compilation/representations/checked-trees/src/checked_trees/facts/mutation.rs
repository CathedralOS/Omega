//! Exact body-derived mutation summaries. These frames are implementation
//! evidence and remain independent from the published machine contract.

use arena::Arena;
use symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MutationFacts {
    /// One entry per checked machine, in machine-table order.
    pub machines: Vec<MachineMutationFact>,
    /// Every directly authored integer assignment, in source occurrence order.
    /// Calls compose these rows separately; this is not a transitive write frame.
    pub integer_writes: Arena<PerformedIntegerWrite>,
}

/// One performed write before its destination is invalidated. Distinct source
/// occurrences remain distinct even when they target the same place or a later
/// write restores the entry value. Missing value provenance is unproven, never
/// an empty effect; opaque destination frames remain explicit rows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PerformedIntegerWrite {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub statement_index: usize,
    pub destination: typed_trees::expression::ExpressionHandle,
    pub value: typed_trees::expression::ExpressionHandle,
    pub primitive_type: Option<typed_trees::types::PrimitiveType>,
    pub frame: facts::NormalizedWriteFrame,
    pub integer_range: Option<crate::CheckedIntegerRange>,
    /// A relation to unchanged invocation-entry inputs, not a reread of locals.
    pub entry_value: Option<crate::CheckedScalarExpression>,
    /// Path selecting this occurrence, in the same invocation namespace.
    pub entry_condition: Option<crate::CheckedBooleanExpression>,
}

impl MutationFacts {
    pub fn for_machine(&self, machine: SymbolHandle) -> Option<&MachineMutationFact> {
        self.machines.iter().find(|fact| fact.machine == machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineMutationFact {
    pub machine: SymbolHandle,
    /// Exact state-symbol frames in that machine's state-table order.
    pub state_write_frames: Vec<StateWriteFramePlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateWriteFramePlan {
    pub state: SymbolHandle,
    pub frame: facts::NormalizedWriteFrame,
}
