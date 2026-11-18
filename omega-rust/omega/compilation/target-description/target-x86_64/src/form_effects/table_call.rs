use selected_instructions::SelectedNormalizedForeignCall;

pub fn table_address(record: &SelectedNormalizedForeignCall) -> Option<(u8, u32)> {
    let (byte_offset, _) = record.call.binding.table_dispatch()?;
    if !record.call.callbacks.is_empty() || byte_offset > i32::MAX as u32 {
        return None;
    }
    let plan = &record.call.binding.boundary_entry_plan.call;
    let is_scratch = |location: &calling_conventions::ValueLocation| {
        location
            .direct_register()
            .and_then(super::normalized_foreign_call::x86_64_machine_register_encoding)
            == Some(11)
    };
    if plan
        .parameters
        .iter()
        .flat_map(|parameter| &parameter.locations)
        .any(is_scratch)
        || plan
            .result
            .iter()
            .flat_map(|result| &result.locations)
            .any(|location| {
                matches!(
                    location,
                    calling_conventions::ValueLocation::Indirect { .. }
                ) && is_scratch(location)
            })
    {
        return None;
    }
    let operands = record.call.binding.operand_placements();
    let parameter = operands.get(0)?;
    let [
        calling_conventions::ValueLocation::Register {
            register,
            byte_size: 8,
            value_byte_offset: 0,
        },
    ] = parameter.locations.as_slice()
    else {
        return None;
    };
    let register = super::normalized_foreign_call::x86_64_machine_register_encoding(*register)?;
    (register & 7 != 4).then_some((register, byte_offset))
}
