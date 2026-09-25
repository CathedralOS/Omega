//! The address model for a structural access through runtime-selected
//! elements, shared by leaf copies, primitive reads and stores, and scalar
//! field stores (`grid[i][j]`, `ents[i].pos.x`).
//!
//! The access's static part is the row's `byte_offset`, which the load or
//! store applies itself. Each runtime element joins the place pointer in path
//! order: its selector is sign- or zero-extended to 64 bits, multiplied by
//! the array's element stride, and added to the running address. Every
//! selector is proven inside its array by the obligation its Terminal path
//! segment carries, and legalization joined that certificate to the element,
//! so `index * stride` stays inside the array span and the wrapping multiply
//! is exact for every reachable operand. Nothing here re-derives a bound: the
//! obligation rides on the address instruction's provenance and on the
//! access's footprint rows. Validation mirrors this sequence instruction for
//! instruction (`validation/.../runtime_address.rs`).
use super::{
    Builder, IntegerSign, LegalizedScalarInstruction, ScalarType, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedMemoryAccessRole, VirtualRegisterId, memory,
    transport_register,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::invalid;
use legalized_operations::LegalizedRuntimeIndexOperand;
use semantic_vocabulary::{IntegerValue, PlaceId, ValueId};

/// Join each runtime element's scaled selector to `address`, in path order,
/// and return the register the access addresses with its static offset.
pub(super) fn scale(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    byte_offset: u32,
    mut address: VirtualRegisterId,
    indices: &[LegalizedRuntimeIndexOperand],
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    for index in indices {
        let (_, index_register, _, index_type) = builder
            .resolve(index.operand.value)
            .ok_or_else(|| invalid())?;
        let ScalarType::Integer(integer) = index.operand.scalar_type else {
            return Err(invalid());
        };
        if integer.bits() > 64 || index_type != index.operand.scalar_type {
            return Err(invalid());
        }
        // Narrow selectors join the 64-bit address model after the same
        // sign- or zero-normalization scalar transport applies elsewhere.
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
                    _ => return Err(invalid()),
                };
                let extended = transport_register(builder, place, byte_offset)?;
                builder.emit(
                    kind,
                    builder.constraints.keys.copy_i64,
                    &[index_register, extended],
                    SelectedInstructionProvenance {
                        operations: vec![row.operation],
                        values: vec![index.operand.value],
                        ..Default::default()
                    },
                )?;
                extended
            }
        };
        let stride = transport_register(builder, place, byte_offset)?;
        builder.emit(
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(u128::from(index.stride)),
            },
            builder.constraints.keys.materialize_i64,
            &[stride],
            SelectedInstructionProvenance {
                operations: vec![row.operation],
                ..Default::default()
            },
        )?;
        let scaled = transport_register(builder, place, byte_offset)?;
        builder.emit(
            SelectedInstructionKind::WrappingMultiplyI64,
            builder.constraints.keys.multiply_i64,
            &[index_register, stride, scaled],
            SelectedInstructionProvenance {
                operations: vec![row.operation],
                values: vec![index.operand.value],
                ..Default::default()
            },
        )?;
        let joined = transport_register(builder, place, byte_offset)?;
        builder.emit(
            SelectedInstructionKind::ByteViewAddress,
            builder.constraints.keys.add_i64,
            &[address, scaled, joined],
            SelectedInstructionProvenance {
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

/// Record the footprint of the next access instruction: the exact place
/// extent for a static projection, or one row per runtime element, in path
/// order, whose certificate proves that element's selector inside its array.
/// A write names the stored value on each element row.
pub(super) fn footprint(
    builder: &mut Builder<'_>,
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
        return memory(builder, row, place, byte_offset, byte_count, role);
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
        memory(builder, row, place, byte_offset, byte_count, role)?;
    }
    Ok(())
}
