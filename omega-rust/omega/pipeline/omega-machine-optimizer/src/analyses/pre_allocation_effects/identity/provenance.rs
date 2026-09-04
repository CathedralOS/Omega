use omega_optimization_unit::PsiProvenance;
use omega_selected_instructions::SelectedInstructionProvenance;

use super::values::{encode_ids, encode_len};

pub(crate) fn encode_provenance(bytes: &mut Vec<u8>, provenance: &SelectedInstructionProvenance) {
    encode_ids(bytes, provenance.operations.iter().map(|id| id.get()));
    encode_ids(bytes, provenance.values.iter().map(|id| id.get()));
    encode_ids(bytes, provenance.edges.iter().map(|id| id.get()));
    encode_ids(bytes, provenance.obligations.iter().map(|id| id.get()));
    encode_len(bytes, provenance.fuel.len());
    for settlement in &provenance.fuel {
        match settlement.site {
            PsiProvenance::Operation(operation) => {
                bytes.push(0);
                bytes.extend_from_slice(&operation.get().to_le_bytes());
            }
            PsiProvenance::Edge(edge) => {
                bytes.push(1);
                bytes.extend_from_slice(&edge.get().to_le_bytes());
            }
        }
        bytes.extend_from_slice(&settlement.units.to_le_bytes());
    }
}
