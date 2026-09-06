//! Scalar identity lookup across an authored successor binding.

use selected_instructions::{
    SelectedFunction, SelectedSuccessor, VirtualRegisterId, VirtualRegisterOrigin,
};

use super::LivenessError;

/// One local relation, shared by the independent fixed-point traversals.
/// Non-parameter values pass through unchanged. A live destination parameter
/// must have exactly one typed incoming argument and one selected source value.
pub(crate) fn incoming_argument(
    function_index: usize,
    function: &SelectedFunction,
    successor: &SelectedSuccessor,
    destination: VirtualRegisterId,
) -> Result<VirtualRegisterId, LivenessError> {
    let mismatch = || LivenessError::FunctionMismatch {
        function: function_index,
    };
    let destination_register = function
        .virtual_registers
        .iter()
        .find(|register| register.id == destination)
        .ok_or_else(mismatch)?;
    let VirtualRegisterOrigin::BlockParameter {
        source_value,
        block,
        ..
    } = destination_register.origin
    else {
        return Ok(destination);
    };
    if block != successor.block {
        return Ok(destination);
    }
    let mut bindings = successor
        .bindings
        .iter()
        .filter(|binding| binding.parameter == source_value);
    let binding = bindings.next().ok_or_else(mismatch)?;
    if bindings.next().is_some() || binding.scalar_type != destination_register.scalar_type {
        return Err(mismatch());
    }
    let mut sources = function.virtual_registers.iter().filter(|register| {
        let value = match register.origin {
            VirtualRegisterOrigin::EntryParameter { source_value, .. }
            | VirtualRegisterOrigin::BlockParameter { source_value, .. }
            | VirtualRegisterOrigin::InstructionResult { source_value, .. } => source_value,
            VirtualRegisterOrigin::LegalizationTemporary { .. } => return false,
        };
        value == binding.argument
            && register.scalar_type == binding.scalar_type
            && register.class == destination_register.class
    });
    let source = sources.next().ok_or_else(mismatch)?;
    if sources.next().is_some() {
        return Err(mismatch());
    }
    Ok(source.id)
}
