//! Replay mirror of construction `runtime_address`: each runtime element's
//! selector extends, scales by its stride and joins the running address in
//! path order, carrying its obligation on the joining instruction; the access
//! then records one footprint row per element (or the exact place extent for
//! a static projection).
use super::{
    IntegerSign, LegalizedScalarInstruction, ScalarType, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedMemoryAccessRole, VirtualRegisterId, memory,
};
use crate::SelectedInstructionError;
use crate::selection::validation::scalar_graph::Replay;
use legalized_operations::LegalizedRuntimeIndexOperand;
use semantic_vocabulary::{IntegerValue, PlaceId, ValueId};

/// Resolve the storage observed by a projection, before its static and runtime
/// displacements. View homes describe a loan: load its backing pointer
/// and retain a derived view's offset, never index the descriptor itself.
/// A missing address denotes ABI value fragments, not an empty or invalid loan;
/// callers separately admit the operations that can consume those fragments.
pub(super) fn projection_base(
    source: &super::LegalizedScalarFunction,
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    byte_offset: u32,
) -> Result<Option<VirtualRegisterId>, SelectedInstructionError> {
    let byte_view = crate::selection::established_view_input::view_type(source, place).is_some();
    let view_offset = if byte_view {
        replay
            .transport
            .views
            .iter()
            .find(|view| view.place == place)
            .map(|view| view.byte_offset)
    } else {
        replay
            .transport
            .element_views
            .iter()
            .find(|view| view.place == place)
            .map(|view| view.byte_offset)
    };
    if !byte_view
        && view_offset.is_none()
        && crate::selection::established_view_input::element_view_type(source, place).is_none()
    {
        return Ok(replay
            .transport
            .pointers
            .iter()
            .find(|(stored, _)| *stored == place)
            .map(|(_, pointer)| *pointer));
    }
    let backing = if byte_view {
        super::byte_views::backing_pointer(source, replay, row, place)?
    } else {
        super::element_views::element_backing_pointer(replay, row, place)?
    };
    let Some(view_offset) = view_offset else {
        return Ok(Some(backing));
    };
    let base = super::result(replay, place, byte_offset)?;
    replay.check_instruction(
        SelectedInstructionKind::ByteViewAddress,
        replay.constraints.keys.add_i64,
        &[backing, view_offset, base],
        &super::provenance(row),
    )?;
    Ok(Some(base))
}

pub(in crate::selection) fn scale(
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    byte_offset: u32,
    mut address: VirtualRegisterId,
    indices: &[LegalizedRuntimeIndexOperand],
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    for index in indices {
        let (_, index_register, _, index_type) = replay
            .resolve(index.operand.value)
            .ok_or_else(|| replay.invalid())?;
        let ScalarType::Integer(integer) = index.operand.scalar_type else {
            return Err(replay.invalid());
        };
        if integer.bits() > 64 || index_type != index.operand.scalar_type {
            return Err(replay.invalid());
        }
        let index_register = match integer.bits() {
            64 => index_register,
            bits => {
                let kind = match (integer.sign(), bits) {
                    (IntegerSign::Unsigned, 8) => SelectedInstructionKind::ZeroExtendU8,
                    (IntegerSign::Unsigned, 16) => SelectedInstructionKind::ZeroExtendU16,
                    (IntegerSign::Unsigned, 32) => SelectedInstructionKind::ZeroExtendU32,
                    (IntegerSign::Signed, 8) => SelectedInstructionKind::SignExtendI8,
                    (IntegerSign::Signed, 16) => SelectedInstructionKind::SignExtendI16,
                    (IntegerSign::Signed, 32) => SelectedInstructionKind::SignExtendI32,
                    _ => return Err(replay.invalid()),
                };
                let extended = super::result(replay, place, byte_offset)?;
                replay.check_instruction(
                    kind,
                    replay.constraints.keys.copy_i64,
                    &[index_register, extended],
                    &SelectedInstructionProvenance {
                        operations: vec![row.operation],
                        values: vec![index.operand.value],
                        ..Default::default()
                    },
                )?;
                extended
            }
        };
        let stride = super::result(replay, place, byte_offset)?;
        replay.check_instruction(
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(u128::from(index.stride)),
            },
            replay.constraints.keys.materialize_i64,
            &[stride],
            &SelectedInstructionProvenance {
                operations: vec![row.operation],
                ..Default::default()
            },
        )?;
        let scaled = super::result(replay, place, byte_offset)?;
        replay.check_instruction(
            SelectedInstructionKind::WrappingMultiplyI64,
            replay.constraints.keys.multiply_i64,
            &[index_register, stride, scaled],
            &SelectedInstructionProvenance {
                operations: vec![row.operation],
                values: vec![index.operand.value],
                ..Default::default()
            },
        )?;
        let joined = super::result(replay, place, byte_offset)?;
        replay.check_instruction(
            SelectedInstructionKind::ByteViewAddress,
            replay.constraints.keys.add_i64,
            &[address, scaled, joined],
            &SelectedInstructionProvenance {
                operations: vec![row.operation],
                values: vec![index.operand.value],
                obligations: vec![index.obligation],
                ..Default::default()
            },
        )?;
        address = joined;
    }
    Ok(address)
}

/// Replay mirror of construction `Written`; see construction
/// `runtime_address::footprint`.
pub(super) enum Written {
    None,
    Scalar(ValueId),
    Structural,
}

pub(super) fn footprint(
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    byte_offset: u32,
    byte_count: u32,
    indices: &[LegalizedRuntimeIndexOperand],
    written: Written,
) -> Result<(), SelectedInstructionError> {
    if indices.is_empty() {
        let role = if matches!(written, Written::None) {
            SelectedMemoryAccessRole::ReadPlace
        } else {
            SelectedMemoryAccessRole::WritePlace
        };
        return memory(replay, row, place, byte_offset, byte_count, role);
    }
    for index in indices {
        let role = match written {
            Written::Scalar(value) => SelectedMemoryAccessRole::WriteIndexedPrimitive {
                index: index.operand.value,
                value,
                stride: index.stride,
                extent: index.extent,
                obligation: index.obligation,
                accepted_fact: index.accepted_fact,
            },
            Written::Structural => SelectedMemoryAccessRole::WriteIndexedStructural {
                index: index.operand.value,
                stride: index.stride,
                extent: index.extent,
                obligation: index.obligation,
                accepted_fact: index.accepted_fact,
            },
            Written::None => SelectedMemoryAccessRole::ReadIndexedPrimitive {
                index: index.operand.value,
                stride: index.stride,
                extent: index.extent,
                obligation: index.obligation,
                accepted_fact: index.accepted_fact,
            },
        };
        memory(replay, row, place, byte_offset, byte_count, role)?;
    }
    Ok(())
}
