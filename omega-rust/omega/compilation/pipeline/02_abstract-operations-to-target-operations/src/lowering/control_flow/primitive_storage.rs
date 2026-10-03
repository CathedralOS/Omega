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

/// The bounded byte field `path`/`field` beneath a parameter or
/// established owned home, as the structural argument its observations keep. Neither a
/// length nor a byte read establishes a borrowed view or other authority.
pub(super) fn readable_byte_field(
    function: &AbstractFunction,
    types: &crate::lowering::structural_type_lookup::StructuralTypeLookup<'_>,
    prepared: &PreparedFunctionSignature,
    live: &LiveDefinitions,
    source: semantic_vocabulary::PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
    field: semantic_vocabulary::StructuralFieldId,
) -> Result<terminal_psi::StructuralArgument, LoweringError> {
    let unsupported = || LoweringError::unsupported_control_flow(function.machine);
    // Observations need not share the narrower store grammar. The semantic
    // check below constrains newly admitted deep paths to fixed storage before
    // the general projection resolver computes their physical extent.
    if super::references::check_projected_access(function, live, types, source, path, Some(field))
        || function
            .entry_claims
            .iter()
            .any(|claim| claim.input == source)
    {
        return Err(unsupported());
    }
    let (structural_type, access) = field_root(function, types, prepared, live, source)?;
    if !terminal_psi::is_structural_scalar_store_path(path) {
        return Err(unsupported());
    }
    let carrier = crate::lowering::structural_layout::runtime_projection(
        structural_type,
        path,
        types,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
    )?
    .0;
    let StructuralTypeShape::Record { fields } =
        &types.get(&carrier).ok_or_else(unsupported)?.shape
    else {
        return Err(unsupported());
    };
    let mut matching = fields.iter().filter(|candidate| candidate.id == field);
    let selected = matching.next().ok_or_else(unsupported)?;
    // A shared view field is observed through its stored descriptor: the
    // length is the descriptor's extent word and bytes live behind its
    // pointer. Neither observation creates a borrow of that backing.
    if matching.next().is_some()
        || selected.relevance.is_erased()
        || !(matches!(
            selected.field_type.byte_sequence_carrier(),
            Some(
                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                    | terminal_psi::ByteSequenceCarrier::BorrowedView {
                        access: Some(StructuralAccess::SharedBorrow),
                    }
            )
        ) || matches!(
            selected.field_type,
            StructuralFieldType::Structural(child)
                if types.get(&child).is_some_and(|declaration| {
                    matches!(
                        declaration.shape,
                        StructuralTypeShape::ElementView { .. }
                            | StructuralTypeShape::ByteSequence(
                                terminal_psi::ByteSequenceCarrier::BorrowedView { .. },
                            )
                    )
                })
        ))
    {
        return Err(unsupported());
    }
    Ok(terminal_psi::StructuralArgument {
        place: source,
        access,
        path: path.to_vec(),
    })
}

/// Resolve descriptor authority separately from ownership of its physical
/// home. A mutable view owns an affine descriptor but writes borrowed backing.
pub(super) fn field_root(
    function: &AbstractFunction,
    types: &crate::lowering::structural_type_lookup::StructuralTypeLookup<'_>,
    prepared: &PreparedFunctionSignature,
    live: &LiveDefinitions,
    place: semantic_vocabulary::PlaceId,
) -> Result<(StructuralTypeId, StructuralAccess), LoweringError> {
    let unsupported = || LoweringError::unsupported_control_flow(function.machine);
    let structural_type = live
        .views
        .get(&place)
        .map(|(_, identity)| *identity)
        .or_else(|| {
            live.structural_homes
                .get(&place)
                .map(|home| home.structural_type())
        })
        .or_else(|| {
            function
                .arrival_parameter(place)
                .map(|parameter| parameter.structural_type)
        })
        .ok_or_else(unsupported)?;
    if matches!(
        types
            .get(&structural_type)
            .map(|declaration| &declaration.shape),
        Some(StructuralTypeShape::ElementView { .. })
    ) {
        let values = super::observations::scalar_values(live, &prepared.scalar_parameters)?;
        let (_, resolved_type, access) =
            crate::lowering::scalar::element_views::resolve_element_view(
                function,
                types,
                &prepared.parameters,
                crate::lowering::scalar::element_views::ElementViewBackings::new(
                    &live.structural_homes,
                    &live.address_joins,
                ),
                &values,
                &live.lengths,
                place,
                &mut Vec::new(),
            )?;
        if resolved_type != structural_type {
            return Err(unsupported());
        }
        return Ok((structural_type, access));
    }
    if let Some(home) = live.structural_homes.get(&place) {
        if home.has_claims()
            || home.multiplicity() == StructuralMultiplicity::Linear
            || !home.qualifications().is_empty()
            || !home.projected_qualifications().is_empty()
        {
            return Err(unsupported());
        }
        return Ok((structural_type, StructuralAccess::Owned));
    }
    let parameter = function.arrival_parameter(place).ok_or_else(unsupported)?;
    if parameter.multiplicity == StructuralMultiplicity::Linear
        || !parameter.qualifications.is_empty()
        || !parameter.projected_qualifications.is_empty()
    {
        return Err(unsupported());
    }
    Ok((structural_type, parameter.access))
}

/// Keep every runtime selector in target operand custody, alongside the
/// authoritative path whose operation-owned bounds justify the address.
pub(super) fn field_indices(
    function: &AbstractFunction,
    types: &crate::lowering::structural_type_lookup::StructuralTypeLookup<'_>,
    prepared: &PreparedFunctionSignature,
    live: &LiveDefinitions,
    argument: &terminal_psi::StructuralArgument,
) -> Result<Vec<target_operations::TargetStructuralRuntimeIndex>, LoweringError> {
    let (root, _) = field_root(function, types, prepared, live, argument.place)?;
    let (root, path) =
        if let Some(terminal_psi::StructuralPathSegment::Referent) = argument.path.first() {
            let Some(StructuralTypeShape::Reference { referent, .. }) =
                types.get(&root).map(|row| &row.shape)
            else {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            };
            (*referent, &argument.path[1..])
        } else {
            (root, argument.path.as_slice())
        };
    let (_, _, _, runs) = crate::lowering::structural_layout::runtime_projection(
        root,
        path,
        types,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
    )?;
    super::scalar_sources::runtime_indices(
        runs,
        function,
        &super::scalar_sources::ScalarSources::from(live),
    )
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
        crate::lowering::unit::known_integers::insert_known_unit_integer(
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
    types: &crate::lowering::structural_type_lookup::StructuralTypeLookup<'_>,
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
            let source =
                readable_byte_field(function, types, prepared, live, *source, path, *field)?;
            // Metadata keeps the original home and static projection. It
            // does not establish a borrowed view or grant byte-content access.
            let indices = field_indices(function, types, prepared, live, &source)?;
            retain_result(*psi_operation, *result, live)?;
            (
                *psi_operation,
                TargetUnitOperation::StructuralByteSequenceFieldLength {
                    psi_operation: *psi_operation,
                    result: *result,
                    source,
                    field: *field,
                    indices,
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
            let argument =
                readable_byte_field(function, types, prepared, live, *source, path, *field)?;
            // Content needs a shared or mutable borrow; a write-only borrow
            // may observe the live length but not the bytes. The index is
            // bounded by this same field's live length: the length operand
            // must be that exact field's observation.
            if argument.access == StructuralAccess::WriteOnlyBorrow
                || result.scalar_type != unsigned(function, 8)?
                || !function.operations.iter().any(|operation| matches!(operation,
                    AbstractOperation::StructuralByteSequenceFieldLength { source: measured, path: measured_path, field: measured_field, result: measured_result, .. }
                    if measured == source && terminal_semantics::structural_paths_select_same_location(measured_path, path) && measured_field == field && measured_result.value == *length))
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
            let indices = field_indices(function, types, prepared, live, &argument)?;
            retain_result(*psi_operation, *result, live)?;
            (
                *psi_operation,
                TargetUnitOperation::StructuralByteSequenceFieldRead {
                    psi_operation: *psi_operation,
                    result: *result,
                    source: argument,
                    field: *field,
                    indices,
                    index: index_value.into_target_source(*index),
                    length: *length,
                    obligation: *obligation,
                },
            )
        }
        AbstractOperation::StructuralScalarFieldRead {
            psi_operation,
            result,
            source,
            path,
            field,
        } => {
            let (psi_operation, result, place, field) = (*psi_operation, *result, *source, *field);
            let mut through_reference = false;
            let (structural_type, access) = if let Some((_, structural_type)) =
                live.views.get(&place)
            {
                // A field read through a view place observes one element:
                // the descriptor root is the carrier, the index step stays in
                // the path, and either view family lends a shared read. An
                // affine view is also a structural home, so views win first.
                (*structural_type, StructuralAccess::SharedBorrow)
            } else if let Some(custody) = live.references.get(&(place, Vec::new())).and_then(
                |custody| match types
                    .get(&custody.result.structural_type)
                    .map(|declaration| &declaration.shape)
                {
                    Some(StructuralTypeShape::Reference {
                        access: StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow,
                        ..
                    }) => Some(custody),
                    _ => None,
                },
            ) {
                // The home stores the selected address. Origins constrain
                // custody; choosing a static origin would lose a callee's
                // runtime selection among several admissible referents.
                let (referent, access) = match types
                    .get(&custody.result.structural_type)
                    .map(|declaration| &declaration.shape)
                {
                    Some(StructuralTypeShape::Reference { referent, access }) => {
                        (*referent, *access)
                    }
                    _ => return Err(LoweringError::unsupported_control_flow(function.machine)),
                };
                let home = live
                    .structural_homes
                    .get(&place)
                    .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
                if home.operation_result().map(|(_, result)| result) != Some(&custody.result)
                    || custody.referent != referent
                    || home.has_claims()
                    || home.multiplicity() == StructuralMultiplicity::Linear
                    || !home.qualifications().is_empty()
                    || !home.projected_qualifications().is_empty()
                    || super::references::is_suspended_root(live, place)
                    || live
                        .references
                        .values()
                        .any(|child| child.parent.as_ref() == Some(&custody.identity))
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                through_reference = true;
                (referent, access)
            } else if let Some(home) = live.structural_homes.get(&place) {
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
                    .chain(
                        function
                            .block_entries
                            .iter()
                            .flat_map(|entry| &entry.structural_parameters)
                            .filter(|parameter| live.address_joins.contains(&parameter.place)),
                    )
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
            let access = terminal_semantics::observed_field_backing_access(
                types.values().copied(),
                structural_type,
                path,
                access,
            )
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            let declaration = terminal_semantics::observed_field_declaration(
                types.values().copied(),
                structural_type,
                path,
                field,
            )
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            if matches!(declaration.field_type, StructuralFieldType::Atomic(_))
                && !matches!(
                    access,
                    StructuralAccess::Owned | StructuralAccess::MutableBorrow
                )
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            let mut carrier = structural_type;
            let mut runtime_path = Vec::with_capacity(path.len());
            if through_reference {
                runtime_path.push(terminal_psi::StructuralPathSegment::Referent);
            }
            let mut case = None;
            // A trailing `Case` step names the declared case whose payload
            // field the read observes; its payload overlays the sum's own
            // place, so the step joins no runtime path segment.
            let carrier_end = match path.last() {
                Some(semantic_vocabulary::CanonicalStructuralPathSegment::Case(_)) => {
                    path.len() - 1
                }
                _ => path.len(),
            };
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
                    // Fixed-array indices compose with later record fields;
                    // each hop retains its own declared bound. Element views
                    // keep the separately verified terminal-index contract.
                    semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(index) => {
                        let shape = &types
                            .get(&carrier)
                            .ok_or_else(|| {
                                LoweringError::unsupported_control_flow(function.machine)
                            })?
                            .shape;
                        let element = match shape {
                            StructuralTypeShape::FixedArray { element, length } => {
                                if index >= length {
                                    return Err(LoweringError::unsupported_control_flow(
                                        function.machine,
                                    ));
                                }
                                element
                            }
                            // An element view's live extent is the bound proven
                            // by its producer; the index names one member.
                            StructuralTypeShape::ElementView { element }
                                if position + 1 == carrier_end =>
                            {
                                element
                            }
                            _ => {
                                return Err(LoweringError::unsupported_control_flow(
                                    function.machine,
                                ));
                            }
                        };
                        runtime_path.push(terminal_psi::StructuralPathSegment::FixedIndex(*index));
                        carrier = *element;
                    }
                    semantic_vocabulary::CanonicalStructuralPathSegment::Case(identity)
                        if position == carrier_end =>
                    {
                        case = Some(*identity);
                    }
                    _ => return Err(LoweringError::unsupported_control_flow(function.machine)),
                }
            }
            let declaration = types
                .get(&carrier)
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            // A mixed shape's common field needs no `Case` step: it sits on the
            // carrier beside the tag whatever case is active.
            let fields = match (&declaration.shape, case) {
                (
                    StructuralTypeShape::Record { fields }
                    | StructuralTypeShape::Mixed { fields, .. },
                    None,
                ) => fields,
                (
                    StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. },
                    Some(case),
                ) => {
                    &cases
                        .iter()
                        .find(|candidate| candidate.id == case)
                        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?
                        .fields
                }
                _ => return Err(LoweringError::unsupported_control_flow(function.machine)),
            };
            if !fields.iter().any(|candidate| {
                candidate.id == field
                    && !candidate.relevance.is_erased()
                    && candidate.field_type.scalar_type() == Some(result.scalar_type)
            }) {
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
                    case,
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
                // The same established home supplies whole primitive reads
                // and projected aggregate reads. The path below determines
                // the leaf and offset; borrowing this home for an earlier
                // call must not make later reads observe an argument copy.
                home.structural_type()
            } else {
                function
                    .structural_parameters
                    .iter()
                    .chain(
                        function
                            .block_entries
                            .iter()
                            .flat_map(|entry| &entry.structural_parameters)
                            .filter(|parameter| {
                                live.address_joins.contains(&parameter.place)
                                    || live.block_views.contains(&parameter.place)
                            }),
                    )
                    .find(|parameter| {
                        parameter.place == *source
                            && parameter.multiplicity != StructuralMultiplicity::Linear
                            && (!path.is_empty()
                                || parameter.multiplicity == StructuralMultiplicity::Unrestricted)
                            && parameter.projected_qualifications.is_empty()
                            && (matches!(
                                parameter.access,
                                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                            ) || (parameter.access == StructuralAccess::Owned
                                && !path.is_empty()))
                    })
                    .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?
                    .structural_type
            };
            // Each runtime element scales its selector's exact dominating
            // source by the array stride; its bound is the path segment's
            // verified obligation.
            let (leaf, _, runs) = crate::lowering::structural_layout::primitive_read_projection(
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
