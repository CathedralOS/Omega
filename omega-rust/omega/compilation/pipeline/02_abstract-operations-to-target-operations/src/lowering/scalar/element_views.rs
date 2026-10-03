//! Element observation and view operations for common graph lowering.
use super::{KnownInteger, KnownScalar};
use crate::LoweringError;
use crate::lowering::scalar::expressions::insert_value;
use crate::lowering::structural_layout::{
    direct_scalar_field_offset, resolve_structural_projection_path,
};
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractFunction, AbstractOperation};
use calling_conventions::ValueShape;
use semantic_vocabulary::{
    IntegerSign, IntegerType, OperationId, PlaceId, ScalarType, StructuralTypeId, ValueId,
};
use std::collections::{BTreeMap, BTreeSet};
use target_operations::{
    TargetElementView, TargetIntegerExpression, TargetStructuralArgumentSource,
    TargetStructuralHomeOrigin, TargetStructuralHomeRequirement, TargetStructuralParameter,
};
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
};

/// Dominating graph storage and borrowed addresses are different sources of
/// the same collection backing. A borrowed arrival carries a pointer; it must
/// never acquire an owned home merely to construct a descriptor over its array.
/// Independent ownership replay still governs access and lifetime.
#[derive(Clone, Copy)]
pub(in crate::lowering) struct ElementViewBackings<'a> {
    pub(super) homes: &'a BTreeMap<PlaceId, TargetStructuralHomeRequirement>,
    address_joins: &'a BTreeSet<PlaceId>,
}

impl<'a> ElementViewBackings<'a> {
    pub(in crate::lowering) fn new(
        homes: &'a BTreeMap<PlaceId, TargetStructuralHomeRequirement>,
        address_joins: &'a BTreeSet<PlaceId>,
    ) -> Self {
        Self {
            homes,
            address_joins,
        }
    }

    fn address(
        self,
        function: &AbstractFunction,
        structural_types: &StructuralTypeLookup<'_>,
        source: &StructuralArgument,
    ) -> Option<(StructuralTypeId, TargetStructuralArgumentSource)> {
        if !self.address_joins.contains(&source.place) {
            return None;
        }
        let (entry, parameter) = function.block_entries.iter().find_map(|entry| {
            entry
                .structural_parameters
                .iter()
                .find(|parameter| parameter.place == source.place)
                .map(|parameter| (entry, parameter))
        })?;
        if entry.block == function.entry
            || !abstract_operations::control_flow::address_joins::is_address_join(
                parameter,
                |identity| structural_types.get(&identity).copied(),
            )
            || (source.access == StructuralAccess::MutableBorrow
                && parameter.access != StructuralAccess::MutableBorrow)
        {
            return None;
        }
        Some((
            parameter.structural_type,
            TargetStructuralArgumentSource::BlockParameter {
                block: entry.block,
                place: parameter.place,
            },
        ))
    }
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
    backings: ElementViewBackings<'_>,
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
            backings,
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
        backings,
        values,
        lengths,
        source,
        &mut Vec::new(),
    )?;
    let known = match operation {
        AbstractOperation::ElementViewLength { .. } => {
            let ScalarType::Integer(scalar_type) = result.scalar_type else {
                return Err(invalid());
            };
            if scalar_type.sign() != IntegerSign::Unsigned || scalar_type.bits() != 64 {
                return Err(invalid());
            }
            KnownScalar::Integer {
                scalar_type,
                value: KnownInteger::Runtime(TargetIntegerExpression::ElementViewLength {
                    psi_operation,
                    source_value: result.value,
                    source,
                    view: Box::new(view),
                    length_byte_offset: 8,
                }),
            }
        }
        AbstractOperation::ElementViewRead {
            index,
            length,
            obligation,
            path,
            ..
        } => {
            if !observes_length(values, lengths, *length, source) {
                return Err(invalid());
            }
            let element_byte_offset = element_leaf_offset(
                structural_types,
                view_structural_type,
                path,
                result.scalar_type,
            )
            .ok_or_else(invalid)?;
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
            match result.scalar_type {
                ScalarType::Integer(scalar_type) => KnownScalar::Integer {
                    scalar_type,
                    value: KnownInteger::Runtime(TargetIntegerExpression::ElementViewRead {
                        psi_operation,
                        source_value: result.value,
                        source,
                        view: Box::new(view),
                        index: Box::new(index_value.clone().into_expression(*index)),
                        length: *length,
                        obligation: *obligation,
                        path: path.clone(),
                        element_byte_offset,
                    }),
                },
                ScalarType::Boolean => KnownScalar::BooleanRuntime(
                    target_operations::TargetBooleanExpression::ElementViewRead {
                        psi_operation,
                        source_value: result.value,
                        source,
                        view: Box::new(view),
                        index: Box::new(index_value.clone().into_expression(*index)),
                        length: *length,
                        obligation: *obligation,
                        path: path.clone(),
                        element_byte_offset,
                    },
                ),
                _ => return Err(invalid()),
            }
        }
        _ => return Ok(false),
    };
    insert_value(values, result.value, known)?;
    provenance.push(psi_operation);
    Ok(true)
}

/// The index advances by whole elements; the static path selects bytes inside
/// that element. A scalar field need not have a standalone type declaration.
fn element_leaf_offset(
    structural_types: &StructuralTypeLookup<'_>,
    view_structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    scalar: ScalarType,
) -> Option<u32> {
    let StructuralTypeDeclaration {
        shape: StructuralTypeShape::ElementView { element },
        ..
    } = structural_types.get(&view_structural_type)?
    else {
        return None;
    };
    scalar_leaf_offset(structural_types, *element, path, scalar)
}

fn scalar_leaf_offset(
    structural_types: &StructuralTypeLookup<'_>,
    element: StructuralTypeId,
    path: &[StructuralPathSegment],
    scalar: ScalarType,
) -> Option<u32> {
    if terminal_semantics::element_view_leaf_scalar(
        structural_types.catalog().as_slice(),
        element,
        path,
    ) != Some(scalar)
    {
        return None;
    }
    if path.is_empty() {
        return Some(0);
    }
    let mut cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    if let Some((StructuralPathSegment::Field(identity), prefix)) = path.split_last() {
        let (parent, parent_offset) = if prefix.is_empty() {
            (element, 0)
        } else {
            let (parent, _, offset) = resolve_structural_projection_path(
                element,
                prefix,
                structural_types,
                &mut cache,
                &mut active,
            )
            .ok()?;
            (parent, offset)
        };
        let StructuralTypeShape::Record { fields } = &structural_types.get(&parent)?.shape else {
            return None;
        };
        let field = fields.iter().find(|field| field.identity == *identity)?;
        if field.field_type.scalar_type() == Some(scalar) {
            return parent_offset.checked_add(
                direct_scalar_field_offset(parent, field.id, scalar, structural_types).ok()?,
            );
        }
    }
    resolve_structural_projection_path(element, path, structural_types, &mut cache, &mut active)
        .ok()
        .map(|(_, _, offset)| offset)
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
    backings: ElementViewBackings<'_>,
    values: &BTreeMap<ValueId, KnownScalar>,
    lengths: &BTreeMap<ValueId, PlaceId>,
    place: PlaceId,
    active: &mut Vec<PlaceId>,
) -> Result<(TargetElementView, StructuralTypeId), LoweringError> {
    resolve_element_view(
        function,
        structural_types,
        parameters,
        backings,
        values,
        lengths,
        place,
        active,
    )
    .map(|(view, structural_type, _)| (view, structural_type))
}

pub(in crate::lowering) fn resolve_element_view(
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    parameters: &[TargetStructuralParameter],
    backings: ElementViewBackings<'_>,
    values: &BTreeMap<ValueId, KnownScalar>,
    lengths: &BTreeMap<ValueId, PlaceId>,
    place: PlaceId,
    active: &mut Vec<PlaceId>,
) -> Result<(TargetElementView, StructuralTypeId, StructuralAccess), LoweringError> {
    let invalid = || LoweringError::UnsupportedOperationInScalarFunction(function.machine);
    if active.contains(&place) {
        return Err(invalid());
    }
    if let Some((psi_operation, result)) =
        super::byte_views::descriptor_result(function, place, structural_types)
        && matches!(
            structural_types
                .get(&result.structural_type)
                .map(|row| &row.shape),
            Some(StructuralTypeShape::ElementView { .. })
        )
    {
        if backings
            .homes
            .get(&place)
            .and_then(|home| home.operation_result())
            != Some((psi_operation, result))
        {
            return Err(invalid());
        }
        return Ok((
            TargetElementView::Result {
                psi_operation,
                place,
                structural_type: result.structural_type,
            },
            result.structural_type,
            StructuralAccess::SharedBorrow,
        ));
    }
    if let Some(AbstractOperation::EstablishElementView {
        psi_operation,
        result,
        source,
        element,
        region,
        ..
    }) = function.operations.iter().find(|operation| {
        matches!(operation,
            AbstractOperation::EstablishElementView { destination, .. } if *destination == place)
    }) {
        // A shared establishment mints the immutable shell; the mutable
        // loan's result carries the exclusive custody's affine multiplicity.
        let expected_multiplicity = match source.access {
            StructuralAccess::MutableBorrow => StructuralMultiplicity::Affine,
            _ => StructuralMultiplicity::Unrestricted,
        };
        if result.place != place
            || result.multiplicity != expected_multiplicity
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
            || !structural_types
                .get(&result.structural_type)
                .is_some_and(|declaration| match &declaration.shape {
                    StructuralTypeShape::ElementView { element: declared } => declared == element,
                    StructuralTypeShape::ByteSequence(_) => {
                        declaration.shape.is_borrowed_byte_view()
                            && structural_types
                                .get(element)
                                .is_some_and(|element_declaration| {
                                    matches!(
                                        element_declaration.shape,
                                        StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                                            ref integer
                                        )) if *integer
                                            == IntegerType::new(IntegerSign::Unsigned, 8)
                                                .expect("u8 is a valid integer type")
                                    )
                                })
                    }
                    _ => false,
                })
        {
            return Err(invalid());
        }
        let (root_structural_type, source_byte_offset, extent, element_stride, source_home) =
            established_source(
                function,
                structural_types,
                parameters,
                backings,
                source,
                *element,
                *region,
            )?;
        let region = match region {
            terminal_psi::ElementViewRegion::WholeCollection => {
                target_operations::TargetElementViewRegion::WholeCollection
            }
            terminal_psi::ElementViewRegion::RepresentationBytes {
                start,
                end,
                obligation,
            } => {
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
                target_operations::TargetElementViewRegion::RepresentationBytes {
                    start: endpoint(*start)?,
                    end: endpoint(*end)?,
                    obligation: *obligation,
                }
            }
        };
        return Ok((
            TargetElementView::Established {
                psi_operation: *psi_operation,
                place,
                structural_type: result.structural_type,
                element: *element,
                root_structural_type,
                source_byte_offset,
                extent,
                region,
                element_stride,
                source: source_home,
            },
            result.structural_type,
            source.access,
        ));
    }
    if let Some((entry, parameter)) = function.block_entries.iter().find_map(|entry| {
        entry
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == place)
            .map(|parameter| (entry, parameter))
    }) {
        if entry.block == function.entry || !is_element_view_parameter(parameter, structural_types)
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
            parameter.access,
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
        if !is_element_view_parameter(semantic, structural_types)
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
            semantic.access,
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
    if !matches!(
        result.multiplicity,
        StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
    ) || !result.qualifications.is_empty()
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
    let (source, source_type, source_access) = resolve_element_view(
        function,
        structural_types,
        parameters,
        backings,
        values,
        lengths,
        *source,
        active,
    )?;
    active.pop();
    let access = match result.multiplicity {
        StructuralMultiplicity::Unrestricted => StructuralAccess::SharedBorrow,
        StructuralMultiplicity::Affine if source_access == StructuralAccess::MutableBorrow => {
            StructuralAccess::MutableBorrow
        }
        _ => return Err(invalid()),
    };
    if source_type != result.structural_type {
        return Err(invalid());
    }
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
        access,
    ))
}

/// A writable descriptor uses the same source-chain and geometry replay as
/// observations. Affine result custody alone never creates mutable access.
pub(in crate::lowering) fn mutable_element_view_for_place(
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    parameters: &[TargetStructuralParameter],
    backings: ElementViewBackings<'_>,
    values: &BTreeMap<ValueId, KnownScalar>,
    lengths: &BTreeMap<ValueId, PlaceId>,
    place: PlaceId,
) -> Result<(TargetElementView, StructuralTypeId), LoweringError> {
    let (view, structural_type, access) = resolve_element_view(
        function,
        structural_types,
        parameters,
        backings,
        values,
        lengths,
        place,
        &mut Vec::new(),
    )?;
    if access != StructuralAccess::MutableBorrow {
        return Err(LoweringError::UnsupportedOperationInScalarFunction(
            function.machine,
        ));
    }
    Ok((view, structural_type))
}

/// Resolve an establishment's collection argument to its exact backing
/// projection: an all-field path, possibly empty, over a signature parameter
/// or a local aggregate home whose tip is the fixed array carrying the view's
/// element type. The backing keeps its storage identity: a parameter lends its
/// incoming placement and a local home names its one establishing producer.
fn established_source(
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    parameters: &[TargetStructuralParameter],
    backings: ElementViewBackings<'_>,
    source: &StructuralArgument,
    element: StructuralTypeId,
    region: terminal_psi::ElementViewRegion,
) -> Result<
    (
        StructuralTypeId,
        u32,
        target_operations::TargetViewExtent,
        u32,
        TargetStructuralArgumentSource,
    ),
    LoweringError,
> {
    let invalid = || LoweringError::UnsupportedOperationInScalarFunction(function.machine);
    // A `&[u8]` view over `collection[a..b]` ends its projection in a
    // `FixedByteRange` window: the field path still names the fixed byte
    // array and the window contributes the descriptor's own byte offset and
    // extent, leaving the backing stride at the `u8` element's.
    let (field_path, byte_window) = match source.path.last() {
        Some(StructuralPathSegment::FixedByteRange { start, end }) => {
            (&source.path[..source.path.len() - 1], Some((*start, *end)))
        }
        _ => (&source.path[..], None),
    };
    if !matches!(
        source.access,
        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
    ) {
        return Err(invalid());
    }
    let (root_structural_type, backing) = if let Some(semantic) = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == source.place)
    {
        if !matches!(
            semantic.access,
            StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::Owned
        ) || (source.access == StructuralAccess::MutableBorrow
            && !matches!(
                semantic.access,
                StructuralAccess::MutableBorrow | StructuralAccess::Owned
            ))
            || semantic.multiplicity != StructuralMultiplicity::Unrestricted
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
        (
            semantic.structural_type,
            TargetStructuralArgumentSource::from(parameter.placement.clone()),
        )
    } else if let Some(backing) = live_aggregate_backing(backings.homes, source.place) {
        backing
    } else {
        backings
            .address(function, structural_types, source)
            .ok_or_else(invalid)?
    };
    let mut shape_cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    let (projected_type, projected_shape, source_byte_offset) = resolve_structural_projection_path(
        root_structural_type,
        field_path,
        structural_types,
        &mut shape_cache,
        &mut active,
    )?;
    let geometry = terminal_semantics::element_view_region_geometry(
        structural_types.values().copied(),
        projected_type,
        element,
        region,
        source.access,
    )
    .ok_or_else(invalid)?;
    if region == terminal_psi::ElementViewRegion::WholeCollection {
        if let Some(StructuralTypeShape::FixedArray {
            element: original, ..
        }) = structural_types.get(&projected_type).map(|row| &row.shape)
        {
            if *original != element {
                let relation = terminal_semantics::element_representation_relation(
                    structural_types.values().copied(),
                    *original,
                    element,
                    source.access,
                )
                .ok_or_else(invalid)?;
                for (identity, layout) in [
                    (*original, &relation.source_layout),
                    (element, &relation.target_layout),
                ] {
                    let shape = crate::lowering::structural_layout::structural_shape(
                        identity,
                        structural_types,
                        &mut shape_cache,
                        &mut active,
                    )?;
                    if u64::from(shape.byte_size) != layout.size
                        || u64::from(shape.alignment) != layout.alignment
                        || layout.leaves.iter().any(|leaf| {
                            scalar_leaf_offset(
                                structural_types,
                                identity,
                                &leaf.path,
                                leaf.scalar_type,
                            )
                            .map(u64::from)
                                != Some(leaf.offset)
                        })
                    {
                        return Err(invalid());
                    }
                }
            }
        }
    }
    let (extent_kind, storage, representation) = match geometry {
        terminal_semantics::ElementViewRegionGeometry::WholeCollection(collection) => {
            (collection.extent, collection.storage, None)
        }
        terminal_semantics::ElementViewRegionGeometry::RepresentationBytes {
            byte_extent,
            element: layout,
        } => {
            if byte_window.is_some() {
                return Err(invalid());
            }
            let storage = if matches!(
                structural_types.get(&projected_type).map(|row| &row.shape),
                Some(StructuralTypeShape::ByteSequence(_))
            ) {
                terminal_semantics::CollectionStorage::Borrowed
            } else {
                terminal_semantics::CollectionStorage::Inline
            };
            (
                terminal_semantics::CollectionExtent::Fixed(byte_extent),
                storage,
                Some(layout),
            )
        }
    };
    let element_shape = crate::lowering::structural_layout::structural_shape(
        element,
        structural_types,
        &mut shape_cache,
        &mut active,
    )?;
    let element_stride = crate::lowering::structural_layout::checked_align_up_u32(
        element_shape.byte_size,
        u32::from(element_shape.alignment),
    )
    .ok_or_else(invalid)?;
    if element_stride == 0
        || match storage {
            terminal_semantics::CollectionStorage::Inline => {
                (if representation.is_some() {
                    match extent_kind {
                        terminal_semantics::CollectionExtent::Fixed(length) => Some(length),
                        _ => None,
                    }
                } else {
                    match extent_kind {
                        terminal_semantics::CollectionExtent::Fixed(length) => {
                            u64::from(element_stride).checked_mul(length)
                        }
                        // An owned byte carrier's inline elements follow its
                        // live-length word; capacity bounds their span.
                        terminal_semantics::CollectionExtent::LiveLength { capacity } => {
                            u64::from(element_stride)
                                .checked_mul(capacity)
                                .and_then(|span| span.checked_add(8))
                        }
                    }
                }) != Some(u64::from(projected_shape.byte_size))
            }
            // The carrier contains one pointer, not capacity bytes. Dynamic
            // narrowing is a following subslice over the established descriptor.
            terminal_semantics::CollectionStorage::Borrowed => {
                projected_shape.byte_size != 8 || byte_window.is_some()
            }
        }
    {
        return Err(invalid());
    }
    if let Some(layout) = representation {
        if layout.size != u64::from(element_stride)
            || layout.alignment != u64::from(element_shape.alignment)
            || layout.leaves.iter().any(|leaf| {
                scalar_leaf_offset(structural_types, element, &leaf.path, leaf.scalar_type)
                    .map(u64::from)
                    != Some(leaf.offset)
            })
        {
            return Err(invalid());
        }
    }
    let (window_offset, extent) = match (byte_window, extent_kind) {
        (Some((start, end)), terminal_semantics::CollectionExtent::Fixed(length))
            if start <= end && end <= length =>
        {
            (
                u32::try_from(
                    start
                        .checked_mul(u64::from(element_stride))
                        .ok_or_else(invalid)?,
                )
                .map_err(|_| invalid())?,
                target_operations::TargetViewExtent::Fixed(end - start),
            )
        }
        // A static window cannot bound a live count; dynamic narrowing is a
        // following subslice over the established descriptor.
        (Some(_), _) => return Err(invalid()),
        (None, terminal_semantics::CollectionExtent::Fixed(length)) => {
            (0, target_operations::TargetViewExtent::Fixed(length))
        }
        // An owned byte carrier's viewed elements start eight bytes into its
        // storage, after the live-length word.
        (None, terminal_semantics::CollectionExtent::LiveLength { .. }) => {
            (8, target_operations::TargetViewExtent::LiveLength)
        }
    };
    let source_byte_offset = source_byte_offset
        .checked_add(window_offset)
        .ok_or_else(invalid)?;
    Ok((
        root_structural_type,
        source_byte_offset,
        extent,
        element_stride,
        backing,
    ))
}

/// Resolve dominating storage supplied by the graph walk. Its origin
/// distinguishes a local establishment from an owned state arrival. The caller
/// resolves the exact array projection, excluding descriptor shells as backing.
/// This inventory is not loan authority: independent ownership replay still
/// rejects consumed roots and incompatible loans, including across transfers.
fn live_aggregate_backing(
    homes: &BTreeMap<PlaceId, TargetStructuralHomeRequirement>,
    place: PlaceId,
) -> Option<(StructuralTypeId, TargetStructuralArgumentSource)> {
    let home = homes.get(&place)?;
    if home.place() != place
        || home.access() != StructuralAccess::Owned
        || !matches!(
            home.multiplicity(),
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
        )
        || !home.qualifications().is_empty()
        || !home.projected_qualifications().is_empty()
        || home.has_claims()
    {
        return None;
    }
    let source = match &home.origin {
        TargetStructuralHomeOrigin::OperationResult { operation, .. } => {
            TargetStructuralArgumentSource::StructuralHome {
                psi_operation: *operation,
            }
        }
        TargetStructuralHomeOrigin::BlockParameter { block, declaration } => {
            TargetStructuralArgumentSource::BlockParameter {
                block: *block,
                place: declaration.place,
            }
        }
    };
    Some((home.structural_type(), source))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_view_backing_requires_a_live_exact_borrowed_arrival() {
        let element = StructuralTypeId::new(1).unwrap();
        let array = StructuralTypeId::new(2).unwrap();
        let place = PlaceId::new(2).unwrap();
        let block = semantic_vocabulary::BlockId::new(2).unwrap();
        let catalog: abstract_operations::StructuralTypeCatalog = vec![
            StructuralTypeDeclaration {
                id: element,
                identity: "u8".into(),
                shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                )),
            },
            StructuralTypeDeclaration {
                id: array,
                identity: "[u8; 4]".into(),
                shape: StructuralTypeShape::FixedArray { element, length: 4 },
            },
        ]
        .into();
        let types = StructuralTypeLookup::new(&catalog);
        let mut function = AbstractFunction {
            machine: semantic_vocabulary::MachineId::new(1).unwrap(),
            attachment: None,
            entry: semantic_vocabulary::BlockId::new(1).unwrap(),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: abstract_operations::AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![abstract_operations::AbstractBlockEntry {
                block,
                parameters: Vec::new(),
                operation_offset: 0,
                structural_parameters: vec![StructuralParameterDeclaration {
                    place,
                    position: 0,
                    is_self: false,
                    structural_type: array,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access: StructuralAccess::MutableBorrow,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                }],
            }],
            operations: Vec::new(),
        };
        let homes = BTreeMap::new();
        let addresses = BTreeSet::from([place]);
        let backings = ElementViewBackings::new(&homes, &addresses);
        let mut argument = StructuralArgument {
            place,
            path: Vec::new(),
            access: StructuralAccess::MutableBorrow,
        };
        let expected = (
            array,
            0,
            target_operations::TargetViewExtent::Fixed(4),
            1,
            TargetStructuralArgumentSource::BlockParameter { block, place },
        );
        assert_eq!(
            established_source(
                &function,
                &types,
                &[],
                backings,
                &argument,
                element,
                terminal_psi::ElementViewRegion::WholeCollection
            )
            .unwrap(),
            expected
        );
        argument.access = StructuralAccess::SharedBorrow;
        assert_eq!(
            established_source(
                &function,
                &types,
                &[],
                backings,
                &argument,
                element,
                terminal_psi::ElementViewRegion::WholeCollection
            )
            .unwrap(),
            expected
        );
        function.block_entries[0].structural_parameters[0].access = StructuralAccess::SharedBorrow;
        assert_eq!(
            established_source(
                &function,
                &types,
                &[],
                backings,
                &argument,
                element,
                terminal_psi::ElementViewRegion::WholeCollection
            )
            .unwrap(),
            expected
        );
        argument.access = StructuralAccess::MutableBorrow;
        assert!(
            established_source(
                &function,
                &types,
                &[],
                backings,
                &argument,
                element,
                terminal_psi::ElementViewRegion::WholeCollection
            )
            .is_err(),
            "a shared arrival cannot authorize a mutable collection view"
        );
        argument.access = StructuralAccess::SharedBorrow;
        let absent = BTreeSet::new();
        assert!(
            established_source(
                &function,
                &types,
                &[],
                ElementViewBackings::new(&homes, &absent),
                &argument,
                element,
                terminal_psi::ElementViewRegion::WholeCollection
            )
            .is_err(),
            "a declaration without dominating live custody cannot supply backing"
        );
        function.block_entries[0].structural_parameters[0].structural_type = element;
        assert!(
            established_source(
                &function,
                &types,
                &[],
                backings,
                &argument,
                element,
                terminal_psi::ElementViewRegion::WholeCollection
            )
            .is_err(),
            "a same-address scalar cannot impersonate the declared array"
        );
    }

    #[test]
    fn local_view_borrows_affine_backing_without_copying_ownership() {
        let place = PlaceId::new(1).unwrap();
        let producer = OperationId::new(1).unwrap();
        let result = terminal_psi::StructuralOperationResult {
            place,
            structural_type: StructuralTypeId::new(1).unwrap(),
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        };
        let mut homes = BTreeMap::from([(
            place,
            TargetStructuralHomeRequirement {
                origin: TargetStructuralHomeOrigin::OperationResult {
                    operation: producer,
                    result,
                },
                layout: target_operations::TargetStructuralHomeLayout::Aggregate(
                    ValueShape::borrowed_reference(16, 8),
                ),
            },
        )]);
        assert_eq!(
            live_aggregate_backing(&homes, place),
            Some((
                StructuralTypeId::new(1).unwrap(),
                TargetStructuralArgumentSource::StructuralHome {
                    psi_operation: producer
                },
            ))
        );
        let TargetStructuralHomeOrigin::OperationResult { result, .. } =
            &mut homes.get_mut(&place).unwrap().origin
        else {
            unreachable!()
        };
        result.multiplicity = StructuralMultiplicity::Linear;
        assert!(
            live_aggregate_backing(&homes, place).is_none(),
            "linear backing requires its own custody support"
        );
        assert!(live_aggregate_backing(&homes, PlaceId::new(2).unwrap()).is_none());
        homes.clear();
        assert!(
            live_aggregate_backing(&homes, place).is_none(),
            "a home absent from this graph context cannot supply backing"
        );
    }
}
