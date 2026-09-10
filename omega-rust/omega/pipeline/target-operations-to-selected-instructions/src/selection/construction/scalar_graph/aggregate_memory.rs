//! Select exact aggregate fragment accesses without widening storage.
//! Packed expansions own an instruction-local temporary, never a source value.
//! The caller retains the exact place/offset/width access record; the physical
//! constraints keep scratch and early-written outputs disjoint from live inputs.
use super::*;

pub(super) fn load(
    builder: &mut Builder<'_>,
    pointer: VirtualRegisterId,
    output: VirtualRegisterId,
    byte_offset: u32,
    byte_size: u16,
) -> Result<(), SelectedInstructionError> {
    let invalid = || SelectedInstructionError::SourceCustodyMismatch;
    let mut operands = vec![pointer, output];
    let (kind, key) = match byte_size {
        1 => (
            SelectedInstructionKind::Load8 { byte_offset },
            builder.constraints.keys.load8,
        ),
        2 => (
            SelectedInstructionKind::Load16 { byte_offset },
            builder.constraints.keys.load16,
        ),
        4 => (
            SelectedInstructionKind::Load32 { byte_offset },
            builder.constraints.keys.load32,
        ),
        8 => (
            SelectedInstructionKind::Load64 { byte_offset },
            builder.constraints.keys.load64,
        ),
        _ => {
            let width = selected_instructions::PackedByteWidth::from_byte_size(
                byte_size.try_into().map_err(|_| invalid())?,
            )
            .ok_or_else(invalid)?;
            operands.push(scratch(builder)?);
            (
                SelectedInstructionKind::LoadPacked { byte_offset, width },
                builder.constraints.keys.load_packed,
            )
        }
    };
    builder.emit(
        kind,
        key.ok_or_else(invalid)?,
        &operands,
        Default::default(),
    )
}

pub(super) fn store(
    builder: &mut Builder<'_>,
    pointer: VirtualRegisterId,
    value: VirtualRegisterId,
    byte_offset: u32,
    byte_size: u16,
) -> Result<(), SelectedInstructionError> {
    let invalid = || SelectedInstructionError::SourceCustodyMismatch;
    let byte_size = u8::try_from(byte_size).map_err(|_| invalid())?;
    let mut operands = vec![pointer, value];
    let (kind, key) =
        if let Some(width) = selected_instructions::PackedByteWidth::from_byte_size(byte_size) {
            operands.push(scratch(builder)?);
            (
                SelectedInstructionKind::StorePacked { byte_offset, width },
                builder.constraints.keys.store_packed,
            )
        } else if matches!(byte_size, 1 | 2 | 4 | 8) {
            (
                SelectedInstructionKind::Store {
                    byte_offset,
                    byte_size,
                },
                builder.constraints.keys.store,
            )
        } else {
            return Err(invalid());
        };
    builder.emit(
        kind,
        key.ok_or_else(invalid)?,
        &operands,
        Default::default(),
    )
}

fn scratch(builder: &mut Builder<'_>) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let invalid = || SelectedInstructionError::SourceCustodyMismatch;
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
            semantic_vocabulary::IntegerType::new(IntegerSign::Unsigned, 64)
                .map_err(|_| invalid())?,
        ),
        class: builder.class,
        origin: VirtualRegisterOrigin::InstructionScratch {
            instruction,
            operand: 2,
        },
        definition_site: None,
        entry_fixed_view: None,
    });
    Ok(id)
}
