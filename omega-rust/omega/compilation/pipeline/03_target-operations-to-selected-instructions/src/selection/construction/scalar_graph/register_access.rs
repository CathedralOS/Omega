//! Select one register event without dropping its observed result or replaying
//! its input. The ISA chooses the actual instruction and admitted register bank.

use super::{Builder, SelectedInstructionKind, SelectedInstructionProvenance};
use crate::SelectedInstructionError;
use legalized_operations::{LegalizedScalarInstruction, LegalizedScalarInstructionKind};
use semantic_vocabulary::{IntegerSign, ScalarType};

pub(super) fn emit(
    function: usize,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<bool, SelectedInstructionError> {
    let LegalizedScalarInstructionKind::RegisterAccess { event, .. } = row.kind else {
        return Ok(false);
    };
    let is_u64 = |scalar| {
        matches!(scalar, ScalarType::Integer(integer)
        if !integer.is_address() && integer.bits() == 64 && integer.sign() == IntegerSign::Unsigned)
    };
    let operation = event.operation();
    let key = if operation.produces_value() {
        builder.constraints.keys.system_register_read
    } else {
        builder.constraints.keys.system_register_write
    }
    .ok_or_else(|| SelectedInstructionError::unsupported_shape(function))?;
    let (register, output) = match (event.inputs(), row.result) {
        ([], Some(result)) if is_u64(result.scalar_type) => (
            builder.register(result.value, result.definition_site, result.scalar_type)?,
            Some(result),
        ),
        ([value], None) => {
            let (_, register, _, scalar) = builder
                .resolve(*value)
                .ok_or_else(SelectedInstructionError::custody)?;
            if !is_u64(scalar) {
                return Err(SelectedInstructionError::custody());
            }
            (register, None)
        }
        _ => return Err(SelectedInstructionError::custody()),
    };
    builder.emit(
        SelectedInstructionKind::RegisterAccess { operation },
        key,
        &[register],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: event
                .inputs()
                .iter()
                .copied()
                .chain(output.map(|result| result.value))
                .collect(),
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    if let Some(result) = output {
        builder.definitions.push((
            result.value,
            register,
            result.definition_site,
            result.scalar_type,
        ));
    }
    Ok(true)
}
