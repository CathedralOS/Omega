//! Foreign decoding for the current source-site and literal contract rows.

use std::sync::Arc;

use native_vocabulary::provenance::PsiProvenance;
use semantic_vocabulary::{EdgeId, MachineId, OperationId, PlaceId};
use target_vocabulary::{NativeByteLiteral, NativeFunctionPublication, NativePublicationFacts};

use super::fixed_view_copy_codec::{Cursor, decode_id};
use super::program_contracts::decode_rows;
use crate::FixedViewCopyDecodeError as Error;

pub(super) fn decode(cursor: &mut Cursor<'_>) -> Result<NativePublicationFacts, Error> {
    let domain = target_vocabulary::publication::NATIVE_PUBLICATION_DOMAIN;
    if cursor.take(domain.len())? != domain {
        return Err(Error::InvalidProgramContracts(
            "publication facts have another wire domain",
        ));
    }
    // Machine ID, operation count, site count and literal count.
    let functions = decode_rows(cursor, 28, |cursor| {
        let machine = decode_id(cursor, MachineId::new)?;
        let operation_count = cursor.u32()?;
        // A site tag, semantic ID and original operation ordinal.
        let sites = decode_rows(cursor, 13, |cursor| {
            let site = match cursor.byte()? {
                0 => PsiProvenance::Operation(decode_id(cursor, OperationId::new)?),
                1 => PsiProvenance::Edge(decode_id(cursor, EdgeId::new)?),
                _ => {
                    return Err(Error::InvalidProgramContracts(
                        "publication site has another kind",
                    ));
                }
            };
            Ok((site, cursor.u32()?))
        })?;
        // Operation ID, place ID and the byte count, before the payload.
        let byte_literals = decode_rows(cursor, 24, |cursor| {
            let operation = decode_id(cursor, OperationId::new)?;
            let place = decode_id(cursor, PlaceId::new)?;
            let length = cursor.length()?;
            let bytes = Arc::from(cursor.take(length)?);
            Ok(NativeByteLiteral {
                operation,
                place,
                bytes,
            })
        })?;
        Ok(NativeFunctionPublication {
            machine,
            operation_count,
            sites,
            byte_literals,
        })
    })?;
    NativePublicationFacts::seal(functions).map_err(Error::InvalidProgramContracts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> NativePublicationFacts {
        let operation = OperationId::new(7).unwrap();
        NativePublicationFacts::seal(vec![NativeFunctionPublication {
            machine: MachineId::new(3).unwrap(),
            operation_count: 2,
            sites: vec![
                (PsiProvenance::Operation(operation), 0),
                (PsiProvenance::Edge(EdgeId::new(9).unwrap()), 1),
            ],
            byte_literals: vec![NativeByteLiteral {
                operation,
                place: PlaceId::new(4).unwrap(),
                bytes: Arc::from(b"quoted\0bytes".as_slice()),
            }],
        }])
        .unwrap()
    }

    #[test]
    fn foreign_publication_keeps_nonempty_sites_and_quoted_octets() {
        let original = facts();
        let mut bytes = Vec::new();
        original.encode(&mut bytes);
        assert_eq!(decode(&mut Cursor::new(&bytes)).unwrap(), original);
        for prefix in 0..bytes.len() {
            assert!(decode(&mut Cursor::new(&bytes[..prefix])).is_err());
        }
    }

    #[test]
    fn foreign_publication_refuses_unknown_sites_and_unbounded_counts() {
        let mut bytes = Vec::new();
        facts().encode(&mut bytes);
        let function_count = target_vocabulary::publication::NATIVE_PUBLICATION_DOMAIN.len();
        let mut huge = bytes.clone();
        huge[function_count..function_count + 8].copy_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(
            decode(&mut Cursor::new(&huge)).unwrap_err(),
            Error::Truncated
        );
        let first_site = function_count + 8 + 8 + 4 + 8;
        bytes[first_site] = 255;
        assert_eq!(
            decode(&mut Cursor::new(&bytes)).unwrap_err(),
            Error::InvalidProgramContracts("publication site has another kind")
        );
    }
}
