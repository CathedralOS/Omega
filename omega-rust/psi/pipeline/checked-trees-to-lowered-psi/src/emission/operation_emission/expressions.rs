//! Prepared scalar expressions, qualification lookup, and ordered leaf emission.

use super::boolean::{LoweredBooleanReturnExpression, emit_boolean_expression};
use super::buffer::OperationBuffer;
use super::integer::LoweredIntegerBinaryKind;
use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;
use crate::terminal_identities::obligation_id;
use crate::terminal_identities::value_id;
use semantic_vocabulary::{
    IeeeFloatValue, IntegerSign, IntegerType, IntegerValue, PlaceId, QualifiedScalarType,
    ScalarType, StructuralFieldId, ValueId,
};
use terminal_psi::{Operation, OperationKind, OperationResult, ValueDeclaration};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoweredDirectExpression {
    PrimitiveRead {
        source: PlaceId,
        path: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
        scalar_type: ScalarType,
    },
    StructuralField {
        source: PlaceId,
        path: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
        field: StructuralFieldId,
        scalar_type: ScalarType,
    },
    ByteSequenceLength {
        source: PlaceId,
        scalar_type: ScalarType,
    },
    ByteSequenceFieldLength {
        source: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        field: StructuralFieldId,
        scalar_type: ScalarType,
    },
    ByteSequenceRead {
        source: PlaceId,
        index: Box<LoweredDirectExpression>,
        scalar_type: ScalarType,
    },
    Parameter {
        position: usize,
        scalar_type: ScalarType,
    },
    /// Proof-only reference to one erased formal in the enclosing contract
    /// roster. Runtime emission and evaluation reject this form; only scalar
    /// contract terms may carry it.
    ErasedParameter {
        position: usize,
        scalar_type: ScalarType,
    },
    Local {
        position: usize,
        scalar_type: ScalarType,
    },
    IntegerLiteral {
        value: IntegerValue,
        scalar_type: ScalarType,
    },
    IeeeFloatLiteral {
        value: IeeeFloatValue,
    },
    IntegerBinary {
        kind: LoweredIntegerBinaryKind,
        scalar_type: ScalarType,
        left: Box<LoweredDirectExpression>,
        right: Box<LoweredDirectExpression>,
    },
    IntegerBitwiseNot {
        scalar_type: ScalarType,
        operand: Box<LoweredDirectExpression>,
    },
    IntegerWiden {
        scalar_type: ScalarType,
        operand: Box<LoweredDirectExpression>,
    },
    IntegerExactCast {
        scalar_type: ScalarType,
        operand: Box<LoweredDirectExpression>,
    },
    Boolean {
        expression: Box<LoweredBooleanReturnExpression>,
    },
}

impl LoweredDirectExpression {
    pub(crate) fn value_type(
        &self,
        parameters: &[QualifiedScalarType],
    ) -> Result<QualifiedScalarType, LoweringError> {
        let position = match self {
            Self::Parameter { position, .. } | Self::Local { position, .. } => Some(*position),
            Self::Boolean { expression } => match expression.as_ref() {
                LoweredBooleanReturnExpression::Parameter { position }
                | LoweredBooleanReturnExpression::Local { position } => Some(*position),
                _ => None,
            },
            _ => None,
        };
        if let Some(position) = position {
            let value_type =
                parameters
                    .get(position)
                    .copied()
                    .ok_or(LoweringError::Unsupported(
                        "scalar value has no typed parameter",
                    ))?;
            if value_type.scalar_type != self.scalar_type() {
                return unsupported("scalar value carrier disagrees with its parameter");
            }
            Ok(value_type)
        } else {
            Ok(self.scalar_type().into())
        }
    }

    pub(crate) const fn scalar_type(&self) -> ScalarType {
        match self {
            Self::Parameter { scalar_type, .. }
            | Self::ErasedParameter { scalar_type, .. }
            | Self::PrimitiveRead { scalar_type, .. }
            | Self::StructuralField { scalar_type, .. }
            | Self::ByteSequenceLength { scalar_type, .. }
            | Self::ByteSequenceFieldLength { scalar_type, .. }
            | Self::ByteSequenceRead { scalar_type, .. }
            | Self::Local { scalar_type, .. }
            | Self::IntegerLiteral { scalar_type, .. }
            | Self::IntegerBinary { scalar_type, .. }
            | Self::IntegerBitwiseNot { scalar_type, .. }
            | Self::IntegerWiden { scalar_type, .. }
            | Self::IntegerExactCast { scalar_type, .. } => *scalar_type,
            Self::IeeeFloatLiteral { value } => ScalarType::IeeeFloat(value.format()),
            Self::Boolean { .. } => ScalarType::Boolean,
        }
    }
}

pub(super) fn emit_scalar_leaf(
    kind: OperationKind,
    scalar_type: ScalarType,
    next_value_identity: &mut u64,
    operations: &mut OperationBuffer,
) -> ValueId {
    let id = value_id(*next_value_identity);
    *next_value_identity = next_value_identity
        .checked_add(1)
        .expect("generated value identity advances after a scalar leaf");
    let operation = operations.allocate();
    operations.push(Operation {
        static_reach_binding: None,
        id: operation,
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id,
            scalar_type,
        }),
        kind,
    });
    id
}

pub(crate) fn emit_byte_length(
    source: PlaceId,
    next_value_identity: &mut u64,
    operations: &mut OperationBuffer,
) -> ValueId {
    // Immutable descriptor extent is unchanged on this emission path. Reuse
    // its dominating observation so later bounds retain the selected guard's
    // exact value identity; branch emitters scope this cache to their path.
    if let Some(value) = operations
        .byte_lengths
        .iter()
        .rev()
        .find_map(|(place, value)| (*place == source).then_some(*value))
    {
        return value;
    }
    let scalar_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid"));
    let value = emit_scalar_leaf(
        OperationKind::ByteSequenceLength { source },
        scalar_type,
        next_value_identity,
        operations,
    );
    operations.byte_lengths.push((source, value));
    value
}

pub(crate) fn emit_direct_expression(
    expression: &LoweredDirectExpression,
    parameters: &[ValueDeclaration],
    next_value_identity: &mut u64,
    operations: &mut OperationBuffer,
) -> ValueId {
    match expression {
        LoweredDirectExpression::PrimitiveRead {
            source,
            path,
            scalar_type,
        } => emit_scalar_leaf(
            OperationKind::PrimitiveScalarRead {
                source: *source,
                path: path.clone(),
            },
            *scalar_type,
            next_value_identity,
            operations,
        ),
        LoweredDirectExpression::StructuralField {
            source,
            path,
            field,
            scalar_type,
        } => emit_scalar_leaf(
            OperationKind::IntegerStructuralField {
                source: *source,
                path: path.clone(),
                field: *field,
            },
            *scalar_type,
            next_value_identity,
            operations,
        ),
        LoweredDirectExpression::ErasedParameter { .. } => {
            unreachable!("erased formal is proof-only and has no runtime operand")
        }
        LoweredDirectExpression::Parameter { position, .. }
        | LoweredDirectExpression::Local { position, .. } => parameters[*position].id,
        LoweredDirectExpression::IntegerLiteral { value, scalar_type } => emit_scalar_leaf(
            OperationKind::IntegerConstant { value: *value },
            *scalar_type,
            next_value_identity,
            operations,
        ),
        LoweredDirectExpression::ByteSequenceLength { source, .. } => {
            emit_byte_length(*source, next_value_identity, operations)
        }
        LoweredDirectExpression::ByteSequenceFieldLength {
            source,
            path,
            field,
            scalar_type,
        } => {
            // Field length is live storage metadata, not a whole-view extent.
            // Emit at this occurrence; replacement and calls can change it.
            emit_scalar_leaf(
                OperationKind::StructuralByteSequenceFieldLength {
                    source: *source,
                    path: path.clone(),
                    field: *field,
                },
                *scalar_type,
                next_value_identity,
                operations,
            )
        }
        LoweredDirectExpression::ByteSequenceRead {
            source,
            index,
            scalar_type,
        } => {
            let index = emit_direct_expression(index, parameters, next_value_identity, operations);
            let length = operations
                .byte_lengths
                .iter()
                .rev()
                .find_map(|(place, value)| (*place == *source).then_some(*value))
                .unwrap_or_else(|| emit_byte_length(*source, next_value_identity, operations));
            // A missing dominating observation may still form a valid read
            // shape. Its canonical bounds certificate must then be produced;
            // constructing a fresh length does not prove the read is in bounds.
            let obligation = obligation_id(
                operations
                    .next_identity
                    .checked_add(1)
                    .expect("read obligation follows its operation identity"),
            );
            emit_scalar_leaf(
                OperationKind::ByteSequenceRead {
                    source: *source,
                    index,
                    length,
                    obligation,
                },
                *scalar_type,
                next_value_identity,
                operations,
            )
        }
        LoweredDirectExpression::IeeeFloatLiteral { value } => emit_scalar_leaf(
            OperationKind::IeeeFloatConstant { value: *value },
            ScalarType::IeeeFloat(value.format()),
            next_value_identity,
            operations,
        ),
        LoweredDirectExpression::IntegerBinary {
            kind,
            scalar_type,
            left,
            right,
        } => {
            let left = emit_direct_expression(left, parameters, next_value_identity, operations);
            let right = emit_direct_expression(right, parameters, next_value_identity, operations);
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after a binary operation");
            let operation = operations.allocate();
            operations.push(Operation {
                static_reach_binding: None,
                id: operation,
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id,
                    scalar_type: *scalar_type,
                }),
                kind: kind.operation(operation, left, right),
            });
            id
        }
        LoweredDirectExpression::IntegerBitwiseNot {
            scalar_type,
            operand,
        } => {
            let operand =
                emit_direct_expression(operand, parameters, next_value_identity, operations);
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after bitwise complement");
            let operation = operations.allocate();
            operations.push(Operation {
                static_reach_binding: None,
                id: operation,
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id,
                    scalar_type: *scalar_type,
                }),
                kind: OperationKind::IntegerBitwiseNot { operand },
            });
            id
        }
        LoweredDirectExpression::IntegerWiden {
            scalar_type,
            operand,
        } => {
            let operand =
                emit_direct_expression(operand, parameters, next_value_identity, operations);
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after integer widening");
            let operation = operations.allocate();
            operations.push(Operation {
                static_reach_binding: None,
                id: operation,
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id,
                    scalar_type: *scalar_type,
                }),
                kind: OperationKind::IntegerWiden { operand },
            });
            id
        }
        LoweredDirectExpression::IntegerExactCast {
            scalar_type,
            operand,
        } => {
            let operand =
                emit_direct_expression(operand, parameters, next_value_identity, operations);
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after an exact integer cast");
            let operation = operations.allocate();
            operations.push(Operation {
                static_reach_binding: None,
                id: operation,
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id,
                    scalar_type: *scalar_type,
                }),
                kind: OperationKind::IntegerExactCast {
                    operand,
                    obligation: obligation_id(
                        operation
                            .get()
                            .checked_add(1)
                            .expect("exact-cast obligation follows its operation identity"),
                    ),
                },
            });
            id
        }
        LoweredDirectExpression::Boolean { expression } => {
            emit_boolean_expression(expression, parameters, next_value_identity, operations)
        }
    }
}
