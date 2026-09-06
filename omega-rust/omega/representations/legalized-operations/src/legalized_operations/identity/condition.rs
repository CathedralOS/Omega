//! Canonical scalar condition identity, shared by direct and joined control.

use super::scalar::{encode_definition_site, encode_immediate, encode_register};
use super::shared::*;

pub(super) fn encode_condition(
    bytes: &mut Vec<u8>,
    source: semantic_vocabulary::ValueId,
    condition: &LegalizedCondition,
) {
    bytes.extend_from_slice(&source.get().to_le_bytes());
    match condition {
        LegalizedCondition::DirectParameter {
            parameter_index,
            register,
            definition_site,
        } => {
            // Preserve the exact pre-V13 byte layout for every existing
            // direct-condition recipe.
            bytes.extend_from_slice(&(*parameter_index as u64).to_le_bytes());
            encode_register(bytes, *register);
            encode_definition_site(bytes, *definition_site);
        }
        LegalizedCondition::IntegerEqualParametersV1 {
            operation,
            result_definition_site,
            fuel,
            left,
            right,
        } => {
            bytes.push(0xff);
            bytes.extend_from_slice(&operation.get().to_le_bytes());
            encode_definition_site(bytes, *result_definition_site);
            encode_fuel(bytes, fuel);
            for parameter in [left, right] {
                bytes.extend_from_slice(&parameter.source_value.get().to_le_bytes());
                bytes.extend_from_slice(&(parameter.parameter_index as u64).to_le_bytes());
                encode_register(bytes, parameter.register);
                encode_definition_site(bytes, parameter.definition_site);
            }
        }
        LegalizedCondition::IntegerLessThanParametersV1 {
            operation,
            result_definition_site,
            fuel,
            left,
            right,
        } => {
            bytes.push(0xfe);
            bytes.extend_from_slice(&operation.get().to_le_bytes());
            encode_definition_site(bytes, *result_definition_site);
            encode_fuel(bytes, fuel);
            for parameter in [left, right] {
                bytes.extend_from_slice(&parameter.source_value.get().to_le_bytes());
                bytes.extend_from_slice(&(parameter.parameter_index as u64).to_le_bytes());
                encode_register(bytes, parameter.register);
                encode_definition_site(bytes, parameter.definition_site);
            }
        }
        LegalizedCondition::IntegerLessOrEqualParametersV1 {
            operation,
            result_definition_site,
            fuel,
            left,
            right,
        } => {
            bytes.push(0xfd);
            bytes.extend_from_slice(&operation.get().to_le_bytes());
            encode_definition_site(bytes, *result_definition_site);
            encode_fuel(bytes, fuel);
            for parameter in [left, right] {
                bytes.extend_from_slice(&parameter.source_value.get().to_le_bytes());
                bytes.extend_from_slice(&(parameter.parameter_index as u64).to_le_bytes());
                encode_register(bytes, parameter.register);
                encode_definition_site(bytes, parameter.definition_site);
            }
        }
        LegalizedCondition::IntegerNotEqualParametersV1 {
            equality_operation,
            equality_result,
            equality_result_definition_site,
            equality_fuel,
            boolean_not_operation,
            boolean_not_result,
            boolean_not_result_definition_site,
            boolean_not_fuel,
            left,
            right,
        } => {
            bytes.push(0xfc);
            bytes.extend_from_slice(&equality_operation.get().to_le_bytes());
            bytes.extend_from_slice(&equality_result.get().to_le_bytes());
            encode_definition_site(bytes, *equality_result_definition_site);
            encode_fuel(bytes, equality_fuel);
            bytes.extend_from_slice(&boolean_not_operation.get().to_le_bytes());
            bytes.extend_from_slice(&boolean_not_result.get().to_le_bytes());
            encode_definition_site(bytes, *boolean_not_result_definition_site);
            encode_fuel(bytes, boolean_not_fuel);
            for parameter in [left, right] {
                bytes.extend_from_slice(&parameter.source_value.get().to_le_bytes());
                bytes.extend_from_slice(&(parameter.parameter_index as u64).to_le_bytes());
                encode_register(bytes, parameter.register);
                encode_definition_site(bytes, parameter.definition_site);
            }
        }
        LegalizedCondition::I64LessThanParametersV1 {
            operation,
            result_definition_site,
            fuel,
            left,
            right,
        } => {
            bytes.push(0xfb);
            bytes.extend_from_slice(&operation.get().to_le_bytes());
            encode_definition_site(bytes, *result_definition_site);
            encode_fuel(bytes, fuel);
            for parameter in [left, right] {
                bytes.extend_from_slice(&parameter.source_value.get().to_le_bytes());
                bytes.extend_from_slice(&(parameter.parameter_index as u64).to_le_bytes());
                encode_register(bytes, parameter.register);
                encode_definition_site(bytes, parameter.definition_site);
            }
        }
        LegalizedCondition::U64EqualZeroParameterV1 {
            operation,
            result_definition_site,
            fuel,
            parameter,
            zero,
        } => {
            bytes.push(0xfa);
            bytes.extend_from_slice(&operation.get().to_le_bytes());
            encode_definition_site(bytes, *result_definition_site);
            encode_fuel(bytes, fuel);
            bytes.extend_from_slice(&parameter.source_value.get().to_le_bytes());
            bytes.extend_from_slice(&(parameter.parameter_index as u64).to_le_bytes());
            encode_register(bytes, parameter.register);
            encode_definition_site(bytes, parameter.definition_site);
            encode_immediate(bytes, zero);
        }
        LegalizedCondition::U64NotEqualZeroParameterV1 {
            equality_operation,
            equality_result,
            equality_result_definition_site,
            equality_fuel,
            boolean_not_operation,
            boolean_not_result,
            boolean_not_result_definition_site,
            boolean_not_fuel,
            parameter,
            zero,
        } => {
            bytes.push(0xf9);
            bytes.extend_from_slice(&equality_operation.get().to_le_bytes());
            bytes.extend_from_slice(&equality_result.get().to_le_bytes());
            encode_definition_site(bytes, *equality_result_definition_site);
            encode_fuel(bytes, equality_fuel);
            bytes.extend_from_slice(&boolean_not_operation.get().to_le_bytes());
            bytes.extend_from_slice(&boolean_not_result.get().to_le_bytes());
            encode_definition_site(bytes, *boolean_not_result_definition_site);
            encode_fuel(bytes, boolean_not_fuel);
            bytes.extend_from_slice(&parameter.source_value.get().to_le_bytes());
            bytes.extend_from_slice(&(parameter.parameter_index as u64).to_le_bytes());
            encode_register(bytes, parameter.register);
            encode_definition_site(bytes, parameter.definition_site);
            encode_immediate(bytes, zero);
        }
        LegalizedCondition::I64LessOrEqualParametersV1 {
            operation,
            result_definition_site,
            fuel,
            left,
            right,
        } => {
            bytes.push(0xf8);
            bytes.extend_from_slice(&operation.get().to_le_bytes());
            encode_definition_site(bytes, *result_definition_site);
            encode_fuel(bytes, fuel);
            for parameter in [left, right] {
                bytes.extend_from_slice(&parameter.source_value.get().to_le_bytes());
                bytes.extend_from_slice(&(parameter.parameter_index as u64).to_le_bytes());
                encode_register(bytes, parameter.register);
                encode_definition_site(bytes, parameter.definition_site);
            }
        }
    }
}
