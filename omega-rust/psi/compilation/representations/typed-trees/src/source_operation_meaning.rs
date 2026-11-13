//! Compare source operations before mathematical declarations share a binding.
//! Every operand keeps its resolved value, scalar policy or embedding carrier.

use crate::expression::{BinaryOperator, ExpressionHandle};
use crate::types::{PrimitiveType, TypeReferenceHandle};
use numerics::arithmetic::ArithmeticDomain;
use numerics::bignum::BigInt;
use numerics::literals::FloatLiteral;
use symbols::SymbolHandle;

/// Source values retain observations while projections and recasts normalize.
/// Only complete scalar meanings can share a mathematical source binding.
#[derive(Clone, PartialEq)]
pub enum SourceOperation {
    BoundValue(SymbolHandle),
    /// Retain a formed source occurrence while a surrounding projection reduces.
    /// This value cannot itself supply a shared mathematical binding.
    Unshared(ExpressionHandle),
    Integer(BigInt),
    Float(FloatLiteral),
    ScalarBinary {
        operator: BinaryOperator,
        meaning: [(PrimitiveType, ArithmeticDomain); 2],
        left: Box<Self>,
        right: Box<Self>,
    },
    MathematicalBinary {
        operator: BinaryOperator,
        left: Box<Self>,
        right: Box<Self>,
    },
    IntegerEmbedding {
        reference: TypeReferenceHandle,
        value: Box<Self>,
    },
    Observed {
        reference: TypeReferenceHandle,
        value: Box<Self>,
    },
    Representation {
        source: PrimitiveType,
        target: PrimitiveType,
        value: Box<Self>,
    },
    RepeatedArray {
        length: u64,
        element: Option<Box<Self>>,
    },
    Constructor {
        carrier: crate::type_identity::TypeIdentityId,
        case: SymbolHandle,
        fields: Vec<(SymbolHandle, Self)>,
    },
}

impl SourceOperation {
    pub fn is_complete(&self) -> bool {
        match self {
            Self::BoundValue(symbol) => symbol.is_valid(),
            Self::Integer(_) | Self::Float(_) => true,
            Self::ScalarBinary { left, right, .. }
            | Self::MathematicalBinary { left, right, .. } => {
                left.is_complete() && right.is_complete()
            }
            Self::IntegerEmbedding { value, .. } => value.is_complete(),
            Self::Unshared(_)
            | Self::Observed { .. }
            | Self::Representation { .. }
            | Self::Constructor { .. }
            | Self::RepeatedArray { .. } => false,
        }
    }

    pub fn semantic_value(&self) -> &Self {
        match self {
            Self::Observed { value, .. } => value.semantic_value(),
            _ => self,
        }
    }

    pub fn boolean(value: bool) -> Self {
        Self::Integer(BigInt::from_i64(i64::from(value)))
    }
}

/// The supplying source phase resolves the declaration's parameter telescope,
/// selected operators and prepared mathematical applications. This reader has
/// no body hypotheses or proof verdict to supply.
pub trait SourceOperationReader {
    fn read(
        &self,
        expression: ExpressionHandle,
        operand: &dyn Fn(ExpressionHandle) -> Option<SourceOperation>,
    ) -> Option<SourceOperation>;
}

pub fn read_operation(
    reader: &dyn SourceOperationReader,
    expression: ExpressionHandle,
) -> Option<SourceOperation> {
    reader.read(expression, &|operand| read_operation(reader, operand))
}

pub fn same_meaning(
    reader: &dyn SourceOperationReader,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    matches!(
        (read_operation(reader, left), read_operation(reader, right)),
        (Some(left), Some(right)) if left.is_complete() && right.is_complete() && left == right
    )
}
