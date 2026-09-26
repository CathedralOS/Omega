use crate::ValidatedLegalizedOperations;
use selected_instructions::{SelectedFixedInputConstraint, SelectedSelectionConstraints};
use semantic_vocabulary::MachineId;
use target_operations::MachineRegister;

use register_environment::ValidatedTargetRegisterEnvironment;

pub fn selection_constraints(
    legalized: &ValidatedLegalizedOperations,
    environment: &ValidatedTargetRegisterEnvironment,
) -> SelectedSelectionConstraints {
    let mut fixed_inputs = Vec::new();
    for function in &legalized.plan().scalar_functions {
        let required = crate::selection::value_transport::required_values(function);
        for (index, parameter) in function.parameters.iter().enumerate() {
            if !required.contains(&parameter.value) {
                continue;
            }
            if let [
                calling_conventions::ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size: _,
                },
            ] = parameter.placement.locations.as_slice()
            {
                push_fixed_input(
                    &mut fixed_inputs,
                    environment,
                    function.machine,
                    parameter.value,
                    index,
                    *register,
                );
            }
        }
    }
    SelectedSelectionConstraints {
        keys: environment.selected_keys(),
        fixed_inputs,
    }
}

fn push_fixed_input(
    inputs: &mut Vec<SelectedFixedInputConstraint>,
    environment: &ValidatedTargetRegisterEnvironment,
    machine: MachineId,
    source_value: semantic_vocabulary::ValueId,
    parameter_index: usize,
    register: MachineRegister,
) {
    if inputs.iter().any(|input| {
        input.machine == machine
            && input.source_value == source_value
            && input.parameter_index == parameter_index
            && input.register == register
    }) {
        return;
    }
    let Some(fixed_view) = environment.fixed_register_view(register) else {
        return;
    };
    inputs.push(SelectedFixedInputConstraint {
        machine,
        source_value,
        parameter_index,
        register,
        fixed_view,
    });
}
