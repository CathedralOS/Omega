use selected_instructions::SelectedNormalizedForeignCall;

pub fn table_address(record: &SelectedNormalizedForeignCall) -> Option<(u8, u32)> {
    use calling_conventions::{MachineRegister, ValueLocation};
    let (byte_offset, dispatch_only) = record.call.binding.table_dispatch()?;
    if !record.call.callbacks.is_empty() {
        return None;
    }
    let plan = &record.call.binding.boundary_entry_plan.call;
    let scratch = |location: &ValueLocation| {
        location.direct_register() == Some(MachineRegister::Aarch64X(17))
            || dispatch_only && location.direct_register() == Some(MachineRegister::Aarch64X(16))
    };
    if plan
        .parameters
        .iter()
        .flat_map(|parameter| &parameter.locations)
        .any(scratch)
        || plan
            .result
            .iter()
            .flat_map(|result| &result.locations)
            .any(|location| matches!(location, ValueLocation::Indirect { .. }) && scratch(location))
    {
        return None;
    }
    let operands = record.call.binding.operand_placements();
    let parameter = operands.get(0)?;
    let [
        ValueLocation::Register {
            register: MachineRegister::Aarch64X(register),
            byte_size: 8,
            value_byte_offset: 0,
        },
    ] = parameter.locations.as_slice()
    else {
        return None;
    };
    (*register <= 30 && *register != 17).then_some((*register, byte_offset))
}
