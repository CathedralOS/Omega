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
    // Scalar sums use the ordinary graph; their physical result convention
    // remains a checked downstream decision, not a separate machine family.
    if let Some(result) = machine.result.structural()
        && !plain_scalar_sum_result(machine, structural_types)
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
    )
}

fn plain_scalar_sum_result(machine: &TerminalMachine, types: &[terminal_psi::StructuralTypeDeclaration]) -> bool {
    machine.result.structural().is_some_and(|result| {
        matches!(result.multiplicity, terminal_psi::StructuralMultiplicity::Affine | terminal_psi::StructuralMultiplicity::Unrestricted)
            && result.qualifications.is_empty() && result.projected_qualifications.is_empty()
            && types.iter().find(|declaration| declaration.id == result.structural_type)
                .is_some_and(|declaration| matches!(&declaration.shape, terminal_psi::StructuralTypeShape::Sum { cases }
                    if cases.iter().all(|case| case.fields.iter().all(|field|
                        field.relevance == terminal_psi::BindingRelevance::Relevant
                            && field.field_type.scalar_type().is_some()))))
    })
}
