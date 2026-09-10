//! Independent entry-register ABI replay and payload normalization.
use super::*;
use crate::selection::constraints::fixed_input_constraint;

pub(super) fn validate(
    source: &LegalizedScalarFunction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    catalog: &ValidatedRegisterConstraintCatalog,
    replay: &mut Replay<'_>,
) -> Result<(), SelectedInstructionError> {
    let function = replay.function;
    let invalid = || SelectedInstructionError::FunctionProjectionMismatch { function };
    let constraints = replay.constraints;
    for (index, parameter) in source.parameters.iter().enumerate() {
        if crate::selection::scalar_call_abi::scalar_stack_placement(&parameter.placement).is_some()
        {
            continue;
        }
        if !replay.required_values.contains(&parameter.value) {
            continue;
        }
        let [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] = parameter.placement.locations.as_slice()
        else {
            return Err(invalid());
        };
        if *byte_size
            != match parameter.scalar_type {
                ScalarType::Boolean => 1,
                ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32) => 4,
                ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64) => 8,
                ScalarType::Integer(integer) if matches!(integer.bits(), 8 | 16 | 32 | 64) => {
                    integer.bits() / 8
                }
                _ => return Err(invalid()),
            }
        {
            return Err(invalid());
        }
        let fixed = fixed_input_constraint(
            source.machine,
            parameter.value,
            index,
            *register,
            &constraints.fixed_inputs,
        )
        .ok_or_else(invalid)?;
        if environment.fixed_register_view(*register) != Some(fixed.fixed_view) {
            return Err(invalid());
        }
        let parameter_class = if let Some((_, key)) =
            crate::selection::scalar_call_abi::incoming_float_transfer(
                parameter.scalar_type,
                &constraints.keys,
            ) {
            row(catalog, key)?
                .operands
                .first()
                .ok_or_else(invalid)?
                .class
        } else {
            replay.class
        };
        let id = replay.check_register_class(
            parameter_class,
            parameter.definition_site,
            parameter.scalar_type,
            VirtualRegisterOrigin::EntryParameter {
                source_value: parameter.value,
                parameter_index: index,
            },
            Some(fixed.fixed_view),
        )?;
        replay.definitions.push((
            parameter.value,
            id,
            parameter.definition_site,
            parameter.scalar_type,
        ));
    }
    for index in 0..replay.definitions.len() {
        let (value, input, site, scalar_type) = replay.definitions[index];
        let output = if let Some((kind, key)) =
            crate::selection::scalar_call_abi::incoming_float_transfer(
                scalar_type,
                &constraints.keys,
            ) {
            let output = replay.result_register(value, site, scalar_type)?;
            replay.check_instruction(
                kind,
                key,
                &[input, output],
                &SelectedInstructionProvenance {
                    values: vec![value],
                    ..Default::default()
                },
            )?;
            output
        } else if crate::selection::scalar_call_abi::integer_abi_normalization(scalar_type)
            != SelectedInstructionKind::CopyI64
        {
            let output = replay.result_register(value, site, scalar_type)?;
            replay.check_instruction(
                crate::selection::scalar_call_abi::integer_abi_normalization(scalar_type),
                constraints.keys.copy_i64,
                &[input, output],
                &SelectedInstructionProvenance {
                    values: vec![value],
                    ..Default::default()
                },
            )?;
            output
        } else {
            replay.check_copy(input, value, site, scalar_type)?
        };
        replay.definitions[index].1 = output;
    }

    Ok(())
}
