//! Placed-view inputs on the wire: the source identities, access and
//! binding of one placed view with its placement commitment.

use super::super::CodecError;
use super::super::wire::{Reader, Writer, decode_canonical, decode_counted};
use terminal_psi::{StructuralAccess, TerminalIdentityIndex, TerminalPlacedViewInput};

/// Preserve exact formal demands independently of the carrier's storage shape.
pub fn encode_placed_view_input_catalog(
    inputs: &[TerminalPlacedViewInput],
) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    writer.len("placed view inputs", inputs.len())?;
    for input in inputs {
        encode_placed_view_input(&mut writer, input)?;
    }
    Ok(writer.finish())
}

pub fn decode_placed_view_input_catalog(
    bytes: &[u8],
) -> Result<Vec<TerminalPlacedViewInput>, CodecError> {
    decode_canonical(
        bytes,
        |reader| decode_counted(reader, decode_placed_view_input),
        |inputs| encode_placed_view_input_catalog(inputs),
    )
}

pub(super) fn encode_placed_view_input(
    writer: &mut Writer,
    input: &TerminalPlacedViewInput,
) -> Result<(), CodecError> {
    writer.id(input.machine);
    writer.u32(input.position);
    writer.u32(input.runtime_position);
    writer.u32(input.carrier_identity.0);
    writer.u32(input.source_machine_identity.0);
    writer.u32(input.source_state_identity.0);
    writer.u32(input.source_parameter_identity.0);
    writer.u8(match input.access {
        StructuralAccess::Owned => 1,
        StructuralAccess::SharedBorrow => 2,
        StructuralAccess::MutableBorrow => 3,
        StructuralAccess::WriteOnlyBorrow => 4,
    });
    writer.boolean(input.binding_is_const);
    writer.boolean(input.binding_is_mutable);
    writer.u32(input.view_identity.0);
    writer.u32(input.policy_identity.0);
    writer.u32(input.policy_plan_machine_identity.0);
    writer.u32(input.schema_identity.0);
    writer.u64(input.placement_report_fingerprint);
    writer.bytes(&input.placement_commitment);
    Ok(())
}

pub(super) fn decode_placed_view_input(
    reader: &mut Reader<'_>,
) -> Result<TerminalPlacedViewInput, CodecError> {
    Ok(TerminalPlacedViewInput {
        machine: reader.id("MachineId")?,
        position: reader.u32()?,
        runtime_position: reader.u32()?,
        carrier_identity: TerminalIdentityIndex(reader.u32()?),
        source_machine_identity: TerminalIdentityIndex(reader.u32()?),
        source_state_identity: TerminalIdentityIndex(reader.u32()?),
        source_parameter_identity: TerminalIdentityIndex(reader.u32()?),
        access: match reader.u8()? {
            1 => StructuralAccess::Owned,
            2 => StructuralAccess::SharedBorrow,
            3 => StructuralAccess::MutableBorrow,
            4 => StructuralAccess::WriteOnlyBorrow,
            tag => return Err(CodecError::InvalidTag("StructuralAccess", tag)),
        },
        binding_is_const: reader.boolean()?,
        binding_is_mutable: reader.boolean()?,
        view_identity: TerminalIdentityIndex(reader.u32()?),
        policy_identity: TerminalIdentityIndex(reader.u32()?),
        policy_plan_machine_identity: TerminalIdentityIndex(reader.u32()?),
        schema_identity: TerminalIdentityIndex(reader.u32()?),
        placement_report_fingerprint: reader.u64()?,
        placement_commitment: reader.array()?,
    })
}
