//! Independent replay of place-backed snapshots, copy accesses and call operands.
use super::{
    IntegerSign, LegalizedScalarFunction, LegalizedScalarInstructionKind, RegisterViewId,
    ScalarType, SelectedInstructionId, SelectedInstructionKind, SelectedInstructionProvenance,
    VirtualRegisterId, VirtualRegisterOrigin,
};
use crate::SelectedInstructionError;
use crate::selection::byte_view_homes::ByteViewHomes;
use crate::selection::element_view_homes::ElementViewHomes;
use crate::selection::validation::scalar_graph::Replay;
use calling_conventions::{IndirectPointerLocation, ValueLocation};
use legalized_operations::{LegalizedScalarArgument, LegalizedScalarInstruction};
use register_model::RegisterOperandAccess;
use selected_instructions::{
    OutgoingArgumentSlotId, SelectedBoundarySettlement, SelectedCallContract, SelectedMemoryAccess,
    SelectedMemoryAccessRole, SelectedOutgoingArgumentSlot,
};
use semantic_vocabulary::{IntegerType, PlaceId};
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
mod scalar_array;
mod scalar_case;
mod structural_array;
mod structural_local;
mod zeroed;
pub(super) use primitive_locals::read;
pub(super) mod runtime_address;
mod scalar_store;
mod subslice;

pub(super) use byte_field_read::read_byte as byte_field_read;
pub(super) use byte_views::{byte_equal, byte_observation, byte_predicate};
pub(super) use element_views::element_observation;

#[derive(Default)]
pub(super) struct Transport {
    pub(super) pointers: Vec<(PlaceId, VirtualRegisterId)>,
    pub(super) result_pointer: Option<VirtualRegisterId>,
    views: Vec<ByteViewHomes>,
    pub(super) element_views: Vec<ElementViewHomes>,
    pub fragments: Vec<(PlaceId, u32, VirtualRegisterId)>,
    pub slots: Vec<SelectedOutgoingArgumentSlot>,
    pub local_slots: Vec<selected_instructions::SelectedLocalStorageSlot>,
    pub calls: Vec<SelectedCallContract>,
    pub normalized_foreign_calls: Vec<selected_instructions::SelectedNormalizedForeignCall>,
    pub memory: Vec<SelectedMemoryAccess>,
    pub settlements: Vec<SelectedBoundarySettlement>,
}

fn register(
    replay: &mut Replay<'_>,
    origin: VirtualRegisterOrigin,
    fixed: Option<RegisterViewId>,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let proposed = replay
        .selected
        .virtual_registers
        .get(replay.register_cursor)
        .ok_or_else(|| replay.invalid())?;
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| replay.invalid())?;
    if proposed.id.0 as usize != replay.register_cursor
        || proposed.origin != origin
        || proposed.definition_site.is_some()
        || proposed.entry_fixed_view != fixed
        || proposed.class != replay.class
        || proposed.scalar_type != ScalarType::Integer(integer)
    {
        return Err(replay.invalid());
    }
    replay.register_cursor += 1;
    Ok(proposed.id)
}
pub(super) fn result(
    replay: &mut Replay<'_>,
    place: PlaceId,
    byte_offset: u32,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    register(
        replay,
        VirtualRegisterOrigin::AbiTransport {
            instruction: SelectedInstructionId(
                replay
                    .instruction_cursor
                    .try_into()
                    .map_err(|_| replay.invalid())?,
            ),
            place,
            byte_offset,
        },
        None,
    )
}

/// Replay the durable root pointer resolution for `place` and its exact
/// offset transport. Shared by ordinary borrowed call arguments and
/// normalized foreign structural arguments; each call family validates its
/// own custody before reaching this replay.
pub(super) fn place_pointer(
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    byte_offset: u32,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    place_pointer_at_site(
        replay,
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            ..Default::default()
        },
        place,
        byte_offset,
    )
}

pub(super) fn place_pointer_at_site(
    replay: &mut Replay<'_>,
    provenance: SelectedInstructionProvenance,
    place: PlaceId,
    byte_offset: u32,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let incoming = replay
        .transport
        .pointers
        .iter()
        .find(|(source, _)| *source == place)
        .map(|(_, pointer)| *pointer);
    let input = if let Some(pointer) = incoming {
        pointer
    } else {
        let mut homes = replay
            .transport
            .local_slots
            .iter()
            .filter(|home| home.id.structural_place() == Some(place));
        let home = homes.next().ok_or_else(|| replay.invalid())?.clone();
        if homes.next().is_some() {
            return Err(replay.invalid());
        }
        let pointer = result(replay, place, 0)?;
        let origin = match (
            provenance.operations.as_slice(),
            provenance.edges.as_slice(),
        ) {
            ([operation], []) => {
                selected_instructions::SelectedMemoryAccessOrigin::Operation(*operation)
            }
            ([], [edge]) => selected_instructions::SelectedMemoryAccessOrigin::Edge(*edge),
            _ => return Err(replay.invalid()),
        };
        replay.transport.memory.push(SelectedMemoryAccess {
            instruction: selected_instructions::SelectedInstructionId(
                replay
                    .instruction_cursor
                    .try_into()
                    .map_err(|_| replay.invalid())?,
            ),
            origin,
            place,
            byte_offset: 0,
            byte_count: home.byte_size,
            role: SelectedMemoryAccessRole::AddressLocal { slot: home.id },
        });
        replay.check_instruction(
            SelectedInstructionKind::FrameAddress {
                slot: selected_instructions::FrameStorageSlotId::Local(home.id),
                byte_offset: 0,
            },
            replay
                .constraints
                .keys
                .frame_address
                .ok_or_else(|| replay.invalid())?,
            &[pointer],
            &provenance,
        )?;
        pointer
    };
    let output = result(replay, place, byte_offset)?;
    replay.check_instruction(
        if byte_offset == 0 {
            SelectedInstructionKind::CopyI64
        } else {
            SelectedInstructionKind::AddressOffset { byte_offset }
        },
        if byte_offset == 0 {
            replay.constraints.keys.copy_i64
        } else {
            replay
                .constraints
                .keys
                .address_offset
                .ok_or_else(|| replay.invalid())?
        },
        &[input, output],
        &provenance,
    )?;
    Ok(output)
}

/// Replay the element-loan pointer: the view's backing pointer, never the
/// descriptor's own address, projected to the loaned element.
pub(super) fn view_element_pointer(
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    byte_offset: u32,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let input = element_views::element_backing_pointer(replay, row, place)?;
    let output = result(replay, place, byte_offset)?;
    replay.check_instruction(
        if byte_offset == 0 {
            SelectedInstructionKind::CopyI64
        } else {
            SelectedInstructionKind::AddressOffset { byte_offset }
        },
        if byte_offset == 0 {
            replay.constraints.keys.copy_i64
        } else {
            replay
                .constraints
                .keys
                .address_offset
                .ok_or_else(|| replay.invalid())?
        },
        &[input, output],
        &SelectedInstructionProvenance {
            operations: vec![row.operation],
            ..Default::default()
        },
    )?;
    Ok(output)
}

pub(super) fn operation(
    source: &LegalizedScalarFunction,
    node: &LegalizedScalarInstruction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    replay: &mut Replay<'_>,
) -> Result<bool, SelectedInstructionError> {
    if matches!(
        node.kind,
        LegalizedScalarInstructionKind::EstablishByteSequenceConcatenation { .. }
    ) {
        byte_concatenation::establish(source, replay, node)?;
        return Ok(true);
    }
    if matches!(
        node.kind,
        LegalizedScalarInstructionKind::StructuralByteSequenceFieldByteStore { .. }
    ) {
        byte_field_store::replace_byte(source, replay, node)?;
        return Ok(true);
    }
    if matches!(
        node.kind,
        LegalizedScalarInstructionKind::ByteSequenceStore { .. }
            | LegalizedScalarInstructionKind::StructuralByteSequenceFieldStore { .. }
    ) {
        byte_field_store::replace(source, replay, node)?;
        return Ok(true);
    }
    if matches!(
        node.kind,
        LegalizedScalarInstructionKind::StructuralByteViewFieldStore { .. }
    ) {
        byte_view_field_store::replace(source, replay, node)?;
        return Ok(true);
    }
    if matches!(
        node.kind,
        LegalizedScalarInstructionKind::ByteSequenceWrite { .. }
    ) {
        byte_views::write(source, replay, node)?;
        return Ok(true);
    }
    if matches!(
        node.kind,
        LegalizedScalarInstructionKind::ElementViewWrite { .. }
    ) {
        element_view_write::write(source, replay, node)?;
        return Ok(true);
    }
    if matches!(
        node.kind,
        LegalizedScalarInstructionKind::EstablishPrimitiveLocal { .. }
            | LegalizedScalarInstructionKind::PrimitiveLocalStore { .. }
    ) {
        primitive_locals::write(source, node, replay)?;
        return Ok(true);
    }
    if matches!(
        node.kind,
        LegalizedScalarInstructionKind::EstablishRecord { .. }
    ) {
        record::establish(source, node, replay)?;
        return Ok(true);
    }
    if matches!(
        node.kind,
        LegalizedScalarInstructionKind::EstablishCase { .. }
    ) {
        scalar_case::establish(source, node, replay)?;
        return Ok(true);
    }
    if matches!(
        node.kind,
        LegalizedScalarInstructionKind::StructuralLeafCopy { .. }
    ) {
        leaf_copy::copy(source, node, replay)?;
        return Ok(true);
    }
    if matches!(
        node.kind,
        LegalizedScalarInstructionKind::StoreStructuralField { .. }
    ) {
        leaf_copy::reseat(source, node, replay)?;
        return Ok(true);
    }
    if matches!(
        node.kind,
        LegalizedScalarInstructionKind::EstablishStructuralArray { .. }
    ) {
        structural_array::establish(source, node, replay)?;
        return Ok(true);
    }
    if matches!(
        node.kind,
        LegalizedScalarInstructionKind::EstablishScalarArray { .. }
    ) {
        scalar_array::establish(source, node, replay)?;
        return Ok(true);
    }
    if matches!(
        node.kind,
        LegalizedScalarInstructionKind::EstablishZeroedStructural { .. }
    ) {
        zeroed::establish(source, node, replay)?;
        return Ok(true);
    }
    if matches!(
        node.kind,
        LegalizedScalarInstructionKind::EstablishStructuralLocal { .. }
    ) {
        structural_local::establish(source, node, replay)?;
        return Ok(true);
    }
    if matches!(
        node.kind,
        LegalizedScalarInstructionKind::StructuralScalarFieldStore { .. }
            | LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore { .. }
    ) {
        scalar_store::validate(source, node, replay)?;
        return Ok(true);
    }
    if matches!(
        node.kind,
        LegalizedScalarInstructionKind::EstablishByteSequenceLiteral { .. }
    ) {
        literals::establish(replay, node)?;
        return Ok(true);
    }
    if matches!(
        node.kind,
        LegalizedScalarInstructionKind::ByteSequenceSubslice { .. }
    ) {
        subslice::create(source, replay, node)?;
        return Ok(true);
    }
    if matches!(
        node.kind,
        LegalizedScalarInstructionKind::EstablishElementView { .. }
    ) {
        element_establishment::establish(source, replay, node)?;
        return Ok(true);
    }
    if matches!(
        node.kind,
        LegalizedScalarInstructionKind::ElementViewSubslice { .. }
    ) {
        element_subslice::create(source, replay, node)?;
        return Ok(true);
    }
    if matches!(
        node.kind,
        LegalizedScalarInstructionKind::EstablishReference { .. }
    ) {
        reference::establish(source, node, replay)?;
        return Ok(true);
    }
    // Release changes only the independently replayed loan ledger.
    if matches!(
        node.kind,
        LegalizedScalarInstructionKind::ReleaseReference { .. }
    ) {
        if node.result.is_some() {
            return Err(replay.invalid());
        }
        return Ok(true);
    }
    // An empty-record local has no initialization payload: its place
    // must be the trivial local of the signature's own empty record.
    if let LegalizedScalarInstructionKind::EstablishTrivialAffineLocal {
        place,
        structural_type,
    } = &node.kind
    {
        let valid = source.structural.as_ref().is_some_and(|signature| {
            signature.structural_places.contains(place)
                && signature.structural_types.contains(structural_type)
        }) && matches!(
            &structural_type.shape,
            terminal_psi::StructuralTypeShape::Record { fields } if fields.is_empty()
        );
        if node.result.is_some() || !valid {
            return Err(replay.invalid());
        }
        return Ok(true);
    }
    if let LegalizedScalarInstructionKind::BoundarySettlement(settlement) = &node.kind {
        if node.result.is_some()
            || node.operation != settlement.operation
            || node.fuel != settlement.fuel
            || node.effect != settlement.effect
            || node.ownership != settlement.ownership
        {
            return Err(replay.invalid());
        }
        replay
            .transport
            .settlements
            .push(SelectedBoundarySettlement {
                block: replay.block.id,
                instruction_index: replay
                    .block_cursor
                    .try_into()
                    .map_err(|_| replay.invalid())?,
                settlement:
                    selected_instructions::SelectedBoundarySettlementPayload::ClaimCompletion(
                        settlement.clone(),
                    ),
            });
        return Ok(true);
    }
    // Evaluated normalized foreign calls replay their own argument transport
    // and result custody; they never take the internal Call path.
    if matches!(
        node.kind,
        LegalizedScalarInstructionKind::NormalizedForeignCall(_)
    ) {
        super::normalized_foreign::validate(source, node, environment, replay)?;
        return Ok(true);
    }
    if node.result.is_some() {
        return Ok(false);
    }
    let LegalizedScalarInstructionKind::Call(call) = &node.kind else {
        return Err(replay.invalid());
    };
    // Mirror the construction routing: an owned actual that re-presents the
    // caller's complete signature replays the slot-addressed forward lane.
    let signature_equal_forward = source.structural.as_ref().is_some_and(|signature| {
        call.result_placement.is_none()
            && call.arguments.len() == signature.parameters.len()
            && !call.arguments.is_empty()
            && call.call_plan == source.call_plan
            && call.arguments.iter().all(|argument| {
                matches!(argument, LegalizedScalarArgument::Structural { semantic, target, .. }
                if semantic.access == StructuralAccess::Owned
                    && semantic.path.is_empty()
                    && signature.parameters.iter().any(|parameter|
                        parameter.semantic.place == semantic.place)
                    && matches!(target.destination.locations.as_slice(),
                        [ValueLocation::Indirect {
                            pointer: IndirectPointerLocation::Register(_),
                            copy_stack_byte_offset: Some(_),
                            ..
                        }]))
            })
    });
    if !signature_equal_forward
        && call.arguments.iter().all(|argument| !matches!(argument, LegalizedScalarArgument::Structural { semantic, target, .. }
        if semantic.access == StructuralAccess::Owned
            && crate::selection::scalar_call_abi::owned_value_shape(source, target.structural_type).is_none())) {
        super::unit_call::validate(source, node, environment, replay)?;
        return Ok(true);
    }
    let signature = source.structural.as_ref().ok_or_else(|| replay.invalid())?;
    call.validate_shape().map_err(|_| replay.invalid())?;
    call.validate_source(&node.ownership)
        .map_err(|_| replay.invalid())?;
    if call.result_placement.is_some()
        || call.call_plan != source.call_plan
        || call.arguments.len() != signature.parameters.len()
    {
        return Err(replay.invalid());
    }
    let mut pointers = Vec::new();
    for (argument_index, argument) in call.arguments.iter().enumerate() {
        let LegalizedScalarArgument::Structural {
            semantic, target, ..
        } = argument
        else {
            return Err(replay.invalid());
        };
        let parameter = signature
            .parameters
            .iter()
            .find(|parameter| parameter.semantic.place == semantic.place)
            .ok_or_else(|| replay.invalid())?;
        if semantic.access != StructuralAccess::Owned
            || !semantic.path.is_empty()
            || target.place != semantic.place
            || target.source
                != target_operations::TargetStructuralArgumentSource::Placement(
                    parameter.target.placement.clone(),
                )
            || target.destination != call.call_plan.parameters[argument_index]
            || target.source_byte_offset != 0
        {
            return Err(replay.invalid());
        }
        let [
            ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(pointer),
                copy_stack_byte_offset: Some(offset),
                byte_size,
                alignment,
            },
        ] = target.destination.locations.as_slice()
        else {
            return Err(replay.invalid());
        };
        let slot = OutgoingArgumentSlotId {
            role: selected_instructions::OutgoingArgumentSlotRole::Argument,
            operation: node.operation,
            argument_index: argument_index.try_into().map_err(|_| replay.invalid())?,
        };
        replay.transport.slots.push(SelectedOutgoingArgumentSlot {
            id: slot,
            byte_size: (*byte_size),
            alignment: *alignment,
            abi_stack_byte_offset: *offset,
        });
        let input = replay
            .transport
            .pointers
            .iter()
            .find(|(place, _)| *place == semantic.place)
            .map(|(_, register)| *register)
            .ok_or_else(|| replay.invalid())?;
        for byte_offset in [0, 8] {
            let value = result(replay, semantic.place, byte_offset)?;
            memory(
                replay,
                node,
                semantic.place,
                byte_offset,
                8,
                SelectedMemoryAccessRole::ReadPlace,
            )?;
            replay.check_instruction(
                SelectedInstructionKind::Load64 { byte_offset },
                replay
                    .constraints
                    .keys
                    .load64
                    .ok_or_else(|| replay.invalid())?,
                &[input, value],
                &provenance(node),
            )?;
            memory(
                replay,
                node,
                semantic.place,
                byte_offset,
                8,
                SelectedMemoryAccessRole::WriteOutgoing { slot },
            )?;
            replay.check_instruction(
                SelectedInstructionKind::Store64 {
                    slot: selected_instructions::FrameStorageSlotId::Outgoing(slot),
                    byte_offset,
                },
                replay
                    .constraints
                    .keys
                    .store64
                    .ok_or_else(|| replay.invalid())?,
                &[value],
                &provenance(node),
            )?;
        }
        pointers.push((semantic.place, *byte_size, *pointer, slot));
    }
    // Construction materializes each outgoing-slot address after the argument
    // copies; replay follows the same instruction order.
    let pointers = pointers
        .into_iter()
        .map(|(place, byte_size, pointer, slot)| {
            let address = result(replay, place, 0)?;
            memory(
                replay,
                node,
                place,
                0,
                byte_size,
                SelectedMemoryAccessRole::AddressOutgoing { slot },
            )?;
            replay.check_instruction(
                SelectedInstructionKind::FrameAddress {
                    slot: selected_instructions::FrameStorageSlotId::Outgoing(slot),
                    byte_offset: 0,
                },
                replay
                    .constraints
                    .keys
                    .frame_address
                    .ok_or_else(|| replay.invalid())?,
                &[address],
                &provenance(node),
            )?;
            Ok((
                address,
                environment
                    .fixed_register_view(pointer)
                    .ok_or_else(|| replay.invalid())?,
            ))
        })
        .collect::<Result<Vec<_>, SelectedInstructionError>>()?;
    let key = replay
        .constraints
        .keys
        .call_unit
        .get(call.arguments.len())
        .copied()
        .ok_or_else(|| replay.invalid())?;
    let constraint = environment
        .constraint(key)
        .ok_or_else(|| replay.invalid())?;
    if constraint.operands.len() != pointers.len()
        || constraint
            .operands
            .iter()
            .zip(&pointers)
            .any(|(operand, (_, view))| {
                operand.access != RegisterOperandAccess::Use || operand.fixed_view != Some(*view)
            })
    {
        return Err(replay.invalid());
    }
    replay.transport.calls.push(SelectedCallContract {
        instruction: SelectedInstructionId(
            replay
                .instruction_cursor
                .try_into()
                .map_err(|_| replay.invalid())?,
        ),
        owner: target_operations::CallSiteOwner::Operation(node.operation),
        call: call.clone(),
        effect: node.effect,
        ownership: node.ownership.clone(),
    });
    let provenance = SelectedInstructionProvenance {
        operations: vec![node.operation],
        fuel: node.fuel.clone(),
        obligations: call.requirement_obligations.clone(),
        ..Default::default()
    };
    replay.check_instruction(
        SelectedInstructionKind::CallUnit {
            callee: call.callee,
        },
        key,
        &pointers
            .iter()
            .map(|(register, _)| *register)
            .collect::<Vec<_>>(),
        &provenance,
    )?;
    Ok(true)
}

fn provenance(row: &LegalizedScalarInstruction) -> SelectedInstructionProvenance {
    SelectedInstructionProvenance {
        operations: vec![row.operation],
        ..Default::default()
    }
}
/// Keep native displacement limits separate from semantic storage bounds.
/// Address staging has no memory effect: callers record the original place
/// offset on the following access, even when its encoded displacement is zero.
fn memory_pointer(
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    pointer: VirtualRegisterId,
    byte_offset: u32,
    byte_size: u32,
) -> Result<(VirtualRegisterId, u32), SelectedInstructionError> {
    if replay
        .environment
        .admits_pointer_displacement(byte_offset, byte_size)
    {
        return Ok((pointer, byte_offset));
    }
    let address = result(replay, place, byte_offset)?;
    replay.check_instruction(
        SelectedInstructionKind::AddressOffset { byte_offset },
        replay
            .constraints
            .keys
            .address_offset
            .ok_or_else(SelectedInstructionError::custody)?,
        &[pointer, address],
        &provenance(row),
    )?;
    Ok((address, 0))
}

fn memory(
    replay: &mut Replay<'_>,
    node: &LegalizedScalarInstruction,
    place: PlaceId,
    byte_offset: u32,
    byte_count: u32,
    role: SelectedMemoryAccessRole,
) -> Result<(), SelectedInstructionError> {
    replay.transport.memory.push(SelectedMemoryAccess {
        instruction: SelectedInstructionId(
            replay
                .instruction_cursor
                .try_into()
                .map_err(|_| replay.invalid())?,
        ),
        origin: selected_instructions::SelectedMemoryAccessOrigin::Operation(node.operation),
        place,
        byte_offset,
        byte_count,
        role,
    });
    Ok(())
}
