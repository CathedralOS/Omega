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
use crate::legalized_operations::LegalizedRuntimeIndexOperand;
use crate::selection::validation::scalar_graph::Replay;
use semantic_vocabulary::{IntegerValue, PlaceId, ValueId};

pub(super) fn scale(
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

pub(super) fn footprint(
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    byte_offset: u32,
    byte_count: u32,
    indices: &[LegalizedRuntimeIndexOperand],
    written: Option<ValueId>,
) -> Result<(), SelectedInstructionError> {
    if indices.is_empty() {
        let role = if written.is_some() {
            SelectedMemoryAccessRole::WritePlace
        } else {
            SelectedMemoryAccessRole::ReadPlace
        };
        return memory(replay, row, place, byte_offset, byte_count, role);
    }
    for index in indices {
        let role = match written {
            Some(value) => SelectedMemoryAccessRole::WriteIndexedPrimitive {
                index: index.operand.value,
                value,
                stride: index.stride,
                extent: index.extent,
                obligation: index.obligation,
                accepted_fact: index.accepted_fact,
            },
            None => SelectedMemoryAccessRole::ReadIndexedPrimitive {
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
