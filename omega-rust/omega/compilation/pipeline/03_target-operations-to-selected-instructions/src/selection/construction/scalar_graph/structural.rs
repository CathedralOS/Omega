//! Place-backed ABI transport in the ordinary instruction stream.
use super::{
    Builder, IntegerSign, LegalizedScalarFunction, LegalizedScalarInstructionKind, ScalarType,
    SelectedBlockId, SelectedInstructionId, SelectedInstructionKind, SelectedInstructionProvenance,
    ValueLocation, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use crate::SelectedInstructionError;
use crate::selection::byte_view_homes::ByteViewHomes;
use crate::selection::element_view_homes::ElementViewHomes;
use calling_conventions::IndirectPointerLocation;
use legalized_operations::LegalizedScalarInstruction;
use optimization_core::AcceptedObligationFactIdentity;
use selected_instructions::{
    SelectedBoundarySettlement, SelectedCallContract, SelectedMemoryAccess,
    SelectedMemoryAccessRole, SelectedOutgoingArgumentSlot,
};
use semantic_vocabulary::{IntegerType, IntegerValue, ObligationId, OperationId, PlaceId, ValueId};
use terminal_psi::StructuralAccess;

mod block_views;
mod entry;
pub(super) use block_views::block_entry;
pub(super) use entry::entry;
mod byte_concatenation;
mod byte_field_read;
mod byte_field_store;
mod byte_view_field_store;
mod byte_views;
mod descriptor_operands;
mod leaf_copy;
mod literals;
mod local_storage;
pub(super) use local_storage::{byte_field_argument, fixed_array_argument};
mod case_membership;
mod element_establishment;
mod element_subslice;
mod element_view_write;
mod element_views;
mod primitive_locals;
pub(super) use case_membership::observe;
pub(super) mod indirect_results;
mod record;
pub(super) mod reference;
pub(super) use local_storage::descriptor as stage_descriptor;
mod ieee_float_classify;
mod scalar_array;
mod scalar_case;
mod structural_array;
mod structural_local;
mod zeroed;
pub(super) use primitive_locals::read;
mod encoded_scalar;
mod fragmented_scalar;
pub(super) mod runtime_address;
mod scalar_store;
mod subslice;

pub(super) use byte_field_read::read_byte as byte_field_read;
pub(super) use byte_views::{byte_equal, byte_observation, byte_predicate};
pub(super) use element_views::element_observation;

#[derive(Default)]
pub(super) struct Transport {
    pub(super) result_pointer: Option<VirtualRegisterId>,
    pub pointers: Vec<(PlaceId, VirtualRegisterId)>,
    pub(super) views: Vec<ByteViewHomes>,
    pub(super) element_views: Vec<ElementViewHomes>,
    pub fragments: Vec<(PlaceId, u32, VirtualRegisterId)>,
    pub slots: Vec<SelectedOutgoingArgumentSlot>,
    pub local_slots: Vec<selected_instructions::SelectedLocalStorageSlot>,
    pub calls: Vec<SelectedCallContract>,
    pub normalized_foreign_calls: Vec<selected_instructions::SelectedNormalizedForeignCall>,
    pub memory: Vec<SelectedMemoryAccess>,
    pub settlements: Vec<SelectedBoundarySettlement>,
}

impl Transport {
    /// The `Structural` local storage row a result's operation/place pair
    /// always builds; callers keep the id when they address the slot again.
    pub(super) fn push_structural_local_slot(
        &mut self,
        operation: OperationId,
        place: PlaceId,
        byte_size: u32,
        alignment: u16,
    ) -> selected_instructions::LocalStorageSlotId {
        let id = selected_instructions::LocalStorageSlotId::Structural { operation, place };
        self.local_slots
            .push(selected_instructions::SelectedLocalStorageSlot {
                id,
                byte_size,
                alignment,
            });
        id
    }

    /// The byte view arriving at `place`, when one was registered.
    pub(super) fn view(&self, place: PlaceId) -> Option<&ByteViewHomes> {
        self.views.iter().find(|view| view.place == place)
    }

    /// The element view arriving at `place`, when one was registered.
    pub(super) fn element_view(&self, place: PlaceId) -> Option<&ElementViewHomes> {
        self.element_views.iter().find(|view| view.place == place)
    }
}

#[track_caller]
fn invalid() -> SelectedInstructionError {
    SelectedInstructionError::custody()
}

pub(super) fn transport_register(
    builder: &mut Builder<'_>,
    place: PlaceId,
    byte_offset: u32,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let id = VirtualRegisterId(builder.registers.len().try_into().map_err(|_| invalid())?);
    let instruction = SelectedInstructionId(
        builder
            .instructions
            .len()
            .try_into()
            .map_err(|_| invalid())?,
    );
    builder.registers.push(VirtualRegister {
        id,
        scalar_type: ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?,
        ),
        class: builder.class,
        origin: VirtualRegisterOrigin::AbiTransport {
            instruction,
            place,
            byte_offset,
        },
        definition_site: None,
        entry_fixed_view: None,
    });
    Ok(id)
}

/// The byte offset of element `index` inside `place`'s view home:
/// i * K <= L * K <= R * K <= root bytes, so the element-to-byte scaling of a
/// checked in-bounds index cannot overflow. When the view carries a byte
/// offset O, only O + i * K <= O + L * K <= R * K, so only that byte-offset
/// sum is claimed exact.
pub(super) fn scaled_element_index(
    builder: &mut Builder<'_>,
    place: PlaceId,
    stride: u32,
    index_register: VirtualRegisterId,
    index: ValueId,
    length: ValueId,
    obligation: ObligationId,
    accepted_fact: AcceptedObligationFactIdentity,
    operation: OperationId,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let view = builder.transport.element_view(place).copied();
    let stride_register = transport_register(builder, place, 0)?;
    let scaled_index = transport_register(builder, place, 0)?;
    builder.emit_materialize_i64(
        IntegerValue::Unsigned(u128::from(stride)),
        stride_register,
        SelectedInstructionProvenance {
            operations: vec![operation],
            values: vec![index, length],
            obligations: vec![obligation],
            ..Default::default()
        },
    )?;
    builder.emit(
        SelectedInstructionKind::WrappingMultiplyI64,
        builder.constraints.keys.multiply_i64,
        &[index_register, stride_register, scaled_index],
        SelectedInstructionProvenance {
            operations: vec![operation],
            values: vec![index, length],
            obligations: vec![obligation],
            ..Default::default()
        },
    )?;
    let physical_index = if let Some(view) = view {
        let offset = transport_register(builder, place, 0)?;
        builder.emit(
            SelectedInstructionKind::ExactAddI64 {
                obligation,
                accepted_fact,
            },
            builder.constraints.keys.add_i64,
            &[view.byte_offset, scaled_index, offset],
            SelectedInstructionProvenance {
                operations: vec![operation],
                values: vec![index, length, view.root_length],
                obligations: vec![obligation],
                ..Default::default()
            },
        )?;
        offset
    } else {
        scaled_index
    };
    Ok(physical_index)
}

/// Resolve the durable root pointer for `place` and offset it into a fresh
/// transport register. This is shared by ordinary borrowed call arguments and
/// normalized foreign structural arguments; each call family validates its
/// own custody before reaching this transport.
pub(super) fn place_pointer(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    byte_offset: u32,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    place_pointer_at_site(
        builder,
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            ..Default::default()
        },
        place,
        byte_offset,
    )
}

pub(super) fn place_pointer_at_site(
    builder: &mut Builder<'_>,
    provenance: SelectedInstructionProvenance,
    place: PlaceId,
    byte_offset: u32,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let incoming = builder
        .transport
        .pointers
        .iter()
        .find(|(source, _)| *source == place)
        .map(|(_, pointer)| *pointer);
    let input = if let Some(pointer) = incoming {
        pointer
    } else {
        // Borrow the completed aggregate's original home, not ABI fragments.
        let mut homes = builder
            .transport
            .local_slots
            .iter()
            .filter(|home| home.id.structural_place() == Some(place));
        let home = homes.next().ok_or_else(|| invalid())?.clone();
        if homes.next().is_some() {
            return Err(invalid());
        }
        let pointer = transport_register(builder, place, 0)?;
        let origin = match (
            provenance.operations.as_slice(),
            provenance.edges.as_slice(),
        ) {
            ([operation], []) => {
                selected_instructions::SelectedMemoryAccessOrigin::Operation(*operation)
            }
            ([], [edge]) => selected_instructions::SelectedMemoryAccessOrigin::Edge(*edge),
            _ => return Err(invalid()),
        };
        builder.transport.memory.push(SelectedMemoryAccess {
            instruction: selected_instructions::SelectedInstructionId(
                builder
                    .instructions
                    .len()
                    .try_into()
                    .map_err(|_| invalid())?,
            ),
            origin,
            place,
            byte_offset: 0,
            byte_count: home.byte_size,
            role: SelectedMemoryAccessRole::AddressLocal { slot: home.id },
        });
        builder.emit_local_frame_address(home.id, pointer, provenance.clone())?;
        pointer
    };
    let output = transport_register(builder, place, byte_offset)?;
    transport_at_byte_offset(builder, input, output, byte_offset, provenance)
}

/// An element loan rides on the view's backing pointer, never the
/// descriptor's own address: `element_backing_pointer` yields the stored base
/// (or the derived home's live pointer) and the projected offset scales onto
/// it exactly as a parameter-rooted subloan does.
pub(super) fn view_element_pointer(
    source: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    byte_offset: u32,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    // The same backing an element read or store through this view resolves:
    // an established or received element view, or a byte view.
    let input = runtime_address::projection_base(source, builder, row, place, byte_offset)?
        .ok_or_else(|| invalid())?;
    let output = transport_register(builder, place, byte_offset)?;
    transport_at_byte_offset(
        builder,
        input,
        output,
        byte_offset,
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            ..Default::default()
        },
    )
}

pub(super) fn operation(
    function: usize,
    source: &LegalizedScalarFunction,
    block: SelectedBlockId,
    block_start: usize,
    row: &LegalizedScalarInstruction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    builder: &mut Builder<'_>,
) -> Result<bool, SelectedInstructionError> {
    if matches!(
        row.kind,
        LegalizedScalarInstructionKind::IeeeFloatClassify { .. }
    ) {
        ieee_float_classify::establish(source, builder, row)?;
        return Ok(true);
    }
    if matches!(
        row.kind,
        LegalizedScalarInstructionKind::EstablishByteSequenceConcatenation { .. }
    ) {
        byte_concatenation::establish(source, builder, row)?;
        return Ok(true);
    }
    if matches!(
        row.kind,
        LegalizedScalarInstructionKind::StructuralByteSequenceFieldByteStore { .. }
    ) {
        byte_field_store::replace_byte(source, builder, row)?;
        return Ok(true);
    }
    if matches!(
        row.kind,
        LegalizedScalarInstructionKind::ByteSequenceStore { .. }
            | LegalizedScalarInstructionKind::StructuralByteSequenceFieldStore { .. }
    ) {
        byte_field_store::replace(source, builder, row)?;
        return Ok(true);
    }
    if matches!(
        row.kind,
        LegalizedScalarInstructionKind::StructuralByteViewFieldStore { .. }
    ) {
        byte_view_field_store::replace(source, builder, row)?;
        return Ok(true);
    }
    if matches!(
        row.kind,
        LegalizedScalarInstructionKind::ByteSequenceWrite { .. }
    ) {
        byte_views::write(source, builder, row)?;
        return Ok(true);
    }
    if matches!(
        row.kind,
        LegalizedScalarInstructionKind::ElementViewWrite { .. }
    ) {
        element_view_write::write(source, builder, row)?;
        return Ok(true);
    }
    if matches!(
        row.kind,
        LegalizedScalarInstructionKind::EstablishPrimitiveLocal { .. }
            | LegalizedScalarInstructionKind::PrimitiveLocalStore { .. }
    ) {
        primitive_locals::write(source, row, builder)?;
        return Ok(true);
    }
    if matches!(
        row.kind,
        LegalizedScalarInstructionKind::EstablishRecord { .. }
    ) {
        record::establish(source, row, builder)?;
        return Ok(true);
    }
    if matches!(
        row.kind,
        LegalizedScalarInstructionKind::EstablishCase { .. }
    ) {
        scalar_case::establish(source, row, builder)?;
        return Ok(true);
    }
    if matches!(
        row.kind,
        LegalizedScalarInstructionKind::StructuralLeafCopy { .. }
    ) {
        leaf_copy::copy(source, row, builder)?;
        return Ok(true);
    }
    if matches!(
        row.kind,
        LegalizedScalarInstructionKind::StoreStructuralField { .. }
    ) {
        leaf_copy::reseat(source, row, builder)?;
        return Ok(true);
    }
    if matches!(
        row.kind,
        LegalizedScalarInstructionKind::EstablishStructuralArray { .. }
    ) {
        structural_array::establish(source, row, builder)?;
        return Ok(true);
    }
    if matches!(
        row.kind,
        LegalizedScalarInstructionKind::EstablishScalarArray { .. }
    ) {
        scalar_array::establish(source, row, builder)?;
        return Ok(true);
    }
    if matches!(
        row.kind,
        LegalizedScalarInstructionKind::EstablishZeroedStructural { .. }
    ) {
        zeroed::establish(source, row, builder)?;
        return Ok(true);
    }
    if matches!(
        row.kind,
        LegalizedScalarInstructionKind::EstablishStructuralLocal { .. }
    ) {
        structural_local::establish(source, row, builder)?;
        return Ok(true);
    }
    if matches!(
        row.kind,
        LegalizedScalarInstructionKind::StructuralScalarFieldStore { .. }
            | LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore { .. }
    ) {
        scalar_store::emit(source, row, builder)?;
        return Ok(true);
    }
    if matches!(
        row.kind,
        LegalizedScalarInstructionKind::EstablishByteSequenceLiteral { .. }
    ) {
        literals::establish(builder, row)?;
        return Ok(true);
    }
    if matches!(
        row.kind,
        LegalizedScalarInstructionKind::ByteSequenceSubslice { .. }
    ) {
        subslice::create(source, builder, row)?;
        return Ok(true);
    }
    if matches!(
        row.kind,
        LegalizedScalarInstructionKind::EstablishElementView { .. }
    ) {
        element_establishment::establish(source, builder, row)?;
        return Ok(true);
    }
    if matches!(
        row.kind,
        LegalizedScalarInstructionKind::ElementViewSubslice { .. }
    ) {
        element_subslice::create(source, builder, row)?;
        return Ok(true);
    }
    if matches!(
        row.kind,
        LegalizedScalarInstructionKind::EstablishReference { .. }
            | LegalizedScalarInstructionKind::EstablishPlacedReference { .. }
    ) {
        reference::establish(source, row, builder)?;
        return Ok(true);
    }
    // Closing a loan changes custody, not referent bytes. Trivial empty locals
    // likewise carry no executable initialization.
    if matches!(
        row.kind,
        LegalizedScalarInstructionKind::ReleaseReference { .. }
            | LegalizedScalarInstructionKind::EstablishTrivialAffineLocal { .. }
    ) {
        if row.result.is_some() {
            return Err(invalid());
        }
        return Ok(true);
    }
    if let LegalizedScalarInstructionKind::BoundarySettlement(settlement) = &row.kind {
        if row.result.is_some()
            || row.operation != settlement.operation
            || row.effect != settlement.effect
            || row.ownership != settlement.ownership
            || row.fuel != settlement.fuel
        {
            return Err(invalid());
        }
        let instruction_index = builder
            .instructions
            .len()
            .checked_sub(block_start)
            .ok_or_else(|| invalid())?;
        builder
            .transport
            .settlements
            .push(SelectedBoundarySettlement {
                block,
                instruction_index: instruction_index.try_into().map_err(|_| invalid())?,
                settlement:
                    selected_instructions::SelectedBoundarySettlementPayload::ClaimCompletion(
                        settlement.clone(),
                    ),
            });
        return Ok(true);
    }
    // Evaluated normalized foreign calls own their own argument transport and
    // result custody; they are never routed through the internal Call family.
    if matches!(
        row.kind,
        LegalizedScalarInstructionKind::NormalizedForeignCall(_)
    ) {
        super::normalized_foreign::emit(function, source, row, environment, builder)?;
        return Ok(true);
    }
    if row.result.is_some() {
        return Ok(false);
    }
    if !matches!(row.kind, LegalizedScalarInstructionKind::Call(_)) {
        return Err(invalid());
    }
    // A forwarded owner is an ordinary argument. Its complete value geometry
    // and outgoing ABI belong to the shared marshaller, independently of the
    // caller's signature; a private fixed-width forward copy loses live bytes.
    super::unit_call::emit(function, source, row, environment, builder)?;
    Ok(true)
}

pub(super) fn provenance(row: &LegalizedScalarInstruction) -> SelectedInstructionProvenance {
    SelectedInstructionProvenance {
        operations: vec![row.operation],
        fuel: Vec::new(),
        ..Default::default()
    }
}
/// Keep native displacement limits separate from semantic storage bounds.
/// Address staging has no memory effect: callers record the original place
/// offset on the following access, even when its encoded displacement is zero.
fn memory_pointer(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    pointer: VirtualRegisterId,
    byte_offset: u32,
    byte_size: u32,
) -> Result<(VirtualRegisterId, u32), SelectedInstructionError> {
    displaced_memory_pointer(
        builder,
        place,
        pointer,
        byte_offset,
        byte_size,
        provenance(row),
    )
}

/// Prepare an encodable address without changing the semantic memory offset.
/// Terminator observations have no operation provenance; operation observations
/// retain their own provenance through the same address-staging mechanism.
pub(super) fn displaced_memory_pointer(
    builder: &mut Builder<'_>,
    place: PlaceId,
    pointer: VirtualRegisterId,
    byte_offset: u32,
    byte_size: u32,
    provenance: SelectedInstructionProvenance,
) -> Result<(VirtualRegisterId, u32), SelectedInstructionError> {
    if builder
        .environment
        .admits_pointer_displacement(byte_offset, byte_size)
    {
        return Ok((pointer, byte_offset));
    }
    let address = transport_register(builder, place, byte_offset)?;
    builder.emit_address_offset(byte_offset, pointer, address, provenance)?;
    Ok((address, 0))
}

fn memory(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    byte_offset: u32,
    byte_count: u32,
    role: SelectedMemoryAccessRole,
) -> Result<(), SelectedInstructionError> {
    memory_at_origin(
        builder,
        selected_instructions::SelectedMemoryAccessOrigin::Operation(row.operation),
        place,
        byte_offset,
        byte_count,
        role,
    )
}

fn memory_at_origin(
    builder: &mut Builder<'_>,
    origin: selected_instructions::SelectedMemoryAccessOrigin,
    place: PlaceId,
    byte_offset: u32,
    byte_count: u32,
    role: SelectedMemoryAccessRole,
) -> Result<(), SelectedInstructionError> {
    builder.transport.memory.push(SelectedMemoryAccess {
        instruction: SelectedInstructionId(
            builder
                .instructions
                .len()
                .try_into()
                .map_err(|_| invalid())?,
        ),
        origin,
        place,
        byte_offset,
        byte_count,
        role,
    });
    Ok(())
}

/// Copy `input` into `output` verbatim at offset zero, otherwise emit the
/// address-offset step that shifts it by `byte_offset`.
pub(super) fn transport_at_byte_offset(
    builder: &mut Builder<'_>,
    input: VirtualRegisterId,
    output: VirtualRegisterId,
    byte_offset: u32,
    provenance: SelectedInstructionProvenance,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    builder.emit(
        if byte_offset == 0 {
            SelectedInstructionKind::CopyI64
        } else {
            SelectedInstructionKind::AddressOffset { byte_offset }
        },
        if byte_offset == 0 {
            builder.constraints.keys.copy_i64
        } else {
            builder
                .constraints
                .keys
                .address_offset
                .ok_or_else(|| invalid())?
        },
        &[input, output],
        provenance,
    )?;
    Ok(output)
}
