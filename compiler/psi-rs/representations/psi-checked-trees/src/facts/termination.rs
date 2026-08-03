//! TPR3 slice 4 (decision 23): what the checker ESTABLISHED about each
//! claiming machine's termination -- the `checked_summary`'s producer, and
//! the completion of witness elaborations the lowering left pending. Local
//! DIRECT consumers may use the exact checked summary; anything through a
//! trait, import slot, or exported contract uses only the AUTHORED
//! guarantee (the plan's published interface) -- refactoring a body cannot
//! silently change what external callers may assume.

use psi_language_semantics::TerminationGuarantee;
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TerminationFacts {
    /// One local checked summary per machine, in machine-table order. Public
    /// consumers use `MachineContractPlan::termination` instead;
    /// recording an inferred local fact never publishes a promise.
    pub machines: Vec<MachineTerminationFact>,
}

impl TerminationFacts {
    pub fn for_machine(&self, machine: SymbolHandle) -> Option<&MachineTerminationFact> {
        self.machines.iter().find(|fact| fact.machine == machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineTerminationFact {
    pub machine: SymbolHandle,
    /// What the checker established for THIS body: `Terminates` for
    /// an acyclic claimant or a proven ranking witness; `NoGuarantee`
    /// otherwise (an unproven claimant also fails compilation, so a
    /// compiled artifact never carries an unestablished claim).
    pub checked_summary: TerminationGuarantee,
    /// The RESOLVED ranking view's explicit spelling -- the canonical
    /// builtin path, or the authored declared-measure path. This completes
    /// a witness whose lowering-time elaboration stayed PENDING (the
    /// type-directed single-subject short form). Empty when the machine
    /// carries no witness.
    pub resolved_view_path: String,
}
