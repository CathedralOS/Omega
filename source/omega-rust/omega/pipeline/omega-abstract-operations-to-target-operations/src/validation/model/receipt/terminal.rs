//! Terminal scalar-Crash receipt.

use psi_core::{ClaimId, EdgeId, MachineId, ScalarType};
use psi_terminal::{CrashCause, CrashPredicateTerm};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StraightLineScalarCrashTranslationReceipt {
    machine: MachineId,
    result_type: ScalarType,
    crash_edge: EdgeId,
    cause: CrashCause,
    site_guard: Vec<CrashPredicateTerm>,
    frontier_lower_bound: Vec<ClaimId>,
}

impl StraightLineScalarCrashTranslationReceipt {
    pub(in crate::validation) fn new(
        machine: MachineId,
        result_type: ScalarType,
        crash_edge: EdgeId,
        cause: CrashCause,
        site_guard: Vec<CrashPredicateTerm>,
        frontier_lower_bound: Vec<ClaimId>,
    ) -> Self {
        Self {
            machine,
            result_type,
            crash_edge,
            cause,
            site_guard,
            frontier_lower_bound,
        }
    }

    pub const fn machine(&self) -> MachineId {
        self.machine
    }
    pub const fn result_type(&self) -> ScalarType {
        self.result_type
    }
    pub const fn crash_edge(&self) -> EdgeId {
        self.crash_edge
    }
    pub const fn cause(&self) -> CrashCause {
        self.cause
    }
    pub fn site_guard(&self) -> &[CrashPredicateTerm] {
        &self.site_guard
    }
    pub fn frontier_lower_bound(&self) -> &[ClaimId] {
        &self.frontier_lower_bound
    }
}
