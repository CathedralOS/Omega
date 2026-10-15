//! Ordinary structural call operands resolved against the caller's parameter
//! roster, earlier call results, dominating local homes and established views.

use super::super::structural_layout::{
    checked_align_up_u32, resolve_structural_field_path, structural_parameter_shape,
    structural_shape,
};
use crate::LoweringError;
use crate::lowering::control_flow::scalar_sources::{ScalarSources, runtime_indices};
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::AbstractFunction;
use calling_conventions::{ValuePlacement, ValueShape};
use semantic_vocabulary::{MachineId, OperationId, PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use target_operations::{
    TargetStructuralArgument, TargetStructuralArgumentSource, TargetStructuralHomeRequirement,
    TargetStructuralParameter, TargetUnitOperation,
};
use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralTypeShape};

/// The callee/place pair every structural argument type rejection spells.
fn argument_type_mismatch(callee: MachineId, place: PlaceId) -> LoweringError {
    LoweringError::StructuralCallArgumentTypeMismatch { callee, place }
}

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
    scalar_sources: &ScalarSources<'_>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<TargetStructuralArgument, LoweringError> {
    // Qualifications are discharged by current-graph semantic admission and
    // replayed before native selection. This projection reconstructs physical
    // custody: a domain roster is neither a descriptor layout nor a substitute
    // for the argument's exact producer, access, and storage geometry.
    // A whole owned bounded byte local lends its home to a borrowed bounded
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
            return Err(argument_type_mismatch(callee, argument.place));
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
            return Err(argument_type_mismatch(callee, argument.place));
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
    if let Some(root) = function.structural_parameter(argument.place)
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
    // One geometry walk selects storage for every field/index/case path.
    // An inline byte carrier uses the callee's presentation, not a nominal
    // alias inferred from another equal-capacity declaration. The projection
    // retains runtime coordinates beside the constant offset; resolve them
    // against the same dominating scalar environment as every other argument.
    let (selected, selected_shape, source_byte_offset, indices) =
        super::super::structural_layout::runtime_storage_projection(
            source_structural_type,
            &argument.path,
            structural_types,
            shape_cache,
            active,
        )?;
    let indices = runtime_indices(indices, function, scalar_sources)?;
    let mut byte_field_capacity = None;
    let (projected_type, projected_shape) = match selected {
        super::super::structural_layout::ProjectedStorage::Field(field)
            if field.byte_sequence_carrier().is_some() =>
        {
            match field.byte_sequence_carrier() {
                Some(terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity }) => {
                    byte_field_capacity = Some(capacity);
                }
                Some(terminal_psi::ByteSequenceCarrier::BorrowedView {
                    access: Some(StructuralAccess::SharedBorrow),
                }) if callee_parameter.access == StructuralAccess::SharedBorrow
                    && structural_types
                        .get(&callee_parameter.structural_type)
                        .is_some_and(|row| row.shape.is_shared_readable_byte_view()) => {}
                _ => return Err(argument_type_mismatch(callee, argument.place)),
            }
            (
                callee_parameter.structural_type,
                structural_shape(
                    callee_parameter.structural_type,
                    structural_types,
                    shape_cache,
                    active,
                )?,
            )
        }
        selected => (
            selected.declaration(
                source_structural_type,
                structural_types,
                terminal_psi::StructuralFieldType::canonical_leaf_type,
            )?,
            selected_shape,
        ),
    };
    // Exact typed projections retain their offset and captured index terms.
    // Extent/stride ABI metadata belongs to the array-to-view presentation
    // above, not to an index spelling.
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
                        StructuralAccess::SharedBorrow => {
                            matches!(source.access, terminal_psi::reading_borrow_pair_pattern!())
                        }
                        StructuralAccess::MutableBorrow => {
                            source.access == StructuralAccess::MutableBorrow
                        }
                        StructuralAccess::WriteOnlyBorrow => matches!(
                            source.access,
                            terminal_psi::exclusive_borrow_pair_pattern!()
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
            return Err(argument_type_mismatch(callee, argument.place));
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
            indices,
            source: source_placement.clone().into(),
            destination: destination.clone(),
        });
    }
    let projected_parameter_shape = crate::lowering::structural_layout::internal_parameter_shape(
        projected_type,
        projected_shape,
        callee_parameter.access,
        structural_types,
    );
    if projected_type != callee_parameter.structural_type
        || argument.access != callee_parameter.access
        || projected_parameter_shape != shape
        || projected_shape
            .byte_size
            .checked_add(source_byte_offset)
            .is_none_or(|end| end > source_shape.byte_size)
    {
        return Err(argument_type_mismatch(callee, argument.place));
    }
    Ok(TargetStructuralArgument {
        place: argument.place,
        access: argument.access,
        path: argument.path.clone(),
        root_structural_type: source_structural_type,
        structural_type: projected_type,
        shape,
        source_byte_offset,
        fixed_array_length: None,
        element_stride: None,
        indices,
        source: source_placement.clone().into(),
        destination: destination.clone(),
    })
}

/// The producer of a whole owned bounded byte local (an owned concatenation
/// home) lent to a bounded owner formal of the same carrier and access.
fn owned_owner_home(
    argument: &terminal_psi::StructuralArgument,
    callee_parameter: &terminal_psi::StructuralParameterDeclaration,
    function: &AbstractFunction,
    local_homes: &BTreeMap<PlaceId, TargetStructuralHomeRequirement>,
    structural_types: &StructuralTypeLookup<'_>,
) -> Option<OperationId> {
    if !argument.path.is_empty()
        || argument.access != callee_parameter.access
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
/// loan for a non-linear, unqualified, claim-free operation result; the
/// shared window relation checks the path against that original backing.
/// The declaration retains the home's owned access and multiplicity; lending
/// a view does not transfer or erase the owner's disposal debt.
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
            terminal_psi::reading_borrow_pair_pattern!()
        )
        || result.multiplicity == StructuralMultiplicity::Linear
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
    {
        return Ok(None);
    }
    let presented = terminal_psi::StructuralParameterDeclaration {
        place: result.place,
        position: callee_parameter.position,
        structural_type: result.structural_type,
        multiplicity: result.multiplicity,
        access: StructuralAccess::Owned,
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
    // A received view loan forwarded whole, as the same declared view, is
    // already a presentation: its carrier is the pointer cell (and extent)
    // the callee expects, not the backing it addresses. The ordinary
    // unprojected forward passes that carrier through.
    if argument.path.is_empty()
        && root.structural_type == callee_parameter.structural_type
        && structural_types
            .get(&root.structural_type)
            .is_some_and(|declaration| declaration.shape.is_borrowed_byte_carrier())
    {
        return Ok(None);
    }
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
        // A whole incoming bounded view stores one pointer cell, not its
        // referent bytes. The semantic window proves the referent's extent;
        // native residence independently bounds the pointer cell itself.
        let stored_extent = if argument.path.is_empty()
            && matches!(
                structural_types
                    .get(&root.structural_type)
                    .map(|row| &row.shape),
                Some(StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BoundedView { .. }
                ))
            ) {
            8
        } else {
            window.backing_length
        };
        let end = u64::from(backing_offset).checked_add(stored_extent);
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
