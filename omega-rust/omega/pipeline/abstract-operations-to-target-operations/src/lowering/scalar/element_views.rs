//! Element observation and view operations for common graph lowering.
use super::{
    AbstractFunction, AbstractOperation, BTreeMap, BTreeSet, IntegerSign, KnownInteger,
    KnownScalar, LoweringError, OperationId, PlaceId, ScalarType, StructuralAccess,
    StructuralMultiplicity, StructuralTypeId, StructuralTypeLookup, StructuralTypeShape,
    TargetIntegerExpression, TargetStructuralParameter, ValueId, ValueShape, insert_value,
};
use crate::lowering::structural_layout::resolve_structural_field_path;
use target_operations::{TargetElementView, TargetStructuralArgumentSource};
use terminal_psi::{
    StructuralArgument, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralTypeDeclaration,
};

pub(in crate::lowering) fn is_immutable_element_view_parameter(
    parameter: &StructuralParameterDeclaration,
    structural_types: &StructuralTypeLookup<'_>,
) -> bool {
    parameter.access == StructuralAccess::SharedBorrow
        && is_element_view_parameter(parameter, structural_types)
}

pub(in crate::lowering) fn is_element_view_parameter(
    parameter: &StructuralParameterDeclaration,
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
                matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
            })
}

/// Element-view scalar observations retain residence in the shared scalar
/// value roster; live.lengths retains which exact descriptor each length
/// observation measured.
#[allow(clippy::too_many_arguments)]
pub(in crate::lowering) fn lower_element_observation_with_lengths(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    parameters: &[TargetStructuralParameter],
    values: &mut BTreeMap<ValueId, KnownScalar>,
    lengths: &BTreeMap<ValueId, PlaceId>,
    provenance: &mut Vec<OperationId>,
) -> Result<bool, LoweringError> {
    if let AbstractOperation::ElementViewSubslice {
        psi_operation,
        result,
        ..
    } = operation
    {
        element_view_for_place(
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
        AbstractOperation::ElementViewLength {
            psi_operation,
            result,
            source,
        }
        | AbstractOperation::ElementViewRead {
            psi_operation,
            result,
            source,
            ..
        } => (*psi_operation, *result, *source),
        _ => return Ok(false),
    };
    let invalid = || LoweringError::UnsupportedOperationInScalarFunction(function.machine);
    let (view, view_structural_type) = element_view_for_place(
        function,
        structural_types,
        parameters,
        values,
        lengths,
        source,
        &mut Vec::new(),
    )?;
    let ScalarType::Integer(scalar_type) = result.scalar_type else {
        return Err(invalid());
    };
    let expression = match operation {
        AbstractOperation::ElementViewLength { .. } => {
            if scalar_type.sign() != IntegerSign::Unsigned || scalar_type.bits() != 64 {
                return Err(invalid());
            }
            TargetIntegerExpression::ElementViewLength {
                psi_operation,
                source_value: result.value,
                source,
                view: Box::new(view),
                length_byte_offset: 8,
            }
        }
        AbstractOperation::ElementViewRead {
            index,
            length,
            obligation,
            ..
        } => {
            if !observes_length(values, lengths, *length, source)
                || element_scalar_type(structural_types, view_structural_type)
                    != Some(ScalarType::Integer(scalar_type))
            {
                return Err(invalid());
            }
            let Some(KnownScalar::Integer {
                scalar_type: index_type,
                value: index_value,
            }) = values.get(index)
            else {
                return Err(invalid());
            };
            if index_type.sign() != IntegerSign::Unsigned || index_type.bits() != 64 {
                return Err(invalid());
            }
            TargetIntegerExpression::ElementViewRead {
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

/// The declared element scalar type of one element-view descriptor type, when
/// that element is a primitive scalar.
fn element_scalar_type(
    structural_types: &StructuralTypeLookup<'_>,
    view_structural_type: StructuralTypeId,
) -> Option<ScalarType> {
    let StructuralTypeDeclaration {
        shape: StructuralTypeShape::ElementView { element },
        ..
    } = structural_types.get(&view_structural_type)?
    else {
        return None;
    };
    match structural_types.get(element)?.shape {
        StructuralTypeShape::PrimitiveScalar(scalar) => Some(scalar),
        _ => None,
    }
}

/// Resolve one element-view place's descriptor identity: signature or
/// non-entry block parameters directly, and producer operations for
/// established and subslice views. The paired structural type identifies the
/// view's element declaration.
#[allow(clippy::too_many_arguments)]
pub(in crate::lowering) fn element_view_for_place(
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    parameters: &[TargetStructuralParameter],
    values: &BTreeMap<ValueId, KnownScalar>,
    lengths: &BTreeMap<ValueId, PlaceId>,
    place: PlaceId,
    active: &mut Vec<PlaceId>,
) -> Result<(TargetElementView, StructuralTypeId), LoweringError> {
    let invalid = || LoweringError::UnsupportedOperationInScalarFunction(function.machine);
    if active.contains(&place) {
        return Err(invalid());
    }
    if let Some(AbstractOperation::EstablishElementView {
        psi_operation,
        result,
        source,
        element,
        ..
    }) = function.operations.iter().find(|operation| {
        matches!(operation,
            AbstractOperation::EstablishElementView { destination, .. } if *destination == place)
    }) {
        if result.place != place
            || result.multiplicity != StructuralMultiplicity::Unrestricted
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
            || !structural_types
                .get(&result.structural_type)
                .is_some_and(|declaration| {
                    matches!(declaration.shape,
                        StructuralTypeShape::ElementView { element: declared } if declared == *element)
                })
        {
            return Err(invalid());
        }
        let (root_structural_type, source_byte_offset, extent, element_stride, source_home) =
            established_source(function, structural_types, parameters, source, *element)?;
        return Ok((
            TargetElementView::Established {
                psi_operation: *psi_operation,
                place,
                structural_type: result.structural_type,
                element: *element,
                root_structural_type,
                source_byte_offset,
                extent,
                element_stride,
                source: source_home,
            },
            result.structural_type,
        ));
    }
    if let Some((entry, parameter)) = function.block_entries.iter().find_map(|entry| {
        entry
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == place)
            .map(|parameter| (entry, parameter))
    }) {
        if entry.block == function.entry
            || !is_immutable_element_view_parameter(parameter, structural_types)
        {
            return Err(invalid());
        }
        return Ok((
            TargetElementView::BlockParameter {
                block: entry.block,
                place,
                structural_type: parameter.structural_type,
            },
            parameter.structural_type,
        ));
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
        if !is_immutable_element_view_parameter(semantic, structural_types)
            || parameter.structural_type != semantic.structural_type
            || parameter.access != semantic.access
            || parameter.multiplicity != semantic.multiplicity
            || parameter.projected_qualifications != semantic.projected_qualifications
            || parameter.shape != ValueShape::borrowed_reference(16, 8)
        {
            return Err(invalid());
        }
        return Ok((
            TargetElementView::Parameter {
                place,
                placement: parameter.placement.clone(),
            },
            semantic.structural_type,
        ));
    }
    let Some(AbstractOperation::ElementViewSubslice {
        psi_operation,
        result,
        source,
        start,
        end,
        length,
        obligation,
    }) = function.operations.iter().find(|operation| {
        matches!(operation, AbstractOperation::ElementViewSubslice { result, .. }
            if result.place == place)
    })
    else {
        return Err(invalid());
    };
    if result.multiplicity != StructuralMultiplicity::Unrestricted
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
        || !structural_types
            .get(&result.structural_type)
            .is_some_and(|declaration| {
                matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
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
    let (source, _) = element_view_for_place(
        function,
        structural_types,
        parameters,
        values,
        lengths,
        *source,
        active,
    )?;
    active.pop();
    Ok((
        TargetElementView::Subslice {
            psi_operation: *psi_operation,
            place,
            source: Box::new(source),
            start,
            end,
            length: *length,
            obligation: *obligation,
        },
        result.structural_type,
    ))
}

/// Resolve an establishment's collection argument to its exact backing
/// projection: an all-field path over a signature parameter whose tip is the
/// fixed array carrying the view's element type.
fn established_source(
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    parameters: &[TargetStructuralParameter],
    source: &StructuralArgument,
    element: StructuralTypeId,
) -> Result<
    (
        StructuralTypeId,
        u32,
        u64,
        u32,
        TargetStructuralArgumentSource,
    ),
    LoweringError,
> {
    let invalid = || LoweringError::UnsupportedOperationInScalarFunction(function.machine);
    if source.access != StructuralAccess::SharedBorrow
        || source
            .path
            .iter()
            .any(|segment| !matches!(segment, StructuralPathSegment::Field(_)))
    {
        return Err(invalid());
    }
    let semantic = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == source.place)
        .ok_or_else(invalid)?;
    if !matches!(
        semantic.access,
        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow | StructuralAccess::Owned
    ) || semantic.multiplicity != StructuralMultiplicity::Unrestricted
        || !semantic.qualifications.is_empty()
        || !semantic.projected_qualifications.is_empty()
    {
        return Err(invalid());
    }
    let parameter = parameters
        .iter()
        .find(|parameter| parameter.place == source.place)
        .ok_or_else(invalid)?;
    if parameter.structural_type != semantic.structural_type {
        return Err(invalid());
    }
    let mut shape_cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    let (projected_type, projected_shape, source_byte_offset) = resolve_structural_field_path(
        semantic.structural_type,
        &source.path,
        structural_types,
        &mut shape_cache,
        &mut active,
    )?;
    let declaration = structural_types.get(&projected_type).ok_or_else(invalid)?;
    let StructuralTypeShape::FixedArray {
        element: array_element,
        length,
    } = declaration.shape
    else {
        return Err(invalid());
    };
    if array_element != element || length == 0 {
        return Err(invalid());
    }
    let element_stride = u64::from(u32::from(projected_shape.byte_size))
        .checked_div(length)
        .filter(|stride| *stride > 0)
        .and_then(|stride| u32::try_from(stride).ok())
        .ok_or_else(invalid)?;
    if u64::from(element_stride) * length != u64::from(u32::from(projected_shape.byte_size)) {
        return Err(invalid());
    }
    Ok((
        semantic.structural_type,
        source_byte_offset,
        length,
        element_stride,
        parameter.placement.clone().into(),
    ))
}

/// An element-view length observation measured exactly this descriptor.
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
        KnownInteger::Runtime(TargetIntegerExpression::ElementViewLength {
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
