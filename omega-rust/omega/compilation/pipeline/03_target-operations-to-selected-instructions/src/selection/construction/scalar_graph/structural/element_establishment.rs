//! Establish an element-view descriptor over resolved fixed-array storage.
use super::local_storage::{address, store};
use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    SelectedInstructionKind, VirtualRegisterId,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::invalid;
use crate::selection::construction::scalar_graph::structural::provenance;
use crate::selection::construction::scalar_graph::structural::transport_register;
use selected_instructions::{LocalStorageSlotId, SelectedLocalStorageSlot};
use semantic_vocabulary::{IntegerValue, StructuralPlaceKind};
use terminal_psi::{StructuralAccess, StructuralTypeShape};

pub(super) fn establish(
    function: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::EstablishElementView {
        result,
        destination,
        source,
        element,
        region,
        accepted_fact,
        indices,
    } = &row.kind
    else {
        return Err(invalid());
    };
    if row.result.is_some()
        || result.place != *destination
        // A mutable loan's result carries the exclusive custody's affine
        // multiplicity; descriptor layout is independent of loan custody.
        || !matches!(
            result.multiplicity,
            terminal_psi::StructuralMultiplicity::Unrestricted
                | terminal_psi::StructuralMultiplicity::Affine
        )
        || !result.qualifications.is_empty()
        || result.projected_qualifications.iter().any(|row| !matches!(row.path.first(), Some(terminal_psi::StructuralPathSegment::AllElements)))
        || !result.claims.is_empty()
        || !matches!(
            source.access,terminal_psi::reading_borrow_pair_pattern!())
        || builder
            .transport
            .pointers
            .iter()
            .any(|(place, _)| *place == *destination)
    {
        return Err(invalid());
    }
    let Some(signature) = function.structural.as_ref() else {
        return Err(invalid());
    };
    let Some(place) = signature.structural_place(*destination) else {
        return Err(invalid());
    };
    if !matches!(
        place.kind,
        StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        } if producer == row.operation && structural_type == result.structural_type
    ) {
        return Err(invalid());
    }
    let Some(declaration) = signature.structural_type(result.structural_type) else {
        return Err(invalid());
    };
    match &declaration.shape {
        StructuralTypeShape::ElementView {
            element: view_element,
            ..
        } if *view_element == *element => {}
        // A `&[u8]` result declares its view over the `u8` element instead of
        // an `ElementView` shell; the descriptor shape is identical. A bounded
        // carrier's extent rides its declared bound rather than a stored
        // length, the same access-bearing descriptor a `&[u8]` spells.
        StructuralTypeShape::ByteSequence(_)
            if declaration.shape.is_borrowed_byte_carrier()
                && signature
                    .structural_type(*element)
                    .is_some_and(|element_declaration| {
                        matches!(
                            element_declaration.shape,
                            StructuralTypeShape::PrimitiveScalar(
                                semantic_vocabulary::ScalarType::Integer(integer)
                            ) if Ok(integer)
                                == semantic_vocabulary::IntegerType::new(
                                    semantic_vocabulary::IntegerSign::Unsigned,
                                    8,
                                )
                        )
                    }) => {}
        _ => return Err(invalid()),
    }
    // Invocation parameters and address joins lend their live referent pointer;
    // operation results and owned block arrivals lend their exact storage slot.
    // A join's carrier slot contains an address, never the aggregate's bytes.
    let parameter = signature
        .parameters
        .iter()
        .find(|parameter| parameter.semantic.place == source.place)
        .map(|parameter| &parameter.semantic)
        .or_else(|| {
            crate::selection::address_join_input::join(function, source.place)
                .map(|(_, parameter)| parameter)
        });
    let home = crate::selection::aggregate_result_input::home(function, source.place);
    let local_root = home.map(|(_, home)| local_home_root(home));
    let block_home = crate::selection::aggregate_result_input::block_home(function, source.place);
    // A block-carried borrowed descriptor arrives as the collection's own
    // backing: the edge transported the pointer whose referent carries the
    // view's storage, the same root an address join lends a plain referent.
    let descriptor_root = function
        .blocks
        .iter()
        .find_map(|block| {
            (block.id != function.entry_block)
                .then_some(block)
                .and_then(|block| block.structural_parameter(source.place))
        })
        .filter(|parameter| {
            matches!(
                parameter.access,
                terminal_psi::reading_borrow_pair_pattern!()
            ) && (parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                || matches!(
                    (parameter.access, parameter.multiplicity),
                    (
                        terminal_psi::StructuralAccess::MutableBorrow
                            | terminal_psi::StructuralAccess::WriteOnlyBorrow,
                        terminal_psi::StructuralMultiplicity::Affine,
                    )
                ))
                && parameter.projected_qualifications.is_empty()
                && signature
                    .structural_type(parameter.structural_type)
                    .is_some_and(|declaration| declaration.shape.is_borrowed_byte_carrier())
        });
    let Some(root) = parameter
        .or(local_root.as_ref())
        .or(block_home.map(|(_, declaration)| declaration))
        .or(descriptor_root)
    else {
        return Err(invalid());
    };
    let Some(geometry) =
        crate::structural_inputs::structural_reference_input::collection_view_region_geometry(
            root,
            source,
            result.structural_type,
            &signature.structural_types,
            *region,
        )
    else {
        return Err(invalid());
    };
    if !crate::structural_inputs::structural_reference_input::runtime_indices_match(
        indices,
        &geometry.elements,
    ) {
        return Err(invalid());
    }
    let field_offset = geometry.offset;
    let element_count = geometry.extent;
    let descriptor = terminal_semantics::collection_view_descriptor(
        &declaration.shape,
        Some(element_count),
        *region,
        &source.path,
        signature.structural_type(*element),
    )
    .ok_or_else(invalid)?;
    let descriptor_size = match descriptor {
        terminal_semantics::CollectionViewDescriptor::StoredLength => 16,
        terminal_semantics::CollectionViewDescriptor::DeclaredLength(_) => 8,
    };
    // An empty view has no dereferenceable payload. Keep its independently
    // checked backing identity, but do not require a slot for a zero-byte
    // array or invent dummy storage. Its descriptor is the empty (0, 0) view.
    let base = if element_count == terminal_semantics::CollectionExtent::Fixed(0)
        && matches!(region, terminal_psi::ElementViewRegion::WholeCollection)
    {
        constant(builder, row, 0, 0)?
    } else if parameter.is_some() || descriptor_root.is_some() {
        // An empty path selects the collection carrier itself. Its geometry
        // below resolves borrowed backing once; projection_base would already
        // dereference a view and make that load interpret payload as a pointer.
        // A child projection instead starts in the root's element storage.
        if source.path.is_empty() {
            builder
                .transport
                .pointers
                .iter()
                .find(|(place, _)| *place == source.place)
                .map(|(_, pointer)| *pointer)
                .ok_or_else(|| invalid())?
        } else {
            super::runtime_address::projection_base(
                function,
                builder,
                row,
                source.place,
                field_offset,
            )?
            .ok_or_else(|| invalid())?
        }
    } else {
        let mut homes = builder.transport.local_slots.iter().filter(|slot| {
            Some(slot.id)
                == home
                    .map(|(operation, _)| LocalStorageSlotId::Structural {
                        operation,
                        place: source.place,
                    })
                    .or_else(|| {
                        block_home.map(|(block, _)| LocalStorageSlotId::StructuralBlockParameter {
                            block,
                            place: source.place,
                        })
                    })
        });
        let (Some(home), None) = (homes.next().cloned(), homes.next()) else {
            return Err(invalid());
        };
        address(builder, row, home.id, 0, home.byte_size, false)?
    };
    let base = if element_count == terminal_semantics::CollectionExtent::Fixed(0)
        && matches!(region, terminal_psi::ElementViewRegion::WholeCollection)
    {
        base
    } else {
        super::runtime_address::scale(builder, row, source.place, field_offset, base, indices)?
    };
    let slot = LocalStorageSlotId::Structural {
        operation: row.operation,
        place: *destination,
    };
    builder
        .transport
        .local_slots
        .push(SelectedLocalStorageSlot {
            id: slot,
            byte_size: descriptor_size,
            alignment: 8,
        });
    let backing = if element_count == terminal_semantics::CollectionExtent::Fixed(0)
        && matches!(region, terminal_psi::ElementViewRegion::WholeCollection)
    {
        base
    } else if geometry.storage == terminal_semantics::CollectionStorage::Borrowed {
        let pointer = transport_register(builder, *destination, 0)?;
        super::runtime_address::footprint(
            builder,
            row,
            source.place,
            field_offset,
            8,
            indices,
            super::runtime_address::Written::None,
        )?;
        builder.emit_load64(field_offset, base, pointer, provenance(row))?;
        pointer
    } else if field_offset == 0 {
        base
    } else {
        let offset = constant(builder, row, 0, u64::from(field_offset))?;
        let backing = transport_register(builder, *destination, 0)?;
        builder.emit_byte_view_address(base, offset, backing, provenance(row))?;
        backing
    };
    let (backing, length) = match region {
        terminal_psi::ElementViewRegion::WholeCollection => {
            if accepted_fact.is_some() {
                return Err(invalid());
            }
            let length = match element_count {
                terminal_semantics::CollectionExtent::Fixed(length) => {
                    constant(builder, row, 8, length)?
                }
                // An owned byte carrier's live-length word precedes its
                // viewed bytes at `field_offset`, so the descriptor extent
                // is the load of the u64 header.
                terminal_semantics::CollectionExtent::LiveLength { .. } => {
                    let header = field_offset.checked_sub(8).ok_or(invalid())?;
                    let pointer = transport_register(builder, *destination, 0)?;
                    super::runtime_address::footprint(
                        builder,
                        row,
                        source.place,
                        header,
                        8,
                        indices,
                        super::runtime_address::Written::None,
                    )?;
                    builder.emit_load64(header, base, pointer, provenance(row))?;
                    pointer
                }
                terminal_semantics::CollectionExtent::StoredLength => {
                    let count_offset = field_offset.checked_add(8).ok_or(invalid())?;
                    let count = transport_register(builder, *destination, 8)?;
                    super::runtime_address::footprint(
                        builder,
                        row,
                        source.place,
                        count_offset,
                        8,
                        indices,
                        super::runtime_address::Written::None,
                    )?;
                    builder.emit_load64(count_offset, base, count, provenance(row))?;
                    count
                }
            };
            (backing, length)
        }
        terminal_psi::ElementViewRegion::RepresentationBytes {
            start,
            end,
            obligation,
        } => {
            let fact = accepted_fact.ok_or_else(|| invalid())?;
            let (_, start_register, _, start_type) =
                builder.resolve(*start).ok_or_else(|| invalid())?;
            let (_, end_register, _, end_type) = builder.resolve(*end).ok_or_else(|| invalid())?;
            let word = semantic_vocabulary::ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    64,
                )
                .map_err(|_| invalid())?,
            );
            if start_type != word || end_type != word || geometry.stride == 0 {
                return Err(invalid());
            }
            let region_provenance = selected_instructions::SelectedInstructionProvenance {
                operations: vec![row.operation],
                values: vec![*start, *end],
                obligations: vec![*obligation],
                ..Default::default()
            };
            let selected_backing = transport_register(builder, *destination, 0)?;
            builder.emit_byte_view_address(
                backing,
                start_register,
                selected_backing,
                region_provenance.clone(),
            )?;
            let bytes = transport_register(builder, *destination, 8)?;
            builder.emit(
                SelectedInstructionKind::ExactSubtractI64 {
                    obligation: *obligation,
                    accepted_fact: fact,
                },
                builder.constraints.keys.subtract_i64,
                &[end_register, start_register, bytes],
                region_provenance.clone(),
            )?;
            let stride = constant(builder, row, 8, u64::from(geometry.stride))?;
            let length = transport_register(builder, *destination, 8)?;
            let mut operands = vec![bytes, stride, length];
            if builder.environment.target().architecture == target::Architecture::X86_64 {
                operands.push(super::super::integer_arithmetic::division_scratch(builder)?);
            }
            builder.emit(
                SelectedInstructionKind::ExactDivideU64 {
                    obligation: *obligation,
                    accepted_fact: fact,
                },
                builder.constraints.keys.divide_u64,
                &operands,
                region_provenance.clone(),
            )?;
            (selected_backing, length)
        }
    };
    store(builder, row, slot, 0, backing)?;
    if descriptor == terminal_semantics::CollectionViewDescriptor::StoredLength {
        store(builder, row, slot, 8, length)?;
    }
    let descriptor = address(builder, row, slot, 0, descriptor_size, true)?;
    builder.transport.pointers.push((*destination, descriptor));
    Ok(())
}

fn constant(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    offset: u32,
    value: u64,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let LegalizedScalarInstructionKind::EstablishElementView { destination, .. } = &row.kind else {
        return Err(invalid());
    };
    let register = transport_register(builder, *destination, offset)?;
    builder.emit_materialize_i64(
        IntegerValue::Unsigned(u128::from(value)),
        register,
        provenance(row),
    )?;
    Ok(register)
}

/// Present a local aggregate home as the owned root of an element loan. The
/// declaration only supplies window geometry and access; the home's own
/// multiplicity and qualifications still bound what it may lend.
fn local_home_root(
    home: &terminal_psi::StructuralOperationResult,
) -> terminal_psi::StructuralParameterDeclaration {
    terminal_psi::StructuralParameterDeclaration {
        place: home.place,
        position: 0,
        structural_type: home.structural_type,
        multiplicity: home.multiplicity,
        access: StructuralAccess::Owned,
        qualifications: home.qualifications.clone(),
        projected_qualifications: home.projected_qualifications.clone(),
    }
}
