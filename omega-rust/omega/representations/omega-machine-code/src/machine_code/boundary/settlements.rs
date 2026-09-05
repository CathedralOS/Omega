//! Boundary arguments, result roles, and exact settlement occurrences.

use crate::{
    BoundaryExecutionRecord, CompletionProviderCustodyBinding, ForeignCallScalarArgumentRecord,
};
use omega_calling_conventions::{ConventionalSumLayout, ValuePlacement};
use omega_target_operations::{BoundaryRealization, BoundaryScalarArgument, CompletionClaimSource};
use psi_core::{BoundaryMachineId, EdgeId, OperationId, ScalarType, ValueId};
use psi_terminal::{
    CompletionReceipt, StructuralArgument, StructuralOperationResult, StructuralTypeDeclaration,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryScalarResultRecord {
    pub value: ValueId,
    pub scalar_type: ScalarType,
    pub placement: ValuePlacement,
    /// Exact terminal edge that returns this boundary-produced value.
    pub return_edge: EdgeId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryStructuralResultRecord {
    pub defining_operation: OperationId,
    pub result: StructuralOperationResult,
    pub layout: ConventionalSumLayout,
    /// Offset within the complete allocated Unit frame.
    pub home_byte_offset: u32,
}

/// Closed physical result role for one emitted boundary settlement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryResultRecord {
    Unit,
    Scalar(BoundaryScalarResultRecord),
    Structural(BoundaryStructuralResultRecord),
}

impl BoundaryResultRecord {
    pub const fn is_unit(&self) -> bool {
        matches!(self, Self::Unit)
    }

    pub const fn scalar(&self) -> Option<&BoundaryScalarResultRecord> {
        match self {
            Self::Scalar(result) => Some(result),
            Self::Unit | Self::Structural(_) => None,
        }
    }

    pub const fn structural(&self) -> Option<&BoundaryStructuralResultRecord> {
        match self {
            Self::Structural(result) => Some(result),
            Self::Unit | Self::Scalar(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundarySettlementRecord {
    pub psi_operation: OperationId,
    pub boundary: BoundaryMachineId,
    pub execution: BoundaryExecutionRecord,
    pub realization: BoundaryRealization,
    /// Ordered scalar inputs consumed by executable boundary code. Metadata-
    /// only settlements retain an empty list; native realizations retain the
    /// exact terminal value, type, immediate, and ABI destination.
    pub scalar_arguments: Vec<BoundaryScalarArgument>,
    /// Exact assigned source and emitted materialization for returning
    /// compiler-builtin scalar inputs.
    pub runtime_scalar_arguments: Vec<ForeignCallScalarArgumentRecord>,
    /// Exact typed Psi custody arguments, including structural projections.
    /// These are provider-settlement evidence and do not describe an internal
    /// Unit-call ABI.
    pub arguments: Vec<StructuralArgument>,
    /// Exact native byte-sequence arguments and their independently replayable
    /// semantic and physical custody.
    pub byte_sequence_arguments: Vec<BoundaryByteSequenceArgumentRecord>,
    /// Complete canonical caller claim-source catalog needed to independently
    /// reconstruct the exact successful-completion receipt set.
    pub completion_claim_sources: Vec<CompletionClaimSource>,
    pub completion_receipts: Vec<CompletionReceipt>,
    /// Exact structural join between each successful-completion receipt, its
    /// retained caller source, and the admitted provider execution that owns
    /// this settlement. This is custody evidence only: it does not authorize
    /// content introduction, prove backing, or derive residual geometry.
    pub completion_provider_custody: Vec<CompletionProviderCustodyBinding>,
    /// Exact closed physical result role. Unit settlements cannot manufacture
    /// a scalar or structural result after lowering.
    pub native_result: BoundaryResultRecord,
    /// Position in the verified Unit operation sequence. This remains the
    /// canonical tie-break when multiple metadata rows share a code offset.
    pub operation_ordinal: usize,
    /// Byte offset immediately after all preceding executable operations.
    pub code_offset: usize,
    /// Zero for metadata-only settlements; otherwise the exact provider
    /// instruction interval that produces the boundary result.
    pub byte_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryByteSequenceArgumentRecord {
    pub argument: StructuralArgument,
    pub literal_operation: OperationId,
    pub structural_type: StructuralTypeDeclaration,
    pub bytes: Vec<u8>,
    /// Function-relative executable interval that consumes this payload.
    pub code_offset: usize,
    pub code_byte_count: usize,
    /// Function-relative exact literal-plus-newline interval.
    pub data_offset: usize,
    pub data_byte_count: usize,
}
