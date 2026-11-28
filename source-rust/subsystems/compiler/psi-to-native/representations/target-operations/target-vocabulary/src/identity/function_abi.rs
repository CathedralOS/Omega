//! Canonical bytes for the per-function target ABI rows.

use super::calling::{encode_call_plan, encode_placement};
use super::structural_types::encode_target_structural_parameter;
use super::{encode_len, encode_option, encode_scalar_type};
use crate::LegalizedFunctionAbi;
use crate::ScalarAbiValue;

pub fn encode_function_abi(bytes: &mut Vec<u8>, abi: &LegalizedFunctionAbi) {
    crate::codec_bytes::push_encoded(bytes, abi.machine.get().to_le_bytes());
    encode_option(bytes, abi.assembly_return, |bytes, view| {
        bytes.push(view.slot())
    });
    encode_len(bytes, abi.assembly_register_versions.len());
    for version in &abi.assembly_register_versions {
        bytes.extend_from_slice(&version.edge.get().to_le_bytes());
        bytes.push(version.view.slot());
        bytes.extend_from_slice(&version.operation.get().to_le_bytes());
        bytes.extend_from_slice(&version.value.get().to_le_bytes());
        let operation_operands::values::AssemblyRegisterMaterialization::IntegerImmediate(literal) =
            version.materialization;
        bytes.push(0); // register-immediate materialization
        match literal {
            semantic_vocabulary::IntegerValue::Signed(value) => {
                bytes.push(0);
                bytes.extend_from_slice(&value.to_le_bytes());
            }
            semantic_vocabulary::IntegerValue::Unsigned(value) => {
                bytes.push(1);
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
    }
    encode_call_plan(bytes, &abi.call_plan);
    encode_len(bytes, abi.entry_scalar_codecs.len());
    for codec in &abi.entry_scalar_codecs {
        encode_option(bytes, *codec, |bytes, word| {
            crate::codec_bytes::push_encoded(bytes, word.to_le_bytes());
        });
    }
    encode_scalar_values(bytes, &abi.scalar_parameters);
    encode_len(bytes, abi.structural_parameters.len());
    for parameter in &abi.structural_parameters {
        encode_target_structural_parameter(bytes, parameter);
    }
    encode_option(bytes, abi.scalar.as_ref(), |bytes, scalar| {
        encode_call_plan(bytes, &scalar.call_plan);
        encode_scalar_values(bytes, &scalar.parameters);
        encode_option(bytes, scalar.result.as_ref(), encode_scalar_value);
    });
    encode_option(
        bytes,
        abi.mixed_structural_scalar.as_ref(),
        |bytes, mixed| {
            encode_call_plan(bytes, &mixed.call_plan);
            encode_scalar_values(bytes, &mixed.scalar_parameters);
            encode_len(bytes, mixed.structural_parameters.len());
            for parameter in &mixed.structural_parameters {
                encode_target_structural_parameter(bytes, parameter);
            }
            encode_scalar_value(bytes, &mixed.result);
        },
    );
}

fn encode_scalar_values(bytes: &mut Vec<u8>, values: &[ScalarAbiValue]) {
    encode_len(bytes, values.len());
    for value in values {
        encode_scalar_value(bytes, value);
    }
}

fn encode_scalar_value(bytes: &mut Vec<u8>, value: &ScalarAbiValue) {
    crate::codec_bytes::push_encoded(bytes, value.value.get().to_le_bytes());
    encode_scalar_type(bytes, value.scalar_type);
    encode_placement(bytes, &value.placement);
}
