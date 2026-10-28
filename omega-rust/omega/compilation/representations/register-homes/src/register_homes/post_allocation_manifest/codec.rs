//! Canonical manifest persistence. Decoding yields an unvalidated record.

use crate::codec_bytes::Envelope;
use crate::cursor::Cursor;
use optimization_core::{
    PostAllocationOptimizationManifestIdentity, PreAllocationOptimizationCompletionIdentity,
    PrePhysicalOptimizationManifestIdentity, SelectedLoweringOptimizationCompletionIdentity,
};
use register_model::TargetRegisterEnvironmentIdentity;
use selected_instructions::{LiveRangeIdentity, LivenessIdentity, SelectedInstructionPlanIdentity};
use target::{NativeTarget, TargetWireIdentityError};

use crate::{AllocationLegalityIdentity, AllocatorAvailabilityIdentity, RegisterHomeIdentity};

use super::{
    PostAllocationManifestStage, PostAllocationOptimizationManifest,
    PostAllocationOptimizationManifestDecodeError, PostAllocationSelectedTransformation,
    PostAllocationSpillStatus, PostAllocationStatistics, PostAllocationUnavailableData,
};

const POST_ALLOCATION_MANIFEST_ENVELOPE: Envelope = Envelope::new(b"OMGPAO\0\0", 16);

impl PostAllocationOptimizationManifest {
    pub fn encode(&self) -> Vec<u8> {
        let content = encode_manifest_content(self);
        POST_ALLOCATION_MANIFEST_ENVELOPE.frame(self.identity.bytes(), &content)
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, PostAllocationOptimizationManifestDecodeError> {
        let (mut cursor, identity) = POST_ALLOCATION_MANIFEST_ENVELOPE.open(
            encoded,
            || PostAllocationOptimizationManifestDecodeError::WrongMagic,
            PostAllocationOptimizationManifestDecodeError::UnsupportedVersion,
        )?;
        let identity = PostAllocationOptimizationManifestIdentity::from_bytes(identity);
        let stage = match cursor.byte()? {
            1 => PostAllocationManifestStage::RegisterHomes,
            tag => {
                return Err(PostAllocationOptimizationManifestDecodeError::UnknownStage(
                    tag,
                ));
            }
        };
        let pre_physical = PrePhysicalOptimizationManifestIdentity::from_bytes(cursor.array()?);
        let target = decode_target(&mut cursor)?;
        let selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
        let selected_lowering_completion = crate::cursor::optional(
            &mut cursor,
            |cursor| {
                Ok(SelectedLoweringOptimizationCompletionIdentity::from_bytes(
                    cursor.array()?,
                ))
            },
            PostAllocationOptimizationManifestDecodeError::UnknownCompletionStatus,
        )?;
        let pre_allocation_completion = crate::cursor::optional(
            &mut cursor,
            |cursor| {
                Ok(PreAllocationOptimizationCompletionIdentity::from_bytes(
                    cursor.array()?,
                ))
            },
            PostAllocationOptimizationManifestDecodeError::UnknownCompletionStatus,
        )?;
        let selected_transformations = crate::codec_bytes::decode_list(
            &mut cursor,
            PostAllocationSelectedTransformation::decode,
        )?;
        let liveness = LivenessIdentity::from_bytes(cursor.array()?);
        let ranges = LiveRangeIdentity::from_bytes(cursor.array()?);
        let legality = AllocationLegalityIdentity::from_bytes(cursor.array()?);
        let register_environment = TargetRegisterEnvironmentIdentity::from_bytes(cursor.array()?);
        let allocator_availability = AllocatorAvailabilityIdentity::from_bytes(cursor.array()?);
        let homes = RegisterHomeIdentity::from_bytes(cursor.array()?);
        let spills = match cursor.byte()? {
            1 => PostAllocationSpillStatus::NotRequiredForValidatedHomePlan,
            2 => PostAllocationSpillStatus::RealizedInSelectedProgram,
            tag => {
                return Err(PostAllocationOptimizationManifestDecodeError::UnknownSpillStatus(tag));
            }
        };
        let frame = decode_unavailable(&mut cursor)?;
        let emission = decode_unavailable(&mut cursor)?;
        let publication = decode_unavailable(&mut cursor)?;
        let statistics = PostAllocationStatistics {
            functions: cursor.u64()?,
            assignments: cursor.u64()?,
            distinct_physical_views: cursor.u64()?,
            virtual_interferences: cursor.u64()?,
            fixed_view_transitions: cursor.u64()?,
        };
        if cursor.remaining() != 0 {
            return Err(PostAllocationOptimizationManifestDecodeError::TrailingBytes);
        }
        let manifest = Self {
            identity,
            stage,
            pre_physical,
            target,
            selected,
            selected_lowering_completion,
            pre_allocation_completion,
            selected_transformations,
            liveness,
            ranges,
            legality,
            register_environment,
            allocator_availability,
            homes,
            spills,
            frame,
            emission,
            publication,
            statistics,
        };
        if manifest.identity != manifest.recomputed_identity() {
            return Err(PostAllocationOptimizationManifestDecodeError::IdentityMismatch);
        }
        Ok(manifest)
    }
}

pub(super) fn encode_manifest_content(manifest: &PostAllocationOptimizationManifest) -> Vec<u8> {
    let mut canonical = Vec::new();
    canonical.push(match manifest.stage {
        PostAllocationManifestStage::RegisterHomes => 1,
    });
    canonical.extend_from_slice(&manifest.pre_physical.bytes());
    encode_target(&mut canonical, manifest.target);
    canonical.extend_from_slice(&manifest.selected.bytes());
    match manifest.selected_lowering_completion {
        Some(identity) => {
            canonical.push(1);
            canonical.extend_from_slice(&identity.bytes());
        }
        None => canonical.push(0),
    }
    match manifest.pre_allocation_completion {
        Some(identity) => {
            canonical.push(1);
            canonical.extend_from_slice(&identity.bytes());
        }
        None => canonical.push(0),
    }
    canonical.extend_from_slice(
        &u64::try_from(manifest.selected_transformations.len())
            .expect("post-allocation transformation length fits u64")
            .to_le_bytes(),
    );
    for transformation in &manifest.selected_transformations {
        canonical.push(transformation.wire_tag());
        canonical.extend_from_slice(&transformation.identity_bytes());
    }
    canonical.extend_from_slice(&manifest.liveness.bytes());
    canonical.extend_from_slice(&manifest.ranges.bytes());
    canonical.extend_from_slice(&manifest.legality.bytes());
    canonical.extend_from_slice(&manifest.register_environment.bytes());
    canonical.extend_from_slice(&manifest.allocator_availability.bytes());
    canonical.extend_from_slice(&manifest.homes.bytes());
    canonical.push(match manifest.spills {
        PostAllocationSpillStatus::NotRequiredForValidatedHomePlan => 1,
        PostAllocationSpillStatus::RealizedInSelectedProgram => 2,
    });
    for unavailable in [manifest.frame, manifest.emission, manifest.publication] {
        canonical.push(match unavailable {
            PostAllocationUnavailableData::Unavailable => 1,
        });
    }
    for value in statistics_values(manifest.statistics) {
        crate::codec_bytes::push_encoded(&mut canonical, value.to_le_bytes());
    }
    canonical
}

fn statistics_values(statistics: PostAllocationStatistics) -> [u64; 5] {
    [
        statistics.functions,
        statistics.assignments,
        statistics.distinct_physical_views,
        statistics.virtual_interferences,
        statistics.fixed_view_transitions,
    ]
}

fn encode_target(bytes: &mut Vec<u8>, target: NativeTarget) {
    bytes.extend_from_slice(&target.wire_identity_bytes());
}

fn decode_target(
    cursor: &mut Cursor<'_>,
) -> Result<NativeTarget, PostAllocationOptimizationManifestDecodeError> {
    let encoded = cursor.array::<18>()?;
    NativeTarget::from_wire_identity_bytes(&encoded).map_err(|error| match error {
        TargetWireIdentityError::UnknownArchitecture(tag) => {
            PostAllocationOptimizationManifestDecodeError::UnknownArchitecture(tag)
        }
        TargetWireIdentityError::UnknownObjectFormat(tag) => {
            PostAllocationOptimizationManifestDecodeError::UnknownObjectFormat(tag)
        }
        TargetWireIdentityError::LayoutOverflow => {
            PostAllocationOptimizationManifestDecodeError::TargetLayoutOverflow
        }
    })
}

fn decode_unavailable(
    cursor: &mut Cursor<'_>,
) -> Result<PostAllocationUnavailableData, PostAllocationOptimizationManifestDecodeError> {
    match cursor.byte()? {
        1 => Ok(PostAllocationUnavailableData::Unavailable),
        tag => Err(PostAllocationOptimizationManifestDecodeError::UnknownUnavailableStatus(tag)),
    }
}
