//! Scalar value categories and exact literal bits.

use crate::{TargetBooleanExpression, TargetIntegerExpression};
use psi_core::{IntegerType, IntegerValue, ScalarType};

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
    Boolean(TargetBooleanExpression),
    Integer {
        scalar_type: IntegerType,
        expression: TargetIntegerExpression,
    },
}

impl TargetScalarExpression {
    pub const fn scalar_type(&self) -> ScalarType {
        match self {
            Self::Boolean(_) => ScalarType::Boolean,
            Self::Integer { scalar_type, .. } => ScalarType::Integer(*scalar_type),
        }
    }
}
