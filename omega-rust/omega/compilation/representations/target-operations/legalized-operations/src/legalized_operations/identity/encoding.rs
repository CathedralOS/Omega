//! Byte-encoding primitives every identity encoder composes: lengths,
//! identifier rosters, optional identifiers, the native target, and fuel
//! settlements. Each encoder owns what it encodes; this module owns only how
//! the shared scalar shapes are laid down.

use optimization_unit::{FuelSettlement, PsiProvenance};

pub fn encode_fuel(bytes: &mut Vec<u8>, fuel: &[FuelSettlement]) {
    encode_len(bytes, fuel.len());
    for settlement in fuel {
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


pub(super) fn encode_ids(bytes: &mut Vec<u8>, values: impl IntoIterator<Item = u64>) {
    let values = values.into_iter().collect::<Vec<_>>();
    encode_len(bytes, values.len());
    for value in values {
        crate::codec_bytes::push_encoded(bytes, value.to_le_bytes());
    }
}

pub(super) fn encode_len(bytes: &mut Vec<u8>, len: usize) {
    crate::codec_bytes::push_encoded(bytes, (len as u64).to_le_bytes());
}
