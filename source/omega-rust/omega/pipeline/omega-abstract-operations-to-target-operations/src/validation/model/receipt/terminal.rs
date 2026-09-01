//! Terminal Unit-return and scalar-Crash receipts.

use psi_core::{ClaimId, EdgeId, MachineId, OperationId, ScalarType, ServiceId};
use psi_terminal::{CrashCause, CrashPredicateTerm};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StraightLineUnitReturnTranslationReceipt {
    machine: MachineId,
    return_edge: EdgeId,
}

impl StraightLineUnitReturnTranslationReceipt {
    pub(in crate::validation) const fn new(machine: MachineId, return_edge: EdgeId) -> Self {
        Self {
            machine,
            return_edge,
        }
    }

    pub const fn machine(&self) -> MachineId {
        self.machine
    }

    pub const fn return_edge(&self) -> EdgeId {
        self.return_edge
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StraightLinePortWriteUnitReturnTranslationReceipt {
    machine: MachineId,
    port_operation: OperationId,
    service: ServiceId,
    port: u16,
    value: u8,
    return_edge: EdgeId,
}

impl StraightLinePortWriteUnitReturnTranslationReceipt {
    pub(in crate::validation) const fn new(
        machine: MachineId,
        port_operation: OperationId,
        service: ServiceId,
        port: u16,
        value: u8,
        return_edge: EdgeId,
    ) -> Self {
        Self {
            machine,
            port_operation,
            service,
            port,
            value,
            return_edge,
        }
    }

    pub const fn machine(&self) -> MachineId {
        self.machine
    }

    pub const fn port_operation(&self) -> OperationId {
        self.port_operation
    }

    pub const fn service(&self) -> ServiceId {
        self.service
    }

    pub const fn port(&self) -> u16 {
        self.port
    }

    pub const fn value(&self) -> u8 {
        self.value
    }

    pub const fn return_edge(&self) -> EdgeId {
        self.return_edge
    }
}

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
