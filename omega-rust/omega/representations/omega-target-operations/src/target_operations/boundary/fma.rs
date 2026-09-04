//! Target-selected float mechanism and exact operand provenance.

use psi_core::{IeeeFloatFormat, IeeeFloatValue, OperationId, ValueId};

/// Exact selected-plan and deployment custody for one target-lowered scalar
/// x86 FMA occurrence.
///
/// The collision-resistant plan digest binds this row back to the complete
/// selected provider plan retained by native realization. The compact report
/// identity remains diagnostic data and cannot authorize lowering by itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetX86ScalarFmaSettlement {
    pub terminal_operation: OperationId,
    pub provider_plan_report_identity: u64,
    pub provider_plan_digest: [u8; 32],
    pub format: IeeeFloatFormat,
    pub slot: omega_target::X86ScalarFmaSlot,
    pub provider: omega_target::AdmittedX86ScalarFmaProvider,
}

/// One exact preceding Terminal IEEE constant consumed by a bounded scalar
/// FMA. Keeping the defining operation and value identity with the raw bits
/// prevents a same-valued or same-typed constant from being substituted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetIeeeFloatFmaOperand {
    pub defining_operation: OperationId,
    pub source_value: ValueId,
    pub value: IeeeFloatValue,
}
