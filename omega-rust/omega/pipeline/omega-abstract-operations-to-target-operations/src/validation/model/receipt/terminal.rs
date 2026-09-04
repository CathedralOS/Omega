//! Terminal Unit-return and scalar-Crash receipts.

mod integer_ieee_float_literal_sequence;
mod integer_literal_sequence;

pub use integer_ieee_float_literal_sequence::{
    IntegerIeeeFloatLiteralSequenceMember,
    StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationReceipt,
};
pub use integer_literal_sequence::{
    IntegerLiteralSequenceMember, StraightLineIntegerLiteralSequenceUnitReturnTranslationReceipt,
};

use psi_core::{
    ClaimId, EdgeId, IeeeFloatValue, IntegerType, IntegerValue, MachineId, ObligationId,
    OperationId, ScalarType, ServiceId, ValueId,
};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StraightLineIntegerLiteralUnitReturnTranslationReceipt {
    machine: MachineId,
    literal_operation: OperationId,
    literal_result: ValueId,
    scalar_type: IntegerType,
    value: IntegerValue,
    return_edge: EdgeId,
}

impl StraightLineIntegerLiteralUnitReturnTranslationReceipt {
    pub(in crate::validation) const fn new(
        machine: MachineId,
        literal_operation: OperationId,
        literal_result: ValueId,
        scalar_type: IntegerType,
        value: IntegerValue,
        return_edge: EdgeId,
    ) -> Self {
        Self {
            machine,
            literal_operation,
            literal_result,
            scalar_type,
            value,
            return_edge,
        }
    }

    pub const fn machine(&self) -> MachineId {
        self.machine
    }

    pub const fn literal_operation(&self) -> OperationId {
        self.literal_operation
    }

    pub const fn literal_result(&self) -> ValueId {
        self.literal_result
    }

    pub const fn scalar_type(&self) -> IntegerType {
        self.scalar_type
    }

    pub const fn value(&self) -> IntegerValue {
        self.value
    }

    pub const fn return_edge(&self) -> EdgeId {
        self.return_edge
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StraightLineIeeeFloatLiteralUnitReturnTranslationReceipt {
    machine: MachineId,
    literal_operation: OperationId,
    literal_result: ValueId,
    value: IeeeFloatValue,
    return_edge: EdgeId,
}

impl StraightLineIeeeFloatLiteralUnitReturnTranslationReceipt {
    pub(in crate::validation) const fn new(
        machine: MachineId,
        literal_operation: OperationId,
        literal_result: ValueId,
        value: IeeeFloatValue,
        return_edge: EdgeId,
    ) -> Self {
        Self {
            machine,
            literal_operation,
            literal_result,
            value,
            return_edge,
        }
    }

    pub const fn machine(&self) -> MachineId {
        self.machine
    }

    pub const fn literal_operation(&self) -> OperationId {
        self.literal_operation
    }

    pub const fn literal_result(&self) -> ValueId {
        self.literal_result
    }

    pub const fn value(&self) -> IeeeFloatValue {
        self.value
    }

    pub const fn return_edge(&self) -> EdgeId {
        self.return_edge
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IeeeFloatLiteralSequenceMember {
    operation: OperationId,
    result: ValueId,
    value: IeeeFloatValue,
}

impl IeeeFloatLiteralSequenceMember {
    pub(in crate::validation) const fn new(
        operation: OperationId,
        result: ValueId,
        value: IeeeFloatValue,
    ) -> Self {
        Self {
            operation,
            result,
            value,
        }
    }

    pub const fn operation(&self) -> OperationId {
        self.operation
    }

    pub const fn result(&self) -> ValueId {
        self.result
    }

    pub const fn value(&self) -> IeeeFloatValue {
        self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StraightLineIeeeFloatLiteralSequenceUnitReturnTranslationReceipt {
    machine: MachineId,
    literals: Vec<IeeeFloatLiteralSequenceMember>,
    return_edge: EdgeId,
}

impl StraightLineIeeeFloatLiteralSequenceUnitReturnTranslationReceipt {
    pub(in crate::validation) fn new(
        machine: MachineId,
        literals: Vec<IeeeFloatLiteralSequenceMember>,
        return_edge: EdgeId,
    ) -> Self {
        Self {
            machine,
            literals,
            return_edge,
        }
    }

    pub const fn machine(&self) -> MachineId {
        self.machine
    }

    pub fn literals(&self) -> &[IeeeFloatLiteralSequenceMember] {
        &self.literals
    }

    pub const fn return_edge(&self) -> EdgeId {
        self.return_edge
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IeeeFloatFusedMultiplyAddOperandReceipt {
    defining_operation: OperationId,
    source_value: ValueId,
    value: IeeeFloatValue,
}

impl IeeeFloatFusedMultiplyAddOperandReceipt {
    pub(in crate::validation) const fn new(
        defining_operation: OperationId,
        source_value: ValueId,
        value: IeeeFloatValue,
    ) -> Self {
        Self {
            defining_operation,
            source_value,
            value,
        }
    }

    pub const fn defining_operation(&self) -> OperationId {
        self.defining_operation
    }

    pub const fn source_value(&self) -> ValueId {
        self.source_value
    }

    pub const fn value(&self) -> IeeeFloatValue {
        self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationReceipt {
    machine: MachineId,
    literals: [IeeeFloatLiteralSequenceMember; 3],
    fma_operation: OperationId,
    fma_result: ValueId,
    format: psi_core::IeeeFloatFormat,
    operands: [IeeeFloatFusedMultiplyAddOperandReceipt; 3],
    provider_plan_report_identity: u64,
    provider_plan_digest: [u8; 32],
    slot: omega_target::X86ScalarFmaSlot,
    provider: omega_target::AdmittedX86ScalarFmaProvider,
    return_edge: EdgeId,
}

impl StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationReceipt {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::validation) const fn new(
        machine: MachineId,
        literals: [IeeeFloatLiteralSequenceMember; 3],
        fma_operation: OperationId,
        fma_result: ValueId,
        format: psi_core::IeeeFloatFormat,
        operands: [IeeeFloatFusedMultiplyAddOperandReceipt; 3],
        provider_plan_report_identity: u64,
        provider_plan_digest: [u8; 32],
        slot: omega_target::X86ScalarFmaSlot,
        provider: omega_target::AdmittedX86ScalarFmaProvider,
        return_edge: EdgeId,
    ) -> Self {
        Self {
            machine,
            literals,
            fma_operation,
            fma_result,
            format,
            operands,
            provider_plan_report_identity,
            provider_plan_digest,
            slot,
            provider,
            return_edge,
        }
    }

    pub const fn machine(&self) -> MachineId {
        self.machine
    }

    pub const fn literals(&self) -> &[IeeeFloatLiteralSequenceMember; 3] {
        &self.literals
    }

    pub const fn fma_operation(&self) -> OperationId {
        self.fma_operation
    }

    pub const fn fma_result(&self) -> ValueId {
        self.fma_result
    }

    pub const fn format(&self) -> psi_core::IeeeFloatFormat {
        self.format
    }

    pub const fn operands(&self) -> &[IeeeFloatFusedMultiplyAddOperandReceipt; 3] {
        &self.operands
    }

    pub const fn provider_plan_report_identity(&self) -> u64 {
        self.provider_plan_report_identity
    }

    pub const fn provider_plan_digest(&self) -> [u8; 32] {
        self.provider_plan_digest
    }

    pub const fn slot(&self) -> omega_target::X86ScalarFmaSlot {
        self.slot
    }

    pub const fn provider(&self) -> omega_target::AdmittedX86ScalarFmaProvider {
        self.provider
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
