//! Terminal Unit-return and scalar-Crash receipts.

use psi_core::{ClaimId, EdgeId, MachineId, ObligationId, OperationId, ScalarType, ServiceId};
use psi_terminal::{
    CrashCause, CrashPredicateTerm, CrashRouteBucket, StructuralPlaceDeclaration,
    StructuralTypeDeclaration,
};

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
pub struct StraightLineUnitCallReturnTranslationReceipt {
    machine: MachineId,
    call_operation: OperationId,
    callee: MachineId,
    requirement_obligations: Vec<ObligationId>,
    crash_continuations: Vec<CrashRouteBucket>,
    return_edge: EdgeId,
}

impl StraightLineUnitCallReturnTranslationReceipt {
    pub(in crate::validation) fn new(
        machine: MachineId,
        call_operation: OperationId,
        callee: MachineId,
        requirement_obligations: Vec<ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
        return_edge: EdgeId,
    ) -> Self {
        Self {
            machine,
            call_operation,
            callee,
            requirement_obligations,
            crash_continuations,
            return_edge,
        }
    }

    pub const fn machine(&self) -> MachineId {
        self.machine
    }

    pub const fn call_operation(&self) -> OperationId {
        self.call_operation
    }

    pub const fn callee(&self) -> MachineId {
        self.callee
    }

    pub fn requirement_obligations(&self) -> &[ObligationId] {
        &self.requirement_obligations
    }

    pub fn crash_continuations(&self) -> &[CrashRouteBucket] {
        &self.crash_continuations
    }

    pub const fn return_edge(&self) -> EdgeId {
        self.return_edge
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StraightLineByteSequenceLiteralUnitReturnTranslationReceipt {
    machine: MachineId,
    establishment_operation: OperationId,
    place: StructuralPlaceDeclaration,
    structural_type: StructuralTypeDeclaration,
    bytes: Vec<u8>,
    return_edge: EdgeId,
}

impl StraightLineByteSequenceLiteralUnitReturnTranslationReceipt {
    pub(in crate::validation) fn new(
        machine: MachineId,
        establishment_operation: OperationId,
        place: StructuralPlaceDeclaration,
        structural_type: StructuralTypeDeclaration,
        bytes: Vec<u8>,
        return_edge: EdgeId,
    ) -> Self {
        Self {
            machine,
            establishment_operation,
            place,
            structural_type,
            bytes,
            return_edge,
        }
    }

    pub const fn machine(&self) -> MachineId {
        self.machine
    }

    pub const fn establishment_operation(&self) -> OperationId {
        self.establishment_operation
    }

    pub const fn place(&self) -> &StructuralPlaceDeclaration {
        &self.place
    }

    pub const fn structural_type(&self) -> &StructuralTypeDeclaration {
        &self.structural_type
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn return_edge(&self) -> EdgeId {
        self.return_edge
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StraightLineTrivialAffineLocalUnitReturnTranslationReceipt {
    machine: MachineId,
    establishment_operation: OperationId,
    place: StructuralPlaceDeclaration,
    structural_type: StructuralTypeDeclaration,
    return_edge: EdgeId,
}

impl StraightLineTrivialAffineLocalUnitReturnTranslationReceipt {
    pub(in crate::validation) fn new(
        machine: MachineId,
        establishment_operation: OperationId,
        place: StructuralPlaceDeclaration,
        structural_type: StructuralTypeDeclaration,
        return_edge: EdgeId,
    ) -> Self {
        Self {
            machine,
            establishment_operation,
            place,
            structural_type,
            return_edge,
        }
    }

    pub const fn machine(&self) -> MachineId {
        self.machine
    }

    pub const fn establishment_operation(&self) -> OperationId {
        self.establishment_operation
    }

    pub const fn place(&self) -> &StructuralPlaceDeclaration {
        &self.place
    }

    pub const fn structural_type(&self) -> &StructuralTypeDeclaration {
        &self.structural_type
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
