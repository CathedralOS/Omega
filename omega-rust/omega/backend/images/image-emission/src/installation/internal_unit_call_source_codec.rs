//! Retained semantic origin for installed internal Unit calls.
use super::completion_custody_codec::{
    decode_completion_claim_source, encode_completion_claim_source,
};
use super::{InstallationError, Reader, push_u32, push_u64};
use machine_code::InternalUnitCallSource;
use semantic_vocabulary::{BoundaryMachineId, ClaimId};
use terminal_psi::CompletionReceipt;

pub(super) fn encode(
    bytes: &mut Vec<u8>,
    source: &InternalUnitCallSource,
) -> Result<(), InstallationError> {
    match source {
        InternalUnitCallSource::Authored => bytes.push(1),
        InternalUnitCallSource::InstalledProvider {
            boundary,
            provider,
            completion_claim_sources,
            completion_receipts,
        } => {
            bytes.push(2);
            push_u64(bytes, boundary.get());
            let encoded = terminal_codec::encode_provider_candidate_record(provider)
                .map_err(InstallationError::InvalidProviderCandidateRecord)?;
            count(bytes, encoded.len())?;
            bytes.extend_from_slice(&encoded);
            count(bytes, completion_claim_sources.len())?;
            for source in completion_claim_sources {
                encode_completion_claim_source(bytes, source)?;
            }
            count(bytes, completion_receipts.len())?;
            for receipt in completion_receipts {
                push_u64(bytes, receipt.claim.get());
                push_u32(bytes, receipt.argument_index);
            }
        }
    }
    Ok(())
}
pub(super) fn decode(reader: &mut Reader<'_>) -> Result<InternalUnitCallSource, InstallationError> {
    match reader.u8()? {
        1 => Ok(InternalUnitCallSource::Authored),
        2 => {
            let boundary = BoundaryMachineId::new(reader.u64()?)
                .ok_or(InstallationError::ZeroInternalUnitCallIdentity)?;
            let len = reader.u32()? as usize;
            let provider = terminal_codec::decode_provider_candidate_record(reader.take(len)?)
                .map_err(InstallationError::InvalidProviderCandidateRecord)?;
            let source_count = reader.u32()?;
            let mut completion_claim_sources = Vec::new();
            for _ in 0..source_count {
                completion_claim_sources.push(decode_completion_claim_source(reader)?);
            }
            let receipt_count = reader.u32()?;
            let mut completion_receipts = Vec::new();
            for _ in 0..receipt_count {
                completion_receipts.push(CompletionReceipt {
                    claim: ClaimId::new(reader.u64()?)
                        .ok_or(InstallationError::ZeroInternalUnitCallIdentity)?,
                    argument_index: reader.u32()?,
                });
            }
            Ok(InternalUnitCallSource::InstalledProvider {
                boundary,
                provider: Box::new(provider),
                completion_claim_sources,
                completion_receipts,
            })
        }
        tag => Err(InstallationError::InvalidInternalUnitCallSourceTag(tag)),
    }
}
fn count(bytes: &mut Vec<u8>, len: usize) -> Result<(), InstallationError> {
    push_u32(
        bytes,
        u32::try_from(len)
            .map_err(|_| InstallationError::CountNotRepresentable("internal call source"))?,
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;

    fn provider_source() -> InternalUnitCallSource {
        let boundary = BoundaryMachineId::new(3).unwrap();
        InternalUnitCallSource::InstalledProvider {
            boundary,
            provider: Box::new(terminal_psi::ProviderCandidateConformance {
                boundary,
                requirement_identity: "requirement".into(),
                provider_identity: "provider".into(),
                candidate_identity: "candidate".into(),
                candidate: semantic_vocabulary::MachineId::new(4).unwrap(),
                signature: terminal_psi::ProviderSignature {
                    parameters: Vec::new(),
                },
                refinement: terminal_psi::ProviderRefinement {
                    positional_parameters: Vec::new(),
                    required_domains: Vec::new(),
                    realized_service_ceiling: Vec::new(),
                },
            }),
            completion_claim_sources: Vec::new(),
            completion_receipts: Vec::new(),
        }
    }

    #[test]
    fn internal_call_origins_round_trip_without_erasing_provider_identity() {
        for source in [InternalUnitCallSource::Authored, provider_source()] {
            let mut bytes = Vec::new();
            encode(&mut bytes, &source).unwrap();
            let mut reader = Reader::new(&bytes);
            assert_eq!(decode(&mut reader).unwrap(), source);
            assert_eq!(reader.remaining(), 0);
        }
    }

    #[test]
    fn internal_call_origin_rejects_unknown_role_and_stale_nested_codec() {
        assert_eq!(
            decode(&mut Reader::new(&[9])),
            Err(InstallationError::InvalidInternalUnitCallSourceTag(9))
        );
        let mut bytes = Vec::new();
        encode(&mut bytes, &provider_source()).unwrap();
        // Role, boundary, and byte count precede the Terminal-owned format marker.
        bytes[13..15].copy_from_slice(&0u16.to_le_bytes());
        assert_eq!(
            decode(&mut Reader::new(&bytes)),
            Err(InstallationError::InvalidProviderCandidateRecord(
                terminal_codec::CodecError::UnsupportedFormatMarker(0)
            ))
        );
    }
}
