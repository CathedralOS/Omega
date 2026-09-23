//! Element read observation shape; control flow owns availability and liveness.

use super::{ModuleError, PlaceId, TerminalMachine, TerminalModule, ValueId};

pub(super) fn validate(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    source: PlaceId,
    length: ValueId,
) -> Result<(), ModuleError> {
    let element =
        super::element_view_length::validate_source(module, machine, operation, source, || {
            ModuleError::InvalidElementViewReadSource {
                operation: operation.id,
                source,
            }
        })?;
    let Some(expected) = super::primitive_storage::scalar_type(module, element) else {
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
    if !super::element_view_length::is_exact_length(machine, source, length) {
        return Err(ModuleError::InvalidElementViewReadLength {
            operation: operation.id,
            source,
            length,
        });
    }
    Ok(())
}
