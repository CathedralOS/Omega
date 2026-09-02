use crate::selection::constraints::{fixed_input_constraint, row};
use crate::selection::shared::*;

pub(super) fn validate_virtual_registers(
    function_index: usize,
    source: &SourceFunction,
    function: &SelectedFunction,
    constraints: &SelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let u64_type =
        ScalarType::Integer(psi_core::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"));
    let condition_parameters = match &source.condition {
        LegalizedCondition::DirectParameter {
            parameter_index,
            register,
            definition_site,
        } => vec![(
            source.condition_source,
            *parameter_index,
            *register,
            *definition_site,
            ScalarType::Boolean,
        )],
        LegalizedCondition::IntegerEqualParametersV1 { left, right, .. }
        | LegalizedCondition::IntegerLessThanParametersV1 { left, right, .. }
        | LegalizedCondition::IntegerLessOrEqualParametersV1 { left, right, .. } => [left, right]
            .into_iter()
            .map(|parameter| {
                (
                    parameter.source_value,
                    parameter.parameter_index,
                    parameter.register,
                    parameter.definition_site,
                    u64_type,
                )
            })
            .collect(),
    };
    let mut expected = Vec::new();
    for (source_value, parameter_index, register, definition_site, scalar_type) in
        condition_parameters
    {
        let input = fixed_input_constraint(
            source.machine,
            source_value,
            parameter_index,
            register,
            &constraints.fixed_inputs,
        )
        .ok_or(SelectedInstructionError::MissingInputRegisterView {
            function: function_index,
        })?;
        let Some(input_view) = physical
            .model()
            .views
            .iter()
            .find(|view| view.id == input.fixed_view)
        else {
            return Err(SelectedInstructionError::MissingInputRegisterView {
                function: function_index,
            });
        };
        expected.push((
            scalar_type,
            input_view.class,
            VirtualRegisterOrigin::EntryParameter {
                source_value,
                parameter_index,
            },
            definition_site,
            Some(input.fixed_view),
        ));
    }
    match (&source.when_true.value, &source.when_false.value) {
        (
            SourceLeafValue::ActiveResidentExactAddChain(chain),
            SourceLeafValue::Immediate {
                definition_site: false_site,
                ..
            },
        ) => {
            let binary = row(catalog, constraints.keys.add_i64)?;
            let materialize = row(catalog, constraints.keys.materialize_i64)?;
            if binary.operands.len() != 3
                || materialize.operands.len() != 1
                || binary
                    .operands
                    .iter()
                    .any(|operand| operand.class != binary.operands[2].class)
                || materialize.operands[0].class != binary.operands[2].class
            {
                return Err(SelectedInstructionError::ConstraintOperandMismatch {
                    function: function_index,
                    instruction: 5,
                });
            }
            for (instruction, source_value, definition_site) in [
                (
                    2,
                    chain.resident.source_value,
                    chain.resident.definition_site,
                ),
                (3, chain.left.source_value, chain.left.definition_site),
                (4, chain.right.source_value, chain.right.definition_site),
                (5, chain.inner.source_value, chain.inner.definition_site),
                (6, chain.middle.source_value, chain.middle.definition_site),
                (7, chain.result.source_value, chain.result.definition_site),
                (9, source.when_false.source_value, *false_site),
            ] {
                expected.push((
                    u64_type,
                    binary.operands[2].class,
                    VirtualRegisterOrigin::InstructionResult {
                        instruction: SelectedInstructionId(instruction),
                        source_value,
                    },
                    definition_site,
                    None,
                ));
            }
        }
        (
            SourceLeafValue::ActiveResidentExactAddBridgeChain(chain),
            SourceLeafValue::Immediate {
                definition_site: false_site,
                ..
            },
        ) => {
            let binary = row(catalog, constraints.keys.add_i64)?;
            let materialize = row(catalog, constraints.keys.materialize_i64)?;
            if binary.operands.len() != 3
                || materialize.operands.len() != 1
                || binary
                    .operands
                    .iter()
                    .any(|operand| operand.class != binary.operands[2].class)
                || materialize.operands[0].class != binary.operands[2].class
            {
                return Err(SelectedInstructionError::ConstraintOperandMismatch {
                    function: function_index,
                    instruction: 5,
                });
            }
            for (instruction, source_value, definition_site) in [
                (
                    2,
                    chain.resident.source_value,
                    chain.resident.definition_site,
                ),
                (3, chain.left.source_value, chain.left.definition_site),
                (4, chain.right.source_value, chain.right.definition_site),
                (5, chain.inner.source_value, chain.inner.definition_site),
                (6, chain.middle.source_value, chain.middle.definition_site),
                (7, chain.bridge.source_value, chain.bridge.definition_site),
                (8, chain.result.source_value, chain.result.definition_site),
                (10, source.when_false.source_value, *false_site),
            ] {
                expected.push((
                    u64_type,
                    binary.operands[2].class,
                    VirtualRegisterOrigin::InstructionResult {
                        instruction: SelectedInstructionId(instruction),
                        source_value,
                    },
                    definition_site,
                    None,
                ));
            }
        }
        (
            SourceLeafValue::ActiveResidentExactAddOriginalVictimChain(chain),
            SourceLeafValue::Immediate {
                definition_site: false_site,
                ..
            },
        ) => {
            let binary = row(catalog, constraints.keys.add_i64)?;
            let materialize = row(catalog, constraints.keys.materialize_i64)?;
            if binary.operands.len() != 3
                || materialize.operands.len() != 1
                || binary
                    .operands
                    .iter()
                    .any(|operand| operand.class != binary.operands[2].class)
                || materialize.operands[0].class != binary.operands[2].class
            {
                return Err(SelectedInstructionError::ConstraintOperandMismatch {
                    function: function_index,
                    instruction: 5,
                });
            }
            for (instruction, source_value, definition_site) in [
                (
                    2,
                    chain.resident.source_value,
                    chain.resident.definition_site,
                ),
                (3, chain.left.source_value, chain.left.definition_site),
                (4, chain.right.source_value, chain.right.definition_site),
                (5, chain.inner.source_value, chain.inner.definition_site),
                (6, chain.middle.source_value, chain.middle.definition_site),
                (7, chain.bridge.source_value, chain.bridge.definition_site),
                (8, chain.join.source_value, chain.join.definition_site),
                (9, chain.result.source_value, chain.result.definition_site),
                (11, source.when_false.source_value, *false_site),
            ] {
                expected.push((
                    u64_type,
                    binary.operands[2].class,
                    VirtualRegisterOrigin::InstructionResult {
                        instruction: SelectedInstructionId(instruction),
                        source_value,
                    },
                    definition_site,
                    None,
                ));
            }
        }
        (
            SourceLeafValue::Immediate {
                definition_site: true_site,
                ..
            },
            SourceLeafValue::Immediate {
                definition_site: false_site,
                ..
            },
        ) => {
            let result_class = row(catalog, constraints.keys.materialize_i64)?.operands[0].class;
            expected.push((
                u64_type,
                result_class,
                VirtualRegisterOrigin::InstructionResult {
                    instruction: SelectedInstructionId(2),
                    source_value: source.when_true.source_value,
                },
                *true_site,
                None,
            ));
            expected.push((
                u64_type,
                result_class,
                VirtualRegisterOrigin::InstructionResult {
                    instruction: SelectedInstructionId(4),
                    source_value: source.when_false.source_value,
                },
                *false_site,
                None,
            ));
        }
        (
            SourceLeafValue::EntryParameter {
                parameter_index,
                register,
                definition_site,
            },
            SourceLeafValue::EntryParameter { .. },
        ) => {
            let result_input = fixed_input_constraint(
                source.machine,
                source.when_true.source_value,
                *parameter_index,
                *register,
                &constraints.fixed_inputs,
            )
            .ok_or(SelectedInstructionError::MissingInputRegisterView {
                function: function_index,
            })?;
            let Some(result_view) = physical
                .model()
                .views
                .iter()
                .find(|view| view.id == result_input.fixed_view)
            else {
                return Err(SelectedInstructionError::MissingInputRegisterView {
                    function: function_index,
                });
            };
            expected.push((
                u64_type,
                result_view.class,
                VirtualRegisterOrigin::EntryParameter {
                    source_value: source.when_true.source_value,
                    parameter_index: *parameter_index,
                },
                *definition_site,
                Some(result_input.fixed_view),
            ));
        }
        (
            SourceLeafValue::WidenedExactAdd {
                widen_definition_site: true_site,
                left_temporary: true_left_temporary,
                right_temporary: true_right_temporary,
                left: true_left,
                right: true_right,
                ..
            }
            | SourceLeafValue::WidenedExactSubtract {
                widen_definition_site: true_site,
                left_temporary: true_left_temporary,
                right_temporary: true_right_temporary,
                left: true_left,
                right: true_right,
                ..
            },
            SourceLeafValue::WidenedExactAdd {
                widen_definition_site: false_site,
                left_temporary: false_left_temporary,
                right_temporary: false_right_temporary,
                left: false_left,
                right: false_right,
                ..
            }
            | SourceLeafValue::WidenedExactSubtract {
                widen_definition_site: false_site,
                left_temporary: false_left_temporary,
                right_temporary: false_right_temporary,
                left: false_left,
                right: false_right,
                ..
            },
        ) => {
            let binary_key = match &source.when_true.value {
                SourceLeafValue::WidenedExactAdd { .. } => constraints.keys.add_i64,
                SourceLeafValue::WidenedExactSubtract { .. } => constraints.keys.subtract_i64,
                _ => unreachable!("matched widened exact binary leaves"),
            };
            let binary = row(catalog, binary_key)?;
            let materialize = row(catalog, constraints.keys.materialize_i64)?;
            if binary.operands.len() != 3
                || materialize.operands.len() != 1
                || binary
                    .operands
                    .iter()
                    .any(|operand| operand.class != binary.operands[2].class)
                || materialize.operands[0].class != binary.operands[2].class
            {
                return Err(SelectedInstructionError::ConstraintOperandMismatch {
                    function: function_index,
                    instruction: 4,
                });
            }
            for (instruction, source_value, definition_site, temporary) in [
                (
                    2,
                    true_left.source_value,
                    true_left.definition_site,
                    Some(*true_left_temporary),
                ),
                (
                    3,
                    true_right.source_value,
                    true_right.definition_site,
                    Some(*true_right_temporary),
                ),
                (4, source.when_true.source_value, *true_site, None),
                (
                    6,
                    false_left.source_value,
                    false_left.definition_site,
                    Some(*false_left_temporary),
                ),
                (
                    7,
                    false_right.source_value,
                    false_right.definition_site,
                    Some(*false_right_temporary),
                ),
                (8, source.when_false.source_value, *false_site, None),
            ] {
                let instruction = SelectedInstructionId(instruction);
                expected.push((
                    u64_type,
                    binary.operands[2].class,
                    match temporary {
                        Some(temporary) => VirtualRegisterOrigin::LegalizationTemporary {
                            instruction,
                            temporary,
                            source_value,
                        },
                        None => VirtualRegisterOrigin::InstructionResult {
                            instruction,
                            source_value,
                        },
                    },
                    definition_site,
                    None,
                ));
            }
        }
        (
            SourceLeafValue::ExactAdd {
                definition_site: true_site,
                left: true_left,
                right: true_right,
                ..
            }
            | SourceLeafValue::ExactSubtract {
                definition_site: true_site,
                left: true_left,
                right: true_right,
                ..
            },
            SourceLeafValue::ExactAdd {
                definition_site: false_site,
                left: false_left,
                right: false_right,
                ..
            }
            | SourceLeafValue::ExactSubtract {
                definition_site: false_site,
                left: false_left,
                right: false_right,
                ..
            },
        ) => {
            let binary_key = match &source.when_true.value {
                SourceLeafValue::ExactAdd { .. } => constraints.keys.add_i64,
                SourceLeafValue::ExactSubtract { .. } => constraints.keys.subtract_i64,
                _ => unreachable!("matched exact binary leaves"),
            };
            let binary = row(catalog, binary_key)?;
            let materialize = row(catalog, constraints.keys.materialize_i64)?;
            if binary.operands.len() != 3
                || materialize.operands.len() != 1
                || binary
                    .operands
                    .iter()
                    .any(|operand| operand.class != binary.operands[2].class)
                || materialize.operands[0].class != binary.operands[2].class
            {
                return Err(SelectedInstructionError::ConstraintOperandMismatch {
                    function: function_index,
                    instruction: 4,
                });
            }
            for (instruction, source_value, definition_site) in [
                (2, true_left.source_value, true_left.definition_site),
                (3, true_right.source_value, true_right.definition_site),
                (4, source.when_true.source_value, *true_site),
                (6, false_left.source_value, false_left.definition_site),
                (7, false_right.source_value, false_right.definition_site),
                (8, source.when_false.source_value, *false_site),
            ] {
                expected.push((
                    u64_type,
                    binary.operands[2].class,
                    VirtualRegisterOrigin::InstructionResult {
                        instruction: SelectedInstructionId(instruction),
                        source_value,
                    },
                    definition_site,
                    None,
                ));
            }
        }
        _ => {
            return Err(SelectedInstructionError::UnsupportedSourceShape {
                function: function_index,
            });
        }
    }
    if function.virtual_registers.len() != expected.len() {
        return Err(SelectedInstructionError::FunctionProjectionMismatch {
            function: function_index,
        });
    }
    for (index, (register, expected)) in function.virtual_registers.iter().zip(expected).enumerate()
    {
        if register.scalar_type != expected.0
            || register.class != expected.1
            || register.origin != expected.2
            || register.definition_site != expected.3
            || register.entry_fixed_view != expected.4
        {
            return Err(
                SelectedInstructionError::VirtualRegisterProjectionMismatch {
                    function: function_index,
                    register: index as u32,
                },
            );
        }
    }
    Ok(())
}
