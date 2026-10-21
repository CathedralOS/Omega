//! Whole-value resident transfers use ordinary aggregate homes and loads.
//!
//! The lifecycle settlement, not a numeric pointer or the layout, must admit
//! backing and complete supply. Extraction copies the original owner into an
//! unpublished result, reads its actual base, and decodes current bytes into
//! the value field. It never restores an initialization-time value snapshot.
//! Publishing the result last preserves the ordinary owned-result transport.
//! Initialization must additionally decide rejection before any backing write;
//! it is deliberately not implemented by an unconditional field-store loop.

use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    PlaceId, SelectedInstructionKind, VirtualRegisterId, fragmented_scalar, record,
};
use crate::SelectedInstructionError;
use crate::structural_inputs::structural_reference_input;
use abstract_operations::PlacementLifecycleOperands;
use layout_plans::{IntegerInterpretation, LayoutPlacementReport};
use semantic_vocabulary::{
    AuthoredBitFragment, AuthoredBitFragments, AuthoredFieldStorage, StructuralTypeId,
};
use terminal_psi::{
    StructuralFieldDeclaration, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalPlacementLifecyclePlan,
};

mod initialization;
mod resource_profile;
#[cfg(test)]
mod tests;

fn failure(reason: &'static str) -> SelectedInstructionError {
    SelectedInstructionError::custody_because(reason)
}

fn coordinate(value: u64) -> Result<u32, SelectedInstructionError> {
    value
        .try_into()
        .map_err(|_| failure("resident image exceeds native byte coordinates"))
}

fn offset(base: u32, displacement: u64) -> Result<u32, SelectedInstructionError> {
    base.checked_add(coordinate(displacement)?)
        .ok_or_else(|| failure("resident image byte offset overflows"))
}

pub(super) fn emit(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::PlacementLifecycle {
        boundary,
        operands,
        indices,
        result,
    } = &row.kind
    else {
        return Err(failure("resident emitter requires a lifecycle operation"));
    };
    let signature = source
        .structural
        .as_ref()
        .ok_or_else(|| failure("resident extraction lost its structural signature"))?;
    let declarations = &signature.structural_types;
    let plan = declarations
        .placement_lifecycles()
        .iter()
        .find(|plan| plan.boundary == *boundary && plan.operation == operands.operation())
        .ok_or_else(|| failure("resident extraction lost its exact retained lifecycle"))?;
    let PlacementLifecycleOperands::MoveOut { storage, profile } = operands else {
        return initialization::emit(source, row, builder, plan);
    };
    let terminal_psi::TerminalPlacementLifecycleExecution::CompilerPrimitive {
        profile_type: Some(profile_type),
        ..
    } = plan.execution
    else {
        return Err(failure("extraction lost its admitted profile type"));
    };
    let mut remaining = indices.as_slice();
    let storage_indices = operand_indices(storage, &mut remaining)?;
    let profile_indices = operand_indices(profile, &mut remaining)?;
    if !remaining.is_empty() {
        return Err(failure(
            "extraction retained selectors outside its operands",
        ));
    }
    let result_layout =
        structural_reference_input::case_layout(result.structural_type, declarations)
            .ok_or_else(|| failure("resident extraction has no native result layout"))?;
    let [moved] = result_layout.cases.as_slice() else {
        return Err(failure("resident extraction must have one Moved case"));
    };
    let [owner_field, value_field] = moved.fields.as_slice() else {
        return Err(failure(
            "resident extraction must return its owner and complete value",
        ));
    };
    // The receiving contract authenticates these roles. Rejoin the retained
    // geometry rather than treating an equal-sized unrelated field as an owner.
    let result_declaration = declarations
        .iter()
        .find(|declaration| declaration.id == result.structural_type)
        .ok_or_else(|| failure("resident extraction result declaration is missing"))?;
    let StructuralTypeShape::Sum { cases } = &result_declaration.shape else {
        return Err(failure(
            "resident extraction result is not its authenticated sum",
        ));
    };
    let [case] = cases.as_slice() else {
        return Err(failure("resident extraction result changed cases"));
    };
    let [owner, value] = case.fields.as_slice() else {
        return Err(failure("resident extraction result changed fields"));
    };
    if owner.field_type.as_structural() != Some(plan.storage.carrier)
        || value.field_type.as_structural() != Some(plan.schema)
    {
        return Err(failure(
            "resident extraction result changed its owner or schema",
        ));
    }
    let result_pointer =
        record::initialize_storage(row, builder, result.place, result_layout.shape)?;
    super::leaf_copy::copy_operand(
        source,
        row,
        builder,
        storage,
        storage_indices,
        super::leaf_copy::CopyEnd {
            place: result.place,
            pointer: result_pointer,
            byte_offset: owner_field.byte_offset,
            elements: &[],
        },
        owner_field.shape,
    )?;
    let owner_home = ImageHome {
        storage: fragmented_scalar::StorageAccess::Place(&[]),
        place: result.place,
        pointer: result_pointer,
        byte_offset: owner_field.byte_offset,
    };
    let backing = owner_word(
        declarations,
        row,
        builder,
        plan,
        owner_home,
        plan.storage.base,
    )?;
    let available = owner_word(
        declarations,
        row,
        builder,
        plan,
        owner_home,
        plan.storage.length,
    )?;
    let projected =
        super::leaf_copy::project_operand(source, row, builder, profile, profile_indices)?;
    let profile_home = ImageHome {
        storage: fragmented_scalar::StorageAccess::Place(projected.indices),
        place: profile.place,
        pointer: projected
            .pointer
            .ok_or_else(|| failure("extraction profile has no addressable home"))?,
        byte_offset: projected.byte_offset,
    };
    let scratch = ImageHome::scratch(
        builder,
        row,
        result.place,
        resource_profile::compatibility::COMPATIBILITY_SCRATCH_BYTES,
    )?;
    let assessment = resource_profile::compatibility::assess(
        declarations,
        profile_type,
        builder,
        row,
        profile_home,
        scratch,
        available,
        &plan.placement,
        plan.operation,
    )?;
    use super::super::operation_control;
    operation_control::require_nonzero(builder, row, assessment.well_formed)?;
    // No recoverable failure case is declared for extraction. Missing read
    // supply traps before touching backing or publishing vacancy; Resident
    // alone supplies neither this physical read nor a replacement claim.
    let decision = initialization::decide(
        builder,
        row,
        result.place,
        backing,
        available,
        plan.placement.access().layout_size_bytes(),
        assessment.modulus,
        assessment.residue,
        assessment.compatible,
    )?;
    operation_control::require_nonzero(builder, row, decision.ready)?;
    transfer_image(
        declarations,
        row,
        builder,
        plan,
        ImageHome {
            storage: fragmented_scalar::StorageAccess::Backing,
            place: storage.place,
            pointer: backing,
            byte_offset: 0,
        },
        ImageHome {
            storage: fragmented_scalar::StorageAccess::Place(&[]),
            place: result.place,
            pointer: result_pointer,
            byte_offset: value_field.byte_offset,
        },
        ImageDirection::Decode,
    )?;
    // initialize_storage zeroed the only case's ordinal and all result padding.
    builder
        .transport
        .pointers
        .push((result.place, result_pointer));
    Ok(())
}

fn operand_indices<'indices>(
    argument: &terminal_psi::StructuralArgument,
    remaining: &mut &'indices [legalized_operations::LegalizedRuntimeIndexOperand],
) -> Result<&'indices [legalized_operations::LegalizedRuntimeIndexOperand], SelectedInstructionError>
{
    let count = argument
        .path
        .iter()
        .filter(|segment| matches!(segment, StructuralPathSegment::RuntimeIndex { .. }))
        .count();
    let (selected, rest) = remaining
        .split_at_checked(count)
        .ok_or_else(|| failure("lifecycle operand lost a runtime selector"))?;
    *remaining = rest;
    Ok(selected)
}

fn owner_word(
    types: &[StructuralTypeDeclaration],
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
    plan: &TerminalPlacementLifecyclePlan,
    home: ImageHome,
    identity: semantic_vocabulary::StructuralFieldId,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let owner = types
        .iter()
        .find(|declaration| declaration.id == plan.storage.carrier)
        .ok_or_else(|| failure("lifecycle owner declaration is missing"))?;
    let StructuralTypeShape::Record { fields, .. } = &owner.shape else {
        return Err(failure("lifecycle owner is not its retained record"));
    };
    let field = fields
        .iter()
        .find(|field| field.id == identity)
        .ok_or_else(|| failure("lifecycle lost an authenticated geometry field"))?;
    let region = field_region(types, plan.storage.carrier, field)?;
    if region.shape.byte_size != 8 {
        return Err(failure("lifecycle geometry must occupy a native u64 word"));
    }
    ScalarImageCodec::field(types, plan.storage.carrier, field)?.read(builder, row, home)
}

/// A physical home is geometry, not a provider receipt. The lifecycle caller
/// supplies admitted backing or unpublished scratch and retains its authority.
#[derive(Clone, Copy)]
struct ImageHome<'indices> {
    storage: fragmented_scalar::StorageAccess<'indices>,
    place: PlaceId,
    pointer: VirtualRegisterId,
    byte_offset: u32,
}

impl ImageHome<'_> {
    /// One private activation allocation contains the operation's scratch.
    /// It is not a source value, argument, or additional copy of its anchor.
    /// The caller partitions counters and staged bytes within this allocation.
    fn scratch(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        anchor: PlaceId,
        byte_size: u32,
    ) -> Result<Self, SelectedInstructionError> {
        use selected_instructions::{
            FrameStorageSlotId, LocalStorageSlotId, SelectedLocalStorageSlot,
            SelectedMemoryAccessRole,
        };
        let slot = LocalStorageSlotId::Boundary {
            operation: row.operation,
        };
        if byte_size == 0
            || builder
                .transport
                .local_slots
                .iter()
                .any(|item| item.id == slot)
        {
            return Err(failure(
                "resident scratch must have one nonempty operation-owned allocation",
            ));
        }
        builder
            .transport
            .local_slots
            .push(SelectedLocalStorageSlot {
                id: slot,
                byte_size,
                alignment: 8,
            });
        let pointer = super::transport_register(builder, anchor, 0)?;
        super::memory(
            builder,
            row,
            anchor,
            0,
            byte_size,
            SelectedMemoryAccessRole::AddressLocal { slot },
        )?;
        builder.emit_frame_address(
            FrameStorageSlotId::Local(slot),
            pointer,
            super::provenance(row),
        )?;
        Ok(Self {
            storage: fragmented_scalar::StorageAccess::Local(slot),
            place: anchor,
            pointer,
            byte_offset: 0,
        })
    }
}

#[derive(Clone, Copy)]
enum ImageDirection {
    Encode,
    Decode,
}

/// Both directions walk the same complete schema/placement join. Encoding
/// targets zeroed disjoint scratch, never provider storage. Decoding targets
/// the unpublished owned result. Neither operation transfers custody itself.
fn transfer_image(
    declarations: &[StructuralTypeDeclaration],
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
    plan: &TerminalPlacementLifecyclePlan,
    image: ImageHome,
    value: ImageHome,
    direction: ImageDirection,
) -> Result<(), SelectedInstructionError> {
    let declaration = declarations
        .iter()
        .find(|declaration| declaration.id == plan.schema)
        .ok_or_else(|| failure("resident image schema is missing"))?;
    let StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
        return Err(failure(
            "resident image requires its retained record schema",
        ));
    };
    let access = plan.placement.access().plan().entries();
    if access.len() != plan.image_fields.len() {
        return Err(failure("resident image lost its complete field-slot join"));
    }
    for (identity, slot) in plan.image_fields.iter().zip(access) {
        let field = fields
            .iter()
            .find(|field| field.id == *identity)
            .ok_or_else(|| failure("resident image field is absent from its schema"))?;
        let placements = plan
            .placement
            .layout()
            .entries
            .iter()
            .filter(|entry| entry.field == slot.field())
            .map(|entry| entry.placement)
            .collect::<Vec<_>>();
        if let Some(scalar) = field.field_type.scalar_type() {
            let width = runtime_abi::ScalarStorageCodec::from_scalar(scalar)
                .ok_or_else(|| failure("resident scalar has no native storage codec"))?
                .byte_size();
            let placed_codec = ScalarImageCodec::placement(u32::from(width), &placements)?;
            let owned_codec = ScalarImageCodec::field(declarations, plan.schema, field)?;
            let (source_home, source_codec, destination_home, destination_codec) = match direction {
                ImageDirection::Encode => (value, &owned_codec, image, &placed_codec),
                ImageDirection::Decode => (image, &placed_codec, value, &owned_codec),
            };
            let bits = source_codec.read(builder, row, source_home)?;
            destination_codec.write(builder, row, destination_home, bits)?;
        } else {
            let region = field_region(declarations, plan.schema, field)?;
            let chunks = aggregate_chunks(
                declarations,
                field,
                u64::from(region.shape.byte_size),
                &placements,
            )?;
            for chunk in chunks {
                let image_offset = offset(image.byte_offset, u64::from(chunk.source_offset))?;
                let value_offset = offset(
                    offset(value.byte_offset, u64::from(region.byte_offset))?,
                    u64::from(chunk.destination_offset),
                )?;
                let (source, source_offset, destination, destination_offset) = match direction {
                    ImageDirection::Encode => (value, value_offset, image, image_offset),
                    ImageDirection::Decode => (image, image_offset, value, value_offset),
                };
                for position in 0..chunk.byte_count {
                    let bits = fragmented_scalar::load_container(
                        builder,
                        row,
                        source.place,
                        source.pointer,
                        offset(source_offset, u64::from(position))?,
                        1,
                        source.storage,
                    )?;
                    fragmented_scalar::store_container(
                        builder,
                        row,
                        destination.place,
                        destination.pointer,
                        offset(destination_offset, u64::from(position))?,
                        1,
                        destination.storage,
                        bits,
                        None,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn field_region(
    declarations: &[StructuralTypeDeclaration],
    schema: StructuralTypeId,
    field: &StructuralFieldDeclaration,
) -> Result<calling_conventions::PackedFieldLayout, SelectedInstructionError> {
    // This is owned native storage, which can contain target-sized addresses
    // and sum payloads. Portable byte-view geometry deliberately excludes those
    // carriers. Use the same projection and field shape as ordinary native
    // record construction; no lifecycle-specific layout or fallback is needed.
    let geometry = structural_reference_input::reference_formation_geometry(
        schema,
        &[StructuralPathSegment::Field(field.identity)],
        declarations,
    )
    .ok_or_else(|| failure("resident field has no native owned storage projection"))?;
    if !geometry.pointer_slots.is_empty()
        || !geometry.elements.is_empty()
        || *geometry.selected != field.field_type
    {
        return Err(failure(
            "resident image field is not its direct owned storage",
        ));
    }
    let shape = structural_reference_input::field_storage_shape(field, declarations)
        .ok_or_else(|| failure("resident field has no native storage shape"))?;
    Ok(calling_conventions::PackedFieldLayout {
        shape,
        byte_offset: geometry.byte_offset,
    })
}

#[derive(Debug, PartialEq, Eq)]
struct ImageChunk {
    source_offset: u32,
    destination_offset: u32,
    byte_count: u32,
}

fn aggregate_chunks(
    declarations: &[StructuralTypeDeclaration],
    field: &StructuralFieldDeclaration,
    byte_count: u64,
    placements: &[LayoutPlacementReport],
) -> Result<Vec<ImageChunk>, SelectedInstructionError> {
    let mut offsets = placements
        .iter()
        .map(|placement| match placement {
            LayoutPlacementReport::At { offset } => coordinate(*offset),
            _ => Err(failure(
                "resident aggregate requires whole or repeated byte placement",
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;
    offsets.sort_unstable();
    let declaration = field
        .field_type
        .as_structural()
        .and_then(|root| {
            declarations
                .iter()
                .find(|declaration| declaration.id == root)
        })
        .ok_or_else(|| failure("resident aggregate lost its structural type"))?;
    if let StructuralTypeShape::FixedArray { element, length } = declaration.shape {
        let element_shape = structural_reference_input::shape(element, declarations)
            .ok_or_else(|| failure("resident array element has no native storage"))?;
        let element_size = u64::from(element_shape.byte_size);
        if (offsets.len() != 1 && u64::try_from(offsets.len()).ok() != Some(length))
            || element_size.checked_mul(length) != Some(byte_count)
        {
            return Err(failure(
                "resident element placements do not cover the complete array",
            ));
        }
        let destination_stride = field
            .storage
            .as_ref()
            .and_then(AuthoredFieldStorage::element_stride)
            .unwrap_or(element_size);
        let mut chunks = Vec::new();
        for element_position in 0..length {
            let source_offset = if offsets.len() == 1 {
                offset(
                    offsets[0],
                    element_position
                        .checked_mul(element_size)
                        .ok_or_else(|| failure("resident array source offset overflows"))?,
                )?
            } else {
                *offsets
                    .get(
                        usize::try_from(element_position)
                            .map_err(|_| failure("resident array exceeds host coordinates"))?,
                    )
                    .ok_or_else(|| failure("resident array lost an element placement"))?
            };
            chunks.push(ImageChunk {
                source_offset,
                destination_offset: coordinate(
                    element_position
                        .checked_mul(destination_stride)
                        .ok_or_else(|| failure("resident array destination offset overflows"))?,
                )?,
                byte_count: coordinate(element_size)?,
            });
        }
        return Ok(chunks);
    }
    let [source_offset] = offsets.as_slice() else {
        return Err(failure(
            "resident non-array aggregate needs one complete image placement",
        ));
    };
    Ok(vec![ImageChunk {
        source_offset: *source_offset,
        destination_offset: 0,
        byte_count: coordinate(byte_count)?,
    }])
}

#[cfg(test)]
fn decode_scalar(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    owner: PlaceId,
    backing: VirtualRegisterId,
    width: u32,
    placements: &[LayoutPlacementReport],
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    ScalarImageCodec::placement(width, placements)?.read(
        builder,
        row,
        ImageHome {
            storage: fragmented_scalar::StorageAccess::Place(&[]),
            place: owner,
            pointer: backing,
            byte_offset: 0,
        },
    )
}

/// Owned fields and placed fields use the same byte/fragment codec. A narrowed
/// integer's signed interpretation belongs to its storage recipe, not to the
/// direction of transfer or the spelling of its source expression.
enum ScalarImageCodec {
    Bytes {
        byte_offset: u32,
        width: u32,
        signed: bool,
    },
    Fragments(AuthoredBitFragments),
}

impl ScalarImageCodec {
    fn field(
        declarations: &[StructuralTypeDeclaration],
        schema: StructuralTypeId,
        field: &StructuralFieldDeclaration,
    ) -> Result<Self, SelectedInstructionError> {
        if let Some(AuthoredFieldStorage::Fragmented(fragments)) = &field.storage {
            return Ok(Self::Fragments(fragments.clone()));
        }
        let region = field_region(declarations, schema, field)?;
        let width = region.shape.byte_size;
        if !(1..=8).contains(&width) {
            return Err(failure(
                "resident owned scalar exceeds its register carrier",
            ));
        }
        let signed = matches!(
            field.storage.as_ref().and_then(AuthoredFieldStorage::stored_scalar),
            Some(semantic_vocabulary::ScalarType::Integer(integer))
                if integer.sign() == semantic_vocabulary::IntegerSign::Signed
        );
        Ok(Self::Bytes {
            byte_offset: region.byte_offset,
            width,
            signed,
        })
    }

    fn placement(
        width: u32,
        placements: &[LayoutPlacementReport],
    ) -> Result<Self, SelectedInstructionError> {
        if !(1..=8).contains(&width) {
            return Err(failure(
                "resident scalar exceeds the native register carrier",
            ));
        }
        match placements {
            [LayoutPlacementReport::At { offset }] => Ok(Self::Bytes {
                byte_offset: coordinate(*offset)?,
                width,
                signed: false,
            }),
            [
                LayoutPlacementReport::IntegerAt {
                    offset,
                    stored_width,
                    interpretation,
                },
            ] => {
                if *stored_width == 0
                    || *stored_width > u64::from(width) * 8
                    || !stored_width.is_multiple_of(8)
                {
                    return Err(failure("resident integer has an invalid stored width"));
                }
                Ok(Self::Bytes {
                    byte_offset: coordinate(*offset)?,
                    width: coordinate(*stored_width / 8)?,
                    signed: *interpretation == IntegerInterpretation::Signed,
                })
            }
            fragments if !fragments.is_empty() => {
                let fragments = fragments
                    .iter()
                    .map(|fragment| {
                        let LayoutPlacementReport::Bits {
                            container,
                            container_width,
                            destination_lsb,
                            source_lsb,
                            width: bit_count,
                        } = *fragment
                        else {
                            return Err(failure(
                                "resident scalar mixes incompatible placement families",
                            ));
                        };
                        if container_width == 0
                            || container_width > 64
                            || !container_width.is_multiple_of(8)
                            || destination_lsb
                                .checked_add(bit_count)
                                .is_none_or(|end| end > container_width)
                            || source_lsb
                                .checked_add(bit_count)
                                .is_none_or(|end| end > u64::from(width) * 8)
                        {
                            return Err(failure(
                                "resident scalar fragment exceeds its container or carrier",
                            ));
                        }
                        Ok(AuthoredBitFragment {
                            container_byte_offset: container,
                            container_width_bits: container_width,
                            destination_lsb,
                            source_lsb,
                            width: bit_count,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Self::Fragments(AuthoredBitFragments::new(fragments)))
            }
            _ => Err(failure("resident scalar has no image placement")),
        }
    }

    fn read(
        &self,
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        home: ImageHome,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        match self {
            Self::Fragments(fragments) => {
                let (first, _) = fragments
                    .byte_envelope()
                    .ok_or_else(|| failure("resident scalar fragments have no envelope"))?;
                fragmented_scalar::read_register(
                    builder,
                    row,
                    home.place,
                    home.pointer,
                    offset(home.byte_offset, first)?,
                    fragments,
                    home.storage,
                )
            }
            Self::Bytes {
                byte_offset,
                width,
                signed,
            } => {
                let value = fragmented_scalar::load_container(
                    builder,
                    row,
                    home.place,
                    home.pointer,
                    offset(home.byte_offset, u64::from(*byte_offset))?,
                    *width,
                    home.storage,
                )?;
                if !signed || *width == 8 {
                    return Ok(value);
                }
                // (bits xor sign_bit) - sign_bit extends two's-complement bits
                // without depending on a target's arithmetic-shift convention.
                let sign = fragmented_scalar::constant(
                    builder,
                    row,
                    home.place,
                    1_u64 << (*width * 8 - 1),
                )?;
                let biased = fragmented_scalar::binary(
                    builder,
                    row,
                    home.place,
                    SelectedInstructionKind::BitwiseXorI64,
                    value,
                    sign,
                )?;
                fragmented_scalar::binary(
                    builder,
                    row,
                    home.place,
                    SelectedInstructionKind::WrappingSubtractI64,
                    biased,
                    sign,
                )
            }
        }
    }

    fn write(
        &self,
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        home: ImageHome,
        bits: VirtualRegisterId,
    ) -> Result<(), SelectedInstructionError> {
        match self {
            Self::Bytes {
                byte_offset, width, ..
            } => fragmented_scalar::store_container(
                builder,
                row,
                home.place,
                home.pointer,
                offset(home.byte_offset, u64::from(*byte_offset))?,
                *width,
                home.storage,
                bits,
                None,
            ),
            Self::Fragments(fragments) => {
                let (first, _) = fragments
                    .byte_envelope()
                    .ok_or_else(|| failure("resident scalar fragments have no envelope"))?;
                fragmented_scalar::write_register(
                    builder,
                    row,
                    home.place,
                    home.pointer,
                    offset(home.byte_offset, first)?,
                    fragments,
                    home.storage,
                    bits,
                    None,
                )
            }
        }
    }
}
