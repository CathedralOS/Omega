//! Scalar value categories and exact literal bits.

use crate::{TargetBooleanExpression, TargetIntegerExpression};
use semantic_vocabulary::{IntegerType, IntegerValue, ScalarType};

/// Exact immediate accepted by the bounded scalar structural-field store.
/// This is intentionally a closed carrier: widening it requires native and
/// replay rules for the new scalar family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetScalarImmediate {
    Boolean(bool),
    Integer {
        scalar_type: IntegerType,
        value: IntegerValue,
    },
}

impl TargetScalarImmediate {
    pub const fn scalar_type(self) -> ScalarType {
        match self {
            Self::Boolean(_) => ScalarType::Boolean,
            Self::Integer { scalar_type, .. } => ScalarType::Integer(scalar_type),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetScalarExpression {
    IeeeFloat(crate::TargetUnitScalarArgumentSource),
    /// A once-only float observation retains the same descriptor and bound
    /// coordinates as an integer element read, with its own declared format.
    IeeeFloatElementViewRead {
        format: semantic_vocabulary::IeeeFloatFormat,
        psi_operation: semantic_vocabulary::OperationId,
        source_value: semantic_vocabulary::ValueId,
        source: semantic_vocabulary::PlaceId,
        view: Box<crate::TargetElementView>,
        index: Box<TargetIntegerExpression>,
        length: semantic_vocabulary::ValueId,
        obligation: semantic_vocabulary::ObligationId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        element_byte_offset: u32,
    },
    Boolean(TargetBooleanExpression),
    Integer {
        scalar_type: IntegerType,
        expression: TargetIntegerExpression,
    },
}

impl TargetScalarExpression {
    pub const fn scalar_type(&self) -> ScalarType {
        match self {
            Self::IeeeFloat(source) => source.scalar_type(),
            Self::IeeeFloatElementViewRead { format, .. } => ScalarType::IeeeFloat(*format),
            Self::Boolean(_) => ScalarType::Boolean,
            Self::Integer { scalar_type, .. } => ScalarType::Integer(*scalar_type),
        }
    }
}
