//! Element observation and view operations for common graph lowering.
use super::{KnownInteger, KnownScalar};
use crate::LoweringError;
use crate::lowering::scalar::expressions::{insert_value, unsigned_u64_endpoint};
use crate::lowering::structural_layout::{
    direct_scalar_field_geometry, runtime_projection, runtime_storage_projection,
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
    views: &'a BTreeMap<PlaceId, (OperationId, StructuralTypeId)>,
    address_joins: &'a BTreeSet<PlaceId>,
    pub(super) selectors: crate::lowering::control_flow::scalar_sources::ScalarSources<'a>,
}

impl<'a> ElementViewBackings<'a> {
    pub(in crate::lowering) fn new(
        homes: &'a BTreeMap<PlaceId, TargetStructuralHomeRequirement>,
        views: &'a BTreeMap<PlaceId, (OperationId, StructuralTypeId)>,
        address_joins: &'a BTreeSet<PlaceId>,
        selectors: crate::lowering::control_flow::scalar_sources::ScalarSources<'a>,
    ) -> Self {
        Self {
            homes,
            views,
            address_joins,
            selectors,
        }
    }

    /// Rejoin every completed descriptor to its exact unique operation result.
    /// A shared descriptor may keep SSA words without an allocated home; that
    /// changes storage realization, not its producer or declared carrier.
    pub(in crate::lowering) fn view_result<'function>(
        self,
        function: &'function AbstractFunction,
        place: PlaceId,
    ) -> Option<&'function terminal_psi::StructuralOperationResult> {
        let (producer, structural_type) = self.views.get(&place)?;
        let mut results = function.operations.iter().filter_map(|operation| {
            operation
                .structural_result()
                .filter(|(_, result)| result.place == place)
        });
        let (operation, result) = results.next()?;
        (results.next().is_none()
            && operation == *producer
            && result.structural_type == *structural_type)
            .then_some(result)
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
                .structural_parameter(source.place)
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

    /// A block-carried borrowed descriptor is the collection's own backing:
    /// the incoming pointer names the referent's storage directly. The
    /// descriptor family covers bounded and unbounded byte views alike; the
    /// geometry walk below decides which descriptor the establish can form.
    fn descriptor(
        self,
        function: &AbstractFunction,
        structural_types: &StructuralTypeLookup<'_>,
        source: &StructuralArgument,
    ) -> Option<(StructuralTypeId, TargetStructuralArgumentSource)> {
        let (entry, parameter) = function.block_entries.iter().find_map(|entry| {
            entry
                .structural_parameter(source.place)
                .map(|parameter| (entry, parameter))
        })?;
        let descriptor_carrier = structural_types
            .get(&parameter.structural_type)
            .is_some_and(|declaration| declaration.shape.is_borrowed_byte_carrier());
        if entry.block == function.entry
            || !matches!(
                parameter.access,
                terminal_psi::reading_borrow_pair_pattern!()
            )
            || (source.access == StructuralAccess::MutableBorrow
                && parameter.access != StructuralAccess::MutableBorrow)
            || parameter.multiplicity != StructuralMultiplicity::Unrestricted
            || !parameter.projected_qualifications.is_empty()
            || !descriptor_carrier
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
    terminal_semantics::view_descriptor_parameter(parameter, structural_types.values().copied())
        && parameter.qualifications.is_empty()
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
    if let AbstractOperation::ElementViewLength {
        psi_operation,
        result,
        source,
    } = operation
    {
        let invalid = || super::descriptor_operands::refusal(function, &operation);
        let Some(scalar_type) = result.scalar_type.as_integer() else {
            return Err(invalid());
        };
        if scalar_type.sign() != IntegerSign::Unsigned || scalar_type.bits() != 64 {
            return Err(invalid());
        }
        let (selected, indices) = super::descriptor_operands::descriptor_projection(
            function,
            structural_types,
            backings,
            source,
        )?;
        if !structural_types.get(&selected).is_some_and(|declaration| {
            matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
        }) {
            return Err(invalid());
        }
        insert_value(
            values,
            result.value,
            KnownScalar::Integer {
                scalar_type,
                value: KnownInteger::Runtime(TargetIntegerExpression::ElementViewLength {
                    psi_operation: *psi_operation,
                    source_value: result.value,
                    source: source.clone(),
                    indices,
                    length_byte_offset: 8,
                }),
            },
        )?;
        provenance.push(*psi_operation);
        return Ok(true);
    }
    let (psi_operation, result, source) = match operation {
        AbstractOperation::ElementViewRead {
            psi_operation,
            result,
            source,
            ..
        } => (*psi_operation, *result, *source),
        _ => return Ok(false),
    };
    let invalid = || super::descriptor_operands::refusal(function, &operation);
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

/// Float observations own durable scalar homes in the graph lowering caller.
/// Resolve the same descriptor, count and scalar leaf as other element reads;
/// never reinterpret a float as an integer expression.
pub(in crate::lowering) fn float_element_read(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    parameters: &[TargetStructuralParameter],
    backings: ElementViewBackings<'_>,
    values: &BTreeMap<ValueId, KnownScalar>,
    lengths: &BTreeMap<ValueId, PlaceId>,
) -> Result<target_operations::TargetScalarExpression, LoweringError> {
    let invalid = || super::descriptor_operands::refusal(function, operation);
    let AbstractOperation::ElementViewRead {
        psi_operation,
        result,
        source,
        index,
        length,
        obligation,
        path,
    } = operation
    else {
        return Err(invalid());
    };
    let ScalarType::IeeeFloat(format) = result.scalar_type else {
        return Err(invalid());
    };
    let (view, view_type) = element_view_for_place(
        function,
        structural_types,
        parameters,
        backings,
        values,
        lengths,
        *source,
        &mut Vec::new(),
    )?;
    if !observes_length(values, lengths, *length, *source) {
        return Err(invalid());
    }
    let element_byte_offset =
        element_leaf_offset(structural_types, view_type, path, result.scalar_type)
            .ok_or_else(invalid)?;
    let Some(KnownScalar::Integer { scalar_type, value }) = values.get(index) else {
        return Err(invalid());
    };
    if scalar_type.sign() != IntegerSign::Unsigned || scalar_type.bits() != 64 {
        return Err(invalid());
    }
    Ok(
        target_operations::TargetScalarExpression::IeeeFloatElementViewRead {
            format,
            psi_operation: *psi_operation,
            source_value: result.value,
            source: *source,
            view: Box::new(view),
            index: Box::new(value.clone().into_expression(*index)),
            length: *length,
            obligation: *obligation,
            path: path.clone(),
            element_byte_offset,
        },
    )
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
        shape: StructuralTypeShape::ElementView { element, .. },
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
            let (parent, _, offset, _) =
                runtime_projection(element, prefix, structural_types, &mut cache, &mut active)
                    .ok()?;
            (parent, offset)
        };
        let StructuralTypeShape::Record { fields, .. } = &structural_types.get(&parent)?.shape
        else {
            return None;
        };
        let field = terminal_psi::field_by_identity(fields, *identity)?;
        if field.field_type.scalar_type() == Some(scalar) {
            return parent_offset.checked_add(
                direct_scalar_field_geometry(parent, field.id, scalar, structural_types)
                    .ok()?
                    .0,
            );
        }
    }
    runtime_projection(element, path, structural_types, &mut cache, &mut active)
        .ok()
        .map(|(_, _, offset, _)| offset)
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
    let invalid = || {
        super::descriptor_operands::refusal(
            function,
            &(place, backings.homes.get(&place), values, lengths),
        )
    };
    if active.contains(&place) {
        return Err(invalid());
    }
    if let Some((psi_operation, result)) =
        super::byte_views::descriptor_result(function, place, structural_types, backings.homes)
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
            terminal_semantics::descriptor_results::result_access(
                structural_types.values().copied(),
                result,
            )
            .ok_or_else(invalid)?,
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
                    StructuralTypeShape::ElementView {
                        element: declared, ..
                    } => declared == element,
                    StructuralTypeShape::ByteSequence(_) => {
                        declaration.shape.is_borrowed_byte_carrier()
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
        let (
            root_structural_type,
            source_byte_offset,
            indices,
            extent,
            element_stride,
            source_home,
        ) = established_source(
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
                let endpoint = unsigned_u64_endpoint(values, &invalid);
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
                indices,
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
            .structural_parameter(place)
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
    if let Some(semantic) = function.structural_parameter(place) {
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
        terminal_psi::nonlinear_multiplicity_pair_pattern!()
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
    let endpoint = unsigned_u64_endpoint(values, &invalid);
    let start = endpoint(*start)?;
    let end = endpoint(*end)?;
    if !super::descriptor_operands::observes_count(function, values, *length, source, false) {
        return Err(invalid());
    }
    let (source_type, indices) = super::descriptor_operands::descriptor_projection(
        function,
        structural_types,
        backings,
        source,
    )?;
    let source_access = source.access;
    let access = match result.multiplicity {
        StructuralMultiplicity::Unrestricted => StructuralAccess::SharedBorrow,
        StructuralMultiplicity::Affine if source_access == StructuralAccess::MutableBorrow => {
            StructuralAccess::MutableBorrow
        }
        _ => return Err(invalid()),
    };
    // Stored descriptors retain their access ceiling in their type. A newly
    // borrowed window may use the canonical presentation of the same element;
    // its loan access was established independently above.
    if !structural_types
        .get(&source_type)
        .zip(structural_types.get(&result.structural_type))
        .is_some_and(|(source, result)| {
            matches!((&source.shape, &result.shape),
            (StructuralTypeShape::ElementView { element: source, .. },
             StructuralTypeShape::ElementView { element: result, access: ceiling })
                if source == result && ceiling.is_none_or(|ceiling| ceiling == access))
        })
    {
        return Err(invalid());
    }
    Ok((
        TargetElementView::Subslice {
            psi_operation: *psi_operation,
            place,
            source: source.clone(),
            indices,
            start,
            end,
            length: *length,
            obligation: *obligation,
        },
        result.structural_type,
        access,
    ))
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
        Vec<target_operations::TargetStructuralRuntimeIndex>,
        target_operations::TargetViewExtent,
        u32,
        TargetStructuralArgumentSource,
    ),
    LoweringError,
> {
    let invalid = || {
        super::descriptor_operands::refusal(
            function,
            &(
                source,
                element,
                region,
                function.structural_parameter(source.place),
                backings.homes.get(&source.place),
                backings.address_joins.contains(&source.place),
            ),
        )
    };
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
    if !matches!(source.access, terminal_psi::reading_borrow_pair_pattern!()) {
        return Err(invalid());
    }
    // Declared domains constrain contents, not the backing layout. The source
    // keeps those obligations; this operation only lends an unqualified view.
    let (root_structural_type, backing) =
        if let Some(semantic) = function.structural_parameter(source.place) {
            if !matches!(
                semantic.access,
                StructuralAccess::SharedBorrow | terminal_psi::mutating_access_pair_pattern!()
            ) || (source.access == StructuralAccess::MutableBorrow
                && !matches!(
                    semantic.access,
                    terminal_psi::mutating_access_pair_pattern!()
                ))
                || semantic.multiplicity != StructuralMultiplicity::Unrestricted
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
                .descriptor(function, structural_types, source)
                .or_else(|| backings.address(function, structural_types, source))
                .ok_or_else(invalid)?
        };
    let mut shape_cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    let (projected_storage, projected_shape, source_byte_offset, runtime_elements) =
        runtime_storage_projection(
            root_structural_type,
            field_path,
            structural_types,
            &mut shape_cache,
            &mut active,
        )?;
    let indices = crate::lowering::control_flow::scalar_sources::runtime_indices(
        runtime_elements,
        function,
        &backings.selectors,
    )?;
    let collection_shape = projected_storage
        .shape(structural_types)
        .ok_or_else(invalid)?;
    let geometry = terminal_semantics::element_view_shape_region_geometry(
        structural_types.values().copied(),
        &collection_shape,
        element,
        region,
        source.access,
    )
    .ok_or_else(invalid)?;
    if region == terminal_psi::ElementViewRegion::WholeCollection
        && let StructuralTypeShape::FixedArray {
            element: original, ..
        } = &*collection_shape
        && *original != element
    {
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
                    scalar_leaf_offset(structural_types, identity, &leaf.path, leaf.scalar_type)
                        .map(u64::from)
                        != Some(leaf.offset)
                })
            {
                return Err(invalid());
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
            let storage = if matches!(&*collection_shape, StructuralTypeShape::ByteSequence(_)) {
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
                        terminal_semantics::CollectionExtent::StoredLength => None,
                    }
                }) != Some(u64::from(projected_shape.byte_size))
            }
            // The carrier contains one pointer, not capacity bytes. Dynamic
            // narrowing is a following subslice over the established descriptor.
            terminal_semantics::CollectionStorage::Borrowed => {
                let carrier_size = match extent_kind {
                    terminal_semantics::CollectionExtent::StoredLength => 16,
                    terminal_semantics::CollectionExtent::Fixed(_) => 8,
                    terminal_semantics::CollectionExtent::LiveLength { .. } => {
                        return Err(invalid());
                    }
                };
                projected_shape.byte_size != carrier_size || byte_window.is_some()
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
        (None, terminal_semantics::CollectionExtent::StoredLength) => {
            (0, target_operations::TargetViewExtent::StoredLength)
        }
    };
    let source_byte_offset = source_byte_offset
        .checked_add(window_offset)
        .ok_or_else(invalid)?;
    Ok((
        root_structural_type,
        source_byte_offset,
        indices,
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
            terminal_psi::nonlinear_multiplicity_pair_pattern!()
        )
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
        }) => observed.place == source && observed.path.is_empty() && *source_value == length,
        KnownInteger::Runtime(TargetIntegerExpression::ScalarHome(home)) => {
            home.source_value == length
                && home.scalar_type == ScalarType::Integer(*scalar_type)
                && lengths.get(&length) == Some(&source)
        }
        _ => false,
    }
}
