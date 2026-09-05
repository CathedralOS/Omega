use register_model::RegisterUnitId;
use selected_instructions::{
    SelectedInstructionId, SelectedStructuralUnitCallInstruction, SelectedStructuralUnitReturn,
};
use semantic_vocabulary::{ClaimId, EdgeId, MachineId, OperationId};
use terminal_psi::ClaimTransfer;

use crate::FixedViewCopyDecodeError;

use super::{
    calling::{decode_call_plan, decode_layout, encode_call_plan, encode_layout},
    declarations::{decode_argument, encode_argument},
    settlements::{
        decode_call_source, decode_effect, decode_ownership, encode_call_source, encode_effect,
        encode_ownership,
    },
};
use crate::rewrites::allocation_recovery::fixed_view_copy::codec::{
    primitives::{Cursor, decode_id, decode_u16s, encode_u16s, length},
    selected::{
        instruction::{decode_instruction, encode_instruction},
        provenance::{decode_provenance, encode_provenance},
    },
    values::{decode_constraint_key, encode_constraint_key},
};

pub(super) fn encode_call_v5(bytes: &mut Vec<u8>, call: &SelectedStructuralUnitCallInstruction) {
    encode_call(bytes, call, false);
}

pub(super) fn encode_call_v6(bytes: &mut Vec<u8>, call: &SelectedStructuralUnitCallInstruction) {
    encode_call(bytes, call, true);
}

fn encode_call(
    bytes: &mut Vec<u8>,
    call: &SelectedStructuralUnitCallInstruction,
    retain_contract: bool,
) {
    bytes.extend_from_slice(&call.id.0.to_le_bytes());
    encode_call_source(bytes, &call.source, retain_contract);
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
    if retain_contract {
        length(bytes, call.requirement_obligations.len());
        for obligation in &call.requirement_obligations {
            bytes.extend_from_slice(&obligation.get().to_le_bytes());
        }
        let crash_continuations =
            terminal_codec::encode_crash_route_buckets(&call.crash_continuations)
                .expect("verified selected call crash continuations remain canonical");
        length(bytes, crash_continuations.len());
        bytes.extend_from_slice(&crash_continuations);
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

pub(super) fn decode_call_v5(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedStructuralUnitCallInstruction, FixedViewCopyDecodeError> {
    decode_call(cursor, false)
}

pub(super) fn decode_call_v6(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedStructuralUnitCallInstruction, FixedViewCopyDecodeError> {
    decode_call(cursor, true)
}

fn decode_call(
    cursor: &mut Cursor<'_>,
    retain_contract: bool,
) -> Result<SelectedStructuralUnitCallInstruction, FixedViewCopyDecodeError> {
    let id = SelectedInstructionId(cursor.u32()?);
    let source = decode_call_source(cursor, retain_contract)?;
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
    let (requirement_obligations, crash_continuations) = if retain_contract {
        let requirement_count = cursor.length()?;
        let mut requirements = Vec::with_capacity(requirement_count.min(cursor.remaining()));
        for _ in 0..requirement_count {
            requirements.push(decode_id(cursor, semantic_vocabulary::ObligationId::new)?);
        }
        let crash_length = cursor.length()?;
        let crash_continuations =
            terminal_codec::decode_crash_route_buckets(cursor.take(crash_length)?)
                .map_err(|_| FixedViewCopyDecodeError::InvalidCrashContinuations)?;
        (requirements, crash_continuations)
    } else {
        (Vec::new(), Vec::new())
    };
    Ok(SelectedStructuralUnitCallInstruction {
        id,
        source,
        operation,
        callee,
        caller_call_plan,
        callee_call_plan,
        arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
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
