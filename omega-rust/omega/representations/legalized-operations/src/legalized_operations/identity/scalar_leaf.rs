//! Canonical straight-line scalar body and exact ABI encoding.

use super::calling::{encode_call_plan, encode_placement};
use super::scalar::{encode_integer_type, encode_leaf};
use super::shared::*;

pub(super) fn encode(bytes: &mut Vec<u8>, function: &LegalizedScalarLeafFunction) {
    bytes.extend_from_slice(&function.machine.get().to_le_bytes());
    encode_option_id(bytes, function.attachment.map(|value| value.get()));
    encode_ids(
        bytes,
        function
            .provenance
            .operations
            .iter()
            .map(|value| value.get()),
    );
    encode_ids(
        bytes,
        function.provenance.edges.iter().map(|value| value.get()),
    );
    bytes.extend_from_slice(&function.entry_block.get().to_le_bytes());
    encode_call_plan(bytes, &function.abi.call_plan);
    encode_len(bytes, function.abi.parameters.len());
    for value in function.abi.parameters.iter().chain([&function.abi.result]) {
        bytes.extend_from_slice(&value.value.get().to_le_bytes());
        encode_integer_type(bytes, value.scalar_type);
        encode_placement(bytes, &value.placement);
    }
    encode_leaf(bytes, &function.leaf);
}
