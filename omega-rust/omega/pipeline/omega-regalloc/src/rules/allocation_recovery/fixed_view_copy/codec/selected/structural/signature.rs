use omega_selected_instructions::{SelectedBlockId, SelectedStructuralUnitFunction};
use omega_target_operations::TerminalPsiProvenance;
use psi_core::{BlockId, EdgeId, MachineId, OperationId, ServiceId, StructuralTypeId};

use crate::FixedViewCopyDecodeError;

use super::{
    calling::{decode_abi, encode_abi},
    declarations::{
        decode_entry_claim, decode_place, decode_type, encode_entry_claim, encode_place,
        encode_type,
    },
};
use crate::rules::allocation_recovery::fixed_view_copy::codec::primitives::{
    Cursor, decode_id, decode_ids, decode_option_u64, encode_ids, encode_option_u64, length,
};

pub(super) struct DecodedSignature {
    pub(super) machine: MachineId,
    pub(super) attachment: Option<StructuralTypeId>,
    pub(super) provenance: TerminalPsiProvenance,
    pub(super) structural_types: Vec<psi_terminal::StructuralTypeDeclaration>,
    pub(super) abi: omega_selected_instructions::SelectedStructuralUnitAbi,
    pub(super) structural_places: Vec<psi_terminal::StructuralPlaceDeclaration>,
    pub(super) entry_claims: Vec<psi_terminal::EntryClaim>,
    pub(super) published_service_ceiling: Vec<ServiceId>,
    pub(super) entry_block: SelectedBlockId,
    pub(super) source_entry_block: BlockId,
}

pub(super) fn encode_signature(
    bytes: &mut Vec<u8>,
    function: &SelectedStructuralUnitFunction,
    retain_projected_qualifications: bool,
) {
    bytes.extend_from_slice(&function.machine.get().to_le_bytes());
    encode_option_u64(bytes, function.attachment.map(|value| value.get()));
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
    length(bytes, function.structural_types.len());
    for declaration in &function.structural_types {
        encode_type(bytes, declaration);
    }
    encode_abi(bytes, &function.abi, retain_projected_qualifications);
    length(bytes, function.structural_places.len());
    for place in &function.structural_places {
        encode_place(bytes, *place);
    }
    length(bytes, function.entry_claims.len());
    for claim in &function.entry_claims {
        encode_entry_claim(bytes, claim);
    }
    encode_ids(
        bytes,
        function
            .published_service_ceiling
            .iter()
            .map(|value| value.get()),
    );
    bytes.extend_from_slice(&function.entry_block.0.to_le_bytes());
    bytes.extend_from_slice(&function.source_entry_block.get().to_le_bytes());
}

pub(super) fn decode_signature(
    cursor: &mut Cursor<'_>,
    retain_projected_qualifications: bool,
) -> Result<DecodedSignature, FixedViewCopyDecodeError> {
    let machine = decode_id(cursor, MachineId::new)?;
    let attachment = match decode_option_u64(cursor)? {
        None => None,
        Some(raw) => Some(
            StructuralTypeId::new(raw).ok_or(FixedViewCopyDecodeError::InvalidSemanticId(raw))?,
        ),
    };
    let provenance = TerminalPsiProvenance {
        operations: decode_ids(cursor, OperationId::new)?,
        edges: decode_ids(cursor, EdgeId::new)?,
    };
    let type_count = cursor.length()?;
    let mut structural_types = Vec::with_capacity(type_count.min(cursor.remaining()));
    for _ in 0..type_count {
        structural_types.push(decode_type(cursor)?);
    }
    let abi = decode_abi(cursor, retain_projected_qualifications)?;
    let place_count = cursor.length()?;
    let mut structural_places = Vec::with_capacity(place_count.min(cursor.remaining()));
    for _ in 0..place_count {
        structural_places.push(decode_place(cursor)?);
    }
    let claim_count = cursor.length()?;
    let mut entry_claims = Vec::with_capacity(claim_count.min(cursor.remaining()));
    for _ in 0..claim_count {
        entry_claims.push(decode_entry_claim(cursor)?);
    }
    Ok(DecodedSignature {
        machine,
        attachment,
        provenance,
        structural_types,
        abi,
        structural_places,
        entry_claims,
        published_service_ceiling: decode_ids(cursor, ServiceId::new)?,
        entry_block: SelectedBlockId(cursor.u32()?),
        source_entry_block: decode_id(cursor, BlockId::new)?,
    })
}
