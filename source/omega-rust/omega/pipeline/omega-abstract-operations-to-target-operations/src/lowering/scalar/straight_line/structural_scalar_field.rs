//! Direct structural-scalar field projection for straight-line scalar functions.

use super::*;

pub(super) fn lower_store(
    operation: &AbstractOperation,
    operation_index: usize,
    function: &AbstractFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    target_structural_parameters: &[TargetStructuralParameter],
    values: &BTreeMap<ValueId, KnownScalar>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<TargetScalarStructuralFieldStore, LoweringError> {
    let AbstractOperation::StructuralScalarFieldStore {
        psi_operation,
        destination,
        path,
        field,
        value,
    } = operation
    else {
        unreachable!("scalar field-store route accepts only its declared operation")
    };
    let Some(defining) = operation_index
        .checked_sub(1)
        .and_then(|index| function.operations.get(index))
    else {
        return Err(LoweringError::UnsupportedOperationInScalarFunction(
            function.machine,
        ));
    };
    let (defining_operation, source_value, immediate) = match defining {
        AbstractOperation::BooleanConstant {
            psi_operation,
            result,
            value,
        } => (
            *psi_operation,
            *result,
            TargetScalarImmediate::Boolean(*value),
        ),
        AbstractOperation::IntegerConstant {
            psi_operation,
            result,
            scalar_type: ScalarType::Integer(scalar_type),
            value,
        } => (
            *psi_operation,
            *result,
            TargetScalarImmediate::Integer {
                scalar_type: *scalar_type,
                value: *value,
            },
        ),
        _ => {
            return Err(LoweringError::UnsupportedOperationInScalarFunction(
                function.machine,
            ));
        }
    };
    let target_parameter = target_structural_parameters
        .iter()
        .find(|parameter| parameter.place == destination.place)
        .filter(|parameter| {
            parameter.structural_type == destination.structural_type
                && parameter.multiplicity == destination.multiplicity
                && parameter.access == destination.access
                && parameter.projected_qualifications == destination.projected_qualifications
        })
        .ok_or(LoweringError::UnsupportedOperationInScalarFunction(
            function.machine,
        ))?;
    let exact_immediate = match immediate {
        TargetScalarImmediate::Boolean(expected) => {
            matches!(values.get(&source_value), Some(KnownScalar::Boolean(actual)) if *actual == expected)
        }
        TargetScalarImmediate::Integer {
            scalar_type,
            value: expected,
        } => matches!(
            values.get(&source_value),
            Some(KnownScalar::Integer {
                scalar_type: actual_type,
                value: KnownInteger::Immediate(actual),
            }) if *actual_type == scalar_type && *actual == expected
        ),
    };
    let exact_path = path.is_empty()
        || matches!(path.as_slice(), [StructuralPathSegment::Field(identity)] if !identity.is_empty());
    if value.value != source_value
        || value.scalar_type != immediate.scalar_type()
        || !exact_immediate
        || !destination.is_self
        || function.attachment != Some(destination.structural_type)
        || !matches!(
            destination.multiplicity,
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
        )
        || destination.access != StructuralAccess::MutableBorrow
        || !destination.qualifications.is_empty()
        || !destination.projected_qualifications.is_empty()
        || !exact_path
    {
        return Err(LoweringError::UnsupportedOperationInScalarFunction(
            function.machine,
        ));
    }
    let (field_owner, path_byte_offset) = match path.as_slice() {
        [] => (destination.structural_type, 0),
        [StructuralPathSegment::Field(_)] => {
            let (nested, _, offset) = resolve_structural_field_path(
                destination.structural_type,
                path,
                structural_types,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
            )?;
            (nested, offset)
        }
        _ => unreachable!("projected scalar store path was checked above"),
    };
    let direct_field_byte_offset = match immediate {
        TargetScalarImmediate::Boolean(_) => {
            direct_boolean_field_offset(field_owner, *field, structural_types)?
        }
        TargetScalarImmediate::Integer { scalar_type, .. } => {
            direct_integer_field_offset(field_owner, *field, scalar_type, structural_types)?
        }
    };
    let field_byte_offset = path_byte_offset
        .checked_add(direct_field_byte_offset)
        .ok_or(LoweringError::StructuralTypeTooLarge(
            destination.structural_type,
        ))?;
    provenance.operations.push(*psi_operation);
    Ok(TargetScalarStructuralFieldStore {
        psi_operation: *psi_operation,
        destination: destination.clone(),
        path: path.clone(),
        field: *field,
        destination_placement: target_parameter.placement.clone(),
        field_byte_offset,
        defining_operation,
        source_value,
        immediate,
    })
}

pub(super) fn lower(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    target_structural_parameters: &[TargetStructuralParameter],
    values: &mut BTreeMap<ValueId, KnownScalar>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    match operation {
        AbstractOperation::BooleanStructuralField {
            psi_operation,
            result,
            source,
            field,
        } => {
            let parameter = target_structural_parameters
                .iter()
                .find(|parameter| parameter.place == *source)
                .ok_or(LoweringError::UnsupportedOperationInScalarFunction(
                    function.machine,
                ))?;
            let field_byte_offset =
                direct_boolean_field_offset(parameter.structural_type, *field, structural_types)?;
            insert_value(
                values,
                *result,
                KnownScalar::BooleanRuntime(TargetBooleanExpression::StructuralField {
                    psi_operation: *psi_operation,
                    source_value: *result,
                    source: *source,
                    field: *field,
                    source_placement: parameter.placement.clone(),
                    field_byte_offset,
                }),
            )?;
            provenance.operations.push(*psi_operation);
        }
        AbstractOperation::IntegerStructuralField {
            psi_operation,
            result,
            source,
            field,
        } => {
            if !function
                .structural_parameters
                .iter()
                .any(|parameter| parameter == source)
            {
                return Err(LoweringError::UnsupportedOperationInScalarFunction(
                    function.machine,
                ));
            }
            let parameter = target_structural_parameters
                .iter()
                .find(|parameter| parameter.place == source.place)
                .filter(|parameter| {
                    parameter.structural_type == source.structural_type
                        && parameter.multiplicity == source.multiplicity
                        && parameter.access == source.access
                        && parameter.projected_qualifications == source.projected_qualifications
                })
                .ok_or(LoweringError::UnsupportedOperationInScalarFunction(
                    function.machine,
                ))?;
            let ScalarType::Integer(scalar_type) = result.scalar_type else {
                return Err(LoweringError::UnsupportedOperationInScalarFunction(
                    function.machine,
                ));
            };
            let field_byte_offset = direct_integer_field_offset(
                parameter.structural_type,
                *field,
                scalar_type,
                structural_types,
            )?;
            insert_value(
                values,
                result.value,
                KnownScalar::Integer {
                    scalar_type,
                    value: KnownInteger::Runtime(TargetIntegerExpression::StructuralField {
                        psi_operation: *psi_operation,
                        source_value: result.value,
                        source: source.place,
                        field: *field,
                        source_placement: parameter.placement.clone(),
                        field_byte_offset,
                        integer_type: scalar_type,
                    }),
                },
            )?;
            provenance.operations.push(*psi_operation);
        }
        _ => unreachable!("structural-scalar field route accepts only its declared operations"),
    }
    Ok(())
}
