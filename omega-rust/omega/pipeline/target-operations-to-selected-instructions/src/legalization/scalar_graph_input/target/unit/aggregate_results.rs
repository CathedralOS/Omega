//! Rejoin aggregate constructors to their exact source operations.
use super::{
    AbstractOperation, AbstractOperationPlan, PsiOptimizationFunction, PsiOptimizationUnit, Source,
    TargetUnitOperation, ValueId,
};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input;

pub(super) fn validate(
    target: &TargetUnitOperation,
    abstracted: &AbstractOperation,
    sources: &[(ValueId, Source)],
    optimized: &PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    match (target, abstracted) {
        (
            TargetUnitOperation::EstablishRecord {
                psi_operation,
                result_home,
                fields,
            },
            AbstractOperation::EstablishRecord {
                psi_operation: expected_operation,
                result,
                fields: expected_fields,
            },
        ) => {
            let declaration = plan
                .structural_types
                .iter()
                .find(|declaration| declaration.id == result.structural_type)
                .ok_or(LegalizationError::custody())?;
            let terminal_psi::StructuralTypeShape::Record {
                fields: declarations,
            } = &declaration.shape
            else {
                return Err(LegalizationError::custody());
            };
            if psi_operation != expected_operation
                || fields != expected_fields
                || fields.len() != declarations.len()
                || *result_home
                    != scalar_graph_input::aggregate_results::result_home(
                        optimized,
                        result.place,
                        plan,
                    )?
                || fields.iter().zip(declarations).any(|(field, declaration)| {
                    field.field != declaration.id
                        || declaration.relevance.is_erased()
                        || match &field.value {
                            terminal_psi::RecordFieldValue::Scalar {
                                value: operand,
                                range_obligation,
                            } => {
                                matches!(
                                    declaration.field_type,
                                    terminal_psi::StructuralFieldType::BoundedInteger(_)
                                ) != range_obligation.is_some()
                                    || !sources.iter().any(|(value, source)| {
                                        *value == *operand
                                            && declaration.field_type.scalar_type()
                                                == Some(source.scalar_type())
                                    })
                            }
                            terminal_psi::RecordFieldValue::Structural(argument) => {
                                !matches!(
                                    declaration.field_type,
                                    terminal_psi::StructuralFieldType::Structural(_)
                                ) || argument.access != terminal_psi::StructuralAccess::Owned
                                    || !argument.path.is_empty()
                            }
                        }
                })
            {
                return Err(LegalizationError::custody());
            }
        }
        (
            TargetUnitOperation::EstablishScalarArray {
                psi_operation,
                result_home,
                elements,
            },
            AbstractOperation::EstablishScalarArray {
                psi_operation: expected_operation,
                result,
                elements: expected_elements,
            },
        ) => {
            let (scalar_type, count, _) = scalar_graph_input::scalar_arrays::shape(result, plan)?;
            if psi_operation != expected_operation
                || elements != expected_elements
                || u64::try_from(elements.len()).ok() != Some(count)
                || *result_home
                    != scalar_graph_input::aggregate_results::result_home(
                        optimized,
                        result.place,
                        plan,
                    )?
                || elements.iter().any(|element| {
                    !sources.iter().any(|(value, source)| {
                        value == element && source.scalar_type() == scalar_type
                    })
                })
            {
                return Err(LegalizationError::custody());
            }
        }
        (
            TargetUnitOperation::StructuralLeafCopy {
                psi_operation,
                result_home,
                source,
                path,
                byte_offset,
                indices,
            },
            AbstractOperation::StructuralLeafCopy {
                psi_operation: expected_operation,
                result,
                source: expected_source,
                path: expected_path,
            },
        ) => {
            let (expected_offset, expected_shape, expected_indices) =
                scalar_graph_input::structural_case::leaf_copy_layout(
                    optimized,
                    *expected_source,
                    expected_path,
                    result,
                    plan,
                )?;
            let expected_indices = expected_indices
                .into_iter()
                .map(|(selector, stride)| {
                    let parameter = usize::try_from(selector)
                        .ok()
                        .and_then(|position| optimized.parameters.get(position))
                        .ok_or(LegalizationError::custody())?;
                    Ok::<_, LegalizationError>(target_operations::TargetStructuralRuntimeIndex {
                        operand: target_operations::TargetUnitScalarArgumentSource::Parameter {
                            parameter_index: selector,
                            source_value: parameter.value,
                            scalar_type: parameter.scalar_type,
                        },
                        stride,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            if psi_operation != expected_operation
                || source != expected_source
                || path != expected_path
                || *byte_offset != expected_offset
                || *indices != expected_indices
                || *result_home
                    != scalar_graph_input::aggregate_results::result_home(
                        optimized,
                        result.place,
                        plan,
                    )?
                || result_home.layout.shape() != expected_shape
            {
                return Err(LegalizationError::custody());
            }
        }
        (
            TargetUnitOperation::EstablishScalarCase {
                psi_operation,
                result_home,
                result_case,
                fields,
            },
            AbstractOperation::EstablishScalarCase {
                psi_operation: expected_operation,
                result,
                result_case: expected_case,
                fields: expected_fields,
            },
        ) => {
            if psi_operation != expected_operation
                || result_case != expected_case
                || fields != expected_fields
                || *result_home
                    != scalar_graph_input::aggregate_results::result_home(
                        optimized,
                        result.place,
                        plan,
                    )?
            {
                return Err(LegalizationError::custody());
            }
            let declaration = plan
                .structural_types
                .iter()
                .find(|declaration| declaration.id == result.structural_type)
                .ok_or(LegalizationError::custody())?;
            let terminal_psi::StructuralTypeShape::Sum { cases } = &declaration.shape else {
                return Err(LegalizationError::custody());
            };
            let case = cases
                .iter()
                .find(|case| case.id == *result_case)
                .ok_or(LegalizationError::custody())?;
            if fields.len() != case.fields.len() {
                return Err(LegalizationError::custody());
            }
            for (field, declared) in fields.iter().zip(&case.fields) {
                if field.field != declared.id || !sources.iter().any(|(value, source)| *value == field.value && declared.field_type.scalar_type() == Some(source.scalar_type()))
                    || matches!(declared.field_type, terminal_psi::StructuralFieldType::BoundedInteger(_)) != field.range_obligation.is_some()
                    || field.range_obligation.is_some_and(|obligation| !unit.accepted_obligation_facts.iter().any(|fact|
                        fact.machine == optimized.machine && fact.operation == *psi_operation && fact.obligation == obligation)
                        || !optimized.facts.iter().any(|fact| matches!(fact,
                            optimization_unit::OptimizationFact::OperationObligationReference { obligation: retained, support }
                            if *retained == obligation && support == psi_operation)))
                { return Err(LegalizationError::custody()); }
            }
        }
        _ => return Err(LegalizationError::custody()),
    }
    Ok(())
}
