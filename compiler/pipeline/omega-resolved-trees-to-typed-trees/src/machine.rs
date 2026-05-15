use crate::expression::lower_expression;
use crate::program::Lowerer;
use crate::state::lower_state;
use crate::type_reference::lower_type_reference;
use omega_core::diagnostics::Diagnostic;
use omega_resolved_trees as resolved;
use omega_typed_trees as typed;

pub(crate) fn lower_machine(
    lowerer: &mut Lowerer,
    machine: &resolved::machine::Machine,
) -> Result<typed::machine::Machine, Diagnostic> {
    let contains = machine
        .contains
        .iter()
        .map(|contained_object| typed::machine::ContainedObject {
            symbol: contained_object.symbol,
            type_symbol: contained_object.type_symbol,
            name: crate::name::lower_name(&contained_object.name),
            type_name: crate::name::lower_name(&contained_object.type_name),
        })
        .collect();

    let owned_data = machine
        .owned_data
        .iter()
        .map(|owned_data| {
            Ok(typed::machine::OwnedData {
                symbol: owned_data.symbol,
                name: crate::name::lower_name(&owned_data.name),
                type_reference: lower_type_reference(lowerer, &owned_data.type_reference)?,
                initial_value: owned_data
                    .initial_value
                    .as_ref()
                    .map(lower_expression)
                    .transpose()?,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;

    let states = lowerer
        .source_program
        .machine_state_handles(machine.states)
        .iter()
        .map(|state| lowerer.source_program.machine_state(*state))
        .map(|state| lower_state(lowerer, state))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(typed::machine::Machine {
        symbol: machine.symbol,
        name: crate::name::lower_name(&machine.name),
        contains,
        owned_data,
        states,
    })
}
