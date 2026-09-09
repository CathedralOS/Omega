//! Optional loop-header assertions and the complete incoming-edge proof roster.
//!
//! Assertions belong to the semantic question, not the choice of certificate.
//! Every arrival must establish the asserted range under exact successor
//! substitution before the module can grant execution authority. Ranking is
//! independent: a terminating loop can still violate a scalar safety bound.

use semantic_vocabulary::{BlockId, BoundedIntegerType, EdgeId, MachineId, ObligationId, ValueId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarRangeInvariant {
    pub machine: MachineId,
    pub header: BlockId,
    pub parameter: ValueId,
    pub bounds: BoundedIntegerType,
    /// Strictly edge-ordered, exhaustive incoming arrivals, including entry.
    pub arrivals: Vec<ScalarRangeInvariantArrival>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarRangeInvariantArrival {
    pub edge: EdgeId,
    pub obligation: ObligationId,
}
