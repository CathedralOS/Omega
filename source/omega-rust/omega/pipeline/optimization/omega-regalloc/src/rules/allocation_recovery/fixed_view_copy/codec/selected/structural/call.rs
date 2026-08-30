use omega_register_model::RegisterUnitId;
use omega_selected_instructions::{
    SelectedInstructionId, SelectedStructuralUnitCallInstruction, SelectedStructuralUnitReturn,
};
use psi_core::{ClaimId, EdgeId, MachineId, OperationId};
use psi_terminal::ClaimTransfer;

use crate::FixedViewCopyDecodeError;

use super::{
    calling::{decode_call_plan, decode_layout, encode_call_plan, encode_layout},
    declarations::{decode_argument, encode_argument},
    settlements::{
        decode_call_source, decode_effect, decode_ownership, encode_call_source, encode_effect,
        encode_ownership,
    },
};
use crate::rules::allocation_recovery::fixed_view_copy::codec::{
    primitives::{Cursor, decode_id, decode_u16s, encode_u16s, length},
    selected::{
        instruction::{decode_instruction, encode_instruction},
        provenance::{decode_provenance, encode_provenance},
    },
    values::{decode_constraint_key, encode_constraint_key},
};

pub(super) fn encode_call(bytes: &mut Vec<u8>, call: &SelectedStructuralUnitCallInstruction) {
    bytes.extend_from_slice(&call.id.0.to_le_bytes());
    encode_call_source(bytes, &call.source);
    bytes.extend_from_slice(&call.operation.get().to_le_bytes());
    bytes.extend_from_slice(&call.callee.get().to_le_bytes());
    encode_call_plan(bytes, &call.caller_call_plan);
    encode_call_plan(bytes, &call.callee_call_plan);
    length(bytes, call.arguments.len());
    for argument in &call.arguments {
        encode_argument(bytes, argument);
    }
    length(bytes, call.claim_transfers.len());
    for transfer in &call.claim_transfers {
        bytes.extend_from_slice(&transfer.claim.get().to_le_bytes());
        bytes.extend_from_slice(&transfer.argument_index.to_le_bytes());
    }
    encode_layout(bytes, call.layout);
    encode_constraint_key(bytes, call.constraint);
    encode_u16s(bytes, call.implicit_uses.iter().map(|unit| unit.0));
    encode_u16s(bytes, call.implicit_defs.iter().map(|unit| unit.0));
    encode_u16s(bytes, call.clobbers.iter().map(|unit| unit.0));
    encode_provenance(bytes, &call.provenance);
    encode_effect(bytes, call.effect);
    encode_ownership(bytes, &call.ownership);
}

pub(super) fn decode_call(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedStructuralUnitCallInstruction, FixedViewCopyDecodeError> {
    let id = SelectedInstructionId(cursor.u32()?);
    let source = decode_call_source(cursor)?;
    let operation = decode_id(cursor, OperationId::new)?;
    let callee = decode_id(cursor, MachineId::new)?;
    let caller_call_plan = decode_call_plan(cursor)?;
    let callee_call_plan = decode_call_plan(cursor)?;
    let argument_count = cursor.length()?;
    let mut arguments = Vec::with_capacity(argument_count.min(cursor.remaining()));
    for _ in 0..argument_count {
        arguments.push(decode_argument(cursor)?);
    }
    let transfer_count = cursor.length()?;
    let mut claim_transfers = Vec::with_capacity(transfer_count.min(cursor.remaining()));
    for _ in 0..transfer_count {
        claim_transfers.push(ClaimTransfer {
            claim: decode_id(cursor, ClaimId::new)?,
            argument_index: cursor.u32()?,
        });
    }
    Ok(SelectedStructuralUnitCallInstruction {
        id,
        source,
        operation,
        callee,
        caller_call_plan,
        callee_call_plan,
        arguments,
        claim_transfers,
        layout: decode_layout(cursor)?,
        constraint: decode_constraint_key(cursor)?,
        implicit_uses: decode_u16s(cursor)?
            .into_iter()
            .map(RegisterUnitId)
            .collect(),
        implicit_defs: decode_u16s(cursor)?
            .into_iter()
            .map(RegisterUnitId)
            .collect(),
        clobbers: decode_u16s(cursor)?
            .into_iter()
            .map(RegisterUnitId)
            .collect(),
        provenance: decode_provenance(cursor)?,
        effect: decode_effect(cursor)?,
        ownership: decode_ownership(cursor)?,
    })
}

pub(super) fn encode_return(bytes: &mut Vec<u8>, value: &SelectedStructuralUnitReturn) {
    encode_instruction(bytes, &value.instruction);
    bytes.extend_from_slice(&value.psi_return_edge.get().to_le_bytes());
    encode_effect(bytes, value.effect);
    encode_ownership(bytes, &value.ownership);
}

pub(super) fn decode_return(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedStructuralUnitReturn, FixedViewCopyDecodeError> {
    Ok(SelectedStructuralUnitReturn {
        instruction: decode_instruction(cursor)?,
        psi_return_edge: decode_id(cursor, EdgeId::new)?,
        effect: decode_effect(cursor)?,
        ownership: decode_ownership(cursor)?,
    })
}
