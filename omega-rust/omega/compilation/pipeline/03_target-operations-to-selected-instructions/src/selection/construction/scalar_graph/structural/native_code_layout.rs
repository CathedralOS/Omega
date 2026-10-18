//! Copy a late relocated image descriptor into ordinary activation-local record
//! storage. No descriptor value is available to semantic evaluation or to the
//! optimizer deciding this program's machine geometry.
use super::local_storage::{address, store};
use super::{
    Builder, LegalizedScalarInstruction, LegalizedScalarInstructionKind, SelectedInstructionKind,
};
use super::{invalid, provenance, transport_register};
use crate::SelectedInstructionError;

pub(super) fn establish(
    source: &legalized_operations::LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::EstablishNativeCodeLayout {
        result, machine, ..
    } = &row.kind
    else {
        return Err(invalid());
    };
    if !crate::selection::native_code_layout_input::valid(source, row) || row.result.is_some() {
        return Err(invalid());
    }
    let slot = builder
        .transport
        .push_structural_local_slot(row.operation, result.place, 24, 8);
    let image = transport_register(builder, result.place, 24)?;
    builder.emit(
        SelectedInstructionKind::ImageAddress {
            reference: selected_instructions::SelectedImageAddressReference::NativeCodeLayout {
                machine: *machine,
            },
        },
        builder
            .constraints
            .keys
            .byte_literal_image_address
            .ok_or_else(invalid)?,
        &[image],
        provenance(row),
    )?;
    for offset in [0, 8, 16] {
        let word = transport_register(builder, result.place, offset)?;
        builder.emit(
            SelectedInstructionKind::Load64 {
                byte_offset: offset,
            },
            builder.constraints.keys.load64.ok_or_else(invalid)?,
            &[image, word],
            provenance(row),
        )?;
        store(builder, row, slot, offset, word)?;
    }
    let descriptor = address(builder, row, slot, 0, 24, true)?;
    builder.transport.pointers.push((result.place, descriptor));
    Ok(())
}
