//! Resolve storage observations before classifying their scalar or sequence
//! endpoint. Authored indexes retain occurrences even when their values are
//! literals: a runtime view still owes a live bound, and representation views
//! need the selected element rather than an already folded scalar field.

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
use typed_trees::types::PrimitiveType;

/// A storage observation retains ordered selections before its endpoint is
/// classified as a scalar or a sequence. Selectors carry both their evaluated
/// value and the source occurrence that owns the bound.
pub(crate) struct ScalarStorageProjection {
    pub root: checked_trees::CheckedStorageRoot,
    pub path: Vec<checked_trees::CheckedScalarReadProjection>,
    pub selected_type: typed_trees::types::TypeReferenceHandle,
}

/// Prepare one scalar indexed observation independently of the arrangement of
/// indexes and fields after its first collection selection. Every runtime
/// selector retains its full scalar value and exact indexing occurrence.
/// Storage roots and declared carriers, not source nesting families, determine
/// how the observation is emitted. Static literal geometry remains explicit.
#[allow(clippy::too_many_arguments)]
pub(crate) fn scalar_storage_read(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    authored_parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<(CheckedScalarExpression, ArithmeticDomain)> {
    scalar_storage_read_with_selectors(
        program,
        operators,
        expression,
        authored_parameters,
        |_, operand, _| {
            pure_selector(
                program,
                operators,
                operand,
                parameters,
                authored_parameters,
                parameter_types,
                locals,
                exact_integer_casts,
            )
        },
    )
}

/// Prepare the same observation with selectors supplied by the consuming
/// scalar namespace. The reader owns indexing occurrences and declaration
/// geometry; supplying a value never supplies an indexing proof.
pub(crate) fn scalar_storage_read_with_selectors(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    authored_parameters: &[StateParameter],
    selector: impl FnMut(
        ExpressionHandle,
        ExpressionHandle,
        typed_trees::types::TypeReferenceHandle,
    ) -> Option<CheckedScalarExpression>,
) -> Option<(CheckedScalarExpression, ArithmeticDomain)> {
    let projection = storage_projection(
        program,
        operators,
        expression,
        authored_parameters,
        selector,
    )?;
    let first_index = projection.path.iter().position(|segment| {
        matches!(
            segment,
            checked_trees::CheckedScalarReadProjection::RuntimeIndex(_)
        )
    });
    let Some(first_index) = first_index else {
        let parameter_position = projection.root.parameter()?;
        let path = projection
            .path
            .into_iter()
            .map(|segment| match segment {
                checked_trees::CheckedScalarReadProjection::Static(segment) => Some(segment),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;
        let mut reference = projection.selected_type;
        if path.is_empty() {
            let shell = validation::reference_shell(program, reference)?;
            if !matches!(
                shell.access,
                language_core::ReferenceAccess::Shared | language_core::ReferenceAccess::Mutable
            ) {
                return None;
            }
            reference = validation::field_domains::readable_type_reference(program, reference)?;
        }
        let primitive_type = program.primitive_type_reference(reference)?;
        let expression = if primitive_type == PrimitiveType::Bool {
            CheckedScalarExpression::Boolean(Box::new(
                checked_trees::CheckedBooleanExpression::StructuralParameterField {
                    parameter_position,
                    path,
                },
            ))
        } else {
            if !is_integer(primitive_type)
                && !matches!(primitive_type, PrimitiveType::F32 | PrimitiveType::F64)
            {
                return None;
            }
            CheckedScalarExpression::StructuralParameterField {
                parameter_position,
                path,
                primitive_type,
            }
        };
        return Some((
            expression,
            program.arithmetic_domain_for_type_reference(reference),
        ));
    };
    let mut segments = projection.path.into_iter();
    let path = segments
        .by_ref()
        .take(first_index)
        .map(|segment| match segment {
            checked_trees::CheckedScalarReadProjection::Static(segment) => Some(segment),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    let checked_trees::CheckedScalarReadProjection::RuntimeIndex(index) = segments.next()? else {
        return None;
    };
    let readable_type =
        validation::field_domains::readable_type_reference(program, projection.selected_type)?;
    let primitive_type = program.primitive_type_reference(readable_type)?;
    if (!is_integer(primitive_type)
        && !matches!(
            primitive_type,
            PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64
        ))
        || primitive_type == PrimitiveType::Addr
    {
        return None;
    }
    Some((
        CheckedScalarExpression::StructuralParameterIndexedRead {
            root: projection.root,
            path,
            index: Box::new(index),
            element_path: segments.collect(),
            primitive_type,
        },
        program.arithmetic_domain_for_type_reference(projection.selected_type),
    ))
}

pub(crate) fn storage_projection(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    authored_parameters: &[StateParameter],
    mut selector: impl FnMut(
        ExpressionHandle,
        ExpressionHandle,
        typed_trees::types::TypeReferenceHandle,
    ) -> Option<CheckedScalarExpression>,
) -> Option<ScalarStorageProjection> {
    enum Projection {
        Field(ExpressionHandle),
        Index {
            occurrence: ExpressionHandle,
            operand: ExpressionHandle,
        },
    }
    let mut projections = Vec::new();
    let mut cursor = expression;
    let (root, path, mut element_type) = loop {
        if let Some(storage) =
            structural_fields::structural_storage_place(program, authored_parameters, cursor)
            && !storage.1.iter().any(|segment| {
                matches!(
                    segment,
                    CheckedStructuralPredicatePathSegment::FixedIndex(_)
                        | CheckedStructuralPredicatePathSegment::RuntimeIndex(_)
                )
            })
        {
            break storage;
        }
        match program.expression_table.expression(cursor) {
            ExpressionNode::Indexed(indexed) => {
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
    let mut element_path = path
        .into_iter()
        .map(checked_trees::CheckedScalarReadProjection::Static)
        .collect::<Vec<_>>();
    for projection in projections.into_iter().rev() {
        match projection {
            Projection::Field(occurrence) => {
                let field = structural_fields::element_member_field(
                    program,
                    element_type,
                    program.expression_table.member_node(occurrence)?,
                )?;
                element_path.push(checked_trees::CheckedScalarReadProjection::Static(
                    CheckedStructuralPredicatePathSegment::Field(
                        checked_trees::CheckedMemberIdentity::of_member(
                            field.identity,
                            field.name.clone(),
                        ),
                    ),
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
                let value = selector(occurrence, operand, element_type)?;
                let carrier = scalar_expression_type(&value)?;
                if !is_integer(carrier) || carrier == PrimitiveType::Addr {
                    return None;
                }
                // Executable reads include byte views. The static-place
                // geometry helper deliberately excludes those descriptors;
                // source operator admission above authenticates this selection.
                element_type =
                    validation::collection_element_type_reference(program, element_type)?;
                element_path.push(checked_trees::CheckedScalarReadProjection::RuntimeIndex(
                    checked_trees::CheckedScalarSelector {
                        expression: value,
                        occurrence,
                    },
                ));
            }
        }
    }
    Some(ScalarStorageProjection {
        root,
        path: element_path,
        selected_type: element_type,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn pure_selector(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    operand: ExpressionHandle,
    parameters: &[StateParameter],
    authored_parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<CheckedScalarExpression> {
    land_anonymous_scalar_expression(program, operators, operand, PrimitiveType::U64).or_else(
        || {
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
            .map(|(value, _)| value)
        },
    )
}

/// `literal`'s bit-width admission against `landed_type` plus the landed
/// expression it mints — both integer-literal lowering paths share the
/// signed/unsigned bound test and the `Exact` domain landing.
pub(crate) fn landed_integer_literal_fits(
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
