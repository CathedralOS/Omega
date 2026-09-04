//! Minimal operation shape shared by dead-scalar producer leaves.

use psi_core::{OperationId, ScalarType, ValueId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::rules::passes) struct DeadScalarShape {
    pub(in crate::rules::passes) source_operation: OperationId,
    pub(in crate::rules::passes) result: ValueId,
    pub(in crate::rules::passes) scalar_type: ScalarType,
}
