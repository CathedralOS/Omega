//! element selection slice of scalar_lowering.

use super::entry_lowering::*;
use crate::values::scalar::expression_facts::{is_integer, scalar_expression_type};
use crate::values::scalar::expression_plans::ScalarLocal;
use crate::values::scalar::structural_fields;
use checked_trees::{
    CheckedOperatorFacts, CheckedScalarExpression, CheckedStructuralPredicatePathSegment,
};
use numerics::arithmetic::ArithmeticDomain;
use numerics::literals::{IntegerLanding, LandedIntegerType};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;
use typed_trees::types::{PrimitiveType, TypeReferenceNode};

/// One builtin element selection `collection[index]` over current structural
/// storage: the root and projection the collection names, the selected
/// element's type, whether the collection is a borrowed view, and the lowered
/// selector. A whole view local reads the view its establishment published,
/// the same element selection a whole view parameter takes.
pub(in crate::values::scalar) struct ElementSelection {
    pub(in crate::values::scalar) root: checked_trees::CheckedStorageRoot,
    pub(in crate::values::scalar) path: Vec<checked_trees::CheckedStructuralPredicatePathSegment>,
    pub(in crate::values::scalar) element_type: typed_trees::types::TypeReferenceHandle,
    pub(in crate::values::scalar) through_view: bool,
    pub(in crate::values::scalar) index: checked_trees::CheckedScalarSelector,
}

#[allow(clippy::too_many_arguments)]
pub(in crate::values::scalar) fn selected_element(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    authored_parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<ElementSelection> {
    let indexed = program.expression_table.indexed_node(expression)?;
    let (root, path, collection_type) = structural_fields::structural_storage_place(
        program,
        authored_parameters,
        indexed.collection,
    )?;
    if !structural_fields::indexed_read_is_builtin(
        program,
        operators,
        authored_parameters,
        expression,
        collection_type,
        indexed.index,
    ) {
        return None;
    }
    let collection_carrier = program
        .type_reference_table
        .peeled_type_reference(collection_type);
    let through_view = match program
        .type_reference_table
        .type_reference(collection_carrier)
    {
        TypeReferenceNode::FixedArray { .. } => false,
        TypeReferenceNode::Slice { .. } => true,
        _ => return None,
    };
    // Geometry uses the storage carrier; the observed value keeps the exact
    // effective element policy retained by the shared projection owner.
    let element_type = validation::collection_element_type_reference(program, collection_type)?;
    let index =
        land_anonymous_scalar_expression(program, operators, indexed.index, PrimitiveType::U64)
            .or_else(|| {
                lower_scalar_expression(
                    program,
                    operators,
                    indexed.index,
                    parameters,
                    authored_parameters,
                    parameter_types,
                    locals,
                    exact_integer_casts,
                )
                .map(|(index, _)| index)
            })?;
    let index_type = scalar_expression_type(&index)?;
    if !is_integer(index_type) || index_type == PrimitiveType::Addr {
        return None;
    }
    Some(ElementSelection {
        root,
        path,
        element_type,
        through_view,
        index: checked_trees::CheckedScalarSelector {
            expression: index,
            occurrence: expression,
        },
    })
}

/// Prepare one scalar indexed observation independently of the arrangement of
/// indexes and fields after its first collection selection. Every runtime
/// selector retains its full scalar value and exact indexing occurrence.
/// Storage roots and declared carriers, not source nesting families, determine
/// how the observation is emitted. Static literal geometry remains explicit.
#[allow(clippy::too_many_arguments)]
pub(super) fn indexed_scalar_read(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    authored_parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<(CheckedScalarExpression, ArithmeticDomain)> {
    enum Projection {
        Field(ExpressionHandle),
        Index {
            occurrence: ExpressionHandle,
            operand: ExpressionHandle,
        },
    }
    let mut projections = Vec::new();
    let mut cursor = expression;
    let first_selection = loop {
        match program.expression_table.expression(cursor) {
            ExpressionNode::Indexed(indexed) => {
                if structural_fields::structural_storage_place(
                    program,
                    authored_parameters,
                    indexed.collection,
                )
                .is_some()
                {
                    break cursor;
                }
                if matches!(
                    program.expression_table.expression(indexed.index),
                    ExpressionNode::Range(_)
                ) {
                    return None;
                }
                projections.push(Projection::Index {
                    occurrence: cursor,
                    operand: indexed.index,
                });
                cursor = indexed.collection;
            }
            ExpressionNode::Member(member) => {
                if member.case_variant.is_some() {
                    return None;
                }
                projections.push(Projection::Field(cursor));
                cursor = member.receiver;
            }
            _ => return None,
        }
    };
    let selection = selected_element(
        program,
        operators,
        first_selection,
        parameters,
        authored_parameters,
        parameter_types,
        locals,
        exact_integer_casts,
    )?;
    let mut element_type = selection.element_type;
    let mut element_path = Vec::with_capacity(projections.len());
    for projection in projections.into_iter().rev() {
        match projection {
            Projection::Field(occurrence) => {
                let field = structural_fields::element_member_field(
                    program,
                    element_type,
                    program.expression_table.member_node(occurrence)?,
                )?;
                element_path.push(checked_trees::CheckedScalarReadProjection::Static(
                    CheckedStructuralPredicatePathSegment::Field(field.path_identity()),
                ));
                element_type = field.type_reference;
            }
            Projection::Index {
                occurrence,
                operand,
            } => {
                if !structural_fields::indexed_read_is_builtin(
                    program,
                    operators,
                    authored_parameters,
                    occurrence,
                    element_type,
                    operand,
                ) {
                    return None;
                }
                if let Some(literal) = program
                    .expression_table
                    .constant_integer_value(operand)
                    .and_then(|literal| u64::try_from(literal).ok())
                {
                    element_type = structural_fields::fixed_index_element_type(
                        program,
                        element_type,
                        literal,
                    )?;
                    element_path.push(checked_trees::CheckedScalarReadProjection::Static(
                        CheckedStructuralPredicatePathSegment::FixedIndex(literal),
                    ));
                } else {
                    let selector = land_anonymous_scalar_expression(
                        program,
                        operators,
                        operand,
                        PrimitiveType::U64,
                    )
                    .or_else(|| {
                        lower_scalar_expression(
                            program,
                            operators,
                            operand,
                            parameters,
                            authored_parameters,
                            parameter_types,
                            locals,
                            exact_integer_casts,
                        )
                        .map(|(selector, _)| selector)
                    })?;
                    let carrier = scalar_expression_type(&selector)?;
                    if !is_integer(carrier) || carrier == PrimitiveType::Addr {
                        return None;
                    }
                    element_type =
                        structural_fields::runtime_index_element_type(program, element_type)?;
                    element_path.push(checked_trees::CheckedScalarReadProjection::RuntimeIndex(
                        checked_trees::CheckedScalarSelector {
                            expression: selector,
                            occurrence,
                        },
                    ));
                }
            }
        }
    }
    let primitive_type = program.primitive_type_reference(element_type)?;
    if !is_integer(primitive_type)
        && !matches!(
            primitive_type,
            PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64
        )
        || primitive_type == PrimitiveType::Addr
    {
        return None;
    }
    Some((
        CheckedScalarExpression::StructuralParameterIndexedRead {
            root: selection.root,
            path: selection.path,
            index: Box::new(selection.index),
            element_path,
            primitive_type,
        },
        program.arithmetic_domain_for_type_reference(element_type),
    ))
}

/// `literal`'s bit-width admission against `landed_type` plus the landed
/// expression it mints — both integer-literal lowering paths share the
/// signed/unsigned bound test and the `Exact` domain landing.
pub(super) fn landed_integer_literal_fits(
    literal: &numerics::literals::IntegerLiteral,
    landed_type: LandedIntegerType,
) -> Option<CheckedScalarExpression> {
    let fits = if landed_type.is_signed() {
        let value = literal.value_i64()?;
        let bits = landed_type.bit_width();
        let minimum = -(1_i128 << (bits - 1));
        let maximum = (1_i128 << (bits - 1)) - 1;
        let value = i128::from(value);
        minimum <= value && value <= maximum
    } else {
        let value = literal.value_u64()?;
        let bits = landed_type.bit_width();
        let maximum = if bits == 64 {
            u64::MAX
        } else {
            (1_u64 << bits) - 1
        };
        value <= maximum
    };
    fits.then(|| CheckedScalarExpression::IntegerLiteral {
        literal: literal.with_landing(IntegerLanding {
            landed_type,
            domain: ArithmeticDomain::Exact,
        }),
    })
}
