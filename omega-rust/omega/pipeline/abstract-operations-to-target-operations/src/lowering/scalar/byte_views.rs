//! Immutable byte observations shared by straight-line and conditional lowering.

use super::*;

pub(super) fn is_immutable_byte_parameter(
    parameter: &terminal_psi::StructuralParameterDeclaration,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> bool {
    !parameter.is_self
        && parameter.access == StructuralAccess::SharedBorrow
        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
        && parameter.qualifications.is_empty()
        && parameter.projected_qualifications.is_empty()
        && structural_types
            .get(&parameter.structural_type)
            .is_some_and(|declaration| {
                matches!(
                    declaration.shape,
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView
                    )
                )
            })
}

pub(super) fn lower_byte_observation(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    parameters: &[TargetStructuralParameter],
    values: &mut BTreeMap<ValueId, KnownScalar>,
    provenance: &mut Vec<OperationId>,
) -> Result<bool, LoweringError> {
    let (psi_operation, result, source) = match operation {
        AbstractOperation::ByteSequenceLength {
            psi_operation,
            result,
            source,
        }
        | AbstractOperation::ByteSequenceRead {
            psi_operation,
            result,
            source,
            ..
        } => (*psi_operation, *result, *source),
        _ => return Ok(false),
    };
    let invalid = || LoweringError::UnsupportedOperationInScalarFunction(function.machine);
    let semantic = function
        .structural_parameters
        .iter()
        .find(|parameter| {
            parameter.place == source && is_immutable_byte_parameter(parameter, structural_types)
        })
        .ok_or_else(invalid)?;
    let parameter = parameters
        .iter()
        .find(|parameter| {
            parameter.place == source
                && parameter.structural_type == semantic.structural_type
                && parameter.access == semantic.access
                && parameter.multiplicity == semantic.multiplicity
                && parameter.projected_qualifications == semantic.projected_qualifications
                && parameter.shape == ValueShape::borrowed_reference(16, 8)
        })
        .ok_or_else(invalid)?;
    let ScalarType::Integer(scalar_type) = result.scalar_type else {
        return Err(invalid());
    };
    let expression = match operation {
        AbstractOperation::ByteSequenceLength { .. } => {
            if scalar_type.sign() != IntegerSign::Unsigned || scalar_type.bits() != 64 {
                return Err(invalid());
            }
            TargetIntegerExpression::ByteSequenceLength {
                psi_operation,
                source_value: result.value,
                source,
                source_placement: parameter.placement.clone(),
                length_byte_offset: 8,
            }
        }
        AbstractOperation::ByteSequenceRead {
            index,
            length,
            obligation,
            ..
        } => {
            if scalar_type.sign() != IntegerSign::Unsigned || scalar_type.bits() != 8 {
                return Err(invalid());
            }
            let Some(KnownScalar::Integer {
                scalar_type: index_type,
                value: index_value,
            }) = values.get(index)
            else {
                return Err(invalid());
            };
            let Some(KnownScalar::Integer {
                scalar_type: length_type,
                value:
                    KnownInteger::Runtime(TargetIntegerExpression::ByteSequenceLength {
                        source: length_source,
                        source_value: length_value,
                        ..
                    }),
            }) = values.get(length)
            else {
                return Err(invalid());
            };
            if index_type.sign() != IntegerSign::Unsigned
                || index_type.bits() != 64
                || length_type != index_type
                || *length_source != source
                || length_value != length
            {
                return Err(invalid());
            }
            TargetIntegerExpression::ByteSequenceRead {
                psi_operation,
                source_value: result.value,
                source,
                source_placement: parameter.placement.clone(),
                index: Box::new(index_value.clone().into_expression(*index)),
                length: *length,
                obligation: *obligation,
            }
        }
        _ => return Ok(false),
    };
    insert_value(
        values,
        result.value,
        KnownScalar::Integer {
            scalar_type,
            value: KnownInteger::Runtime(expression),
        },
    )?;
    provenance.push(psi_operation);
    Ok(true)
}
