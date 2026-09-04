//! Boundary invocation arguments and their settlement binding.

use crate::{BoundaryExecutionBinding, BoundarySettlementRealization};
use omega_calling_conventions::MachineRegister;
use psi_core::{BoundaryMachineId, IntegerValue, OperationId, ScalarType, ValueId};
use psi_terminal::{StructuralArgument, StructuralTypeDeclaration};

/// Exact scalar value consumed by a native boundary realization.
///
/// The value identity and type bind this row back to terminal Psi; the
/// immediate and destination register make the emitted provider interval
/// independently replayable. This is deliberately separate from structural
/// settlement custody and from a machine result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundaryScalarArgument {
    pub source_value: ValueId,
    pub scalar_type: ScalarType,
    pub immediate: IntegerValue,
    pub destination: MachineRegister,
}

/// Exact structural source consumed by one native byte-sequence boundary.
/// The literal operation and declaration bind the payload back to terminal
/// Psi independently of target byte placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryByteSequenceArgument {
    pub argument: StructuralArgument,
    pub literal_operation: OperationId,
    pub structural_type: StructuralTypeDeclaration,
    pub bytes: Vec<u8>,
}

/// The first boundary realization is metadata-only: an exact selected
/// provider execution settles the claim, while the preceding semantic effect
/// (for example `PortWrite`) performs the hardware operation. No code is
/// silently erased; this row must survive installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundarySettlementBinding {
    pub boundary: BoundaryMachineId,
    pub execution: BoundaryExecutionBinding,
    pub realization: BoundarySettlementRealization,
}
