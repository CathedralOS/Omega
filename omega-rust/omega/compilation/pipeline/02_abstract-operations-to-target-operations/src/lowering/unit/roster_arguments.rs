//! Ordinary structural call operands resolved against the caller's parameter
//! roster, earlier call results, dominating local homes and established views.

use super::super::structural_layout::{
    borrowed_view_field_offset, bounded_byte_field_geometry, checked_align_up_u32,
    resolve_structural_field_path, resolve_structural_projection_path, structural_parameter_shape,
    structural_shape,
};
use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::AbstractFunction;
use calling_conventions::{ValuePlacement, ValueShape};
use semantic_vocabulary::{MachineId, OperationId, PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use target_operations::{
    TargetStructuralArgument, TargetStructuralArgumentSource, TargetStructuralHomeRequirement,
    TargetStructuralParameter, TargetUnitOperation,
};
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, StructuralPathSegment, StructuralTypeShape,
};

/// Resolve an ordinary call operand independently of the callee's return kind.
/// Fixed-array views retain the caller's actual storage and exact extent; the
/// descriptor is an ABI presentation, not a change to the source place's type.
#[allow(clippy::too_many_arguments)]
pub(in crate::lowering) fn lower_structural_argument(
    argument: &terminal_psi::StructuralArgument,
    callee_parameter: &terminal_psi::StructuralParameterDeclaration,
    shape: ValueShape,
    destination: &ValuePlacement,
    callee: MachineId,
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    local_homes: &BTreeMap<PlaceId, TargetStructuralHomeRequirement>,
    established_views: &BTreeMap<PlaceId, (OperationId, StructuralTypeId)>,
    operations: &[TargetUnitOperation],
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<TargetStructuralArgument, LoweringError> {
    // Qualifications are discharged by current-graph semantic admission and
    // replayed before native selection. This projection reconstructs physical
    // custody: a domain roster is neither a descriptor layout nor a substitute
    // for the argument's exact producer, access, and storage geometry.
    // A whole owned bounded byte local lends its home to a shared bounded
    // owner formal. Unit validation replayed the formal's byte predicates
    // from the local's own operands, so the home's empty roster is not
    // compared against the formal's qualifications.
    let owner_home = owned_owner_home(
        argument,
        callee_parameter,
        function,
        local_homes,
        structural_types,
    );
    // The owned home is also a readable view of this activation; the owner
    // formal takes the home's own `{length, bytes}` address instead.
    if let Some(producer) = owner_home {
        let home = local_homes.get(&argument.place).ok_or(
            LoweringError::UnknownStructuralArgumentPlace {
                machine: function.machine,
                place: argument.place,
            },
        )?;
        let owner = home.layout.shape();
        if shape != structural_parameter_shape(owner, argument.access) {
            return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                callee,
                place: argument.place,
            });
        }
        return Ok(TargetStructuralArgument {
            place: argument.place,
            access: argument.access,
            path: Vec::new(),
            root_structural_type: callee_parameter.structural_type,
            structural_type: callee_parameter.structural_type,
            shape,
            source_byte_offset: 0,
            fixed_array_length: None,
            element_stride: None,
            indices: Vec::new(),
            source: TargetStructuralArgumentSource::StructuralHome {
                psi_operation: producer,
            },
            destination: destination.clone(),
        });
    }
    if let Some((source, structural_type)) = established_views
        .get(&argument.place)
        .map(|(producer, structural_type)| {
            (
                target_operations::TargetStructuralArgumentSource::EstablishedByteView {
                    psi_operation: *producer,
                },
                *structural_type,
            )
        })
        .or_else(|| {
            super::super::scalar::byte_views::block_source(
                function,
                argument.place,
                structural_types,
            )
        })
    {
        if !argument.path.is_empty()
            || argument.access != StructuralAccess::SharedBorrow
            || !super::super::scalar::byte_views::is_immutable_byte_carrier_parameter(
                callee_parameter,
                structural_types,
            )
            || structural_type != callee_parameter.structural_type
            || shape != ValueShape::borrowed_reference(16, 8)
        {
            return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                callee,
                place: argument.place,
            });
        }
        return Ok(TargetStructuralArgument {
            place: argument.place,
            access: argument.access,
            path: Vec::new(),
            root_structural_type: structural_type,
            structural_type,
            shape,
            source_byte_offset: 0,
            fixed_array_length: None,
            element_stride: None,
            indices: Vec::new(),
            source,
            destination: destination.clone(),
        });
    }
    let result_source = super::projected_result::source(operations, argument.place);
    let (source_structural_type, source_shape, source_placement) =
        if let Some(source) = parameters_by_place.get(&argument.place).copied() {
            (source.structural_type, source.shape, &source.placement)
        } else if let Some((home, placement)) = result_source {
            (home.structural_type(), home.layout.shape(), placement)
        } else if let Some(home) = local_homes.get(&argument.place) {
            // A dominating local home has no incoming placement: it lends its
            // own storage as a whole fixed array presented through a view.
            return local_array_loan(
                argument,
                callee_parameter,
                shape,
                destination,
                callee,
                home,
                structural_types,
                shape_cache,
                active,
            )?
            .ok_or(LoweringError::UnknownStructuralArgumentPlace {
                machine: function.machine,
                place: argument.place,
            });
        } else {
            return Err(LoweringError::UnknownStructuralArgumentPlace {
                machine: function.machine,
                place: argument.place,
            });
        };
    // Fixed byte windows and element views are call presentations of original
    // backing, not immutable subslice producers or a new owned structural value.
    if let Some(root) = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == argument.place)
        && let Some(loan) = fixed_array_view_argument(
            argument,
            callee_parameter,
            shape,
            destination,
            callee,
            root,
            source_shape.byte_size,
            source_placement.clone().into(),
            structural_types,
            shape_cache,
            active,
        )?
    {
        return Ok(loan);
    }
    // Borrowing selects the same original storage for every permitted access
    // attenuation. The existing resolver reconstructs each field/index hop.
    let exact_borrowed_projection = argument.access == callee_parameter.access
        && callee_parameter.multiplicity == StructuralMultiplicity::Unrestricted
        && parameters_by_place
            .get(&argument.place)
            .is_some_and(|source| {
                matches!(
                    (source.access, argument.access),
                    (
                        StructuralAccess::MutableBorrow,
                        StructuralAccess::SharedBorrow
                            | StructuralAccess::MutableBorrow
                            | StructuralAccess::WriteOnlyBorrow
                    ) | (
                        StructuralAccess::SharedBorrow,
                        StructuralAccess::SharedBorrow
                    ) | (
                        StructuralAccess::WriteOnlyBorrow,
                        StructuralAccess::WriteOnlyBorrow
                    )
                ) && source.multiplicity == StructuralMultiplicity::Unrestricted
            })
        && argument
            .path
            .iter()
            .any(|segment| matches!(segment, StructuralPathSegment::FixedIndex(_)));
    let mut byte_field_capacity = None;
    let (projected_type, projected_shape, source_byte_offset, fixed_array_length, element_stride) =
        match argument.path.as_slice() {
            [] => (source_structural_type, source_shape, 0, None, None),
            path if argument.access == StructuralAccess::Owned
                && callee_parameter.multiplicity == StructuralMultiplicity::Affine
                && (result_source.is_some()
                    || parameters_by_place
                        .get(&argument.place)
                        .is_some_and(|source| {
                            source.access == StructuralAccess::Owned
                                && source.multiplicity == StructuralMultiplicity::Affine
                        })) =>
            {
                let (selected_type, selected_shape, offset) = resolve_structural_projection_path(
                    source_structural_type,
                    path,
                    structural_types,
                    shape_cache,
                    active,
                )?;
                let (length, stride) =
                    super::super::structural_layout::root_array_projection_metadata(
                        source_structural_type,
                        structural_types,
                        shape_cache,
                        active,
                    )?;
                (selected_type, selected_shape, offset, length, stride)
            }
            path if exact_borrowed_projection => {
                let projected = resolve_structural_projection_path(
                    source_structural_type,
                    path,
                    structural_types,
                    shape_cache,
                    active,
                )
                .ok()
                .filter(|(projected_type, _, _)| {
                    matches!(
                        structural_types
                            .get(projected_type)
                            .map(|declaration| &declaration.shape),
                        Some(
                            StructuralTypeShape::PrimitiveScalar(_)
                                | StructuralTypeShape::Record { .. }
                        )
                    )
                });
                if let Some((projected_type, projected_shape, offset)) = projected {
                    (projected_type, projected_shape, offset, None, None)
                } else if let Some(field_offset) = stored_shared_view_field(
                    source_structural_type,
                    path,
                    callee_parameter,
                    structural_types,
                    shape_cache,
                    active,
                )? {
                    // A stored shared view field below a fixed element is
                    // lent exactly as one below plain fields: the argument
                    // addresses the stored descriptor in place.
                    (
                        callee_parameter.structural_type,
                        structural_shape(
                            callee_parameter.structural_type,
                            structural_types,
                            shape_cache,
                            active,
                        )?,
                        field_offset,
                        None,
                        None,
                    )
                } else {
                    return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                        callee,
                        place: argument.place,
                    });
                }
            }
            [StructuralPathSegment::FixedIndex(index)] => {
                let declaration = structural_types
                    .get(&source_structural_type)
                    .copied()
                    .ok_or(LoweringError::UnknownStructuralType(source_structural_type))?;
                let StructuralTypeShape::FixedArray { element, length } = declaration.shape else {
                    return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                        callee,
                        place: argument.place,
                    });
                };
                if *index >= length {
                    return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                        callee,
                        place: argument.place,
                    });
                }
                let element_shape =
                    structural_shape(element, structural_types, shape_cache, active)?;
                let stride = checked_align_up_u32(
                    element_shape.byte_size,
                    u32::from(element_shape.alignment),
                )
                .ok_or(LoweringError::StructuralTypeTooLarge(
                    source_structural_type,
                ))?;
                let offset = u64::from(stride)
                    .checked_mul(*index)
                    .and_then(|offset| u32::try_from(offset).ok())
                    .ok_or(LoweringError::StructuralTypeTooLarge(
                        source_structural_type,
                    ))?;
                (element, element_shape, offset, Some(length), Some(stride))
            }
            path @ [StructuralPathSegment::Field(_), ..]
                if path
                    .iter()
                    .all(|segment| matches!(segment, StructuralPathSegment::Field(_))) =>
            {
                match resolve_structural_field_path(
                    source_structural_type,
                    path,
                    structural_types,
                    shape_cache,
                    active,
                ) {
                    Ok((field_type, field_shape, offset)) => {
                        (field_type, field_shape, offset, None, None)
                    }
                    Err(_) => {
                        // A stored shared view field already holds the
                        // descriptor a shared view formal borrows: the
                        // argument addresses that field like a projected
                        // child of the formal's own view type.
                        if let Some(field_offset) = stored_shared_view_field(
                            source_structural_type,
                            path,
                            callee_parameter,
                            structural_types,
                            shape_cache,
                            active,
                        )? {
                            (
                                callee_parameter.structural_type,
                                structural_shape(
                                    callee_parameter.structural_type,
                                    structural_types,
                                    shape_cache,
                                    active,
                                )?,
                                field_offset,
                                None,
                                None,
                            )
                        } else {
                            // An inline bounded byte field has no structural type
                            // identity of its own; its record still supplies exact
                            // offset and capacity for a borrowed-view presentation.
                            let Some((field_offset, capacity)) = bounded_byte_field_geometry(
                                source_structural_type,
                                path,
                                structural_types,
                                shape_cache,
                                active,
                            )?
                            else {
                                return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                                    callee,
                                    place: argument.place,
                                });
                            };
                            byte_field_capacity = Some(capacity);
                            (
                                callee_parameter.structural_type,
                                structural_shape(
                                    callee_parameter.structural_type,
                                    structural_types,
                                    shape_cache,
                                    active,
                                )?,
                                field_offset,
                                None,
                                None,
                            )
                        }
                    }
                }
            }
            _ => {
                return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                    callee,
                    place: argument.place,
                });
            }
        };
    // A bounded inline byte field lends its live bytes through a fresh
    // borrowed-view descriptor. `source_byte_offset` names the field itself:
    // the descriptor's length is the live length word stored there and the
    // bytes begin eight bytes later. Declared capacity bounds the presentation;
    // it is never the descriptor's length.
    if let Some(capacity) = byte_field_capacity {
        let source_is_subloan = parameters_by_place
            .get(&argument.place)
            .is_some_and(|source| {
                source.multiplicity == StructuralMultiplicity::Unrestricted
                    && source.projected_qualifications.is_empty()
                    && match argument.access {
                        StructuralAccess::SharedBorrow => matches!(
                            source.access,
                            StructuralAccess::MutableBorrow | StructuralAccess::SharedBorrow
                        ),
                        StructuralAccess::MutableBorrow => {
                            source.access == StructuralAccess::MutableBorrow
                        }
                        StructuralAccess::WriteOnlyBorrow => matches!(
                            source.access,
                            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                        ),
                        _ => false,
                    }
            });
        let presented_is_byte_view = matches!(
            structural_types
                .get(&callee_parameter.structural_type)
                .map(|declaration| &declaration.shape),
            Some(StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
            ))
        );
        let presented_is_owner = structural_types
            .get(&callee_parameter.structural_type)
            .is_some_and(|declaration| {
                declaration.shape
                    == StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
                    )
            });
        let expected_shape = if presented_is_owner {
            structural_parameter_shape(projected_shape, callee_parameter.access)
        } else {
            ValueShape::borrowed_reference(16, 8)
        };
        if !(presented_is_byte_view || presented_is_owner)
            || argument.access != callee_parameter.access
            || !matches!(
                argument.access,
                StructuralAccess::SharedBorrow
                    | StructuralAccess::MutableBorrow
                    | StructuralAccess::WriteOnlyBorrow
            )
            || callee_parameter.multiplicity != StructuralMultiplicity::Unrestricted
            || (!presented_is_owner && !callee_parameter.qualifications.is_empty())
            || !callee_parameter.projected_qualifications.is_empty()
            || !source_is_subloan
            || (!presented_is_owner && argument.access == StructuralAccess::WriteOnlyBorrow)
            || shape != expected_shape
            || u64::from(source_byte_offset)
                .checked_add(8)
                .and_then(|end| end.checked_add(capacity))
                .is_none_or(|end| end > u64::from(source_shape.byte_size))
        {
            return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                callee,
                place: argument.place,
            });
        }
        return Ok(TargetStructuralArgument {
            place: argument.place,
            access: argument.access,
            path: argument.path.clone(),
            root_structural_type: source_structural_type,
            structural_type: callee_parameter.structural_type,
            shape,
            source_byte_offset,
            fixed_array_length: None,
            element_stride: None,
            indices: Vec::new(),
            source: source_placement.clone().into(),
            destination: destination.clone(),
        });
    }
    let projected_parameter_shape =
        structural_parameter_shape(projected_shape, callee_parameter.access);
    if projected_type != callee_parameter.structural_type
        || argument.access != callee_parameter.access
        || projected_parameter_shape != shape
        || shape
            .byte_size
            .checked_add(source_byte_offset)
            .is_none_or(|end| end > source_shape.byte_size)
    {
        return Err(LoweringError::StructuralCallArgumentTypeMismatch {
            callee,
            place: argument.place,
        });
    }
    Ok(TargetStructuralArgument {
        place: argument.place,
        access: argument.access,
        path: argument.path.clone(),
        root_structural_type: source_structural_type,
        structural_type: projected_type,
        shape,
        source_byte_offset,
        fixed_array_length,
        element_stride,
        indices: Vec::new(),
        source: source_placement.clone().into(),
        destination: destination.clone(),
    })
}

/// The producer of a whole owned bounded byte local (an owned concatenation
/// home) lent shared to a bounded owner formal of the same carrier.
fn owned_owner_home(
    argument: &terminal_psi::StructuralArgument,
    callee_parameter: &terminal_psi::StructuralParameterDeclaration,
    function: &AbstractFunction,
    local_homes: &BTreeMap<PlaceId, TargetStructuralHomeRequirement>,
    structural_types: &StructuralTypeLookup<'_>,
) -> Option<OperationId> {
    if !argument.path.is_empty()
        || argument.access != StructuralAccess::SharedBorrow
        || !super::super::scalar::byte_views::is_owner_loan(callee_parameter, structural_types)
    {
        return None;
    }
    let (producer, result) = local_homes.get(&argument.place)?.operation_result()?;
    (result.place == argument.place
        && result.structural_type == callee_parameter.structural_type
        && result.multiplicity == StructuralMultiplicity::Unrestricted
        && result.projected_qualifications.is_empty()
        && result.qualification_establishments.is_empty()
        && result.claims.is_empty()
        && function.operations.iter().any(|operation| {
            matches!(operation,
                abstract_operations::AbstractOperation::EstablishByteSequenceConcatenation {
                    psi_operation, result, ..
                } if *psi_operation == producer && result.place == argument.place)
        }))
    .then_some(producer)
}

/// An owned local array lent by borrow presents the same fixed backing an
/// incoming borrowed parameter would. Terminal verification admits exactly this
/// loan for an unrestricted, unqualified, claim-free operation result; the
/// shared window relation checks the path against that original backing.
/// The presented declaration carries the argument's own access
/// for window geometry while the home itself stays owned by this activation.
/// `Ok(None)` means the home is not lendable through this parameter's view.
#[allow(clippy::too_many_arguments)]
pub(in crate::lowering) fn local_array_loan(
    argument: &terminal_psi::StructuralArgument,
    callee_parameter: &terminal_psi::StructuralParameterDeclaration,
    shape: ValueShape,
    destination: &ValuePlacement,
    callee: MachineId,
    home: &TargetStructuralHomeRequirement,
    structural_types: &StructuralTypeLookup<'_>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<Option<TargetStructuralArgument>, LoweringError> {
    let Some((producer, result)) = home.operation_result() else {
        return Ok(None);
    };
    if result.place != argument.place
        || !matches!(
            argument.access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        )
        || result.multiplicity != StructuralMultiplicity::Unrestricted
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
    {
        return Ok(None);
    }
    let presented = terminal_psi::StructuralParameterDeclaration {
        place: result.place,
        position: callee_parameter.position,
        is_self: false,
        structural_type: result.structural_type,
        multiplicity: result.multiplicity,
        access: argument.access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    fixed_array_view_argument(
        argument,
        callee_parameter,
        shape,
        destination,
        callee,
        &presented,
        home.layout.shape().byte_size,
        TargetStructuralArgumentSource::StructuralHome {
            psi_operation: producer,
        },
        structural_types,
        shape_cache,
        active,
    )
}

/// Present a fixed array's original backing through a borrowed byte or element
/// view. `root` declares that backing as the call presents it: an incoming
/// parameter, or a local home presented through the loan. The callee receives a
/// descriptor over the exact extent Terminal proved, while `source` still names
/// the caller's own storage rather than a descriptor allocation. A byte window
/// may select a constant range of a field; an element view lends the whole
/// array at `argument.path` with the element's aligned stride. `Ok(None)` means
/// the argument is no fixed-array view of `root`.
#[allow(clippy::too_many_arguments)]
pub(in crate::lowering) fn fixed_array_view_argument(
    argument: &terminal_psi::StructuralArgument,
    callee_parameter: &terminal_psi::StructuralParameterDeclaration,
    shape: ValueShape,
    destination: &ValuePlacement,
    callee: MachineId,
    root: &terminal_psi::StructuralParameterDeclaration,
    root_byte_size: u32,
    source: TargetStructuralArgumentSource,
    structural_types: &StructuralTypeLookup<'_>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<Option<TargetStructuralArgument>, LoweringError> {
    let mismatch = || LoweringError::StructuralCallArgumentTypeMismatch {
        callee,
        place: argument.place,
    };
    let too_large = || LoweringError::StructuralTypeTooLarge(root.structural_type);
    let (source_byte_offset, length, stride, backing_end) = if let Some(window) =
        terminal_semantics::fixed_byte_array_window(
            structural_types.values().copied(),
            root,
            argument,
            callee_parameter,
        ) {
        let backing_offset = if window.backing_path.is_empty() {
            0
        } else {
            resolve_structural_field_path(
                root.structural_type,
                window.backing_path,
                structural_types,
                shape_cache,
                active,
            )?
            .2
        };
        let offset = u64::from(backing_offset)
            .checked_add(window.offset)
            .and_then(|offset| u32::try_from(offset).ok())
            .ok_or_else(too_large)?;
        let end = u64::from(backing_offset).checked_add(window.backing_length);
        (offset, window.length, 1, end)
    } else if let Some(length) = terminal_semantics::fixed_element_array_extent(
        structural_types.values().copied(),
        root,
        argument,
        callee_parameter,
    ) {
        let (array_type, backing_offset) = if argument.path.is_empty() {
            (root.structural_type, 0)
        } else {
            let (array_type, _, backing_offset) = resolve_structural_field_path(
                root.structural_type,
                &argument.path,
                structural_types,
                shape_cache,
                active,
            )?;
            (array_type, backing_offset)
        };
        let Some(StructuralTypeShape::FixedArray { element, .. }) = structural_types
            .get(&array_type)
            .map(|declaration| &declaration.shape)
        else {
            return Err(mismatch());
        };
        let element_shape = structural_shape(*element, structural_types, shape_cache, active)?;
        let stride =
            checked_align_up_u32(element_shape.byte_size, u32::from(element_shape.alignment))
                .ok_or_else(too_large)?;
        let end = u64::from(stride)
            .checked_mul(length)
            .and_then(|extent| u64::from(backing_offset).checked_add(extent));
        (backing_offset, length, stride, end)
    } else {
        return Ok(None);
    };
    // The verified view declaration decides whether extent is carried at
    // runtime. A bounded byte view has one pointer cell; dynamic byte and
    // element views additionally carry their extent. Neither changes backing.
    let carrier = structural_shape(
        callee_parameter.structural_type,
        structural_types,
        shape_cache,
        active,
    )?;
    if shape != ValueShape::borrowed_reference(carrier.byte_size, carrier.alignment)
        || backing_end.is_none_or(|end| end > u64::from(root_byte_size))
    {
        return Err(mismatch());
    }
    Ok(Some(TargetStructuralArgument {
        place: argument.place,
        access: argument.access,
        path: argument.path.clone(),
        root_structural_type: root.structural_type,
        structural_type: callee_parameter.structural_type,
        shape,
        source_byte_offset,
        fixed_array_length: Some(length),
        element_stride: Some(stride),
        indices: Vec::new(),
        source,
        destination: destination.clone(),
    }))
}

/// The descriptor offset of a stored shared view field lent whole to a
/// shared view formal. Neither side may be exclusive: a mutable view would
/// duplicate or widen the stored loan.
fn stored_shared_view_field(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    callee_parameter: &terminal_psi::StructuralParameterDeclaration,
    structural_types: &StructuralTypeLookup<'_>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<Option<u32>, LoweringError> {
    let shared_formal = callee_parameter.access == StructuralAccess::SharedBorrow
        && structural_types
            .get(&callee_parameter.structural_type)
            .is_some_and(|declaration| {
                matches!(
                    declaration.shape,
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView {
                            access: None | Some(StructuralAccess::SharedBorrow),
                        }
                    )
                )
            });
    let Some((leaf, prefix)) = path.split_last() else {
        return Ok(None);
    };
    let StructuralPathSegment::Field(identity) = leaf else {
        return Ok(None);
    };
    let shared_field = terminal_semantics::runtime_structural_path_tip(
        structural_types.values().copied(),
        root,
        prefix,
    )
    .is_some_and(|parent| match &parent.shape {
        StructuralTypeShape::Record { fields } => fields.iter().any(|field| {
            field.identity == *identity
                && !field.relevance.is_erased()
                && field.field_type.byte_sequence_carrier()
                    == Some(terminal_psi::ByteSequenceCarrier::BorrowedView {
                        access: Some(StructuralAccess::SharedBorrow),
                    })
        }),
        _ => false,
    });
    if !shared_formal || !shared_field {
        return Ok(None);
    }
    borrowed_view_field_offset(root, path, structural_types, shape_cache, active)
}
