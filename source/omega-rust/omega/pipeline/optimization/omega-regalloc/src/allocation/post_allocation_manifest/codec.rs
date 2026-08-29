//! Canonical manifest persistence. Decoding yields an unvalidated record.

use omega_optimization_core::{
    PostAllocationOptimizationManifestIdentity, PrePhysicalOptimizationManifestIdentity,
    SelectedLoweringOptimizationCompletionIdentity,
};
use omega_register_model::TargetRegisterEnvironmentIdentity;
use omega_selected_instructions::SelectedInstructionPlanIdentity;
use omega_target::{Architecture, NativeTarget, ObjectFormat};

use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, FixedViewCopyIdentity,
    LiteralFoldIdentity, LiveRangeIdentity, LivenessIdentity, PressureRematerializationIdentity,
    RegisterHomeIdentity,
};

use super::{
    PostAllocationManifestStage, PostAllocationOptimizationManifest,
    PostAllocationOptimizationManifestDecodeError, PostAllocationSelectedTransformation,
    PostAllocationSpillStatus, PostAllocationStatistics, PostAllocationUnavailableData,
};

const POST_ALLOCATION_MANIFEST_MAGIC: &[u8; 8] = b"OMGPAO\0\0";
const POST_ALLOCATION_MANIFEST_VERSION: u32 = 6;

impl PostAllocationOptimizationManifest {
    pub fn encode(&self) -> Vec<u8> {
        let content = encode_manifest_content(self);
        let mut encoded = Vec::with_capacity(44 + content.len());
        encoded.extend_from_slice(POST_ALLOCATION_MANIFEST_MAGIC);
        encoded.extend_from_slice(&POST_ALLOCATION_MANIFEST_VERSION.to_le_bytes());
        encoded.extend_from_slice(&self.identity.bytes());
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, PostAllocationOptimizationManifestDecodeError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(POST_ALLOCATION_MANIFEST_MAGIC.len())? != POST_ALLOCATION_MANIFEST_MAGIC {
            return Err(PostAllocationOptimizationManifestDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != POST_ALLOCATION_MANIFEST_VERSION {
            return Err(PostAllocationOptimizationManifestDecodeError::UnsupportedVersion(version));
        }
        let identity = PostAllocationOptimizationManifestIdentity::from_bytes(cursor.array()?);
        let stage = match cursor.byte()? {
            1 => PostAllocationManifestStage::ValidatedRegisterHomes,
            tag => return Err(PostAllocationOptimizationManifestDecodeError::UnknownStage(tag)),
        };
        let pre_physical = PrePhysicalOptimizationManifestIdentity::from_bytes(cursor.array()?);
        let target = decode_target(&mut cursor)?;
        let selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
        let selected_lowering_completion = match cursor.byte()? {
            0 => None,
            1 => Some(SelectedLoweringOptimizationCompletionIdentity::from_bytes(
                cursor.array()?,
            )),
            tag => {
                return Err(
                    PostAllocationOptimizationManifestDecodeError::UnknownCompletionStatus(tag),
                );
            }
        };
        let transformation_count = cursor.length()?;
        let mut selected_transformations =
            Vec::with_capacity(transformation_count.min(cursor.remaining()));
        for _ in 0..transformation_count {
            selected_transformations.push(match cursor.byte()? {
                1 => PostAllocationSelectedTransformation::FixedViewCopy(
                    FixedViewCopyIdentity::from_bytes(cursor.array()?),
                ),
                2 => PostAllocationSelectedTransformation::LiteralFold(
                    LiteralFoldIdentity::from_bytes(cursor.array()?),
                ),
                3 => PostAllocationSelectedTransformation::PressureRematerialization(
                    PressureRematerializationIdentity::from_bytes(cursor.array()?),
                ),
                tag => {
                    return Err(
                        PostAllocationOptimizationManifestDecodeError::UnknownTransformationTag(
                            tag,
                        ),
                    );
                }
            });
        }
        let liveness = LivenessIdentity::from_bytes(cursor.array()?);
        let ranges = LiveRangeIdentity::from_bytes(cursor.array()?);
        let legality = AllocationLegalityIdentity::from_bytes(cursor.array()?);
        let register_environment = TargetRegisterEnvironmentIdentity::from_bytes(cursor.array()?);
        let allocator_availability = AllocatorAvailabilityIdentity::from_bytes(cursor.array()?);
        let homes = RegisterHomeIdentity::from_bytes(cursor.array()?);
        let spills = match cursor.byte()? {
            1 => PostAllocationSpillStatus::NotRequiredForValidatedHomePlan,
            tag => {
                return Err(PostAllocationOptimizationManifestDecodeError::UnknownSpillStatus(tag));
            }
        };
        let frame = decode_unavailable(&mut cursor)?;
        let emission = decode_unavailable(&mut cursor)?;
        let publication = decode_unavailable(&mut cursor)?;
        let statistics = PostAllocationStatistics {
            functions: u64::from_le_bytes(cursor.array()?),
            structural_unit_functions: u64::from_le_bytes(cursor.array()?),
            assignments: u64::from_le_bytes(cursor.array()?),
            distinct_physical_views: u64::from_le_bytes(cursor.array()?),
            virtual_interferences: u64::from_le_bytes(cursor.array()?),
            fixed_view_transitions: u64::from_le_bytes(cursor.array()?),
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
        PostAllocationManifestStage::ValidatedRegisterHomes => 1,
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
    canonical.extend_from_slice(
        &u64::try_from(manifest.selected_transformations.len())
            .expect("post-allocation transformation length fits u64")
            .to_le_bytes(),
    );
    for transformation in &manifest.selected_transformations {
        match transformation {
            PostAllocationSelectedTransformation::FixedViewCopy(identity) => {
                canonical.push(1);
                canonical.extend_from_slice(&identity.bytes());
            }
            PostAllocationSelectedTransformation::LiteralFold(identity) => {
                canonical.push(2);
                canonical.extend_from_slice(&identity.bytes());
            }
            PostAllocationSelectedTransformation::PressureRematerialization(identity) => {
                canonical.push(3);
                canonical.extend_from_slice(&identity.bytes());
            }
        }
    }
    canonical.extend_from_slice(&manifest.liveness.bytes());
    canonical.extend_from_slice(&manifest.ranges.bytes());
    canonical.extend_from_slice(&manifest.legality.bytes());
    canonical.extend_from_slice(&manifest.register_environment.bytes());
    canonical.extend_from_slice(&manifest.allocator_availability.bytes());
    canonical.extend_from_slice(&manifest.homes.bytes());
    canonical.push(match manifest.spills {
        PostAllocationSpillStatus::NotRequiredForValidatedHomePlan => 1,
    });
    for unavailable in [manifest.frame, manifest.emission, manifest.publication] {
        canonical.push(match unavailable {
            PostAllocationUnavailableData::Unavailable => 1,
        });
    }
    for value in statistics_values(manifest.statistics) {
        canonical.extend_from_slice(&value.to_le_bytes());
    }
    canonical
}

fn statistics_values(statistics: PostAllocationStatistics) -> [u64; 6] {
    [
        statistics.functions,
        statistics.structural_unit_functions,
        statistics.assignments,
        statistics.distinct_physical_views,
        statistics.virtual_interferences,
        statistics.fixed_view_transitions,
    ]
}

fn encode_target(bytes: &mut Vec<u8>, target: NativeTarget) {
    bytes.push(match target.architecture {
        Architecture::Aarch64 => 1,
        Architecture::X86_64 => 2,
    });
    bytes.push(match target.object_format {
        ObjectFormat::Elf => 1,
        ObjectFormat::MachO => 2,
        ObjectFormat::Coff => 3,
    });
    bytes.extend_from_slice(
        &u64::try_from(target.pointer_size)
            .expect("target pointer size fits u64")
            .to_le_bytes(),
    );
    bytes.extend_from_slice(
        &u64::try_from(target.pointer_alignment)
            .expect("target pointer alignment fits u64")
            .to_le_bytes(),
    );
}

fn decode_target(
    cursor: &mut Cursor<'_>,
) -> Result<NativeTarget, PostAllocationOptimizationManifestDecodeError> {
    let architecture = match cursor.byte()? {
        1 => Architecture::Aarch64,
        2 => Architecture::X86_64,
        tag => {
            return Err(PostAllocationOptimizationManifestDecodeError::UnknownArchitecture(tag));
        }
    };
    let object_format = match cursor.byte()? {
        1 => ObjectFormat::Elf,
        2 => ObjectFormat::MachO,
        3 => ObjectFormat::Coff,
        tag => {
            return Err(PostAllocationOptimizationManifestDecodeError::UnknownObjectFormat(tag));
        }
    };
    let pointer_size = usize::try_from(u64::from_le_bytes(cursor.array()?))
        .map_err(|_| PostAllocationOptimizationManifestDecodeError::TargetLayoutOverflow)?;
    let pointer_alignment = usize::try_from(u64::from_le_bytes(cursor.array()?))
        .map_err(|_| PostAllocationOptimizationManifestDecodeError::TargetLayoutOverflow)?;
    Ok(NativeTarget {
        architecture,
        object_format,
        pointer_size,
        pointer_alignment,
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

struct Cursor<'encoded> {
    encoded: &'encoded [u8],
    offset: usize,
}

impl<'encoded> Cursor<'encoded> {
    const fn new(encoded: &'encoded [u8]) -> Self {
        Self { encoded, offset: 0 }
    }

    fn take(
        &mut self,
        length: usize,
    ) -> Result<&'encoded [u8], PostAllocationOptimizationManifestDecodeError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(PostAllocationOptimizationManifestDecodeError::Truncated)?;
        let value = self
            .encoded
            .get(self.offset..end)
            .ok_or(PostAllocationOptimizationManifestDecodeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], PostAllocationOptimizationManifestDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| PostAllocationOptimizationManifestDecodeError::Truncated)
    }

    fn byte(&mut self) -> Result<u8, PostAllocationOptimizationManifestDecodeError> {
        Ok(self.array::<1>()?[0])
    }

    fn length(&mut self) -> Result<usize, PostAllocationOptimizationManifestDecodeError> {
        usize::try_from(u64::from_le_bytes(self.array()?))
            .map_err(|_| PostAllocationOptimizationManifestDecodeError::LengthOverflow)
    }

    fn remaining(&self) -> usize {
        self.encoded.len() - self.offset
    }
}
