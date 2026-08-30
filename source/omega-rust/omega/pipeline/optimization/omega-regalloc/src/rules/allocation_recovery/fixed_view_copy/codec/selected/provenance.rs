use omega_optimization_unit::{FuelSettlement, PsiProvenance};
use omega_selected_instructions::SelectedInstructionProvenance;
use psi_core::{EdgeId, ObligationId, OperationId, ValueId};

use crate::FixedViewCopyDecodeError;

use crate::rules::allocation_recovery::fixed_view_copy::codec::primitives::{
    Cursor, decode_ids, encode_ids, length,
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

pub(super) fn encode_fuel(bytes: &mut Vec<u8>, fuel: &[FuelSettlement]) {
    length(bytes, fuel.len());
    for settlement in fuel {
        match settlement.site {
            PsiProvenance::Operation(id) => {
                bytes.push(0);
                bytes.extend_from_slice(&id.get().to_le_bytes());
            }
            PsiProvenance::Edge(id) => {
                bytes.push(1);
                bytes.extend_from_slice(&id.get().to_le_bytes());
            }
        }
        bytes.extend_from_slice(&settlement.units.to_le_bytes());
    }
}

pub(super) fn decode_fuel(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<FuelSettlement>, FixedViewCopyDecodeError> {
    let count = cursor.length()?;
    let mut fuel = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
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
        fuel.push(FuelSettlement {
            site,
            units: cursor.u64()?,
        });
    }
    Ok(fuel)
}
