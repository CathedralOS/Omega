//! Model-neutral canonical writers shared by rewrite identities and candidate encoding.

use super::*;

pub(super) fn encode_definition_site(bytes: &mut Vec<u8>, site: ValueDefinitionSite) {
    match site {
        ValueDefinitionSite::FunctionParameter(position) => {
            bytes.push(1);
            bytes.extend_from_slice(&position.to_le_bytes());
        }
        ValueDefinitionSite::BlockParameter { block, position } => {
            bytes.push(2);
            bytes.extend_from_slice(&block.get().to_le_bytes());
            bytes.extend_from_slice(&position.to_le_bytes());
        }
        ValueDefinitionSite::Node { block, node } => {
            bytes.push(3);
            bytes.extend_from_slice(&block.get().to_le_bytes());
            bytes.extend_from_slice(&node.to_le_bytes());
        }
    }
}

pub(super) fn encode_len(bytes: &mut Vec<u8>, len: usize) {
    bytes.extend_from_slice(
        &u64::try_from(len)
            .expect("canonical candidate list length fits u64")
            .to_le_bytes(),
    );
}

pub(super) fn encode_scalar_type(bytes: &mut Vec<u8>, scalar_type: ScalarType) {
    match scalar_type {
        ScalarType::Boolean => bytes.push(1),
        ScalarType::Integer(integer) => {
            bytes.push(2);
            encode_integer_type(bytes, integer);
        }
        ScalarType::IeeeFloat(format) => {
            bytes.push(3);
            bytes.push(match format {
                psi_core::IeeeFloatFormat::Binary32 => 1,
                psi_core::IeeeFloatFormat::Binary64 => 2,
            });
        }
    }
}

pub(super) fn encode_integer_type(bytes: &mut Vec<u8>, integer: IntegerType) {
    bytes.push(match integer.carrier() {
        IntegerCarrier::Fixed => 1,
        IntegerCarrier::Address => 2,
    });
    bytes.push(match integer.sign() {
        IntegerSign::Signed => 1,
        IntegerSign::Unsigned => 2,
    });
    bytes.extend_from_slice(&integer.bits().to_le_bytes());
}

pub(super) fn encode_integer_value(bytes: &mut Vec<u8>, value: IntegerValue) {
    match value {
        IntegerValue::Signed(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        IntegerValue::Unsigned(value) => {
            bytes.push(2);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
}
