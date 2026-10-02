//! The address model for a structural access through runtime-selected
//! elements, shared by leaf copies, case observations, primitive reads and
//! stores, and scalar field stores (`grid[i][j]`, `ents[i].pos.x`).
//!
//! Resolve the storage base before adding projection offsets: a view's home
//! holds a descriptor, not its elements. All projected consumers use this
//! distinction; whole-view transport still copies the descriptor itself.
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

/// Resolve the storage observed by a projection, before its static and runtime
/// displacements. View homes describe a loan: load its backing pointer
/// and retain a derived view's offset, never index the descriptor itself.
/// A missing address denotes ABI value fragments, not an empty or invalid loan;
/// callers separately admit the operations that can consume those fragments.
pub(super) fn projection_base(
    source: &super::LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    byte_offset: u32,
) -> Result<Option<VirtualRegisterId>, SelectedInstructionError> {
    let byte_view = crate::selection::established_view_input::view_type(source, place).is_some();
    let view_offset = if byte_view {
        builder
            .transport
            .views
            .iter()
            .find(|view| view.place == place)
            .map(|view| view.byte_offset)
    } else {
        builder
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
        return Ok(builder
            .transport
            .pointers
            .iter()
            .find(|(stored, _)| *stored == place)
            .map(|(_, pointer)| *pointer));
    }
    let backing = if byte_view {
        super::byte_views::backing_pointer(source, builder, row, place)?
    } else {
        super::element_views::element_backing_pointer(builder, row, place)?
    };
    let Some(view_offset) = view_offset else {
        return Ok(Some(backing));
    };
    let base = transport_register(builder, place, byte_offset)?;
    builder.emit(
        SelectedInstructionKind::ByteViewAddress,
        builder.constraints.keys.add_i64,
        &[backing, view_offset, base],
        super::provenance(row),
    )?;
    Ok(Some(base))
}

/// Join each runtime element's scaled selector to `address`, in path order,
/// and return the register the access addresses with its static offset.
pub(in crate::selection) fn scale(
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

/// What a footprint's write stores: a named scalar `value` (a primitive or
/// scalar field store), a structural element byte copy (a borrowed window's
/// reseat — chunks carry no single semantic value), or nothing (a read).
pub(super) enum Written {
    None,
    Scalar(ValueId),
    Structural,
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
    written: Written,
) -> Result<(), SelectedInstructionError> {
    if indices.is_empty() {
        let role = if matches!(written, Written::None) {
            SelectedMemoryAccessRole::ReadPlace
        } else {
            SelectedMemoryAccessRole::WritePlace
        };
        return memory(builder, row, place, byte_offset, byte_count, role);
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
        memory(builder, row, place, byte_offset, byte_count, role)?;
    }
    Ok(())
}
