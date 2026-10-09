use crate::SelectedInstructionProvenance;
use optimization_unit::PsiProvenance;

use super::values::{encode_ids, encode_len};

pub fn encode_provenance(bytes: &mut Vec<u8>, provenance: &SelectedInstructionProvenance) {
    encode_ids(bytes, provenance.operations.iter().map(|id| id.get()));
    encode_ids(bytes, provenance.values.iter().map(|id| id.get()));
    encode_ids(bytes, provenance.edges.iter().map(|id| id.get()));
    encode_ids(bytes, provenance.obligations.iter().map(|id| id.get()));
    encode_len(bytes, provenance.fuel.len());
    for settlement in &provenance.fuel {
        match settlement.site {
            PsiProvenance::Operation(operation) => {
                bytes.push(0);
                crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
            }
            PsiProvenance::Edge(edge) => {
                bytes.push(1);
                crate::codec_bytes::push_encoded(bytes, edge.get().to_le_bytes());
            }
        }
        crate::codec_bytes::push_encoded(bytes, settlement.units.to_le_bytes());
    }
}
