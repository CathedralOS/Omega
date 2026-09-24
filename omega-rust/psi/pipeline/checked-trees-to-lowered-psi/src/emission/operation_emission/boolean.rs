//! Boolean leaves and comparisons; short-circuit expressions belong to control lowering.

use super::buffer::OperationBuffer;
use super::expressions::{LoweredDirectExpression, emit_direct_expression, emit_scalar_leaf};
use super::integer::LoweredIntegerComparisonKind;
use crate::terminal_identities::value_id;
use semantic_vocabulary::{PlaceId, ScalarType, StructuralFieldId, ValueId};
use terminal_psi::{Operation, OperationKind, ValueDeclaration};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoweredBooleanReturnExpression {
    StructuralCaseMembership {
        source: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        case: semantic_vocabulary::StructuralCaseId,
    },
    PrimitiveRead {
        source: PlaceId,
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
        LoweredBooleanReturnExpression::PrimitiveRead { source, path } => emit_scalar_leaf(
            OperationKind::PrimitiveScalarRead {
                source: *source,
                path: path.clone(),
            },
            ScalarType::Boolean,
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
