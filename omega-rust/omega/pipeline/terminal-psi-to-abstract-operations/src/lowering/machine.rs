use super::LoweringError;
use crate::shared::*;

mod operation;
mod ordinary;
mod terminator;

use operation::lower_operation;
use ordinary::lower_ordinary_machine;
use terminator::lower_terminator;

type StructuralLiteral<'a> = (
    &'a terminal_psi::StructuralPlaceDeclaration,
    u32,
    semantic_vocabulary::StructuralTypeId,
);
type LoweredAffineLocal = (
    OperationId,
    terminal_psi::StructuralPlaceDeclaration,
    terminal_psi::StructuralTypeDeclaration,
);

pub(super) fn lower_machine(
    module: &terminal_psi::TerminalModule,
    machine: &TerminalMachine,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
) -> Result<AbstractFunction, LoweringError> {
    // Arrays still lack native storage support. Keep that operation-specific
    // boundary; the result category must not select a different body grammar.
    if let Some(operation) = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::EstablishScalarArray { .. }))
    {
        return Err(LoweringError::UnsupportedScalarArray(operation.id));
    }
    lower_ordinary_machine(
        machine,
        structural_types,
        dynamic_dispatch,
        &module.closed_conformance_applications,
    )
}
