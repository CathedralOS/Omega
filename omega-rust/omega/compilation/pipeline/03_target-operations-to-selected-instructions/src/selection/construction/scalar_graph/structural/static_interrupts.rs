//! Execute admitted token helpers without creating an installation or arrival.
//! Token and result storage follow ordinary structural transport. Root accesses
//! use the separately admitted backing, never the token's own byte permission.

use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    PlaceId, SelectedInstructionKind, SelectedMemoryAccessRole, VirtualRegisterId, memory,
    provenance, transport_register,
};
use crate::SelectedInstructionError;
use crate::structural_inputs::structural_reference_input;
use language_core::static_interrupts::StaticInterruptOperation;
use runtime_abi::static_interrupts::{self as abi, acknowledgement as ack, permit, root};

fn failure() -> SelectedInstructionError {
    SelectedInstructionError::custody_because(
        "installed interrupt helper lost its exact token or result",
    )
}

pub(super) fn emit(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::StaticInterrupt {
        operation,
        token,
        indices,
        result,
        ..
    } = &row.kind
    else {
        return Err(failure());
    };
    if builder.environment.target().architecture != target::Architecture::X86_64
        || *operation == StaticInterruptOperation::Install
        || row.result.is_some()
    {
        return Err(failure());
    }
    builder
        .interrupt_helpers
        .require(row.operation, *operation)
        .map_err(SelectedInstructionError::custody_because)?;
    let declarations = &source
        .structural
        .as_ref()
        .ok_or_else(failure)?
        .structural_types;
    let token_root =
        crate::selection::reference_input::root_type(source, token.place).ok_or_else(failure)?;
    let geometry = structural_reference_input::reference_formation_geometry(
        token_root,
        &token.path,
        declarations,
    )
    .ok_or_else(failure)?;
    let token_type = geometry.selected.as_structural().ok_or_else(failure)?;
    let token_shape = structural_reference_input::stored_value_shape(token_type, declarations)
        .ok_or_else(failure)?;
    let completing = *operation == StaticInterruptOperation::FinishCompletion;
    let token_bytes = if completing {
        permit::BYTES
    } else {
        ack::BYTES
    };
    if !geometry.readable
        || token_shape.byte_size != token_bytes
        || u32::from(token_shape.alignment) != abi::ALIGNMENT
    {
        return Err(failure());
    }
    let (token_pointer, token_offset) =
        super::primitive_locals::borrowed_storage_address(source, builder, row, token, indices)?;
    let token_word = |builder: &mut Builder<'_>, offset| {
        load(
            builder,
            row,
            token.place,
            token_pointer,
            offset,
            token_offset,
            SelectedMemoryAccessRole::ReadPlace,
        )
    };
    let installed = token_word(builder, 0)?;
    super::super::operation_control::require_nonzero(builder, row, installed)?;
    let root_word = |builder: &mut Builder<'_>, offset| {
        load(
            builder,
            row,
            token.place,
            installed,
            offset,
            0,
            SelectedMemoryAccessRole::ReadBacking,
        )
    };
    let active = root_word(builder, root::STATE)?;
    let expected_active =
        super::fragmented_scalar::constant(builder, row, token.place, root::ACTIVE)?;
    require_equal(builder, row, token.place, active, expected_active)?;
    let invocation = token_word(
        builder,
        if completing {
            permit::INVOCATION
        } else {
            ack::INVOCATION
        },
    )?;
    let current_invocation = root_word(builder, root::INVOCATION)?;
    require_equal(builder, row, token.place, invocation, current_invocation)?;
    let debt = token_word(builder, if completing { permit::DEBT } else { ack::DEBT })?;
    super::super::operation_control::require_nonzero(builder, row, debt)?;
    let current_debt = root_word(builder, root::DEBT)?;
    require_equal(builder, row, token.place, debt, current_debt)?;
    if !completing {
        let provider = token_word(builder, ack::PROVIDER)?;
        let current_provider = root_word(builder, root::PROVIDER)?;
        require_equal(builder, row, token.place, provider, current_provider)?;
        let completion = token_word(builder, ack::COMPLETION)?;
        let current_completion = root_word(builder, root::COMPLETION)?;
        require_equal(builder, row, token.place, completion, current_completion)?;
    }
    if completing {
        if result.is_some() {
            return Err(failure());
        }
        let zero = super::fragmented_scalar::constant(builder, row, token.place, 0)?;
        store(
            builder,
            row,
            token.place,
            installed,
            root::DEBT,
            SelectedMemoryAccessRole::WriteBacking,
            zero,
        )?;
        return Ok(());
    }
    let result = result.as_ref().ok_or_else(failure)?;
    let shape =
        structural_reference_input::stored_value_shape(result.structural_type, declarations)
            .ok_or_else(failure)?;
    let expected_bytes = if *operation == StaticInterruptOperation::Context {
        abi::WORD_BYTES
    } else {
        permit::BYTES
    };
    if shape.byte_size != expected_bytes || u32::from(shape.alignment) != abi::ALIGNMENT {
        return Err(failure());
    }
    let pointer = super::record::initialize_storage(row, builder, result.place, shape)?;
    if *operation == StaticInterruptOperation::Context {
        if !declarations.iter().any(|declaration| {
            declaration.id == result.structural_type
                && matches!(
                    declaration.shape,
                    terminal_psi::StructuralTypeShape::Reference {
                        access: terminal_psi::StructuralAccess::SharedBorrow,
                        ..
                    }
                )
        }) {
            return Err(failure());
        }
        let context = root_word(builder, root::CONTEXT)?;
        super::super::operation_control::require_nonzero(builder, row, context)?;
        store(
            builder,
            row,
            result.place,
            pointer,
            0,
            SelectedMemoryAccessRole::WritePlace,
            context,
        )?;
    } else {
        for (offset, value) in [
            (permit::ROOT, installed),
            (permit::INVOCATION, invocation),
            (permit::DEBT, debt),
        ] {
            store(
                builder,
                row,
                result.place,
                pointer,
                offset,
                SelectedMemoryAccessRole::WritePlace,
                value,
            )?;
        }
    }
    builder.transport.pointers.push((result.place, pointer));
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn load(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    pointer: VirtualRegisterId,
    offset: u32,
    footprint_offset: u32,
    role: SelectedMemoryAccessRole,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let value = transport_register(builder, place, offset)?;
    memory(
        builder,
        row,
        place,
        footprint_offset.checked_add(offset).ok_or_else(failure)?,
        abi::WORD_BYTES,
        role,
    )?;
    builder.emit(
        SelectedInstructionKind::Load64 {
            byte_offset: offset,
        },
        builder.constraints.keys.load64.ok_or_else(failure)?,
        &[pointer, value],
        provenance(row),
    )?;
    Ok(value)
}

#[allow(clippy::too_many_arguments)]
fn store(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    pointer: VirtualRegisterId,
    offset: u32,
    role: SelectedMemoryAccessRole,
    value: VirtualRegisterId,
) -> Result<(), SelectedInstructionError> {
    memory(builder, row, place, offset, abi::WORD_BYTES, role)?;
    builder.emit_store(
        offset,
        abi::WORD_BYTES as u8,
        pointer,
        value,
        provenance(row),
    )
}

fn require_equal(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    left: VirtualRegisterId,
    right: VirtualRegisterId,
) -> Result<(), SelectedInstructionError> {
    let predicate = transport_register(builder, place, 0)?;
    builder.emit(
        SelectedInstructionKind::CompareI64,
        builder.constraints.keys.compare_i64,
        &[left, right],
        provenance(row),
    )?;
    builder.emit(
        SelectedInstructionKind::MaterializeBooleanEqual,
        builder.constraints.keys.materialize_boolean,
        &[predicate],
        provenance(row),
    )?;
    super::super::operation_control::require_nonzero(builder, row, predicate)
}
