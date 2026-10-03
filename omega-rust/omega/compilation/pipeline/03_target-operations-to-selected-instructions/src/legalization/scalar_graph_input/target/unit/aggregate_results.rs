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
                                terminal_semantics::scalar_field_requires_evidence(&declaration.field_type)
                                    != range_obligation.is_some()
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
                                        | terminal_psi::StructuralFieldType::ByteSequence(
                                            terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                                                | terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                                        )
                                        | terminal_psi::StructuralFieldType::PredicateByteSequence {
                                            carrier: terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                                                | terminal_psi::ByteSequenceCarrier::BorrowedView { .. }, ..
                                        }
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
            // Each runtime index must reference the selector's dominating
            // definition — parameter, block parameter, or computed value —
            // the same source the lowered operand already proved; custody
            // compares value, integer type and stride, not the operand
            // variant the producer dispatched.
            let expected_indices = expected_indices
                .into_iter()
                .map(|element| {
                    let scalar_type = crate::legalization::runtime_indices::defined_type(
                        optimized,
                        element.index,
                    )
                    .filter(|scalar| {
                        matches!(scalar, semantic_vocabulary::ScalarType::Integer(integer)
                            if integer.bits() <= 64)
                    })
                    .ok_or(LegalizationError::custody())?;
                    Ok::<_, LegalizationError>((element.index, scalar_type, element.stride))
                })
                .collect::<Result<Vec<_>, _>>()?;
            if psi_operation != expected_operation
                || source != expected_source
                || path != expected_path
                || *byte_offset != expected_offset
                || indices.len() != expected_indices.len()
                || indices.iter().zip(&expected_indices).any(
                    |(index, (value, scalar_type, stride))| {
                        index.stride != *stride
                            || index.operand.source_value() != *value
                            || index.operand.scalar_type() != *scalar_type
                    },
                )
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
            TargetUnitOperation::EstablishCase {
                psi_operation,
                result_home,
                result_case,
                fields,
            },
            AbstractOperation::EstablishCase {
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
            // A mixed case binds its runtime common fields, then the selected
            // payload, in the order Terminal verification checked.
            let (common, cases) = match &declaration.shape {
                terminal_psi::StructuralTypeShape::Sum { cases } => (&[][..], cases),
                terminal_psi::StructuralTypeShape::Mixed { fields, cases } => {
                    (fields.as_slice(), cases)
                }
                _ => return Err(LegalizationError::custody()),
            };
            let case = cases
                .iter()
                .find(|case| case.id == *result_case)
                .ok_or(LegalizationError::custody())?;
            let roster = common
                .iter()
                .chain(&case.fields)
                .filter(|field| !field.relevance.is_erased())
                .collect::<Vec<_>>();
            if fields.len() != roster.len() {
                return Err(LegalizationError::custody());
            }
            for (field, declared) in fields.iter().zip(roster) {
                if field.field != declared.id || declared.relevance.is_erased() {
                    return Err(LegalizationError::custody());
                }
                match &field.value {
                    terminal_psi::RecordFieldValue::Scalar { value, range_obligation } => {
                        if !sources.iter().any(|(source_value, source)| *source_value == *value
                            && declared.field_type.scalar_type() == Some(source.scalar_type()))
                            || terminal_semantics::scalar_field_requires_evidence(&declared.field_type) != range_obligation.is_some()
                            || range_obligation.is_some_and(|obligation|
                                !unit.accepted_obligation_facts.iter().any(|fact| fact.machine == optimized.machine
                                    && fact.operation == *psi_operation && fact.obligation == obligation)
                                || !optimized.facts.iter().any(|fact| matches!(fact,
                                    optimization_unit::OptimizationFact::OperationObligationReference { obligation: retained, support }
                                    if *retained == obligation && support == psi_operation)))
                        { return Err(LegalizationError::custody()); }
                    }
                    terminal_psi::RecordFieldValue::Structural(argument) => {
                        let terminal_psi::StructuralFieldType::Structural(expected) = declared.field_type else {
                            return Err(LegalizationError::custody());
                        };
                        if argument.access != terminal_psi::StructuralAccess::Owned
                            || !argument.path.is_empty() || argument.place == result.place
                        { return Err(LegalizationError::custody()); }
                        let (actual, multiplicity) = owned_field_source(optimized, argument.place)?;
                        if actual != expected || (result.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                            && multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted)
                        { return Err(LegalizationError::custody()); }
                    }
                }
            }
        }
        _ => return Err(LegalizationError::custody()),
    }
    Ok(())
}

/// Structural payloads retain their exact owned producer or incoming binding.
fn owned_field_source(
    function: &PsiOptimizationFunction,
    place: semantic_vocabulary::PlaceId,
) -> Result<
    (
        semantic_vocabulary::StructuralTypeId,
        terminal_psi::StructuralMultiplicity,
    ),
    LegalizationError,
> {
    let parameters = function
        .structural_parameters
        .iter()
        .chain(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .filter(|parameter| parameter.place == place)
        .map(|parameter| {
            (
                parameter.structural_type,
                parameter.multiplicity,
                parameter.access == terminal_psi::StructuralAccess::Owned
                    && parameter.qualifications.is_empty()
                    && parameter.projected_qualifications.is_empty(),
            )
        });
    let result = scalar_graph_input::structural_case::source_result(function, place)
        .ok()
        .map(|(_, result)| {
            (
                result.structural_type,
                result.multiplicity,
                result.claims.is_empty()
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty(),
            )
        });
    let mut sources = parameters.chain(result);
    let (structural_type, multiplicity, plain) =
        sources.next().ok_or(LegalizationError::custody())?;
    if sources.next().is_some()
        || !plain
        || multiplicity == terminal_psi::StructuralMultiplicity::Linear
    {
        return Err(LegalizationError::custody());
    }
    Ok((structural_type, multiplicity))
}
