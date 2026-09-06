use crate::ValidatedLegalizedOperations;
use legalized_operations::{LegalizedCondition, LegalizedLeafValue};
use selected_instructions::{SelectedFixedInputConstraint, SelectedSelectionConstraints};
use semantic_vocabulary::MachineId;
use target_operations::MachineRegister;

use register_environment::ValidatedTargetRegisterEnvironment;

pub fn selection_constraints(
    legalized: &ValidatedLegalizedOperations,
    environment: &ValidatedTargetRegisterEnvironment,
) -> SelectedSelectionConstraints {
    let mut fixed_inputs = Vec::new();
    for function in &legalized.plan().functions {
        let function = match function {
            legalized_operations::LegalizedFunction::Conditional(function) => function,
            legalized_operations::LegalizedFunction::SharedReturnConditional(function) => {
                for (index, parameter) in function.abi.parameters.iter().enumerate() {
                    if let [
                        calling_conventions::ValueLocation::Register {
                            register,
                            value_byte_offset: 0,
                            byte_size: 8,
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
                continue;
            }
            legalized_operations::LegalizedFunction::Leaf(function) => {
                if let LegalizedLeafValue::ExactIntegerSequence(sequence) = &function.leaf.value {
                    for (index, parameter) in function.abi.parameters.iter().enumerate() {
                        if parameter.value != function.leaf.source_value
                            && !sequence.steps.iter().any(|step| {
                                matches!(step, legalized_operations::LegalizedIntegerStep::ExactBinary(binary)
                                    if binary.left == parameter.value || binary.right == parameter.value)
                            })
                        {
                            continue;
                        }
                        if let [
                            calling_conventions::ValueLocation::Register {
                                register,
                                value_byte_offset: 0,
                                byte_size: 8,
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
                if let LegalizedLeafValue::EntryParameter {
                    parameter_index,
                    register,
                    ..
                } = function.leaf.value
                {
                    push_fixed_input(
                        &mut fixed_inputs,
                        environment,
                        function.machine,
                        function.leaf.source_value,
                        parameter_index,
                        register,
                    );
                }
                continue;
            }
        };
        match &function.condition {
            LegalizedCondition::DirectParameter {
                parameter_index,
                register,
                ..
            } => push_fixed_input(
                &mut fixed_inputs,
                environment,
                function.machine,
                function.condition_source,
                *parameter_index,
                *register,
            ),
            LegalizedCondition::IntegerEqualParametersV1 { left, right, .. }
            | LegalizedCondition::IntegerLessThanParametersV1 { left, right, .. }
            | LegalizedCondition::I64LessThanParametersV1 { left, right, .. }
            | LegalizedCondition::I64LessOrEqualParametersV1 { left, right, .. }
            | LegalizedCondition::IntegerLessOrEqualParametersV1 { left, right, .. }
            | LegalizedCondition::IntegerNotEqualParametersV1 { left, right, .. } => {
                for parameter in [left, right] {
                    push_fixed_input(
                        &mut fixed_inputs,
                        environment,
                        function.machine,
                        parameter.source_value,
                        parameter.parameter_index,
                        parameter.register,
                    );
                }
            }
            LegalizedCondition::U64EqualZeroParameterV1 { parameter, .. }
            | LegalizedCondition::U64NotEqualZeroParameterV1 { parameter, .. } => push_fixed_input(
                &mut fixed_inputs,
                environment,
                function.machine,
                parameter.source_value,
                parameter.parameter_index,
                parameter.register,
            ),
        }
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
        projected_structural_call: environment
            .scalar_call_constraint()
            .map(|constraint| constraint.key),
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
