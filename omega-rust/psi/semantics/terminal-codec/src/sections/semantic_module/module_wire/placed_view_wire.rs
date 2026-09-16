//! Placed-view inputs on the wire: the source identities, access and
//! binding of one placed view with its placement commitment.

use super::super::CodecError;
use super::super::wire::{Reader, Writer};
use terminal_psi::{StructuralAccess, TerminalPlacedViewInput};

pub(super) fn encode_placed_view_input(
    writer: &mut Writer,
    input: &TerminalPlacedViewInput,
) -> Result<(), CodecError> {
    writer.id(input.machine);
    writer.u32(input.position);
    writer.string(
        "placed-view source machine identity",
        &input.source_machine_identity,
    )?;
    writer.string(
        "placed-view source state identity",
        &input.source_state_identity,
    )?;
    writer.string(
        "placed-view source parameter identity",
        &input.source_parameter_identity,
    )?;
    writer.u8(match input.access {
        StructuralAccess::Owned => 1,
        StructuralAccess::SharedBorrow => 2,
        StructuralAccess::MutableBorrow => 3,
        StructuralAccess::WriteOnlyBorrow => 4,
    });
    writer.boolean(input.binding_is_const);
    writer.boolean(input.binding_is_mutable);
    writer.string("placed-view identity", &input.view_identity)?;
    writer.string("placed-view policy identity", &input.policy_identity)?;
    writer.string(
        "placed-view policy-plan machine identity",
        &input.policy_plan_machine_identity,
    )?;
    writer.string("placed-view schema identity", &input.schema_identity)?;
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
        source_machine_identity: reader.string("placed-view source machine identity")?,
        source_state_identity: reader.string("placed-view source state identity")?,
        source_parameter_identity: reader.string("placed-view source parameter identity")?,
        access: match reader.u8()? {
            1 => StructuralAccess::Owned,
            2 => StructuralAccess::SharedBorrow,
            3 => StructuralAccess::MutableBorrow,
            4 => StructuralAccess::WriteOnlyBorrow,
            tag => return Err(CodecError::InvalidTag("StructuralAccess", tag)),
        },
        binding_is_const: reader.boolean()?,
        binding_is_mutable: reader.boolean()?,
        view_identity: reader.string("placed-view identity")?,
        policy_identity: reader.string("placed-view policy identity")?,
        policy_plan_machine_identity: reader.string("placed-view policy-plan machine identity")?,
        schema_identity: reader.string("placed-view schema identity")?,
        placement_report_fingerprint: reader.u64()?,
        placement_commitment: reader.array()?,
    })
}
