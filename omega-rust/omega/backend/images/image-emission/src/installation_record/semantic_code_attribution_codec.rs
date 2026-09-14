//! Canonical codec for semantic-code attribution rows.
//!
//! The installation parent retains upfront count conversion, row ordering,
//! canonicality. This child owns exact collection bytes.

use machine_code::{SemanticCodeAttribution, SemanticCodeSite};
use semantic_vocabulary::{EdgeId, MachineId, OperationId};

use super::{InstallationError, ObjectCodeAttribution, Reader, push_u32, push_u64};

pub(super) fn encode_semantic_code_attributions(
    bytes: &mut Vec<u8>,
    count: u32,
    installed: &[ObjectCodeAttribution],
) -> Result<(), InstallationError> {
    push_u32(bytes, count);
    for installed in installed {
        let attribution = &installed.attribution;
        push_u64(bytes, installed.machine.get());
        match attribution.site {
            SemanticCodeSite::Operation(operation) => {
                bytes.push(1);
                bytes.extend_from_slice(&[0; 3]);
                push_u64(bytes, operation.get());
            }
            SemanticCodeSite::Edge(edge) => {
                bytes.push(2);
                bytes.extend_from_slice(&[0; 3]);
                push_u64(bytes, edge.get());
            }
        }
        push_u64(
            bytes,
            u64::try_from(attribution.operation_ordinal)
                .map_err(|_| InstallationError::SemanticCodeAttributionOffsetNotRepresentable)?,
        );
        push_u64(
            bytes,
            u64::try_from(installed.text_offset)
                .map_err(|_| InstallationError::SemanticCodeAttributionOffsetNotRepresentable)?,
        );
        push_u64(
            bytes,
            u64::try_from(attribution.code_offset)
                .map_err(|_| InstallationError::SemanticCodeAttributionOffsetNotRepresentable)?,
        );
        push_u64(
            bytes,
            u64::try_from(attribution.byte_count)
                .map_err(|_| InstallationError::SemanticCodeAttributionOffsetNotRepresentable)?,
        );
    }
    Ok(())
}

pub(super) fn decode_semantic_code_attributions(
    reader: &mut Reader<'_>,
) -> Result<Vec<ObjectCodeAttribution>, InstallationError> {
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManySemanticCodeAttributions)?;
    if count > reader.remaining() / 52 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut semantic_code_attribution = Vec::with_capacity(count);
    for _ in 0..count {
        let machine = MachineId::new(reader.u64()?).ok_or(
            InstallationError::ZeroSemanticCodeAttributionIdentity("MachineId"),
        )?;
        let site_tag = reader.u8()?;
        if reader.take(3)? != [0; 3] {
            return Err(InstallationError::NonzeroReservedField);
        }
        let site_identity = reader.u64()?;
        let site = match site_tag {
            1 => SemanticCodeSite::Operation(OperationId::new(site_identity).ok_or(
                InstallationError::ZeroSemanticCodeAttributionIdentity("OperationId"),
            )?),
            2 => SemanticCodeSite::Edge(EdgeId::new(site_identity).ok_or(
                InstallationError::ZeroSemanticCodeAttributionIdentity("EdgeId"),
            )?),
            _ => return Err(InstallationError::InvalidSemanticCodeSiteTag(site_tag)),
        };
        let operation_ordinal = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::SemanticCodeAttributionOffsetNotRepresentable)?;
        let text_offset = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::SemanticCodeAttributionOffsetNotRepresentable)?;
        let code_offset = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::SemanticCodeAttributionOffsetNotRepresentable)?;
        let byte_count = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::SemanticCodeAttributionOffsetNotRepresentable)?;
        semantic_code_attribution.push(ObjectCodeAttribution {
            machine,
            attribution: SemanticCodeAttribution {
                site,
                operation_ordinal,
                code_offset,
                byte_count,
            },
            text_offset,
        });
    }
    Ok(semantic_code_attribution)
}
