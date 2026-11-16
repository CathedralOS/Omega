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
    let dynamic_length = cursor.length()?;
    let dynamic_dispatch = terminal_wire::decode_dynamic_dispatch_catalog(
        cursor.take(dynamic_length)?,
    )
    .map_err(|cause| Error::InvalidCodeCatalog {
        catalog: "dynamic dispatch",
        cause,
    })?;
    let callable_length = cursor.length()?;
    let callables =
        terminal_wire::decode_callable_catalog(cursor.take(callable_length)?).map_err(|cause| {
            Error::InvalidCodeCatalog {
                catalog: "callable values",
                cause,
            }
        })?;
    let catalogs = target_vocabulary::NativeCodeCatalogs::seal(dynamic_dispatch, callables)
        .map_err(|cause| Error::InvalidCodeCatalog {
            catalog: "canonical publication",
            cause,
        })?;
    NativePublicationFacts::seal(functions, catalogs).map_err(Error::InvalidProgramContracts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> NativePublicationFacts {
        let operation = OperationId::new(7).unwrap();
        NativePublicationFacts::seal(
            vec![NativeFunctionPublication {
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
            }],
            target_vocabulary::NativeCodeCatalogs::seal(
                Default::default(),
                terminal_psi::TerminalCallableCatalog {
                    machine_envelopes: vec![Some(terminal_psi::TerminalMachineEnvelope {
                        machine: MachineId::new(3).unwrap(),
                        envelope: terminal_psi::TerminalCallableEnvelope {
                            service_reach: Vec::new(),
                            invocations: vec![terminal_psi::TerminalCallableInvocation::Parameter(
                                1,
                            )],
                            suspends: true,
                            blocks: false,
                            termination: terminal_psi::TerminalCallableTermination::NotGuaranteed,
                        },
                    })],
                    ..Default::default()
                },
            )
            .unwrap(),
        )
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
    fn foreign_catalog_bounds_and_wire_errors_keep_their_own_refusals() {
        let mut bytes = Vec::new();
        NativePublicationFacts::default().encode(&mut bytes);
        let dynamic_length = target_vocabulary::publication::NATIVE_PUBLICATION_DOMAIN.len() + 8;
        let mut unbounded = bytes.clone();
        unbounded[dynamic_length..dynamic_length + 8].copy_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(
            decode(&mut Cursor::new(&unbounded)).unwrap_err(),
            Error::Truncated
        );
        let dynamic_payload = dynamic_length + 8;
        bytes[dynamic_payload..dynamic_payload + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            decode(&mut Cursor::new(&bytes)),
            Err(Error::InvalidCodeCatalog {
                catalog: "dynamic dispatch",
                ..
            })
        ));
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
