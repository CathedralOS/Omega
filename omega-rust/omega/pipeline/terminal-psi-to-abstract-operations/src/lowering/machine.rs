use super::LoweringError;
use super::payloadless::exact_unrestricted_payloadless_result;
use super::structural::lower_structural_machine;
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
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
    retain_payloadless_for_optimization: bool,
) -> Result<AbstractFunction, LoweringError> {
    if !retain_payloadless_for_optimization
        && let Some(operation) = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::EstablishPayloadlessCase { .. }
                ) || matches!(operation.kind, OperationKind::CallStructural { .. })
                    && operation.result.structural().is_some_and(|result| {
                        result.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                    })
            })
    {
        return Err(LoweringError::UnsupportedPayloadlessCase(operation.id));
    }
    if let Some(result) = machine.result.structural()
        && !(retain_payloadless_for_optimization
            && exact_unrestricted_payloadless_result(module, machine, machines))
    {
        return lower_structural_machine(machine, result, structural_types);
    }
    lower_ordinary_machine(
        machine,
        structural_types,
        dynamic_dispatch,
        &module.closed_conformance_applications,
        retain_payloadless_for_optimization,
    )
}
