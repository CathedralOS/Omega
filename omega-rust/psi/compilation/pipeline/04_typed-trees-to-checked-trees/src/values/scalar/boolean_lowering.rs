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
    land_contextual_integer_literal, lower_scalar_expression, lower_scalar_operands,
};
use crate::values::scalar::structural_fields;
use checked_trees::{
    CheckedBooleanExpression, CheckedIntegerComparisonKind, CheckedOperatorFacts,
    CheckedScalarExpression,
};
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::signature::StateParameter;
use typed_trees::types::PrimitiveType;

/// Recover a normal-return Boolean using the same selected operations as
/// scalar computation folding, without assumptions about any runtime binding.
/// Callers still evaluate the expression's children: a known result (such as a
/// fixed extent) does not establish that evaluating its source is effect-free.
pub(crate) fn evaluate_closed_boolean_expression(
    program: &TypedTrees,
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
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    authored_parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<CheckedBooleanExpression> {
    if let Some(CheckedScalarExpression::Boolean(read)) = primitive_reference_read::lower(
        program,
        authored_parameters,
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
        program,
        operators,
        expression,
        parameters,
        authored_parameters,
        parameter_types,
        locals,
        exact_integer_casts,
    ) {
        return Some(read);
    }
    if let Some(membership) = case_membership::lower(program, authored_parameters, expression) {
        return Some(membership);
    }
    if let Some((leaf, PrimitiveType::Bool)) =
        validation::closed_record_scalar_projection(program, expression)
    {
        let ExpressionNode::Boolean(value) = program.expression_table.expression(leaf) else {
            return None;
        };
        return Some(CheckedBooleanExpression::Constant(*value));
    }
    if let Some(leaf) = constant_array_projection::selected_leaf(
        program,
        operators,
        authored_parameters,
        expression,
    ) {
        return lower_boolean_expression(
            program,
            operators,
            leaf,
            parameters,
            authored_parameters,
            parameter_types,
            locals,
            exact_integer_casts,
        );
    }
    if matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
    ) && let Some((CheckedScalarExpression::Boolean(field), _)) =
        structural_fields::lower_structural_parameter_field(
            program,
            authored_parameters,
            expression,
        )
    {
        return Some(*field);
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(value) => Some(CheckedBooleanExpression::Constant(*value)),
        ExpressionNode::Name(path) => {
            if let Some(position) = parameter_position(program, path, parameters) {
                let parameter = &parameters[position];
                if parameter.is_mutable {
                    return (crate::values::mutable_scalar_parameter_type(program, parameter)
                        == Some(PrimitiveType::Bool)
                        && path.symbol == parameter.symbol
                        && path.head_symbol == parameter.symbol)
                        .then_some(CheckedBooleanExpression::StorageRead {
                            symbol: parameter.symbol,
                        });
                }
                return (parameter_types[position] == PrimitiveType::Bool)
                    .then_some(CheckedBooleanExpression::Parameter { position });
            }
            let local_position = local_position(program, expression, path, locals)?;
            let local = &locals[local_position];
            if local.is_mutable {
                return (local.primitive_type == PrimitiveType::Bool).then_some(
                    CheckedBooleanExpression::StorageRead {
                        symbol: local.symbol,
                    },
                );
            }
            // A loan local names its captured place, not a formal slot: the
            // scalar read already re-tags the leaf through the loan's referee
            // (`&b as &bool` keeps the Boolean form the position expects).
            if local.shared_loan_place.is_some() {
                let (read, _) = lower_scalar_expression(
                    program,
                    operators,
                    expression,
                    parameters,
                    authored_parameters,
                    parameter_types,
                    locals,
                    exact_integer_casts,
                )?;
                let CheckedScalarExpression::Boolean(read) = read else {
                    return None;
                };
                return Some(*read);
            }
            let position = parameters.len().checked_add(
                locals[..local_position]
                    .iter()
                    .filter(|local| local.is_value_binding())
                    .count(),
            )?;
            (locals[local_position].primitive_type == PrimitiveType::Bool)
                .then_some(CheckedBooleanExpression::Local { position })
        }
        ExpressionNode::Unary(unary)
            if unary.operator == UnaryOperator::LogicalNot
                && operator_is_builtin(operators, expression) =>
        {
            Some(CheckedBooleanExpression::Not(Box::new(
                lower_boolean_expression(
                    program,
                    operators,
                    unary.operand,
                    parameters,
                    authored_parameters,
                    parameter_types,
                    locals,
                    exact_integer_casts,
                )?,
            )))
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
            ) && operator_is_builtin(operators, expression) =>
        {
            let integer_comparison = (|| {
                let ((left, _), (right, _)) = lower_scalar_operands(
                    program,
                    operators,
                    binary,
                    parameters,
                    authored_parameters,
                    parameter_types,
                    locals,
                    exact_integer_casts,
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
                byte_carrier_literal_equality(program, binary, authored_parameters)
                    .or_else(|| byte_carrier_pair_equality(program, binary, authored_parameters))
            {
                return Some(if binary.operator == BinaryOperator::NotEqual {
                    CheckedBooleanExpression::Not(Box::new(equality))
                } else {
                    equality
                });
            }
            let equality = CheckedBooleanExpression::Equal {
                left: Box::new(lower_boolean_expression(
                    program,
                    operators,
                    binary.left,
                    parameters,
                    authored_parameters,
                    parameter_types,
                    locals,
                    exact_integer_casts,
                )?),
                right: Box::new(lower_boolean_expression(
                    program,
                    operators,
                    binary.right,
                    parameters,
                    authored_parameters,
                    parameter_types,
                    locals,
                    exact_integer_casts,
                )?),
            };
            Some(if binary.operator == BinaryOperator::NotEqual {
                CheckedBooleanExpression::Not(Box::new(equality))
            } else {
                equality
            })
        }
        ExpressionNode::Binary(binary)
            if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or)
                && operator_is_builtin(operators, expression) =>
        {
            let left = Box::new(lower_boolean_expression(
                program,
                operators,
                binary.left,
                parameters,
                authored_parameters,
                parameter_types,
                locals,
                exact_integer_casts,
            )?);
            let right = Box::new(lower_boolean_expression(
                program,
                operators,
                binary.right,
                parameters,
                authored_parameters,
                parameter_types,
                locals,
                exact_integer_casts,
            )?);
            Some(if binary.operator == BinaryOperator::And {
                CheckedBooleanExpression::And { left, right }
            } else {
                CheckedBooleanExpression::Or { left, right }
            })
        }
        _ => None,
    }
}

/// `carrier == "literal"` admits no scalar operand pair: content equality is
/// the carrier's live length plus each literal byte at its fixed index. The
/// fold keeps the length comparison leftmost so short-circuit evaluation
/// dominates every indexed read, and each read's `index < length` obligation
/// is the true-path fact of that comparison.
///
/// Two carrier families have a live length and byte reads: a bounded owned
/// byte field (`[u8; N] in Utf8`), whose length is stored beside its bytes,
/// and a readable borrowed byte view (`&[u8]`, `&mut [u8]`, with or without
/// a domain such as `in Utf8`), whose length is its descriptor's extent. A
/// view is compared through the same observations wherever its storage root
/// is: a whole view parameter, a view field below a structural parameter, or
/// an immutable view local an earlier statement established. A write-only
/// view grants no observation of its bytes and a borrowed fixed array has no
/// live length, so both decline, as do element views: they count elements,
/// not the literal's bytes.
fn byte_carrier_literal_equality(
    program: &TypedTrees,
    binary: &typed_trees::expression::TableBinaryExpression,
    authored_parameters: &[StateParameter],
) -> Option<CheckedBooleanExpression> {
    for (carrier, literal) in [(binary.left, binary.right), (binary.right, binary.left)] {
        let ExpressionNode::String(bytes) = program.expression_table.expression(literal) else {
            continue;
        };
        let Some((root, path, _)) = comparable_byte_carrier(program, authored_parameters, carrier)
        else {
            continue;
        };
        return CheckedBooleanExpression::byte_literal_equality(root, path, bytes);
    }
    None
}

/// `left == right` over two live byte carriers is content equality. Two
/// bounded owned carriers (`[u8; N] in Utf8` fields) unroll: equal live
/// lengths, then position by position below the smaller capacity either the
/// left length ends there or both bytes agree and the next position is
/// compared. Equal lengths never exceed that capacity, so later positions
/// cannot differ. Each later position nests inside the previous one's
/// agreeing branch, so evaluation is one chain rather than a product of
/// independent alternatives, and every read is guarded on the path that
/// reaches it: the left read by the false path of `left.len <= index`, the
/// right read by the true path of `index < right.len`. A pair that reaches
/// any unbounded carrier has no static extent to unroll over; two
/// parameter-rooted carriers compare content through the byte-sequence
/// equality atom instead.
fn byte_carrier_pair_equality(
    program: &TypedTrees,
    binary: &typed_trees::expression::TableBinaryExpression,
    authored_parameters: &[StateParameter],
) -> Option<CheckedBooleanExpression> {
    let left = comparable_byte_carrier(program, authored_parameters, binary.left)?;
    let right = comparable_byte_carrier(program, authored_parameters, binary.right)?;
    let ((left_root, left_path, left_carrier), (right_root, right_path, right_carrier)) =
        (left, right);
    let (
        checked_trees::CheckedByteSequenceCarrier::BoundedOwned {
            capacity: left_capacity,
        },
        checked_trees::CheckedByteSequenceCarrier::BoundedOwned {
            capacity: right_capacity,
        },
    ) = (left_carrier, right_carrier)
    else {
        return byte_sequence_equal_atom(left_root, left_path, right_root, right_path);
    };
    let length = |root, path: &Vec<_>| {
        Box::new(CheckedScalarExpression::StructuralParameterByteLength {
            root,
            path: path.clone(),
        })
    };
    let read = |root, path: &Vec<_>, index| {
        Some(Box::new(
            CheckedScalarExpression::StructuralParameterIndexedRead {
                root,
                path: path.clone(),
                index: Box::new(integer_literal(index, PrimitiveType::U64)?),
                inner_indexes: Vec::new(),
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

/// The byte-sequence equality atom names parameter-rooted leaves only: a
/// local-rooted carrier has no field identity in the atom's operand shape,
/// so a pair involving one still declines the content comparison. Both
/// operands mint canonically ordered, matching the atom's own contract mint.
fn byte_sequence_equal_atom(
    left_root: checked_trees::CheckedStorageRoot,
    left_path: Vec<checked_trees::CheckedStructuralPredicatePathSegment>,
    right_root: checked_trees::CheckedStorageRoot,
    right_path: Vec<checked_trees::CheckedStructuralPredicatePathSegment>,
) -> Option<CheckedBooleanExpression> {
    let operand = |root, path| match root {
        checked_trees::CheckedStorageRoot::Parameter {
            index: parameter_position,
        } => Some(checked_trees::CheckedStructuralParameterField {
            parameter_position,
            path,
        }),
        _ => None,
    };
    let mut left = operand(left_root, left_path)?;
    let mut right = operand(right_root, right_path)?;
    if left > right {
        std::mem::swap(&mut left, &mut right);
    }
    Some(CheckedBooleanExpression::ByteSequenceEqual { left, right })
}

/// The storage root, path and carrier of a byte carrier whose live length and
/// bytes a content comparison may observe, or `None` when `carrier` names no
/// such place.
pub(super) fn comparable_byte_carrier(
    program: &TypedTrees,
    authored_parameters: &[StateParameter],
    carrier: ExpressionHandle,
) -> Option<(
    checked_trees::CheckedStorageRoot,
    Vec<checked_trees::CheckedStructuralPredicatePathSegment>,
    checked_trees::CheckedByteSequenceCarrier,
)> {
    let (root, path, type_reference) = if let Some((parameter_position, path, type_reference)) =
        structural_fields::structural_parameter_place(program, authored_parameters, carrier)
    {
        (
            checked_trees::CheckedStorageRoot::Parameter {
                index: parameter_position,
            },
            path,
            type_reference,
        )
    } else if let Some((symbol, path, type_reference)) =
        structural_fields::owned_local_place(program, authored_parameters, carrier)
    {
        (
            checked_trees::CheckedStorageRoot::OwnedLocal { symbol },
            path,
            type_reference,
        )
    } else {
        let (symbol, path, type_reference) =
            structural_fields::view_local_place(program, authored_parameters, carrier)?;
        (
            checked_trees::CheckedStorageRoot::ViewLocal { symbol },
            path,
            type_reference,
        )
    };
    // The selected leaf defines its byte carrier. Its enclosing record may
    // own storage while this field borrows bytes; retain both identities so
    // downstream observation replay checks the actual backing and extent.
    let carrier = comparable_byte_carrier_type(program, type_reference)?;
    Some((root, path, carrier))
}

/// The carrier of `type_reference` when it has a live byte length and
/// readable bytes: a bounded owned byte buffer, or a shared or mutable
/// borrowed byte view.
pub(super) fn comparable_byte_carrier_type(
    program: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Option<checked_trees::CheckedByteSequenceCarrier> {
    crate::execution::terminal_unit::types::byte_sequence_carrier(program, type_reference, &[])
        .filter(|carrier| {
            matches!(
                carrier,
                checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. }
                    | checked_trees::CheckedByteSequenceCarrier::BorrowedView {
                        access: Some(
                            checked_trees::CheckedStructuralAccess::SharedBorrow
                                | checked_trees::CheckedStructuralAccess::MutableBorrow
                        ),
                    }
            )
        })
}

fn integer_literal(value: u64, primitive_type: PrimitiveType) -> Option<CheckedScalarExpression> {
    let landed_type = match primitive_type {
        PrimitiveType::U8 => numerics::literals::LandedIntegerType::U8,
        PrimitiveType::U64 => numerics::literals::LandedIntegerType::U64,
        _ => return None,
    };
    let literal = numerics::literals::IntegerLiteral::from_parts(
        false,
        numerics::literals::IntegerRadix::Decimal,
        &value.to_string(),
    )
    .ok()?
    .with_landing(numerics::literals::IntegerLanding {
        landed_type,
        domain: ArithmeticDomain::Exact,
    });
    Some(CheckedScalarExpression::IntegerLiteral { literal })
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
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    authored_parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<CheckedBooleanExpression> {
    if let Some(value) = lower_closed_integer_literal_guard(program, operators, expression) {
        return Some(value);
    }
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return lower_boolean_expression(
            program,
            operators,
            expression,
            parameters,
            authored_parameters,
            parameter_types,
            locals,
            exact_integer_casts,
        );
    };
    if binary.operator == BinaryOperator::Equal && operator_is_builtin(operators, expression) {
        match (
            program.expression_table.expression(binary.left),
            program.expression_table.expression(binary.right),
        ) {
            (ExpressionNode::Boolean(true), _) => {
                return lower_boolean_expression(
                    program,
                    operators,
                    binary.right,
                    parameters,
                    authored_parameters,
                    parameter_types,
                    locals,
                    exact_integer_casts,
                );
            }
            (_, ExpressionNode::Boolean(true)) => {
                return lower_boolean_expression(
                    program,
                    operators,
                    binary.left,
                    parameters,
                    authored_parameters,
                    parameter_types,
                    locals,
                    exact_integer_casts,
                );
            }
            _ => {}
        }
    }
    lower_boolean_expression(
        program,
        operators,
        expression,
        parameters,
        authored_parameters,
        parameter_types,
        locals,
        exact_integer_casts,
    )
}
