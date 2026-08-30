//! Optimizer module role: stage group. Selected structural-function V5 payload codec.
//!
//! This entrance fixes the signature -> settlements -> optional call -> return
//! order. Leaves own the exhaustive semantic, ABI, provider, and effect fields.

mod call;
mod calling;
mod declarations;
mod settlements;
mod signature;

use omega_selected_instructions::SelectedStructuralUnitFunction;

use crate::FixedViewCopyDecodeError;

use self::{
    call::{decode_call, decode_return, encode_call, encode_return},
    settlements::{decode_boundary_settlement, encode_boundary_settlement},
    signature::{decode_signature, encode_signature},
};
use crate::rules::allocation_recovery::fixed_view_copy::codec::primitives::{Cursor, length};

pub(super) fn encode_structural_function(
    bytes: &mut Vec<u8>,
    function: &SelectedStructuralUnitFunction,
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
            encode_call(bytes, call);
        }
    }
    encode_return(bytes, &function.terminator);
}

pub(super) fn decode_structural_function(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedStructuralUnitFunction, FixedViewCopyDecodeError> {
    let signature = decode_signature(cursor)?;
    let settlement_count = cursor.length()?;
    let mut boundary_settlements = Vec::with_capacity(settlement_count.min(cursor.remaining()));
    for _ in 0..settlement_count {
        boundary_settlements.push(decode_boundary_settlement(cursor)?);
    }
    let call = match cursor.byte()? {
        0 => None,
        1 => Some(decode_call(cursor)?),
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
