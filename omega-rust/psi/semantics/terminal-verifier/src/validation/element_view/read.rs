//! Element read observation shape; control flow owns availability and liveness.

use crate::validation::{ModuleError, PlaceId, TerminalMachine, TerminalModule, ValueId};

pub(in crate::validation) fn validate(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    source: PlaceId,
    length: ValueId,
) -> Result<(), ModuleError> {
    let element = crate::validation::element_view::length::validate_source(
        module,
        machine,
        operation,
        source,
        || ModuleError::InvalidElementViewReadSource {
            operation: operation.id,
            source,
        },
    )?;
    let Some(expected) = crate::validation::primitive_storage::scalar_type(module, element) else {
        return Err(ModuleError::InvalidElementViewReadSource {
            operation: operation.id,
            source,
        });
    };
    if operation
        .result
        .scalar()
        .is_none_or(|result| result.scalar_type != expected)
    {
        return Err(ModuleError::ElementViewReadRequiresElementResult(
            operation.id,
        ));
    }
    if !crate::validation::element_view::length::is_exact_length(machine, source, length) {
        return Err(ModuleError::InvalidElementViewReadLength {
            operation: operation.id,
            source,
            length,
        });
    }
    Ok(())
}
