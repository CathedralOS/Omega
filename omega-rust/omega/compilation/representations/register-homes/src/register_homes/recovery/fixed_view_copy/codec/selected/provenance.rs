use optimization_unit::{FuelSettlement, PsiProvenance};
use selected_instructions::SelectedInstructionProvenance;
use semantic_vocabulary::{EdgeId, ObligationId, OperationId, ValueId};

use crate::FixedViewCopyDecodeError;

use crate::register_homes::recovery::fixed_view_copy::codec::primitives::{
    Cursor, decode_ids, decode_list, encode_ids,
};

pub(super) fn encode_provenance(bytes: &mut Vec<u8>, provenance: &SelectedInstructionProvenance) {
    encode_ids(bytes, provenance.operations.iter().map(|value| value.get()));
    encode_ids(bytes, provenance.values.iter().map(|value| value.get()));
    encode_ids(bytes, provenance.edges.iter().map(|value| value.get()));
    encode_ids(
        bytes,
        provenance.obligations.iter().map(|value| value.get()),
    );
    encode_fuel(bytes, &provenance.fuel);
}

pub(super) fn decode_provenance(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedInstructionProvenance, FixedViewCopyDecodeError> {
    Ok(SelectedInstructionProvenance {
        operations: decode_ids(cursor, OperationId::new)?,
        values: decode_ids(cursor, ValueId::new)?,
        edges: decode_ids(cursor, EdgeId::new)?,
        obligations: decode_ids(cursor, ObligationId::new)?,
        fuel: decode_fuel(cursor)?,
    })
}

pub(super) use legalized_operations::encode_fuel;

pub(super) fn decode_fuel(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<FuelSettlement>, FixedViewCopyDecodeError> {
    decode_list(cursor, |cursor| {
        let tag = cursor.byte()?;
        let raw = cursor.u64()?;
        let site = match tag {
            0 => PsiProvenance::Operation(
                OperationId::new(raw).ok_or(FixedViewCopyDecodeError::InvalidSemanticId(raw))?,
            ),
            1 => PsiProvenance::Edge(
                EdgeId::new(raw).ok_or(FixedViewCopyDecodeError::InvalidSemanticId(raw))?,
            ),
            tag => return Err(FixedViewCopyDecodeError::UnknownFuelSite(tag)),
        };
        Ok(FuelSettlement {
            site,
            units: cursor.u64()?,
        })
    })
}
