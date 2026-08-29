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

pub(super) fn lower_machine(
    module: &psi_terminal::TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    structural_types: &[psi_terminal::StructuralTypeDeclaration],
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
                        result.multiplicity == psi_terminal::StructuralMultiplicity::Unrestricted
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
        retain_payloadless_for_optimization,
    )
}
