//! Canonical format-32 codec for retained completion claim sources.
//!
//! This module owns only the tagged source row. The installation parent keeps
//! settlement ordering and count fields, so extraction cannot reorder bytes or
//! public validation errors.

use omega_terminal_target_operations::TerminalCompletionClaimSource;
use psi_core::{
    ClaimId, ContentAlgebra, ContentAlgebraKind, ContentDomainId, ContentPlaceSegment,
    ContentPlaceVersion, ContentProjectionIdentity, ContentStructuralPlace, PlaceId,
};
use psi_terminal::{ClaimContentProjection, ContentEntryClaim, EntryClaim, StructuralArgument};

use super::{
    Reader, TerminalInstallationError, decode_identity, encode_identity, push_u32, push_u64,
    structural_argument_codec::{decode_structural_argument, encode_structural_argument},
};

pub(super) fn encode_completion_claim_source(
    bytes: &mut Vec<u8>,
    source: &TerminalCompletionClaimSource,
) -> Result<(), TerminalInstallationError> {
    bytes.push(u8::from(source.entry.is_some()) | (u8::from(source.content.is_some()) << 1));
    bytes.extend_from_slice(&[0; 3]);
    push_u64(bytes, source.claim.get());
    if let Some(entry) = &source.entry {
        encode_structural_argument(
            bytes,
            &StructuralArgument {
                place: entry.input,
                path: entry.path.clone(),
            },
        )?;
    }
    if let Some(content) = &source.content {
        bytes.push(match content.input.version {
            ContentPlaceVersion::Entry => 1,
            ContentPlaceVersion::Current => 2,
        });
        bytes.extend_from_slice(&[0; 3]);
        push_u64(bytes, content.input.root.get());
        push_u32(
            bytes,
            u32::try_from(content.input.segments.len()).map_err(|_| {
                TerminalInstallationError::CountNotRepresentable(
                    "completion content subject segments",
                )
            })?,
        );
        for segment in &content.input.segments {
            match segment {
                ContentPlaceSegment::Case(identity) => {
                    bytes.push(1);
                    bytes.extend_from_slice(&[0; 3]);
                    encode_identity(bytes, identity)?;
                }
                ContentPlaceSegment::Field(identity) => {
                    bytes.push(2);
                    bytes.extend_from_slice(&[0; 3]);
                    encode_identity(bytes, identity)?;
                }
                ContentPlaceSegment::FixedIndex(index) => {
                    bytes.push(3);
                    bytes.extend_from_slice(&[0; 3]);
                    push_u64(bytes, *index);
                }
            }
        }
        push_u32(
            bytes,
            u32::try_from(content.projections.len()).map_err(|_| {
                TerminalInstallationError::CountNotRepresentable("completion content projections")
            })?,
        );
        for projection in &content.projections {
            push_u64(bytes, projection.projection.domain.get());
            push_u64(bytes, projection.projection.projection_fingerprint);
            bytes.push(match projection.algebra.kind {
                ContentAlgebraKind::IntervalSet => 1,
                ContentAlgebraKind::CountedQuantity => 2,
            });
            bytes.extend_from_slice(&[0; 3]);
            encode_identity(bytes, &projection.algebra.parameter)?;
        }
    }
    Ok(())
}

pub(super) fn decode_completion_claim_source(
    reader: &mut Reader<'_>,
) -> Result<TerminalCompletionClaimSource, TerminalInstallationError> {
    let tag = reader.u8()?;
    if reader.take(3)? != [0; 3] {
        return Err(TerminalInstallationError::NonzeroReservedField);
    }
    let claim = ClaimId::new(reader.u64()?)
        .ok_or(TerminalInstallationError::ZeroSettlementIdentity("ClaimId"))?;
    if tag == 0 || tag & !3 != 0 {
        return Err(TerminalInstallationError::InvalidCompletionClaimSource);
    }
    let entry = if tag & 1 != 0 {
        let argument = decode_structural_argument(reader)?;
        Some(EntryClaim {
            claim,
            input: argument.place,
            path: argument.path,
        })
    } else {
        None
    };
    let content = if tag & 2 != 0 {
        let version = match reader.u8()? {
            1 => ContentPlaceVersion::Entry,
            2 => ContentPlaceVersion::Current,
            _ => return Err(TerminalInstallationError::InvalidCompletionClaimSource),
        };
        if reader.take(3)? != [0; 3] {
            return Err(TerminalInstallationError::NonzeroReservedField);
        }
        let root = PlaceId::new(reader.u64()?)
            .ok_or(TerminalInstallationError::ZeroSettlementIdentity("PlaceId"))?;
        let segment_count = usize::try_from(reader.u32()?).map_err(|_| {
            TerminalInstallationError::CountNotRepresentable("completion content subject segments")
        })?;
        if segment_count > reader.remaining() / 8 {
            return Err(TerminalInstallationError::UnexpectedEnd);
        }
        let mut segments = Vec::with_capacity(segment_count);
        for _ in 0..segment_count {
            let segment_tag = reader.u8()?;
            if reader.take(3)? != [0; 3] {
                return Err(TerminalInstallationError::NonzeroReservedField);
            }
            segments.push(match segment_tag {
                1 => ContentPlaceSegment::Case(decode_identity(reader)?),
                2 => ContentPlaceSegment::Field(decode_identity(reader)?),
                3 => ContentPlaceSegment::FixedIndex(reader.u64()?),
                _ => return Err(TerminalInstallationError::InvalidCompletionClaimSource),
            });
        }
        let projection_count = usize::try_from(reader.u32()?).map_err(|_| {
            TerminalInstallationError::CountNotRepresentable("completion content projections")
        })?;
        if projection_count > reader.remaining() / 24 {
            return Err(TerminalInstallationError::UnexpectedEnd);
        }
        let mut projections = Vec::with_capacity(projection_count);
        for _ in 0..projection_count {
            let domain = ContentDomainId::new(reader.u64()?).ok_or(
                TerminalInstallationError::ZeroSettlementIdentity("ContentDomainId"),
            )?;
            let projection_fingerprint = reader.u64()?;
            let kind = match reader.u8()? {
                1 => ContentAlgebraKind::IntervalSet,
                2 => ContentAlgebraKind::CountedQuantity,
                _ => return Err(TerminalInstallationError::InvalidCompletionClaimSource),
            };
            if reader.take(3)? != [0; 3] {
                return Err(TerminalInstallationError::NonzeroReservedField);
            }
            projections.push(ClaimContentProjection {
                projection: ContentProjectionIdentity {
                    domain,
                    projection_fingerprint,
                },
                algebra: ContentAlgebra {
                    kind,
                    parameter: decode_identity(reader)?,
                },
            });
        }
        Some(ContentEntryClaim {
            claim,
            input: ContentStructuralPlace {
                version,
                root,
                segments,
            },
            projections,
        })
    } else {
        None
    };
    Ok(TerminalCompletionClaimSource {
        claim,
        entry,
        content,
    })
}
