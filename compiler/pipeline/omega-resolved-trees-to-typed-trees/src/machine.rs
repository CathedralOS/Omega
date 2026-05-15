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
    let mut typed_machine = typed::machine::Machine {
        symbol: machine.symbol,
        name: crate::name::lower_name(&machine.name),
        contains: Vec::new(),
        owned_data: Vec::new(),
        states: Vec::new(),
    };

    for contained_object in lowerer
        .source_program
        .machine_contained_objects(machine.contains)
    {
        typed_machine.contains.push(typed::machine::ContainedObject {
            symbol: contained_object.symbol,
            type_symbol: contained_object.type_symbol,
            name: crate::name::lower_name(&contained_object.name),
            type_name: crate::name::lower_name(&contained_object.type_name),
        });
    }

    for owned_data in lowerer
        .source_program
        .machine_owned_data(machine.owned_data)
    {
        typed_machine.owned_data.push(typed::machine::OwnedData {
            symbol: owned_data.symbol,
            name: crate::name::lower_name(&owned_data.name),
            type_reference: lower_type_reference(lowerer, &owned_data.type_reference)?,
            initial_value: owned_data
                .initial_value
                .as_ref()
                .map(lower_expression)
                .transpose()?,
        });
    }

    for state in lowerer
        .source_program
        .machine_state_handles(machine.states)
    {
        let state = lowerer.source_program.machine_state(*state);
        typed_machine.states.push(lower_state(lowerer, state)?);
    }

    Ok(typed_machine)
}
