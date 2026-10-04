//! Exact target lowering for non-observing primitive storage projections.
//!
//! One store covers every projection Terminal verifies: a whole primitive
//! root, a static leaf, or a leaf reached through runtime-selected elements
//! at any depth (`grid[i][j]`, `ents[i].hp`). A runtime element lowers to an
//! `(index, stride)` run over its selector's dominating source, the way leaf
//! copies scale theirs; the bound stays the segment's verified obligation.
//! Explicit reference crossings retain the carrier path and restart address
//! arithmetic after a pointer load; they never select an inferred loan origin.

use super::super::scalar_abi::fixed_native_integer_shape;
use super::known_integers::KnownUnitInteger;
use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractFunction, AbstractOperation};
use calling_conventions::ValueShape;
use semantic_vocabulary::IeeeFloatValue;
use semantic_vocabulary::{IeeeFloatFormat, OperationId, PlaceId, ScalarType, ValueId};
use std::collections::{BTreeMap, BTreeSet};
use target_operations::TargetUnitWriteOnlyPrimitiveStoreSource;
use target_operations::{
    TargetStructuralParameter, TargetUnitOperation, TargetUnitScalarHomeRequirement,
    TerminalPsiProvenance,
};
use terminal_psi::{StructuralAccess, StructuralMultiplicity};

#[allow(clippy::too_many_arguments)]
pub(in crate::lowering) fn lower_write_only_primitive_store(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    structural_homes: &BTreeMap<PlaceId, target_operations::TargetStructuralHomeRequirement>,
    scalar_values: &BTreeMap<ValueId, KnownUnitInteger>,
    boolean_constants: &BTreeMap<ValueId, (OperationId, bool)>,
    ieee_float_constants: &BTreeMap<ValueId, (OperationId, IeeeFloatValue)>,
    scalar_homes: &BTreeMap<ValueId, TargetUnitScalarHomeRequirement>,
    block_value: Option<target_operations::TargetScalarBlockValue>,
    scalar_sources: &crate::lowering::control_flow::scalar_sources::ScalarSources<'_>,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::WriteOnlyPrimitiveStore {
        psi_operation,
        destination,
        path,
        value,
    } = operation
    else {
        unreachable!("whole-root store lowering receives only primitive stores")
    };
    let invalid = || LoweringError::UnsupportedWriteOnlyPrimitiveStore {
        machine: function.machine,
        operation: *psi_operation,
    };
    let destination_storage = match destination {
        abstract_operations::StructuralScalarStoreDestination::Parameter(parameter) => {
            if parameter.multiplicity == StructuralMultiplicity::Linear
                || (path.is_empty()
                    && parameter.multiplicity != StructuralMultiplicity::Unrestricted)
                || !matches!(
                    parameter.access,
                    StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                )
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
            {
                return Err(invalid());
            }
            if let Some(entry) = function.block_entries.iter().find(|entry| {
                entry.structural_parameters.contains(parameter)
                    && parameter.access == StructuralAccess::MutableBorrow
            }) {
                // Arrival identity and write permission are independent of
                // transport: a byte-view arrival carries a descriptor, not an
                // address join. Selection resolves its backing before storing.
                target_operations::TargetStructuralArgumentSource::BlockParameter {
                    block: entry.block,
                    place: parameter.place,
                }
            } else {
                let root_shape = super::super::structural_layout::structural_shape(
                    parameter.structural_type,
                    structural_types,
                    &mut BTreeMap::new(),
                    &mut BTreeSet::new(),
                )?;
                let expected_shape =
                    ValueShape::borrowed_reference(root_shape.byte_size, root_shape.alignment);
                let target_parameter = parameters_by_place
                    .get(&parameter.place)
                    .copied()
                    .filter(|target| {
                        function.structural_parameters.contains(parameter)
                            && target.structural_type == parameter.structural_type
                            && target.multiplicity == parameter.multiplicity
                            && target.access == parameter.access
                            && target.projected_qualifications == parameter.projected_qualifications
                            && target.shape == expected_shape
                            && target.placement.shape == expected_shape
                    })
                    .ok_or_else(invalid)?;
                target_operations::TargetStructuralArgumentSource::Placement(
                    target_parameter.placement.clone(),
                )
            }
        }
        abstract_operations::StructuralScalarStoreDestination::OwnedResult { producer, result } => {
            let home = structural_homes
                .get(&result.place)
                .filter(|home| home.operation_result() == Some((*producer, result)))
                .ok_or_else(invalid)?;
            if home.has_claims()
                || !home.qualifications().is_empty()
                || !home.projected_qualifications().is_empty()
                || home.multiplicity() == StructuralMultiplicity::Linear
                || path.is_empty()
            {
                return Err(invalid());
            }
            target_operations::TargetStructuralArgumentSource::StructuralHome {
                psi_operation: *producer,
            }
        }
        // Only the scalar field store writes an owned block home natively.
        abstract_operations::StructuralScalarStoreDestination::OwnedBlockParameter { .. } => {
            return Err(invalid());
        }
    };
    let destination_type = structural_types
        .get(&destination.structural_type())
        .copied()
        .ok_or_else(invalid)?;
    // Each runtime element scales its selector's exact dominating source by
    // the array stride; its bound is the path segment's verified obligation.
    let projection = super::super::structural_layout::primitive_leaf_projection(
        destination.structural_type(),
        path,
        structural_types,
    )
    .ok_or_else(invalid)?;
    if projection.scalar != value.scalar_type {
        return Err(invalid());
    }
    // These are operand identities in path order, not one flattened address.
    // The target keeps the complete path, including each pointer crossing;
    // selection must reconstruct and apply each region around its pointer load.
    let runs = projection
        .pointer_slots
        .into_iter()
        .chain([projection.leaf])
        .flat_map(|region| region.indices)
        .collect();
    let indices = crate::lowering::control_flow::scalar_sources::runtime_indices(
        runs,
        function,
        scalar_sources,
    )
    .map_err(|_| invalid())?;
    let (_, source) = if let Some(block_value) = block_value {
        if block_value.value != value.value || block_value.scalar_type != value.scalar_type {
            return Err(invalid());
        }
        let shape = super::super::scalar_abi::fixed_native_scalar_shape(value.scalar_type)
            .ok_or_else(invalid)?;
        (
            ValueShape::borrowed_reference(shape.byte_size, shape.alignment),
            TargetUnitWriteOnlyPrimitiveStoreSource::BlockParameter(block_value),
        )
    } else {
        match value.scalar_type {
            ScalarType::Integer(integer_type) => {
                let referent_shape =
                    fixed_native_integer_shape(integer_type).ok_or_else(invalid)?;
                let Some(known_value) = scalar_values.get(&value.value).copied() else {
                    return Err(invalid());
                };
                if known_value.scalar_type() != integer_type {
                    return Err(invalid());
                }
                let source = match known_value {
                    KnownUnitInteger::Parameter {
                        parameter_index,
                        scalar_type,
                    } => TargetUnitWriteOnlyPrimitiveStoreSource::Parameter {
                        parameter_index,
                        source_value: value.value,
                        scalar_type: ScalarType::Integer(scalar_type),
                    },
                    KnownUnitInteger::Immediate {
                        defining_operation,
                        scalar_type,
                        value: immediate,
                    } => TargetUnitWriteOnlyPrimitiveStoreSource::IntegerImmediate {
                        defining_operation,
                        source_value: value.value,
                        scalar_type,
                        value: immediate,
                    },
                    KnownUnitInteger::Home(home) => {
                        TargetUnitWriteOnlyPrimitiveStoreSource::Home(home)
                    }
                    KnownUnitInteger::BlockParameter {
                        block,
                        value: source_value,
                        scalar_type,
                    } => {
                        if source_value != value.value {
                            return Err(invalid());
                        }
                        TargetUnitWriteOnlyPrimitiveStoreSource::BlockParameter(
                            target_operations::TargetScalarBlockValue {
                                block,
                                value: source_value,
                                scalar_type: ScalarType::Integer(scalar_type),
                            },
                        )
                    }
                };
                (
                    ValueShape::borrowed_reference(
                        referent_shape.byte_size,
                        referent_shape.alignment,
                    ),
                    source,
                )
            }
            ScalarType::Boolean => {
                let source = if let Some((parameter_index, _)) = function
                    .parameters
                    .iter()
                    .enumerate()
                    .find(|(_, parameter)| {
                        parameter.value == value.value
                            && parameter.scalar_type == ScalarType::Boolean
                    }) {
                    TargetUnitWriteOnlyPrimitiveStoreSource::Parameter {
                        parameter_index: u32::try_from(parameter_index).map_err(|_| invalid())?,
                        source_value: value.value,
                        scalar_type: ScalarType::Boolean,
                    }
                } else if let Some(home) = scalar_homes.get(&value.value) {
                    if home.source_value != value.value
                        || home.scalar_type != ScalarType::Boolean
                        || home.shape != ValueShape::integer(1, 1)
                    {
                        return Err(invalid());
                    }
                    TargetUnitWriteOnlyPrimitiveStoreSource::Home(*home)
                } else {
                    let (defining_operation, immediate) = boolean_constants
                        .get(&value.value)
                        .copied()
                        .ok_or_else(invalid)?;
                    TargetUnitWriteOnlyPrimitiveStoreSource::BooleanImmediate {
                        defining_operation,
                        source_value: value.value,
                        value: immediate,
                    }
                };
                (ValueShape::borrowed_reference(1, 1), source)
            }
            ScalarType::IeeeFloat(format) => {
                let byte_size = match format {
                    IeeeFloatFormat::Binary32 => 4,
                    IeeeFloatFormat::Binary64 => 8,
                };
                let source = if let Some((parameter_index, _)) = function
                    .parameters
                    .iter()
                    .enumerate()
                    .find(|(_, parameter)| {
                        parameter.value == value.value && parameter.scalar_type == value.scalar_type
                    }) {
                    TargetUnitWriteOnlyPrimitiveStoreSource::Parameter {
                        parameter_index: u32::try_from(parameter_index).map_err(|_| invalid())?,
                        source_value: value.value,
                        scalar_type: value.scalar_type,
                    }
                } else if let Some(home) = scalar_homes.get(&value.value) {
                    if home.source_value != value.value
                        || home.scalar_type != value.scalar_type
                        || home.shape != ValueShape::float(byte_size)
                    {
                        return Err(invalid());
                    }
                    TargetUnitWriteOnlyPrimitiveStoreSource::Home(*home)
                } else {
                    let (defining_operation, immediate) = ieee_float_constants
                        .get(&value.value)
                        .copied()
                        .ok_or_else(invalid)?;
                    if immediate.format() != format {
                        return Err(invalid());
                    }
                    TargetUnitWriteOnlyPrimitiveStoreSource::IeeeFloatImmediate {
                        defining_operation,
                        source_value: value.value,
                        value: immediate,
                    }
                };
                (
                    ValueShape::borrowed_reference(u32::from(byte_size), byte_size),
                    source,
                )
            }
        }
    };
    operations.push(TargetUnitOperation::WriteOnlyPrimitiveStore {
        psi_operation: *psi_operation,
        destination: destination.clone(),
        path: path.clone(),
        destination_type: destination_type.clone(),
        destination_storage,
        source,
        indices,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}
