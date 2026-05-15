use crate::expression::lower_expression_from_table;
use crate::program::Lowerer;
use crate::state::lower_state;
use crate::type_reference::lower_type_reference;
use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees as resolved;
use omega_typed_trees as typed;

pub(crate) fn lower_machine(
    lowerer: &mut Lowerer,
    machine: &resolved::machine::Machine,
) -> Result<typed::machine::Machine, Diagnostic> {
    let mut typed_machine = typed::machine::Machine {
        symbol: machine.symbol,
        name: crate::name::lower_name(&machine.name),
        contains: omega_core::arena::HandleSpan::empty(),
        owned_data: omega_core::arena::HandleSpan::empty(),
        states: omega_core::arena::HandleSpan::empty(),
    };

    for contained_object in lowerer
        .source_trees
        .machine_contained_objects(machine.contains)
    {
        let contained_object = typed::machine::ContainedObject {
            symbol: contained_object.symbol,
            type_symbol: contained_object.type_symbol,
            name: crate::name::lower_name(&contained_object.name),
            type_name: crate::name::lower_name(&contained_object.type_name),
        };
        lowerer
            .typed_trees
            .push_machine_contained_object(&mut typed_machine, contained_object);
    }

    for owned_data in lowerer.source_trees.machine_owned_data(machine.owned_data) {
        let owned_data = typed::machine::OwnedData {
            symbol: owned_data.symbol,
            name: crate::name::lower_name(&owned_data.name),
            type_reference: lower_type_reference(lowerer, &owned_data.type_reference)?,
            initial_value: match owned_data.initial_value {
                Some(initial_value) => Some(lower_expression_from_table(
                    &lowerer.source_trees.tables.bodies.expressions,
                    initial_value,
                )?),
                None => None,
            },
        };
        lowerer
            .typed_trees
            .push_machine_owned_data(&mut typed_machine, owned_data);
    }

    for state in lowerer.source_trees.machine_state_handles(machine.states) {
        let state = lowerer.source_trees.machine_state(*state);
        let state = lower_state(lowerer, state)?;
        lowerer
            .typed_trees
            .push_machine_state(&mut typed_machine, state);
    }

    Ok(typed_machine)
}
