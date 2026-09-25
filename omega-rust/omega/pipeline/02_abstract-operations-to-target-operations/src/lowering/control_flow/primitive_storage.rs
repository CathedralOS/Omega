//! Activation-local primitive backing and fresh storage observations.
use super::{KnownUnitInteger, LiveDefinitions};
use crate::LoweringError;
use crate::lowering::function_signature::PreparedFunctionSignature;
use abstract_operations::{AbstractFunction, AbstractOperation};
use calling_conventions::ValueShape;
use semantic_vocabulary::{
    IeeeFloatFormat, IntegerSign, IntegerType, OperationId, ScalarType, StructuralTypeId,
};
use std::collections::{BTreeMap, BTreeSet};
use target_operations::{TargetStructuralHomeLayout, TargetStructuralHomeRequirement};
use target_operations::{
    TargetUnitOperation, TargetUnitScalarHomeRequirement, TerminalPsiProvenance,
};
use terminal_psi::{
    StructuralAccess, StructuralFieldType, StructuralMultiplicity, StructuralTypeDeclaration,
    StructuralTypeShape,
};

pub(super) fn is_primitive_reference(
    parameter: &terminal_psi::StructuralParameterDeclaration,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> bool {
    !parameter.is_self
        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
        && parameter.access != StructuralAccess::Owned
        && parameter.qualifications.is_empty()
        && parameter.projected_qualifications.is_empty()
        && types
            .get(&parameter.structural_type)
            .is_some_and(|declaration| {
                matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(scalar)
                if native_shape(scalar).is_some())
            })
}

pub(super) fn shape(
    scalar: abstract_operations::AbstractResult,
) -> Result<ValueShape, LoweringError> {
    native_shape(scalar.scalar_type).ok_or(LoweringError::ValueTypeMismatch(scalar.value))
}

pub(super) fn native_shape(scalar: ScalarType) -> Option<ValueShape> {
    match scalar {
        ScalarType::Integer(integer) => {
            crate::lowering::scalar_abi::fixed_native_integer_shape(integer)
        }
        ScalarType::Boolean => Some(ValueShape::integer(1, 1)),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary32) => Some(ValueShape::float(4)),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary64) => Some(ValueShape::float(8)),
    }
}

fn unsigned(function: &AbstractFunction, bits: u16) -> Result<ScalarType, LoweringError> {
    Ok(ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, bits)
            .map_err(|_| LoweringError::unsupported_control_flow(function.machine))?,
    ))
}

/// The bounded byte field `path`/`field` beneath a readable borrowed
/// parameter, as the structural argument its observations keep. Neither a
/// length nor a byte read establishes a borrowed view or other authority.
fn readable_byte_field(
    function: &AbstractFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    source: semantic_vocabulary::PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
    field: semantic_vocabulary::StructuralFieldId,
) -> Result<terminal_psi::StructuralArgument, LoweringError> {
    let unsupported = || LoweringError::unsupported_control_flow(function.machine);
    let parameter = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == source)
        .ok_or_else(unsupported)?;
    if !matches!(
        parameter.access,
        StructuralAccess::SharedBorrow
            | StructuralAccess::MutableBorrow
            | StructuralAccess::WriteOnlyBorrow
    ) || parameter.multiplicity == StructuralMultiplicity::Linear
        || !parameter.qualifications.is_empty()
        || !parameter.projected_qualifications.is_empty()
        || !terminal_psi::is_bounded_structural_scalar_store_path(path)
        || function
            .entry_claims
            .iter()
            .any(|claim| claim.input == source)
    {
        return Err(unsupported());
    }
    let carrier = if path.is_empty() {
        parameter.structural_type
    } else {
        crate::lowering::structural_layout::resolve_structural_projection_path(
            parameter.structural_type,
            path,
            types,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
        )?
        .0
    };
    let StructuralTypeShape::Record { fields } =
        &types.get(&carrier).ok_or_else(unsupported)?.shape
    else {
        return Err(unsupported());
    };
    let mut matching = fields.iter().filter(|candidate| candidate.id == field);
    let selected = matching.next().ok_or_else(unsupported)?;
    if matching.next().is_some()
        || selected.relevance.is_erased()
        || !matches!(
            selected.field_type,
            StructuralFieldType::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
            )
        )
    {
        return Err(unsupported());
    }
    Ok(terminal_psi::StructuralArgument {
        place: source,
        access: parameter.access,
        path: path.to_vec(),
    })
}

pub(super) fn retain_result(
    operation: OperationId,
    result: abstract_operations::AbstractResult,
    live: &mut LiveDefinitions,
) -> Result<TargetUnitScalarHomeRequirement, LoweringError> {
    let home = TargetUnitScalarHomeRequirement {
        defining_operation: operation,
        source_value: result.value,
        scalar_type: result.scalar_type,
        shape: shape(result)?,
    };
    if matches!(result.scalar_type, ScalarType::Integer(_)) {
        crate::lowering::unit::scalar_call::insert_known_unit_integer(
            &mut live.integers,
            result.value,
            KnownUnitInteger::Home(home),
        )?;
    } else if live.scalar_homes.insert(result.value, home).is_some() {
        return Err(LoweringError::DuplicateValue(result.value));
    }
    Ok(home)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    prepared: &PreparedFunctionSignature,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let scalar_matches = |identity, scalar_type| {
        types.get(&identity).is_some_and(|declaration|
            matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(expected) if expected == scalar_type))
    };
    let (identity, lowered) = match operation {
        AbstractOperation::StructuralByteSequenceFieldLength {
            psi_operation,
            result,
            source,
            path,
            field,
        } => {
            if result.scalar_type != unsigned(function, 64)? {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            let source = readable_byte_field(function, types, *source, path, *field)?;
            // Metadata keeps the original parameter and static projection. It
            // does not establish a borrowed view or grant byte-content access.
            retain_result(*psi_operation, *result, live)?;
            (
                *psi_operation,
                TargetUnitOperation::StructuralByteSequenceFieldLength {
                    psi_operation: *psi_operation,
                    result: *result,
                    source,
                    field: *field,
                },
            )
        }
        AbstractOperation::StructuralByteSequenceFieldRead {
            psi_operation,
            result,
            source,
            path,
            field,
            index,
            length,
            obligation,
        } => {
            let argument = readable_byte_field(function, types, *source, path, *field)?;
            // Content needs a shared or mutable borrow; a write-only borrow
            // may observe the live length but not the bytes. The index is
            // bounded by this same field's live length: the length operand
            // must be that exact field's observation.
            if argument.access == StructuralAccess::WriteOnlyBorrow
                || result.scalar_type != unsigned(function, 8)?
                || !function.operations.iter().any(|operation| matches!(operation,
                    AbstractOperation::StructuralByteSequenceFieldLength { source: measured, path: measured_path, field: measured_field, result: measured_result, .. }
                    if measured == source && measured_path == path && measured_field == field && measured_result.value == *length))
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            let index_value = *live
                .integers
                .get(index)
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            let length_value = live
                .integers
                .get(length)
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            if ScalarType::Integer(index_value.scalar_type()) != unsigned(function, 64)?
                || ScalarType::Integer(length_value.scalar_type()) != unsigned(function, 64)?
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            retain_result(*psi_operation, *result, live)?;
            (
                *psi_operation,
                TargetUnitOperation::StructuralByteSequenceFieldRead {
                    psi_operation: *psi_operation,
                    result: *result,
                    source: argument,
                    field: *field,
                    index: index_value.into_target_source(*index),
                    length: *length,
                    obligation: *obligation,
                },
            )
        }
        AbstractOperation::IntegerStructuralField { .. }
        | AbstractOperation::BooleanStructuralField { .. } => {
            let (psi_operation, result, place, path, field) = match operation {
                AbstractOperation::IntegerStructuralField {
                    psi_operation,
                    result,
                    source,
                    path,
                    field,
                } => (*psi_operation, *result, *source, path, *field),
                AbstractOperation::BooleanStructuralField {
                    psi_operation,
                    result,
                    source,
                    path,
                    field,
                } => (
                    *psi_operation,
                    abstract_operations::AbstractResult {
                        value: *result,
                        scalar_type: ScalarType::Boolean,
                    },
                    *source,
                    path,
                    *field,
                ),
                _ => return Err(LoweringError::unsupported_control_flow(function.machine)),
            };
            let (structural_type, access) = if let Some(home) = live.structural_homes.get(&place) {
                if home.has_claims()
                    || home.multiplicity() == StructuralMultiplicity::Linear
                    || !home.qualifications().is_empty()
                    || !home.projected_qualifications().is_empty()
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                (home.structural_type(), StructuralAccess::Owned)
            } else {
                let source = function
                    .structural_parameters
                    .iter()
                    .find(|parameter| parameter.place == place)
                    .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
                if !matches!(
                    source.access,
                    StructuralAccess::Owned
                        | StructuralAccess::SharedBorrow
                        | StructuralAccess::MutableBorrow
                ) || source.multiplicity == StructuralMultiplicity::Linear
                    || !source.qualifications.is_empty()
                    || !source.projected_qualifications.is_empty()
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                (source.structural_type, source.access)
            };
            let mut carrier = structural_type;
            let mut runtime_path = Vec::with_capacity(path.len());
            for (position, segment) in path.iter().enumerate() {
                match segment {
                    semantic_vocabulary::CanonicalStructuralPathSegment::Field(field) => {
                        let StructuralTypeShape::Record { fields } = &types
                            .get(&carrier)
                            .ok_or_else(|| {
                                LoweringError::unsupported_control_flow(function.machine)
                            })?
                            .shape
                        else {
                            return Err(LoweringError::unsupported_control_flow(function.machine));
                        };
                        let selected = fields
                            .iter()
                            .find(|candidate| {
                                candidate.id == *field && !candidate.relevance.is_erased()
                            })
                            .ok_or_else(|| {
                                LoweringError::unsupported_control_flow(function.machine)
                            })?;
                        let StructuralFieldType::Structural(child) = selected.field_type else {
                            return Err(LoweringError::unsupported_control_flow(function.machine));
                        };
                        runtime_path.push(terminal_psi::StructuralPathSegment::Field(
                            selected.identity.clone(),
                        ));
                        carrier = child;
                    }
                    // The bounded carrier grammar ends with at most one
                    // literal index: the fixed-array element is the record
                    // owning the observed field. The bound is rechecked
                    // against the declared extent rather than trusted from
                    // the producer.
                    semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(index)
                        if position + 1 == path.len() =>
                    {
                        let StructuralTypeShape::FixedArray { element, length } = &types
                            .get(&carrier)
                            .ok_or_else(|| {
                                LoweringError::unsupported_control_flow(function.machine)
                            })?
                            .shape
                        else {
                            return Err(LoweringError::unsupported_control_flow(function.machine));
                        };
                        if index >= length {
                            return Err(LoweringError::unsupported_control_flow(function.machine));
                        }
                        runtime_path.push(terminal_psi::StructuralPathSegment::FixedIndex(*index));
                        carrier = *element;
                    }
                    _ => return Err(LoweringError::unsupported_control_flow(function.machine)),
                }
            }
            if !types.get(&carrier).is_some_and(|declaration|
                matches!(&declaration.shape, StructuralTypeShape::Record { fields }
                    if fields.iter().any(|candidate| candidate.id == field && !candidate.relevance.is_erased()
                        && candidate.field_type.scalar_type() == Some(result.scalar_type))))
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            retain_result(psi_operation, result, live)?;
            (
                psi_operation,
                TargetUnitOperation::StructuralScalarFieldRead {
                    psi_operation,
                    result,
                    source: terminal_psi::StructuralArgument {
                        place,
                        access,
                        path: runtime_path,
                    },
                    field,
                },
            )
        }
        AbstractOperation::EstablishPrimitiveLocal {
            psi_operation,
            result,
            value,
        } => {
            if result.multiplicity != StructuralMultiplicity::Unrestricted
                || !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
                || !result.claims.is_empty()
                || !scalar_matches(result.structural_type, value.scalar_type)
                || super::scalar_sources::source(value.value, function, live)?.scalar_type()
                    != value.scalar_type
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            let shape = shape(*value)?;
            if live
                .structural_homes
                .insert(
                    result.place,
                    TargetStructuralHomeRequirement {
                        origin: target_operations::TargetStructuralHomeOrigin::OperationResult {
                            operation: *psi_operation,
                            result: result.clone(),
                        },
                        layout: TargetStructuralHomeLayout::Aggregate(shape),
                    },
                )
                .is_some()
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            (
                *psi_operation,
                TargetUnitOperation::EstablishPrimitiveLocal {
                    psi_operation: *psi_operation,
                    result: result.clone(),
                    value: *value,
                    shape,
                },
            )
        }
        AbstractOperation::PrimitiveLocalStore {
            psi_operation,
            destination,
            value,
        } => {
            let home = live
                .structural_homes
                .get(destination)
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            if !scalar_matches(home.structural_type(), value.scalar_type)
                || super::scalar_sources::source(value.value, function, live)?.scalar_type()
                    != value.scalar_type
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            (
                *psi_operation,
                TargetUnitOperation::PrimitiveLocalStore {
                    psi_operation: *psi_operation,
                    destination: *destination,
                    value: *value,
                },
            )
        }
        AbstractOperation::PrimitiveScalarRead {
            psi_operation,
            result,
            source,
            path,
        } => {
            let identity = if let Some(home) = live.structural_homes.get(source) {
                if !path.is_empty() {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                home.structural_type()
            } else {
                prepared
                    .parameters
                    .iter()
                    .find(|parameter| {
                        parameter.place == *source
                            && parameter.multiplicity != StructuralMultiplicity::Linear
                            && (!path.is_empty()
                                || parameter.multiplicity == StructuralMultiplicity::Unrestricted)
                            && parameter.projected_qualifications.is_empty()
                            && matches!(
                                parameter.access,
                                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                            )
                    })
                    .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?
                    .structural_type
            };
            // Each runtime element scales its selector's exact dominating
            // source by the array stride; its bound is the path segment's
            // verified obligation.
            let (leaf, _, runs) = crate::lowering::structural_layout::primitive_leaf_projection(
                identity, path, types,
            )
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            if leaf != result.scalar_type {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            let indices = super::scalar_sources::runtime_indices(
                runs,
                function,
                &super::scalar_sources::ScalarSources::from(&*live),
            )?;
            retain_result(*psi_operation, *result, live)?;
            (
                *psi_operation,
                TargetUnitOperation::PrimitiveScalarRead {
                    psi_operation: *psi_operation,
                    result: *result,
                    source: *source,
                    path: path.clone(),
                    indices,
                },
            )
        }
        _ => return Err(LoweringError::unsupported_control_flow(function.machine)),
    };
    operations.push(lowered);
    provenance.operations.push(identity);
    Ok(())
}
