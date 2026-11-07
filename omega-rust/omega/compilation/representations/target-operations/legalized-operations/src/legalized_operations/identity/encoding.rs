//! Byte-encoding primitives every identity encoder composes: lengths,
//! identifier rosters, optional identifiers, the native target, and work
//! settlements. Each encoder owns what it encodes; this module owns only how
//! the shared scalar shapes are laid down.

use optimization_unit::{PsiProvenance, WorkSettlement};

pub fn encode_work(bytes: &mut Vec<u8>, work: &[WorkSettlement]) {
    encode_len(bytes, work.len());
    for settlement in work {
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

/// Tagged optional payload: one `0` discriminant for absence, or `1` followed
/// by the payload its encoder lays down. Every optional field these encoders
/// write uses this one frame, so an absent row and an empty payload never share
/// bytes.
pub(super) fn encode_option<T>(
    bytes: &mut Vec<u8>,
    value: Option<T>,
    encode: impl FnOnce(&mut Vec<u8>, T),
) {
    match value {
        Some(value) => {
            bytes.push(1);
            encode(bytes, value);
        }
        None => bytes.push(0),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn work_settlements_preserve_numeric_wire_encoding() {
        use super::{PsiProvenance, WorkSettlement, encode_work};
        use semantic_vocabulary::{EdgeId, OperationId};
        let mut bytes = Vec::new();
        encode_work(
            &mut bytes,
            &[
                WorkSettlement {
                    site: PsiProvenance::Operation(OperationId::new(7).unwrap()),
                    units: 3,
                },
                WorkSettlement {
                    site: PsiProvenance::Edge(EdgeId::new(9).unwrap()),
                    units: 5,
                },
            ],
        );
        // Length, operation/edge tags, site IDs and units retain their original bytes.
        assert_eq!(
            bytes,
            [
                2, 0, 0, 0, 0, 0, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 1, 9, 0,
                0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0,
            ]
        );
    }
}
