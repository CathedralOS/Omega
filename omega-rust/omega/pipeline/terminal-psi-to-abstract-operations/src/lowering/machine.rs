use super::LoweringError;
use abstract_operations::AbstractFunction;
use semantic_vocabulary::OperationId;
use terminal_psi::TerminalMachine;

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
    lower_ordinary_machine(
        machine,
        structural_types,
        dynamic_dispatch,
        &module.closed_conformance_applications,
    )
}
