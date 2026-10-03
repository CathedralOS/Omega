//! Boolean leaves and comparisons; short-circuit expressions belong to control lowering.

use super::buffer::OperationBuffer;
use super::expressions::{
    LoweredDirectExpression, emit_direct_expression, emit_element_length, emit_scalar_leaf,
};
use super::integer::LoweredIntegerComparisonKind;
use crate::terminal_identities::obligation_id;
use crate::terminal_identities::value_id;
use semantic_vocabulary::{PlaceId, ScalarType, StructuralFieldId, ValueId};
use terminal_psi::{Operation, OperationKind, ValueDeclaration};

/// One side of a byte-sequence content comparison: the place its descriptor
/// occupies plus the element projection and exact byte-sequence field when the
/// operand is a field below that carrier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoweredByteSequenceOperand {
    pub source: PlaceId,
    pub path: Vec<super::expressions::LoweredElementPathSegment>,
    pub field: Option<StructuralFieldId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoweredBooleanReturnExpression {
    StructuralCaseMembership {
        source: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        case: semantic_vocabulary::StructuralCaseId,
    },
    /// Observe the active case of a runtime-selected element
    /// (`self.items[self.w] is Color::Red`): `path` reaches the collection,
    /// `index` is the selector, evaluated first and spelled as the
    /// observation's `RuntimeIndex` segment, and `element_path` walks the
    /// selected element to the sum. The observation owns the selector's bound
    /// obligation, reconstructed against the fixed array's declared extent or
    /// the exact element view's live length; it keys no case fact.
    IndexedCaseMembership {
        source: PlaceId,
        /// Derived from the exact bound carrier declaration, not path spelling.
        observes_element_view_length: bool,
        path: Vec<terminal_psi::StructuralPathSegment>,
        index: Box<LoweredDirectExpression>,
        element_path: Vec<terminal_psi::StructuralPathSegment>,
        case: semantic_vocabulary::StructuralCaseId,
    },
    PrimitiveRead {
        source: super::expressions::PrimitiveReadSource,
        path: Vec<terminal_psi::StructuralPathSegment>,
    },
    Constant {
        value: bool,
    },
    Parameter {
        position: usize,
    },
    Local {
        position: usize,
    },
    UnresolvedStructuralParameterField {
        parameter_position: u32,
        path: Vec<String>,
    },
    StructuralField {
        source: PlaceId,
        path: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
        field: StructuralFieldId,
    },
    /// Evaluate one declared predicate over an immutable byte view. `path` is
    /// the element projection from `source` to the byte-sequence carrier —
    /// empty for a whole-view operand; `field` names the operand's
    /// byte-sequence field below that carrier when the view is a field view.
    ByteSequencePredicate {
        predicate: terminal_psi::ByteSequencePredicate,
        source: PlaceId,
        path: Vec<super::expressions::LoweredElementPathSegment>,
        field: Option<StructuralFieldId>,
    },
    /// Content equality between two readable byte-sequence operands: equal
    /// live lengths and equal bytes at every live position. Each operand's
    /// own dominating length observation bounds its reads; neither operand
    /// is consumed.
    ByteSequenceEqual {
        left: LoweredByteSequenceOperand,
        right: LoweredByteSequenceOperand,
    },
    Not {
        operand: Box<LoweredBooleanReturnExpression>,
    },
    Equal {
        left: Box<LoweredBooleanReturnExpression>,
        right: Box<LoweredBooleanReturnExpression>,
    },
    IntegerComparison {
        kind: LoweredIntegerComparisonKind,
        left: Box<LoweredDirectExpression>,
        right: Box<LoweredDirectExpression>,
    },
    And {
        left: Box<LoweredBooleanReturnExpression>,
        right: Box<LoweredBooleanReturnExpression>,
    },
    Or {
        left: Box<LoweredBooleanReturnExpression>,
        right: Box<LoweredBooleanReturnExpression>,
    },
}

pub(crate) fn emit_boolean_expression(
    expression: &LoweredBooleanReturnExpression,
    parameters: &[ValueDeclaration],
    next_value_identity: &mut u64,
    operations: &mut OperationBuffer,
) -> ValueId {
    match expression {
        LoweredBooleanReturnExpression::Constant { value } => {
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after a Boolean literal");
            let operation = operations.allocate();
            operations.push(Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: operation,
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id,
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::BooleanConstant { value: *value },
            });
            id
        }
        LoweredBooleanReturnExpression::IntegerComparison { kind, left, right } => {
            let left = emit_direct_expression(left, parameters, next_value_identity, operations);
            let right = emit_direct_expression(right, parameters, next_value_identity, operations);
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after integer comparison");
            let operation = operations.allocate();
            operations.push(Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: operation,
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id,
                    scalar_type: ScalarType::Boolean,
                }),
                kind: kind.operation(left, right),
            });
            id
        }
        LoweredBooleanReturnExpression::Parameter { position }
        | LoweredBooleanReturnExpression::Local { position } => parameters[*position].id,
        LoweredBooleanReturnExpression::PrimitiveRead { source, path } => emit_direct_expression(
            &LoweredDirectExpression::PrimitiveRead {
                source: source.clone(),
                path: path.clone(),
                scalar_type: ScalarType::Boolean,
            },
            parameters,
            next_value_identity,
            operations,
        ),
        LoweredBooleanReturnExpression::StructuralCaseMembership { source, path, case } => {
            emit_scalar_leaf(
                OperationKind::StructuralCaseMembership {
                    source: *source,
                    path: path.clone(),
                    case: *case,
                },
                ScalarType::Boolean,
                next_value_identity,
                operations,
            )
        }
        LoweredBooleanReturnExpression::IndexedCaseMembership {
            source,
            observes_element_view_length,
            path,
            index,
            element_path,
            case,
        } => {
            let index = emit_direct_expression(index, parameters, next_value_identity, operations);
            if *observes_element_view_length {
                emit_element_length(*source, next_value_identity, operations);
            }
            let obligation = obligation_id(
                operations
                    .next_identity
                    .checked_add(1)
                    .expect("membership obligation follows its operation identity"),
            );
            let mut path = path.clone();
            path.push(terminal_psi::StructuralPathSegment::RuntimeIndex { index, obligation });
            path.extend(element_path.iter().cloned());
            emit_scalar_leaf(
                OperationKind::StructuralCaseMembership {
                    source: *source,
                    path,
                    case: *case,
                },
                ScalarType::Boolean,
                next_value_identity,
                operations,
            )
        }
        LoweredBooleanReturnExpression::StructuralField {
            source,
            path,
            field,
        } => {
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after a structural Boolean load");
            let operation = operations.allocate();
            operations.push(Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: operation,
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id,
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::BooleanStructuralField {
                    source: *source,
                    path: path.clone(),
                    field: *field,
                },
            });
            id
        }
        LoweredBooleanReturnExpression::ByteSequencePredicate {
            predicate,
            source,
            path,
            field,
        } => {
            let operand = byte_sequence_operand(
                *source,
                path,
                *field,
                parameters,
                next_value_identity,
                operations,
            );
            emit_scalar_leaf(
                OperationKind::ByteSequencePredicate {
                    operand,
                    predicate: *predicate,
                },
                ScalarType::Boolean,
                next_value_identity,
                operations,
            )
        }
        LoweredBooleanReturnExpression::ByteSequenceEqual { left, right } => {
            let left = byte_sequence_operand(
                left.source,
                &left.path,
                left.field,
                parameters,
                next_value_identity,
                operations,
            );
            let right = byte_sequence_operand(
                right.source,
                &right.path,
                right.field,
                parameters,
                next_value_identity,
                operations,
            );
            emit_scalar_leaf(
                OperationKind::ByteSequenceEqual { left, right },
                ScalarType::Boolean,
                next_value_identity,
                operations,
            )
        }
        LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. } => {
            unreachable!("shared Boolean members resolve before terminal operation emission")
        }
        LoweredBooleanReturnExpression::Not { operand } => {
            let operand =
                emit_boolean_expression(operand, parameters, next_value_identity, operations);
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after Boolean negation");
            let operation = operations.allocate();
            operations.push(Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: operation,
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id,
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::BooleanNot { operand },
            });
            id
        }
        LoweredBooleanReturnExpression::Equal { left, right } => {
            let left = emit_boolean_expression(left, parameters, next_value_identity, operations);
            let right = emit_boolean_expression(right, parameters, next_value_identity, operations);
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after Boolean equality");
            let operation = operations.allocate();
            operations.push(Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: operation,
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id,
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::BooleanEqual { left, right },
            });
            id
        }
        LoweredBooleanReturnExpression::And { .. } | LoweredBooleanReturnExpression::Or { .. } => {
            unreachable!("short-circuit Boolean expressions lower through terminal control")
        }
    }
}

/// One byte-observation operand shared by the byte-sequence operations: a
/// field operand emits its dominating field-length observation as the read
/// bound (recorded so sibling observations reuse the exact value identity),
/// while a whole-view operand reads its descriptor extent directly.
fn byte_sequence_operand(
    source: PlaceId,
    path: &[super::expressions::LoweredElementPathSegment],
    field: Option<StructuralFieldId>,
    parameters: &[ValueDeclaration],
    next_value_identity: &mut u64,
    operations: &mut OperationBuffer,
) -> terminal_psi::control_flow::ByteSequenceOperand {
    match field {
        Some(field) => {
            let path = super::expressions::emit_prepared_path(
                path,
                0,
                parameters,
                next_value_identity,
                operations,
            );
            let path =
                super::expressions::path_with_current_bounds(&path, operations.next_identity);
            let length = emit_scalar_leaf(
                OperationKind::StructuralByteSequenceFieldLength {
                    source,
                    path: path.clone(),
                    field,
                },
                ScalarType::Integer(
                    semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned,
                        64,
                    )
                    .expect("u64 is valid"),
                ),
                next_value_identity,
                operations,
            );
            operations
                .field_byte_lengths
                .push((source, path.clone(), field, length));
            terminal_psi::control_flow::ByteSequenceOperand::Field {
                source: terminal_psi::StructuralArgument {
                    place: source,
                    path,
                    access: terminal_psi::StructuralAccess::SharedBorrow,
                },
                field,
                length,
            }
        }
        None => terminal_psi::control_flow::ByteSequenceOperand::Whole {
            source,
            length: super::expressions::emit_byte_length(source, next_value_identity, operations),
        },
    }
}
