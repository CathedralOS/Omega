//! Immutable byte observations shared by straight-line and conditional lowering.

use super::*;
use target_operations::TargetByteView;

pub(in crate::lowering) fn block_source(
    function: &AbstractFunction,
    place: PlaceId,
    structural_types: &StructuralTypeLookup<'_>,
) -> Option<(
    target_operations::TargetStructuralArgumentSource,
    StructuralTypeId,
)> {
    function.block_entries.iter().find_map(|entry| {
        let parameter = entry
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == place)?;
        (entry.block != function.entry && is_immutable_byte_parameter(parameter, structural_types))
            .then_some((
                target_operations::TargetStructuralArgumentSource::BlockParameter {
                    block: entry.block,
                    place,
                },
                parameter.structural_type,
            ))
    })
}

pub(in crate::lowering) fn is_immutable_byte_parameter(
    parameter: &terminal_psi::StructuralParameterDeclaration,
    structural_types: &StructuralTypeLookup<'_>,
) -> bool {
    parameter.access == StructuralAccess::SharedBorrow
        && is_byte_parameter(parameter, structural_types)
}

pub(in crate::lowering) fn is_byte_parameter(
    parameter: &terminal_psi::StructuralParameterDeclaration,
    structural_types: &StructuralTypeLookup<'_>,
) -> bool {
    !parameter.is_self
        && matches!(
            parameter.access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        )
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

/// Mutable views are only whole machine or block parameters, never subslices.
pub(in crate::lowering) fn mutable_parameter_view(
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    parameters: &[TargetStructuralParameter],
    place: PlaceId,
) -> Result<(terminal_psi::StructuralParameterDeclaration, TargetByteView), LoweringError> {
    let invalid = || LoweringError::UnsupportedControlFlow(function.machine);
    if let Some((entry, semantic)) = function.block_entries.iter().find_map(|entry| {
        entry
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == place)
            .map(|parameter| (entry, parameter))
    }) {
        if entry.block == function.entry
            || semantic.access != StructuralAccess::MutableBorrow
            || !is_byte_parameter(semantic, structural_types)
        {
            return Err(invalid());
        }
        return Ok((
            semantic.clone(),
            TargetByteView::BlockParameter {
                block: entry.block,
                place,
                structural_type: semantic.structural_type,
            },
        ));
    }
    let semantic = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == place)
        .ok_or_else(invalid)?;
    let parameter = parameters
        .iter()
        .find(|parameter| parameter.place == place)
        .ok_or_else(invalid)?;
    if semantic.access != StructuralAccess::MutableBorrow
        || !is_byte_parameter(semantic, structural_types)
        || parameter.structural_type != semantic.structural_type
        || parameter.access != semantic.access
        || parameter.multiplicity != semantic.multiplicity
        || parameter.projected_qualifications != semantic.projected_qualifications
        || parameter.shape != ValueShape::borrowed_reference(16, 8)
    {
        return Err(invalid());
    }
    Ok((
        semantic.clone(),
        TargetByteView::Parameter {
            place,
            placement: parameter.placement.clone(),
        },
    ))
}

pub(super) fn lower_byte_observation(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    parameters: &[TargetStructuralParameter],
    values: &mut BTreeMap<ValueId, KnownScalar>,
    provenance: &mut Vec<OperationId>,
) -> Result<bool, LoweringError> {
    lower_byte_observation_with_lengths(
        operation,
        function,
        structural_types,
        parameters,
        values,
        &BTreeMap::new(),
        provenance,
    )
}

/// Unit scalar homes retain residence, while this separate live roster retains
/// which exact descriptor each already executed length observation measured.
#[allow(clippy::too_many_arguments)]
pub(in crate::lowering) fn lower_byte_observation_with_lengths(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    parameters: &[TargetStructuralParameter],
    values: &mut BTreeMap<ValueId, KnownScalar>,
    lengths: &BTreeMap<ValueId, PlaceId>,
    provenance: &mut Vec<OperationId>,
) -> Result<bool, LoweringError> {
    if let AbstractOperation::ByteSequenceSubslice {
        psi_operation,
        result,
        ..
    } = operation
    {
        view_for_place(
            function,
            structural_types,
            parameters,
            values,
            lengths,
            result.place,
            &mut Vec::new(),
        )?;
        provenance.push(*psi_operation);
        return Ok(true);
    }
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
    let mutable_view = matches!(operation, AbstractOperation::ByteSequenceLength { .. })
        .then(|| mutable_parameter_view(function, structural_types, parameters, source).ok())
        .flatten();
    let view = if let Some((_, view)) = mutable_view {
        view
    } else {
        view_for_place(
            function,
            structural_types,
            parameters,
            values,
            lengths,
            source,
            &mut Vec::new(),
        )?
    };
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
                view: Box::new(view),
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
            if index_type.sign() != IntegerSign::Unsigned
                || index_type.bits() != 64
                || !observes_length(values, lengths, *length, source)
            {
                return Err(invalid());
            }
            TargetIntegerExpression::ByteSequenceRead {
                psi_operation,
                source_value: result.value,
                source,
                view: Box::new(view),
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

#[allow(clippy::too_many_arguments)]
pub(in crate::lowering) fn view_for_place(
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    parameters: &[TargetStructuralParameter],
    values: &BTreeMap<ValueId, KnownScalar>,
    lengths: &BTreeMap<ValueId, PlaceId>,
    place: PlaceId,
    active: &mut Vec<PlaceId>,
) -> Result<TargetByteView, LoweringError> {
    let invalid = || LoweringError::UnsupportedOperationInScalarFunction(function.machine);
    if active.contains(&place) {
        return Err(invalid());
    }
    if let Some((entry, parameter)) = function.block_entries.iter().find_map(|entry| {
        entry
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == place)
            .map(|parameter| (entry, parameter))
    }) {
        if entry.block == function.entry
            || !is_immutable_byte_parameter(parameter, structural_types)
        {
            return Err(invalid());
        }
        return Ok(TargetByteView::BlockParameter {
            block: entry.block,
            place,
            structural_type: parameter.structural_type,
        });
    }
    if let Some(semantic) = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == place)
    {
        let parameter = parameters
            .iter()
            .find(|parameter| parameter.place == place)
            .ok_or_else(invalid)?;
        if !is_immutable_byte_parameter(semantic, structural_types)
            || parameter.structural_type != semantic.structural_type
            || parameter.access != semantic.access
            || parameter.multiplicity != semantic.multiplicity
            || parameter.projected_qualifications != semantic.projected_qualifications
            || parameter.shape != ValueShape::borrowed_reference(16, 8)
        {
            return Err(invalid());
        }
        return Ok(TargetByteView::Parameter {
            place,
            placement: parameter.placement.clone(),
        });
    }
    let Some(AbstractOperation::ByteSequenceSubslice { psi_operation, result, source, start, end, length, obligation }) =
        function.operations.iter().find(|operation| matches!(operation, AbstractOperation::ByteSequenceSubslice { result, .. } if result.place == place))
        else { return Err(invalid()); };
    if result.multiplicity != StructuralMultiplicity::Unrestricted
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
        || !structural_types
            .get(&result.structural_type)
            .is_some_and(|declaration| {
                matches!(
                    declaration.shape,
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView
                    )
                )
            })
    {
        return Err(invalid());
    }
    let endpoint = |value: ValueId| {
        let Some(KnownScalar::Integer {
            scalar_type,
            value: known,
        }) = values.get(&value)
        else {
            return Err(invalid());
        };
        if scalar_type.sign() != IntegerSign::Unsigned || scalar_type.bits() != 64 {
            return Err(invalid());
        }
        Ok(Box::new(known.clone().into_expression(value)))
    };
    let start = endpoint(*start)?;
    let end = endpoint(*end)?;
    if !observes_length(values, lengths, *length, *source) {
        return Err(invalid());
    }
    active.push(place);
    let source = view_for_place(
        function,
        structural_types,
        parameters,
        values,
        lengths,
        *source,
        active,
    )?;
    active.pop();
    Ok(TargetByteView::Subslice {
        psi_operation: *psi_operation,
        place,
        source: Box::new(source),
        start,
        end,
        length: *length,
        obligation: *obligation,
    })
}

fn observes_length(
    values: &BTreeMap<ValueId, KnownScalar>,
    lengths: &BTreeMap<ValueId, PlaceId>,
    length: ValueId,
    source: PlaceId,
) -> bool {
    let Some(KnownScalar::Integer { scalar_type, value }) = values.get(&length) else {
        return false;
    };
    if scalar_type.sign() != IntegerSign::Unsigned || scalar_type.bits() != 64 {
        return false;
    }
    match value {
        KnownInteger::Runtime(TargetIntegerExpression::ByteSequenceLength {
            source: observed,
            source_value,
            ..
        }) => *observed == source && *source_value == length,
        KnownInteger::Runtime(TargetIntegerExpression::ScalarHome(home)) => {
            home.source_value == length
                && home.scalar_type == ScalarType::Integer(*scalar_type)
                && lengths.get(&length) == Some(&source)
        }
        _ => false,
    }
}
