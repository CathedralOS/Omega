//! Scalar block predicates and the complete incoming-edge proof roster.
//!
//! Assertions belong to the semantic question, not the choice of certificate.
//! Every arrival must establish the same predicate under exact successor
//! substitution before the module can grant execution authority. Ranking is
//! independent: a terminating loop can still violate a scalar safety bound.

use semantic_vocabulary::{BlockId, EdgeId, MachineId, ObligationId, Proposition};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarBlockInvariant {
    pub machine: MachineId,
    pub header: BlockId,
    /// One predicate per block, over its scalar parameters and immutable machine
    /// formals only. Multiple assertions use an ordinary conjunction. Storage
    /// observations and branch-local identities cannot acquire join scope.
    pub predicate: Proposition,
    /// Strictly edge-ordered, exhaustive incoming arrivals, including entry.
    pub arrivals: Vec<ScalarBlockInvariantArrival>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarBlockInvariantArrival {
    pub edge: EdgeId,
    pub obligation: ObligationId,
}
