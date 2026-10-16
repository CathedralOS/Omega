//! Select register effects through ordinary scalar input and result definitions.
use super::{Builder, SelectedInstructionKind, SelectedInstructionProvenance};
use crate::SelectedInstructionError;
use legalized_operations::{LegalizedScalarInstruction, LegalizedScalarInstructionKind};
use semantic_vocabulary::{IntegerSign, ScalarType};
use terminal_psi::RegisterOperation;

pub(super) fn emit(
    function: usize,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<bool, SelectedInstructionError> {
    let LegalizedScalarInstructionKind::RegisterAccess { event, .. } = row.kind else {
        return Ok(false);
    };
    let operation = event.operation();
    let keys = &builder.constraints.keys;
    let key = match operation {
        RegisterOperation::SystemRead(_) => keys.system_register_read,
        RegisterOperation::SystemWrite(_) => keys.system_register_write,
        RegisterOperation::PortRead => keys.port_register_read,
        RegisterOperation::PortWrite => keys.port_register_write,
        RegisterOperation::ModelSpecificRead => keys.model_specific_read,
        RegisterOperation::ModelSpecificWrite => keys.model_specific_write,
        RegisterOperation::ProcessorHalt => keys.processor_halt,
    }
    .ok_or_else(|| SelectedInstructionError::unsupported_shape(function))?;
    let matches_carrier = |scalar, bits| {
        matches!(scalar, ScalarType::Integer(integer)
        if !integer.is_address() && integer.bits() == bits && integer.sign() == IntegerSign::Unsigned)
    };
    let mut registers = Vec::new();
    for (value, carrier) in event.inputs().zip(operation.input_carriers()) {
        let (_, register, _, scalar) = builder
            .resolve(*value)
            .ok_or_else(SelectedInstructionError::custody)?;
        if !matches_carrier(scalar, carrier.bits()) {
            return Err(SelectedInstructionError::custody());
        }
        registers.push(register);
    }
    let output = match (operation.result_carrier(), row.result) {
        (None, None) => None,
        (Some(carrier), Some(result)) if matches_carrier(result.scalar_type, carrier.bits()) => {
            let register =
                builder.register(result.value, result.definition_site, result.scalar_type)?;
            registers.push(register);
            Some((result, register))
        }
        _ => return Err(SelectedInstructionError::custody()),
    };
    builder.emit(
        SelectedInstructionKind::RegisterAccess { operation },
        key,
        &registers,
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: event
                .inputs()
                .copied()
                .chain(output.map(|(result, _)| result.value))
                .collect(),
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    if let Some((result, register)) = output {
        builder.definitions.push((
            result.value,
            register,
            result.definition_site,
            result.scalar_type,
        ));
    }
    Ok(true)
}
