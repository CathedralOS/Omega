use omega_legalized_operations::LegalizedLeafValue;
use omega_selected_instructions::{SelectedFixedInputConstraint, SelectedSelectionConstraints};
use omega_target_operations::MachineRegister;
use omega_target_operations_to_selected_instructions::ValidatedLegalizedOperations;
use psi_core::MachineId;

use crate::ValidatedTargetRegisterEnvironment;

pub(crate) fn selection_constraints(
    legalized: &ValidatedLegalizedOperations,
    environment: &ValidatedTargetRegisterEnvironment,
) -> SelectedSelectionConstraints {
    let mut fixed_inputs = Vec::new();
    for function in &legalized.plan().functions {
        push_fixed_input(
            &mut fixed_inputs,
            environment,
            function.machine,
            function.condition_source,
            function.condition_parameter_index,
            function.condition_register,
        );
        for arm in [&function.when_true, &function.when_false] {
            let LegalizedLeafValue::EntryParameter {
                parameter_index,
                register,
                ..
            } = &arm.value
            else {
                continue;
            };
            push_fixed_input(
                &mut fixed_inputs,
                environment,
                function.machine,
                arm.source_value,
                *parameter_index,
                *register,
            );
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
    source_value: psi_core::ValueId,
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
