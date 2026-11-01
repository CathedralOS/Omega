//! Access-view inputs on the wire: the source identities, access and
//! binding of one access view with its placement commitment.

use super::super::CodecError;
use super::super::wire::{Writer, decode_canonical};
use super::super::wire_fields::{
    Access, Digest, Field, Flag, Id, Index, List, U32, U64, wire_record,
};
use terminal_psi::TerminalAccessViewInput;

wire_record! {
    Input: TerminalAccessViewInput {
        machine: Id,
        position: U32,
        runtime_position: U32,
        carrier_identity: Index,
        source_machine_identity: Index,
        source_state_identity: Index,
        source_parameter_identity: Index,
        access: Access,
        binding_is_const: Flag,
        binding_is_mutable: Flag,
        view_identity: Index,
        policy_identity: Index,
        policy_plan_machine_identity: Index,
        schema_identity: Index,
        placement_report_fingerprint: U64,
        placement_commitment: Digest,
    }
}

/// Preserve exact formal demands independently of the carrier's storage shape.
pub fn encode_access_view_input_catalog(
    inputs: &[TerminalAccessViewInput],
) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    writer.counted("access view inputs", inputs, Input::write)?;
    Ok(writer.finish())
}

pub fn decode_access_view_input_catalog(
    bytes: &[u8],
) -> Result<Vec<TerminalAccessViewInput>, CodecError> {
    decode_canonical(bytes, List::<Input>::read, |inputs| {
        encode_access_view_input_catalog(inputs)
    })
}
