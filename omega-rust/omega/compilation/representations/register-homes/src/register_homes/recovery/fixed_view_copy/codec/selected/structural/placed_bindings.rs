//! Decode data only; source custody replay answers every retained binding.
use super::declarations::decode_access;
use crate::FixedViewCopyDecodeError;
use crate::register_homes::recovery::fixed_view_copy::codec::primitives::{Cursor, decode_id};
use semantic_vocabulary::{MachineId, StructuralTypeId};
use target_operations::{TargetPlacedCallAssociation, TargetPlacedCallBinding};
use terminal_psi::{TerminalIdentityIndex, TerminalPlacedViewInput};

fn boolean(cursor: &mut Cursor<'_>) -> Result<bool, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        0 => Ok(false),
        1 => Ok(true),
        tag => Err(FixedViewCopyDecodeError::UnknownBoolean(tag)),
    }
}
fn input(cursor: &mut Cursor<'_>) -> Result<TerminalPlacedViewInput, FixedViewCopyDecodeError> {
    Ok(TerminalPlacedViewInput {
        machine: decode_id(cursor, MachineId::new)?,
        position: cursor.u32()?,
        runtime_position: cursor.u32()?,
        carrier_identity: TerminalIdentityIndex(cursor.u32()?),
        source_machine_identity: TerminalIdentityIndex(cursor.u32()?),
        source_state_identity: TerminalIdentityIndex(cursor.u32()?),
        source_parameter_identity: TerminalIdentityIndex(cursor.u32()?),
        access: decode_access(cursor)?,
        binding_is_const: boolean(cursor)?,
        binding_is_mutable: boolean(cursor)?,
        view_identity: TerminalIdentityIndex(cursor.u32()?),
        policy_identity: TerminalIdentityIndex(cursor.u32()?),
        policy_plan_machine_identity: TerminalIdentityIndex(cursor.u32()?),
        schema_identity: TerminalIdentityIndex(cursor.u32()?),
        placement_report_fingerprint: cursor.u64()?,
        placement_commitment: cursor
            .take(32)?
            .try_into()
            .map_err(|_| FixedViewCopyDecodeError::Truncated)?,
    })
}
pub(super) fn decode(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<TargetPlacedCallBinding>, FixedViewCopyDecodeError> {
    let count = cursor.length()?;
    let mut bindings = Vec::new();
    for _ in 0..count {
        let input = input(cursor)?;
        let argument_index = cursor.u32()?;
        let association = match cursor.byte()? {
            0 => TargetPlacedCallAssociation::Captured {
                schema: decode_id(cursor, StructuralTypeId::new)?,
                carrier: decode_id(cursor, StructuralTypeId::new)?,
            },
            1 => TargetPlacedCallAssociation::CallerFormal(self::input(cursor)?),
            tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
        };
        bindings.push(TargetPlacedCallBinding {
            input,
            argument_index,
            association,
        });
    }
    Ok(bindings)
}
