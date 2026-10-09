//! One admitted classification call establishes one complete structural home.
//! The packed register is instruction-local ABI transport, never a Psi value.
use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    SelectedInstructionKind, SelectedMemoryAccessRole, invalid, local_storage, memory, provenance,
    transport_register,
};
use crate::SelectedInstructionError;
use selected_instructions::{
    IeeeFloatArithmeticForm, LocalStorageSlotId, MachineSemanticKind, SelectedLocalStorageSlot,
};
use target_operations::IeeeFloatArithmeticOperation;

pub(super) fn establish(
    source: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::IeeeFloatClassify {
        format,
        argument,
        result,
        layout,
        ..
    } = &row.kind
    else {
        return Err(invalid());
    };
    let signature = source.structural.as_ref().ok_or_else(invalid)?;
    let declaration = signature
        .structural_types
        .iter()
        .find(|declaration| declaration.id == result.structural_type)
        .ok_or_else(invalid)?;
    if !result.is_unrestricted_plain()
        || !result.qualification_establishments.is_empty()
        || target_operations::conventional_float_class_layout(declaration).as_ref() != Some(layout)
    {
        return Err(invalid());
    }
    let (_, input, _, scalar) = builder.resolve(*argument).ok_or_else(invalid)?;
    if scalar != semantic_vocabulary::ScalarType::IeeeFloat(*format) {
        return Err(invalid());
    }
    let packed = transport_register(builder, result.place, 0)?;
    let form = IeeeFloatArithmeticForm {
        operation: IeeeFloatArithmeticOperation::Classify,
        format: *format,
    };
    let constraint = builder
        .constraints
        .keys
        .for_semantic(MachineSemanticKind::NearestIeeeFloatArithmetic(form))
        .ok_or_else(invalid)?;
    let mut computation = provenance(row);
    computation.values.push(*argument);
    computation.fuel = row.fuel.clone();
    builder.emit(
        SelectedInstructionKind::NearestIeeeFloatArithmetic { form },
        constraint,
        &[input, packed],
        computation,
    )?;
    let slot = LocalStorageSlotId::Structural {
        operation: row.operation,
        place: result.place,
    };
    if builder
        .transport
        .local_slots
        .iter()
        .any(|existing| existing.id == slot)
    {
        return Err(invalid());
    }
    builder
        .transport
        .local_slots
        .push(SelectedLocalStorageSlot {
            id: slot,
            byte_size: layout.shape.byte_size,
            alignment: layout.shape.alignment,
        });
    let pointer = local_storage::address(builder, row, slot, 0, layout.shape.byte_size, false)?;
    memory(
        builder,
        row,
        result.place,
        0,
        layout.shape.byte_size,
        SelectedMemoryAccessRole::WritePlace,
    )?;
    builder.emit(
        SelectedInstructionKind::Store {
            byte_offset: 0,
            byte_size: 8,
        },
        builder.constraints.keys.store.ok_or_else(invalid)?,
        &[pointer, packed],
        provenance(row),
    )?;
    builder.transport.pointers.push((result.place, pointer));
    Ok(())
}
