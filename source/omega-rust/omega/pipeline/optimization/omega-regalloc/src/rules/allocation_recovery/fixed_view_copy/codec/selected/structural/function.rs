//! Canonical structural-function field order shared by V5 and V6.

use omega_selected_instructions::SelectedStructuralUnitFunction;

use crate::FixedViewCopyDecodeError;

use super::{
    call::{
        decode_call_v5, decode_call_v6, decode_return, encode_call_v5, encode_call_v6,
        encode_return,
    },
    settlements::{decode_boundary_settlement, encode_boundary_settlement},
    signature::{decode_signature, encode_signature},
};
use crate::rules::allocation_recovery::fixed_view_copy::codec::primitives::{Cursor, length};

pub(super) fn encode(
    bytes: &mut Vec<u8>,
    function: &SelectedStructuralUnitFunction,
    retain_call_contract: bool,
) {
    encode_signature(bytes, function);
    length(bytes, function.boundary_settlements.len());
    for settlement in &function.boundary_settlements {
        encode_boundary_settlement(bytes, settlement);
    }
    match &function.call {
        None => bytes.push(0),
        Some(call) => {
            bytes.push(1);
            if retain_call_contract {
                encode_call_v6(bytes, call);
            } else {
                encode_call_v5(bytes, call);
            }
        }
    }
    encode_return(bytes, &function.terminator);
}

pub(super) fn decode(
    cursor: &mut Cursor<'_>,
    retain_call_contract: bool,
) -> Result<SelectedStructuralUnitFunction, FixedViewCopyDecodeError> {
    let signature = decode_signature(cursor)?;
    let settlement_count = cursor.length()?;
    let mut boundary_settlements = Vec::with_capacity(settlement_count.min(cursor.remaining()));
    for _ in 0..settlement_count {
        boundary_settlements.push(decode_boundary_settlement(cursor)?);
    }
    let call = match cursor.byte()? {
        0 => None,
        1 => Some(if retain_call_contract {
            decode_call_v6(cursor)?
        } else {
            decode_call_v5(cursor)?
        }),
        tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    };
    Ok(SelectedStructuralUnitFunction {
        machine: signature.machine,
        attachment: signature.attachment,
        provenance: signature.provenance,
        structural_types: signature.structural_types,
        abi: signature.abi,
        structural_places: signature.structural_places,
        entry_claims: signature.entry_claims,
        published_service_ceiling: signature.published_service_ceiling,
        entry_block: signature.entry_block,
        source_entry_block: signature.source_entry_block,
        boundary_settlements,
        call,
        terminator: decode_return(cursor)?,
    })
}
