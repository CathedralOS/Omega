//! Lowering Boolean expressions and guards, including closed evaluation and
//! integer comparisons.

use crate::values::scalar::case_membership;
use crate::values::scalar::constant_array_projection;
use crate::values::scalar::expression_facts::{
    is_integer, local_position, operator_is_builtin, parameter_position, scalar_expression_type,
};
use crate::values::scalar::expression_plans::{ScalarLocal, lower_closed_integer_literal_guard};
use crate::values::scalar::primitive_reference_read;
use crate::values::scalar::scalar_lowering::{
    land_contextual_integer_literal, lower_scalar_operands,
};
use crate::values::scalar::structural_fields;
use checked_trees::{
    CheckedBooleanExpression, CheckedIntegerComparisonKind, CheckedOperatorFacts,
    CheckedScalarExpression,
};
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::signature::StateParameter;
use typed_trees::types::PrimitiveType;

/// Recover a normal-return Boolean using the same selected operations as
/// scalar computation folding, without assumptions about any runtime binding.
/// Callers still evaluate the expression's children: a known result (such as a
/// fixed extent) does not establish that evaluating its source is effect-free.
pub(crate) fn evaluate_closed_boolean_expression(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<bool> {
    let selected = lower_boolean_expression(
        program,
        operators,
        expression,
        &[],
        &[],
        &[],
        &[],
        exact_integer_casts,
    )?;
    let facts::ScalarValue::Boolean(value) = crate::values::evaluate_checked_scalar(
        &CheckedScalarExpression::Boolean(Box::new(selected)),
        &mut |_| None,
    )?
    else {
        return None;
    };
    Some(value)
}

pub(crate) fn lower_boolean_expression(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    authored_parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<CheckedBooleanExpression> {
    BooleanLowering::new(
        program,
        operators,
        parameters,
        authored_parameters,
        parameter_types,
        locals,
        exact_integer_casts,
    )
    .expression(expression)
}

// The recursive Boolean walk keeps the same invocation checking input and
// borrowed scalar namespace. Authored coordinates remain separate from dense
// scalar positions; guard normalization and admission keep their existing order.
struct BooleanLowering<'a> {
    program: &'a dyn ::validation::CheckingInput,
    operators: &'a CheckedOperatorFacts,
    parameters: &'a [StateParameter],
    authored_parameters: &'a [StateParameter],
    parameter_types: &'a [PrimitiveType],
    locals: &'a [ScalarLocal],
    exact_integer_casts: &'a [validation::ExactIntegerCastFact],
}

impl<'a> BooleanLowering<'a> {
    fn new(
        program: &'a dyn ::validation::CheckingInput,
        operators: &'a CheckedOperatorFacts,
        parameters: &'a [StateParameter],
        authored_parameters: &'a [StateParameter],
        parameter_types: &'a [PrimitiveType],
        locals: &'a [ScalarLocal],
        exact_integer_casts: &'a [validation::ExactIntegerCastFact],
    ) -> Self {
        Self {
            program,
            operators,
            parameters,
            authored_parameters,
            parameter_types,
            locals,
            exact_integer_casts,
        }
    }
}

impl BooleanLowering<'_> {
    fn expression(&self, expression: ExpressionHandle) -> Option<CheckedBooleanExpression> {
        if let Some(CheckedScalarExpression::Boolean(read)) = primitive_reference_read::lower(
            self.program,
            self.authored_parameters,
            expression,
            PrimitiveType::Bool,
        ) {
            return Some(*read);
        }
        // A `&place as &bool` operand re-views its source place under the bool
        // carrier (§5b address identity): the checked form is the source place's
        // own read retagged to bool. Validation already proved the
        // representation relation.
        if let Some(read) = crate::values::scalar::scalar_lowering::lower_recast_boolean_operand(
            self.program,
            self.operators,
            expression,
            self.parameters,
            self.authored_parameters,
            self.parameter_types,
            self.locals,
            self.exact_integer_casts,
        ) {
            return Some(read);
        }
        if let Some(membership) =
            case_membership::lower(self.program, self.authored_parameters, expression)
        {
            return Some(membership);
        }
        if let Some((leaf, PrimitiveType::Bool)) =
            validation::closed_record_scalar_projection(self.program, expression)
        {
            let value = self.program.expression_table.boolean_node(leaf)?;
            return Some(CheckedBooleanExpression::Constant(*value));
        }
        if let Some(leaf) = constant_array_projection::selected_leaf(
            self.program,
            self.operators,
            self.authored_parameters,
            expression,
        ) {
            return self.expression(leaf);
        }
        if let Some((CheckedScalarExpression::Boolean(field), _)) =
            super::scalar_lowering::scalar_storage_read(
                self.program,
                self.operators,
                expression,
                self.parameters,
                self.authored_parameters,
                self.parameter_types,
                self.locals,
                self.exact_integer_casts,
            )
        {
            return Some(*field);
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Boolean(value) => Some(CheckedBooleanExpression::Constant(*value)),
            ExpressionNode::Name(path) => {
                if let Some(position) = parameter_position(self.program, path, self.parameters) {
                    let parameter = &self.parameters[position];
                    if parameter.is_mutable {
                        return (crate::values::mutable_scalar_parameter_type(
                            self.program,
                            parameter,
                        ) == Some(PrimitiveType::Bool)
                            && path.symbol == parameter.symbol
                            && path.head_symbol == parameter.symbol)
                            .then_some(CheckedBooleanExpression::StorageRead {
                                symbol: parameter.symbol,
                            });
                    }
                    return (self.parameter_types[position] == PrimitiveType::Bool)
                        .then_some(CheckedBooleanExpression::Parameter { position });
                }
                let local_position = local_position(self.program, expression, path, self.locals)?;
                let local = &self.locals[local_position];
                if local.is_mutable || local.is_reference {
                    return (local.primitive_type == PrimitiveType::Bool).then_some(
                        CheckedBooleanExpression::StorageRead {
                            symbol: local.symbol,
                        },
                    );
                }
                let position = self.parameters.len().checked_add(
                    self.locals[..local_position]
                        .iter()
                        .filter(|local| local.is_value_binding())
                        .count(),
                )?;
                (self.locals[local_position].primitive_type == PrimitiveType::Bool)
                    .then_some(CheckedBooleanExpression::Local { position })
            }
            ExpressionNode::Unary(unary)
                if unary.operator == UnaryOperator::LogicalNot
                    && operator_is_builtin(self.operators, expression) =>
            {
                Some(CheckedBooleanExpression::Not(Box::new(
                    self.expression(unary.operand)?,
                )))
            }
            ExpressionNode::Binary(binary)
                if binary.operator.is_comparison()
                    && operator_is_builtin(self.operators, expression) =>
            {
                let integer_comparison = (|| {
                    let ((left, _), (right, _)) = lower_scalar_operands(
                        self.program,
                        self.operators,
                        binary,
                        self.parameters,
                        self.authored_parameters,
                        self.parameter_types,
                        self.locals,
                        self.exact_integer_casts,
                    )?;
                    construct_integer_comparison(binary.operator, left, right)
                })();
                if !matches!(
                    binary.operator,
                    BinaryOperator::Equal | BinaryOperator::NotEqual
                ) || integer_comparison.is_some()
                {
                    return integer_comparison;
                }
                // A byte carrier compared against a byte-sequence literal or
                // another bounded byte carrier has no scalar operand pair:
                // content equality is the live-length observations plus each
                // byte at its fixed index, evaluated left to right so a length
                // comparison dominates every indexed read.
                if let Some(equality) =
                    byte_carrier_literal_equality(self.program, binary, self.authored_parameters)
                        .or_else(|| {
                            byte_carrier_pair_equality(
                                self.program,
                                binary,
                                self.authored_parameters,
                            )
                        })
                {
                    return Some(if binary.operator == BinaryOperator::NotEqual {
                        CheckedBooleanExpression::Not(Box::new(equality))
                    } else {
                        equality
                    });
                }
                let equality = CheckedBooleanExpression::Equal {
                    left: Box::new(self.expression(binary.left)?),
                    right: Box::new(self.expression(binary.right)?),
                };
                Some(if binary.operator == BinaryOperator::NotEqual {
                    CheckedBooleanExpression::Not(Box::new(equality))
                } else {
                    equality
                })
            }
            ExpressionNode::Binary(binary)
                if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or)
                    && operator_is_builtin(self.operators, expression) =>
            {
                let left = Box::new(self.expression(binary.left)?);
                let right = Box::new(self.expression(binary.right)?);
                Some(if binary.operator == BinaryOperator::And {
                    CheckedBooleanExpression::And { left, right }
                } else {
                    CheckedBooleanExpression::Or { left, right }
                })
            }
            _ => None,
        }
    }

    fn guard(&self, expression: ExpressionHandle) -> Option<CheckedBooleanExpression> {
        if let Some(value) =
            lower_closed_integer_literal_guard(self.program, self.operators, expression)
        {
            return Some(value);
        }
        let Some(binary) = self.program.expression_table.binary_node(expression) else {
            return self.expression(expression);
        };
        if binary.operator == BinaryOperator::Equal
            && operator_is_builtin(self.operators, expression)
        {
            match (
                self.program.expression_table.expression(binary.left),
                self.program.expression_table.expression(binary.right),
            ) {
                (ExpressionNode::Boolean(true), _) => {
                    return self.expression(binary.right);
                }
                (_, ExpressionNode::Boolean(true)) => {
                    return self.expression(binary.left);
                }
                _ => {}
            }
        }
        self.expression(expression)
    }
}

/// `carrier == "literal"` admits no scalar operand pair: content equality is
/// the carrier's live length plus each literal byte at its fixed index. The
/// fold keeps the length comparison leftmost so short-circuit evaluation
/// dominates every indexed read, and each read's `index < length` obligation
/// is the true-path fact of that comparison.
///
/// Fixed arrays observe their declared extent; byte views observe the extent
/// in their descriptor. Qualification does not change either geometry. The
/// same observations apply to roots, fields and established immutable locals.
/// Write-only views grant no byte observations. Non-byte element views do not
/// have the byte geometry required by this comparison.
fn byte_carrier_literal_equality(
    program: &dyn ::validation::CheckingInput,
    binary: &typed_trees::expression::TableBinaryExpression,
    authored_parameters: &[StateParameter],
) -> Option<CheckedBooleanExpression> {
    for (carrier, literal) in [(binary.left, binary.right), (binary.right, binary.left)] {
        let Some(bytes) = typed_trees::byte_predicates::literal_bytes(program, literal) else {
            continue;
        };
        let Some((root, path, geometry)) =
            comparable_byte_carrier(program, authored_parameters, carrier)
        else {
            continue;
        };
        let maximum_length = match geometry {
            ByteObservationCarrier::FixedArray { length } => Some(length),
            ByteObservationCarrier::Sequence(
                checked_trees::CheckedByteSequenceCarrier::BoundedView { capacity, .. }
                | checked_trees::CheckedByteSequenceCarrier::BoundedOwned { capacity },
            ) => Some(capacity),
            _ => None,
        };
        return CheckedBooleanExpression::byte_literal_equality_in_extent(
            root,
            path,
            &bytes,
            maximum_length,
        );
    }
    None
}

/// `left == right` over two live byte carriers is content equality. Two
/// fixed arrays compare their exact extents and bytes. Other byte carriers
/// retain their own explicit length observations. Read permissions and typed
/// paths select the actual stored values, including below record fields.
fn byte_carrier_pair_equality(
    program: &dyn ::validation::CheckingInput,
    binary: &typed_trees::expression::TableBinaryExpression,
    authored_parameters: &[StateParameter],
) -> Option<CheckedBooleanExpression> {
    let left = comparable_byte_carrier(program, authored_parameters, binary.left)?;
    let right = comparable_byte_carrier(program, authored_parameters, binary.right)?;
    let ((left_root, left_path, left_carrier), (right_root, right_path, right_carrier)) =
        (left, right);
    let capacities = match (left_carrier, right_carrier) {
        (
            ByteObservationCarrier::FixedArray { length: left },
            ByteObservationCarrier::FixedArray { length: right },
        ) => Some((left, right)),
        (
            ByteObservationCarrier::Sequence(
                checked_trees::CheckedByteSequenceCarrier::BoundedOwned { capacity: left },
            ),
            ByteObservationCarrier::Sequence(
                checked_trees::CheckedByteSequenceCarrier::BoundedOwned { capacity: right },
            ),
        ) => Some((left, right)),
        _ => None,
    };
    let Some((left_capacity, right_capacity)) = capacities else {
        // Runtime observations retain authored storage roots and order.
        // Entry facts separately require parameter provenance and canonicalize
        // their proposition; that namespace cannot restrict executable operands.
        return Some(CheckedBooleanExpression::ByteSequenceEqual {
            left: checked_trees::CheckedByteSequenceOperand {
                root: left_root,
                path: left_path,
            },
            right: checked_trees::CheckedByteSequenceOperand {
                root: right_root,
                path: right_path,
            },
        });
    };
    let length = |root, path: &Vec<_>| {
        Box::new(CheckedScalarExpression::StructuralSequenceLength {
            root,
            path: path
                .iter()
                .cloned()
                .map(checked_trees::CheckedScalarReadProjection::Static)
                .collect(),
        })
    };
    let read = |root, path: &Vec<_>, index| {
        Some(Box::new(
            CheckedScalarExpression::StructuralParameterIndexedRead {
                root,
                path: path.clone(),
                index: Box::new(checked_trees::CheckedScalarSelector {
                    expression: integer_literal(index, PrimitiveType::U64)?,
                    occurrence: ExpressionHandle::invalid(),
                }),
                element_path: Vec::new(),
                primitive_type: PrimitiveType::U8,
            },
        ))
    };
    let mut remaining = None;
    for index in (0..left_capacity.min(right_capacity)).rev() {
        let agreeing = CheckedBooleanExpression::And {
            left: Box::new(CheckedBooleanExpression::IntegerComparison {
                kind: CheckedIntegerComparisonKind::LessThan,
                left: Box::new(integer_literal(index, PrimitiveType::U64)?),
                right: length(right_root, &right_path),
            }),
            right: Box::new(CheckedBooleanExpression::IntegerComparison {
                kind: CheckedIntegerComparisonKind::Equal,
                left: read(left_root, &left_path, index)?,
                right: read(right_root, &right_path, index)?,
            }),
        };
        let continued = match remaining {
            Some(remaining) => CheckedBooleanExpression::And {
                left: Box::new(agreeing),
                right: Box::new(remaining),
            },
            None => agreeing,
        };
        remaining = Some(CheckedBooleanExpression::Or {
            left: Box::new(CheckedBooleanExpression::IntegerComparison {
                kind: CheckedIntegerComparisonKind::LessOrEqual,
                left: length(left_root, &left_path),
                right: Box::new(integer_literal(index, PrimitiveType::U64)?),
            }),
            right: Box::new(continued),
        });
    }
    let lengths = CheckedBooleanExpression::IntegerComparison {
        kind: CheckedIntegerComparisonKind::Equal,
        left: length(left_root, &left_path),
        right: length(right_root, &right_path),
    };
    Some(match remaining {
        Some(remaining) => CheckedBooleanExpression::And {
            left: Box::new(lengths),
            right: Box::new(remaining),
        },
        None => lengths,
    })
}

/// The storage root, path and carrier of a byte carrier whose live length and
/// bytes a content comparison may observe, or `None` when `carrier` names no
/// such place.
pub(crate) fn comparable_byte_carrier(
    program: &dyn ::validation::CheckingInput,
    authored_parameters: &[StateParameter],
    carrier: ExpressionHandle,
) -> Option<(
    checked_trees::CheckedStorageRoot,
    Vec<checked_trees::CheckedStructuralPredicatePathSegment>,
    ByteObservationCarrier,
)> {
    let (root, path, type_reference) =
        structural_fields::structural_storage_place(program, authored_parameters, carrier)?;
    // The selected leaf defines its byte carrier. Its enclosing record may
    // own storage while this field borrows bytes; retain both identities so
    // downstream observation replay checks the actual backing and extent.
    let carrier = comparable_byte_carrier_type(program, type_reference)?;
    Some((root, path, carrier))
}

/// The carrier of `type_reference` when it has a live byte length and
/// readable bytes: a bounded owned byte buffer, or a shared or mutable
/// borrowed byte view.
/// Observation geometry is independent of the stored collection kind. A
/// fixed array contributes its declared extent; an explicit byte carrier
/// contributes the extent its own representation exposes.
#[derive(Clone)]
pub(crate) enum ByteObservationCarrier {
    FixedArray { length: u64 },
    Sequence(checked_trees::CheckedByteSequenceCarrier),
}

pub(crate) fn comparable_byte_carrier_type(
    program: &dyn ::validation::CheckingInput,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Option<ByteObservationCarrier> {
    // Observation follows a readable loan to its actual array. Custody and
    // qualification remain on the original operand; write-only access never
    // authorizes a byte observation.
    let observed_type = match validation::reference_shell(program, type_reference) {
        Some(shell) if shell.access == language_semantics::ReferenceAccess::WriteOnly => {
            return None;
        }
        Some(shell) => shell.referee,
        None => type_reference,
    };
    let storage = program
        .type_reference_table
        .unconstrained_type_reference(observed_type);
    if let typed_trees::types::TypeReferenceNode::FixedArray {
        element_type,
        length: typed_trees::types::FixedArrayLength::Literal(length),
    } = program.type_reference_table.type_reference(storage)
        && program.primitive_type_reference(*element_type) == Some(PrimitiveType::U8)
    {
        return Some(ByteObservationCarrier::FixedArray {
            length: u64::try_from(*length).ok()?,
        });
    }
    crate::execution::terminal_unit::types::byte_sequence_carrier(program, type_reference, &[])
        .filter(|carrier| {
            matches!(
                carrier,
                checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. }
                    | checked_trees::CheckedByteSequenceCarrier::BoundedView {
                        access: Some(
                            checked_trees::CheckedStructuralAccess::SharedBorrow
                                | checked_trees::CheckedStructuralAccess::MutableBorrow
                        ),
                        ..
                    }
                    | checked_trees::CheckedByteSequenceCarrier::BorrowedView {
                        access: Some(
                            checked_trees::CheckedStructuralAccess::SharedBorrow
                                | checked_trees::CheckedStructuralAccess::MutableBorrow
                        ),
                    }
            )
        })
        .map(ByteObservationCarrier::Sequence)
}

fn integer_literal(value: u64, primitive_type: PrimitiveType) -> Option<CheckedScalarExpression> {
    facts::u64_integer_literal(value, primitive_type)
}

/// Comparison normalization may swap completed values for `>` and `>=`; it
/// does not change the source operand evaluation order retained by the caller.
pub(crate) fn construct_integer_comparison(
    operator: BinaryOperator,
    mut left: CheckedScalarExpression,
    mut right: CheckedScalarExpression,
) -> Option<CheckedBooleanExpression> {
    match (
        scalar_expression_type(&left),
        scalar_expression_type(&right),
    ) {
        (Some(primitive_type), None) => {
            right = land_contextual_integer_literal(right, primitive_type)?;
        }
        (None, Some(primitive_type)) => {
            left = land_contextual_integer_literal(left, primitive_type)?;
        }
        (Some(_), Some(_)) => {}
        (None, None) => return None,
    }
    let left_type = scalar_expression_type(&left)?;
    let right_type = scalar_expression_type(&right)?;
    if !is_integer(left_type)
        || !is_integer(right_type)
        || (left_type != right_type
            && (left_type == PrimitiveType::Addr || right_type == PrimitiveType::Addr))
    {
        return None;
    }
    let (kind, negated) = match operator {
        BinaryOperator::Equal => (CheckedIntegerComparisonKind::Equal, false),
        BinaryOperator::NotEqual => (CheckedIntegerComparisonKind::Equal, true),
        BinaryOperator::Less => (CheckedIntegerComparisonKind::LessThan, false),
        BinaryOperator::LessOrEqual => (CheckedIntegerComparisonKind::LessOrEqual, false),
        BinaryOperator::Greater => {
            std::mem::swap(&mut left, &mut right);
            (CheckedIntegerComparisonKind::LessThan, false)
        }
        BinaryOperator::GreaterOrEqual => {
            std::mem::swap(&mut left, &mut right);
            (CheckedIntegerComparisonKind::LessOrEqual, false)
        }
        _ => return None,
    };
    let comparison = CheckedBooleanExpression::IntegerComparison {
        kind,
        left: Box::new(left),
        right: Box::new(right),
    };
    Some(if negated {
        CheckedBooleanExpression::Not(Box::new(comparison))
    } else {
        comparison
    })
}

pub(crate) fn lower_boolean_guard(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    authored_parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<CheckedBooleanExpression> {
    BooleanLowering::new(
        program,
        operators,
        parameters,
        authored_parameters,
        parameter_types,
        locals,
        exact_integer_casts,
    )
    .guard(expression)
}
