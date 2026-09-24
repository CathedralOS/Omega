//! Element read observation shape; control flow owns availability and liveness.

use crate::validation::{ModuleError, PlaceId, TerminalMachine, TerminalModule, ValueId};

/// The read's element path is static and must end at a scalar leaf of the
/// view's element (`terminal_semantics::element_view_leaf_scalar`); the
/// result carries exactly that leaf's scalar type. The element itself is
/// selected by the index against the exact length observation, so the path
/// never bounds anything.
pub(in crate::validation) fn validate(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    source: PlaceId,
    length: ValueId,
    path: &[terminal_psi::StructuralPathSegment],
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
    let Some(expected) =
        terminal_semantics::element_view_leaf_scalar(&module.structural_types, element, path)
    else {
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
