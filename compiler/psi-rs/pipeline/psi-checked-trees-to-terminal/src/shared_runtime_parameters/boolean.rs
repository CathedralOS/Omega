//! Boolean and relevant structural-field runtime-input custody.

use super::*;

pub(super) fn valid_shared_boolean_runtime_inputs(
    inputs: &BTreeSet<SharedBooleanRuntimeInput>,
) -> bool {
    let has_structural_field = inputs
        .iter()
        .any(|input| matches!(input, SharedBooleanRuntimeInput::StructuralField { .. }));
    !inputs.is_empty()
        && (!has_structural_field
            || (inputs
                .iter()
                .any(|input| matches!(input, SharedBooleanRuntimeInput::BooleanScalar(_)))
                && !inputs
                    .iter()
                    .any(|input| matches!(input, SharedBooleanRuntimeInput::IntegerScalar(_)))))
}

pub(super) fn resolve_shared_boolean_member_fields(
    expression: LoweredBooleanReturnExpression,
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<LoweredBooleanReturnExpression, LoweringError> {
    Ok(match expression {
        LoweredBooleanReturnExpression::UnresolvedStructuralParameterField {
            parameter_position,
            path,
        } => {
            let [field_name] = path.as_slice() else {
                return unsupported(
                    "shared Boolean convergence admits only one direct structural field",
                );
            };
            let parameter = parameters
                .iter()
                .find(|parameter| parameter.position == parameter_position)
                .filter(|parameter| {
                    parameter.multiplicity == StructuralMultiplicity::Affine
                        && parameter.qualifications.is_empty()
                })
                .ok_or(LoweringError::Unsupported(
                    "shared Boolean member source is not one claim-free affine parameter",
                ))?;
            let declaration = structural_types
                .iter()
                .find(|declaration| declaration.id == parameter.structural_type)
                .ok_or(LoweringError::Unsupported(
                    "shared Boolean member source type is absent",
                ))?;
            let StructuralTypeShape::Record { fields } = &declaration.shape else {
                return unsupported("shared Boolean member source is not a record");
            };
            let field = fields
                .iter()
                .find(|field| field.identity == *field_name)
                .filter(|field| {
                    !field.relevance.is_erased()
                        && field.field_type == StructuralFieldType::Scalar(ScalarType::Boolean)
                })
                .ok_or(LoweringError::Unsupported(
                    "shared Boolean member is absent, erased, or non-Boolean",
                ))?;
            LoweredBooleanReturnExpression::StructuralField {
                source: parameter.place,
                field: field.id,
            }
        }
        LoweredBooleanReturnExpression::Not { operand } => LoweredBooleanReturnExpression::Not {
            operand: Box::new(resolve_shared_boolean_member_fields(
                *operand,
                parameters,
                structural_types,
            )?),
        },
        LoweredBooleanReturnExpression::Equal { left, right } => {
            LoweredBooleanReturnExpression::Equal {
                left: Box::new(resolve_shared_boolean_member_fields(
                    *left,
                    parameters,
                    structural_types,
                )?),
                right: Box::new(resolve_shared_boolean_member_fields(
                    *right,
                    parameters,
                    structural_types,
                )?),
            }
        }
        LoweredBooleanReturnExpression::And { left, right } => {
            LoweredBooleanReturnExpression::And {
                left: Box::new(resolve_shared_boolean_member_fields(
                    *left,
                    parameters,
                    structural_types,
                )?),
                right: Box::new(resolve_shared_boolean_member_fields(
                    *right,
                    parameters,
                    structural_types,
                )?),
            }
        }
        LoweredBooleanReturnExpression::Or { left, right } => LoweredBooleanReturnExpression::Or {
            left: Box::new(resolve_shared_boolean_member_fields(
                *left,
                parameters,
                structural_types,
            )?),
            right: Box::new(resolve_shared_boolean_member_fields(
                *right,
                parameters,
                structural_types,
            )?),
        },
        expression => expression,
    })
}

/// Normalize the comparison leaves accepted by the checked shared-convergence
/// plan into the existing identity/negation carrier. Boolean equality is
/// admitted only when at least one operand is constant. Checked integer
/// comparisons retain their exact operation and bounded total-computation
/// operands. The one already-resolved structural-field leaf is preserved
/// unchanged.
pub(super) fn normalize_shared_boolean_comparison_leaves(
    expression: &LoweredBooleanReturnExpression,
) -> Option<LoweredBooleanReturnExpression> {
    Some(match expression {
        LoweredBooleanReturnExpression::Constant { .. }
        | LoweredBooleanReturnExpression::Parameter { .. }
        | LoweredBooleanReturnExpression::StructuralField { .. }
        | LoweredBooleanReturnExpression::IntegerComparison { .. } => expression.clone(),
        LoweredBooleanReturnExpression::Not { operand } => LoweredBooleanReturnExpression::Not {
            operand: Box::new(normalize_shared_boolean_comparison_leaves(operand)?),
        },
        LoweredBooleanReturnExpression::Equal { left, right } => {
            let left = normalize_shared_boolean_comparison_leaves(left)?;
            let right = normalize_shared_boolean_comparison_leaves(right)?;
            match (left, right) {
                (
                    LoweredBooleanReturnExpression::Constant { value: left },
                    LoweredBooleanReturnExpression::Constant { value: right },
                ) => LoweredBooleanReturnExpression::Constant {
                    value: left == right,
                },
                (LoweredBooleanReturnExpression::Constant { value: true }, expression)
                | (expression, LoweredBooleanReturnExpression::Constant { value: true }) => {
                    expression
                }
                (LoweredBooleanReturnExpression::Constant { value: false }, expression)
                | (expression, LoweredBooleanReturnExpression::Constant { value: false }) => {
                    LoweredBooleanReturnExpression::Not {
                        operand: Box::new(expression),
                    }
                }
                _ => return None,
            }
        }
        LoweredBooleanReturnExpression::And { left, right } => {
            LoweredBooleanReturnExpression::And {
                left: Box::new(normalize_shared_boolean_comparison_leaves(left)?),
                right: Box::new(normalize_shared_boolean_comparison_leaves(right)?),
            }
        }
        LoweredBooleanReturnExpression::Or { left, right } => LoweredBooleanReturnExpression::Or {
            left: Box::new(normalize_shared_boolean_comparison_leaves(left)?),
            right: Box::new(normalize_shared_boolean_comparison_leaves(right)?),
        },
        LoweredBooleanReturnExpression::Local { .. }
        | LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. } => return None,
    })
}
