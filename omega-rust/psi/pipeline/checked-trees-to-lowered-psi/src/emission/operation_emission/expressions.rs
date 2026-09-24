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
        path: Vec<terminal_psi::StructuralPathSegment>,
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
    ByteSequenceFieldRead {
        source: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        field: StructuralFieldId,
        index: Box<LoweredDirectExpression>,
        scalar_type: ScalarType,
    },
    /// Read one primitive element of a fixed-array record field: `path`
    /// resolves from `source` to the array itself and `index` is the runtime
    /// `u64` selector, evaluated first and then spelled as the read's
    /// `RuntimeIndex` segment.
    IndexedPrimitiveRead {
        source: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        index: Box<LoweredDirectExpression>,
        scalar_type: ScalarType,
    },
    ElementViewLength {
        source: PlaceId,
        scalar_type: ScalarType,
    },
    /// One scalar of one element of an established view: the element itself
    /// when `path` is empty, or the leaf `path` projects to inside a record
    /// element.
    ElementViewRead {
        source: PlaceId,
        index: Box<LoweredDirectExpression>,
        path: Vec<terminal_psi::StructuralPathSegment>,
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
    /// Selected Trapping conversion: the operand's exact value when it is
    /// representable in `scalar_type`, otherwise a `Trap` at this operation.
    IntegerTrappingCast {
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
            | Self::ByteSequenceFieldRead { scalar_type, .. }
            | Self::IndexedPrimitiveRead { scalar_type, .. }
            | Self::ElementViewLength { scalar_type, .. }
            | Self::ElementViewRead { scalar_type, .. }
            | Self::Local { scalar_type, .. }
            | Self::IntegerLiteral { scalar_type, .. }
            | Self::IntegerBinary { scalar_type, .. }
            | Self::IntegerBitwiseNot { scalar_type, .. }
            | Self::IntegerWiden { scalar_type, .. }
            | Self::IntegerExactCast { scalar_type, .. }
            | Self::IntegerTrappingCast { scalar_type, .. } => *scalar_type,
            Self::IeeeFloatLiteral { value } => ScalarType::IeeeFloat(value.format()),
            Self::Boolean { .. } => ScalarType::Boolean,
        }
    }
}

pub(crate) fn emit_scalar_leaf(
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
        suspension_crossing: None,
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

pub(crate) fn emit_element_length(
    source: PlaceId,
    next_value_identity: &mut u64,
    operations: &mut OperationBuffer,
) -> ValueId {
    // Immutable descriptor extent is unchanged on this emission path. Reuse
    // its dominating observation so later bounds retain the selected guard's
    // exact value identity; branch emitters scope this cache to their path.
    if let Some(value) = operations
        .element_lengths
        .iter()
        .rev()
        .find_map(|(place, value)| (*place == source).then_some(*value))
    {
        return value;
    }
    let scalar_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid"));
    let value = emit_scalar_leaf(
        OperationKind::ElementViewLength { source },
        scalar_type,
        next_value_identity,
        operations,
    );
    operations.element_lengths.push((source, value));
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
            // Record the observation so an indexed read on the same field
            // binds its bound against this exact dominating value identity.
            let value = emit_scalar_leaf(
                OperationKind::StructuralByteSequenceFieldLength {
                    source: *source,
                    path: path.clone(),
                    field: *field,
                },
                *scalar_type,
                next_value_identity,
                operations,
            );
            operations
                .field_byte_lengths
                .push((*source, path.clone(), *field, value));
            value
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
        LoweredDirectExpression::ByteSequenceFieldRead {
            source,
            path,
            field,
            index,
            scalar_type,
        } => {
            let index = emit_direct_expression(index, parameters, next_value_identity, operations);
            // A bound proven against an earlier field `.len` observation
            // only discharges this read when the operand names that same
            // dominating value; otherwise mint a current observation, whose
            // bound the certificate must then derive from the extent itself.
            let length = operations
                .field_byte_lengths
                .iter()
                .rev()
                .find_map(|(place, carrier, named, value)| {
                    (*place == *source && *carrier == *path && *named == *field).then_some(*value)
                })
                .unwrap_or_else(|| {
                    emit_scalar_leaf(
                        OperationKind::StructuralByteSequenceFieldLength {
                            source: *source,
                            path: path.clone(),
                            field: *field,
                        },
                        ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid"),
                        ),
                        next_value_identity,
                        operations,
                    )
                });
            let obligation = obligation_id(
                operations
                    .next_identity
                    .checked_add(1)
                    .expect("read obligation follows its operation identity"),
            );
            emit_scalar_leaf(
                OperationKind::StructuralByteSequenceFieldRead {
                    source: *source,
                    path: path.clone(),
                    field: *field,
                    index,
                    length,
                    obligation,
                },
                *scalar_type,
                next_value_identity,
                operations,
            )
        }
        LoweredDirectExpression::IndexedPrimitiveRead {
            source,
            path,
            index,
            scalar_type,
        } => {
            let index = emit_direct_expression(index, parameters, next_value_identity, operations);
            // The certificate proves `index < declared extent` against the
            // array's statically declared length, so no length observation is
            // needed: the verifier resolves the extent from the prefix.
            let obligation = obligation_id(
                operations
                    .next_identity
                    .checked_add(1)
                    .expect("read obligation follows its operation identity"),
            );
            let mut path = path.clone();
            path.push(terminal_psi::StructuralPathSegment::RuntimeIndex { index, obligation });
            emit_scalar_leaf(
                OperationKind::PrimitiveScalarRead {
                    source: *source,
                    path,
                },
                *scalar_type,
                next_value_identity,
                operations,
            )
        }
        LoweredDirectExpression::ElementViewLength { source, .. } => {
            emit_element_length(*source, next_value_identity, operations)
        }
        LoweredDirectExpression::ElementViewRead {
            source,
            index,
            path,
            scalar_type,
        } => {
            let index = emit_direct_expression(index, parameters, next_value_identity, operations);
            let length = operations
                .element_lengths
                .iter()
                .rev()
                .find_map(|(place, value)| (*place == *source).then_some(*value))
                .unwrap_or_else(|| emit_element_length(*source, next_value_identity, operations));
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
                OperationKind::ElementViewRead {
                    source: *source,
                    index,
                    length,
                    obligation,
                    path: path.clone(),
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
                suspension_crossing: None,
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
                suspension_crossing: None,
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
                suspension_crossing: None,
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
                suspension_crossing: None,
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
        LoweredDirectExpression::IntegerTrappingCast {
            scalar_type,
            operand,
        } => {
            let operand =
                emit_direct_expression(operand, parameters, next_value_identity, operations);
            emit_scalar_leaf(
                OperationKind::TrappingInteger {
                    operation: terminal_psi::TrappingIntegerOperation::Convert { operand },
                },
                *scalar_type,
                next_value_identity,
                operations,
            )
        }
        LoweredDirectExpression::Boolean { expression } => {
            emit_boolean_expression(expression, parameters, next_value_identity, operations)
        }
    }
}
