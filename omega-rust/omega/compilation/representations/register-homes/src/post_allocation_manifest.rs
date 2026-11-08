/*
We define the durable post-allocation optimization manifest
(`PostAllocationOptimizationManifest`, framed under `OMGPAO\0\0` version 16),
its append-only selected-transformation ledger
(`PostAllocationSelectedTransformation`), canonical SHA-256 identity
recomputation, wire persistence, and human-readable text rendering.

The selected-transformation roster binds each transformation variant to one
append-only wire tag, its 32-byte artifact identity, and its rendered kind so
that wire encoding, identity hashing, and diagnostic text never drift apart.
*/

use std::fmt::Write;

use optimization_core::{
    PostAllocationOptimizationManifestIdentity, PreAllocationOptimizationCompletionIdentity,
    PrePhysicalOptimizationManifestIdentity, SelectedLoweringOptimizationCompletionIdentity, hex,
};
use register_model::TargetRegisterEnvironmentIdentity;
use selected_instructions::{
    AddressFoldIdentity, ConstantBooleanIdentity, ConstantBranchIdentity,
    CopiedCallOperandIdentity, CopyRemovalIdentity, DeadCompareIdentity, EquivalentCompareIdentity,
    FixedViewCopyIdentity, LiteralFoldIdentity, LiveRangeIdentity, LivenessIdentity,
    PressureRematerializationIdentity, RedundantCompareIdentity, RedundantExtensionIdentity,
    SelectedInstructionPlanIdentity, StoredLoadForwardingIdentity,
};
use target::{NativeTarget, TargetWireIdentityError};

use crate::codec_bytes::{Envelope, decode_list, push_encoded};
use crate::cursor::Cursor;
use crate::{AllocationLegalityIdentity, AllocatorAvailabilityIdentity, RegisterHomeIdentity};

const POST_ALLOCATION_MANIFEST_ENVELOPE: Envelope = Envelope::new(b"OMGPAO\0\0", 16);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostAllocationManifestStage {
    RegisterHomes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostAllocationSpillStatus {
    NotRequiredForValidatedHomePlan,
    RealizedInSelectedProgram,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostAllocationUnavailableData {
    Unavailable,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PostAllocationStatistics {
    pub functions: u64,               // Ordinary selected functions
    pub assignments: u64,             // Virtual-register home assignments
    pub distinct_physical_views: u64, // Distinct physical views assigned
    pub virtual_interferences: u64,   // Virtual-register interferences
    pub fixed_view_transitions: u64,  // Remaining fixed-view transitions
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PostAllocationSelectedTransformation {
    RuntimeSpill(SelectedInstructionPlanIdentity),
    RuntimeRematerialization(SelectedInstructionPlanIdentity),
    RuntimeSplit(SelectedInstructionPlanIdentity),
    FixedOccurrences(SelectedInstructionPlanIdentity),
    FixedViewCopy(FixedViewCopyIdentity),
    LiteralFold(LiteralFoldIdentity),
    PressureRematerialization(PressureRematerializationIdentity),
    CopyRemoval(CopyRemovalIdentity),
    RedundantExtension(RedundantExtensionIdentity),
    AddressFold(AddressFoldIdentity),
    ConstantBoolean(ConstantBooleanIdentity),
    ConstantBranch(ConstantBranchIdentity),
    DeadCompare(DeadCompareIdentity),
    RedundantCompare(RedundantCompareIdentity),
    EquivalentCompare(EquivalentCompareIdentity),
    CopiedCallOperand(CopiedCallOperandIdentity),
    StoredLoadForwarding(StoredLoadForwardingIdentity),
}

macro_rules! selected_transformation_rows {
    ($( $tag:literal => $variant:ident($identity:ty) as $kind:literal; )*) => {
        impl PostAllocationSelectedTransformation {
            pub(super) fn wire_tag(&self) -> u8 {
                match self {
                    $( Self::$variant(_) => $tag, )*
                }
            }

            pub(super) fn identity_bytes(&self) -> [u8; 32] {
                match self {
                    $( Self::$variant(identity) => identity.bytes(), )*
                }
            }

            pub(super) fn kind(&self) -> &'static str {
                match self {
                    $( Self::$variant(_) => $kind, )*
                }
            }

            pub(super) fn decode(
                cursor: &mut Cursor<'_>,
            ) -> Result<Self, PostAllocationOptimizationManifestDecodeError> {
                Ok(match cursor.byte()? {
                    $( $tag => Self::$variant(<$identity>::from_bytes(cursor.array()?)), )*
                    tag => {
                        return Err(
                            PostAllocationOptimizationManifestDecodeError::UnknownTransformationTag(
                                tag,
                            ),
                        );
                    }
                })
            }
        }
    };
}

selected_transformation_rows! {
    1 => FixedViewCopy(FixedViewCopyIdentity) as "fixed-view-copy";
    2 => LiteralFold(LiteralFoldIdentity) as "literal-fold";
    3 => PressureRematerialization(PressureRematerializationIdentity) as "pressure-rematerialization";
    4 => RuntimeSpill(SelectedInstructionPlanIdentity) as "runtime-spill";
    5 => RuntimeRematerialization(SelectedInstructionPlanIdentity) as "runtime-rematerialization";
    6 => CopyRemoval(CopyRemovalIdentity) as "copy-removal";
    7 => RedundantExtension(RedundantExtensionIdentity) as "redundant-extension";
    8 => AddressFold(AddressFoldIdentity) as "address-fold";
    9 => ConstantBoolean(ConstantBooleanIdentity) as "constant-boolean";
    10 => ConstantBranch(ConstantBranchIdentity) as "constant-branch";
    11 => DeadCompare(DeadCompareIdentity) as "dead-compare";
    12 => RedundantCompare(RedundantCompareIdentity) as "redundant-compare";
    13 => EquivalentCompare(EquivalentCompareIdentity) as "equivalent-compare";
    14 => RuntimeSplit(SelectedInstructionPlanIdentity) as "runtime-split";
    15 => CopiedCallOperand(CopiedCallOperandIdentity) as "copied-call-operand";
    16 => StoredLoadForwarding(StoredLoadForwardingIdentity) as "stored-load-forwarding";
    17 => FixedOccurrences(SelectedInstructionPlanIdentity) as "fixed-occurrences";
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostAllocationOptimizationManifest {
    pub identity: PostAllocationOptimizationManifestIdentity, // Canonical manifest digest
    pub stage: PostAllocationManifestStage,                   // Manifest pipeline stage
    pub pre_physical: PrePhysicalOptimizationManifestIdentity, // Pre-physical manifest digest
    pub target: NativeTarget,                                 // Target architecture and format
    pub selected: SelectedInstructionPlanIdentity,            // Final selected plan digest
    pub selected_lowering_completion: Option<SelectedLoweringOptimizationCompletionIdentity>, // Optional selected-lowering completion
    pub pre_allocation_completion: Option<PreAllocationOptimizationCompletionIdentity>, // Optional pre-allocation completion
    pub selected_transformations: Vec<PostAllocationSelectedTransformation>, // Ordered physical-form rewrites
    pub liveness: LivenessIdentity,                                          // Liveness digest
    pub ranges: LiveRangeIdentity,                                           // Live-range digest
    pub legality: AllocationLegalityIdentity, // Allocation-legality digest
    pub register_environment: TargetRegisterEnvironmentIdentity, // Register environment digest
    pub allocator_availability: AllocatorAvailabilityIdentity, // Allocator availability digest
    pub homes: RegisterHomeIdentity,          // Validated register-home digest
    pub spills: PostAllocationSpillStatus,    // Spill realization status
    pub frame: PostAllocationUnavailableData, // Frame status (unavailable at this stage)
    pub emission: PostAllocationUnavailableData, // Emission status (unavailable at this stage)
    pub publication: PostAllocationUnavailableData, // Publication status (unavailable at this stage)
    pub statistics: PostAllocationStatistics,       // Summary statistics
}

impl PostAllocationOptimizationManifest {
    pub fn recomputed_identity(&self) -> PostAllocationOptimizationManifestIdentity {
        let mut canonical = Vec::new();
        canonical.extend_from_slice(b"omega.post-allocation-optimization-manifest.v7\0");
        canonical.extend_from_slice(&encode_manifest_content(self));
        PostAllocationOptimizationManifestIdentity::from_canonical_bytes(&canonical)
    }

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
        let selected_transformations =
            decode_list(&mut cursor, PostAllocationSelectedTransformation::decode)?;
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

    pub fn render_text(&self) -> String {
        let mut output = String::new();
        writeln!(output, "Omega post-allocation optimization manifest").unwrap();
        writeln!(output, "stage: validated register homes").unwrap();
        writeln!(output, "manifest identity: {}", hex(&self.identity.bytes())).unwrap();
        writeln!(
            output,
            "pre-physical manifest: {}",
            hex(&self.pre_physical.bytes())
        )
        .unwrap();
        writeln!(
            output,
            "target: {:?}/{:?}",
            self.target.architecture, self.target.object_format
        )
        .unwrap();
        writeln!(output, "selected plan: {}", hex(&self.selected.bytes())).unwrap();
        match self.selected_lowering_completion {
            Some(identity) => writeln!(
                output,
                "selected-lowering completion: {}",
                hex(&identity.bytes())
            )
            .unwrap(),
            None => writeln!(output, "selected-lowering completion: not run").unwrap(),
        }
        match self.pre_allocation_completion {
            Some(identity) => writeln!(
                output,
                "pre-allocation completion: {}",
                hex(&identity.bytes())
            )
            .unwrap(),
            None => writeln!(output, "pre-allocation completion: not run").unwrap(),
        }
        writeln!(
            output,
            "selected transformations: {}",
            self.selected_transformations.len()
        )
        .unwrap();
        for (index, transformation) in self.selected_transformations.iter().enumerate() {
            writeln!(
                output,
                "selected transformation {index}: {} {}",
                transformation.kind(),
                hex(&transformation.identity_bytes())
            )
            .unwrap();
        }
        writeln!(output, "register homes: {}", hex(&self.homes.bytes())).unwrap();
        writeln!(
            output,
            "allocator availability: {}",
            hex(&self.allocator_availability.bytes())
        )
        .unwrap();
        writeln!(
            output,
            "spills: {}",
            match self.spills {
                PostAllocationSpillStatus::NotRequiredForValidatedHomePlan =>
                    "not required for validated home plan",
                PostAllocationSpillStatus::RealizedInSelectedProgram =>
                    "realized in selected program",
            }
        )
        .unwrap();
        writeln!(output, "frame: unavailable").unwrap();
        writeln!(output, "emission: unavailable").unwrap();
        writeln!(output, "publication: unavailable").unwrap();
        writeln!(output, "functions: {}", self.statistics.functions).unwrap();
        writeln!(output, "assignments: {}", self.statistics.assignments).unwrap();
        writeln!(
            output,
            "distinct physical views: {}",
            self.statistics.distinct_physical_views
        )
        .unwrap();
        writeln!(
            output,
            "virtual interferences: {}",
            self.statistics.virtual_interferences
        )
        .unwrap();
        writeln!(
            output,
            "fixed-view transitions: {}",
            self.statistics.fixed_view_transitions
        )
        .unwrap();
        output
    }
}

fn encode_manifest_content(manifest: &PostAllocationOptimizationManifest) -> Vec<u8> {
    let mut canonical = Vec::new();
    canonical.push(match manifest.stage {
        PostAllocationManifestStage::RegisterHomes => 1,
    });
    canonical.extend_from_slice(&manifest.pre_physical.bytes());
    canonical.extend_from_slice(&manifest.target.wire_identity_bytes());
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
    for value in [
        manifest.statistics.functions,
        manifest.statistics.assignments,
        manifest.statistics.distinct_physical_views,
        manifest.statistics.virtual_interferences,
        manifest.statistics.fixed_view_transitions,
    ] {
        push_encoded(&mut canonical, value.to_le_bytes());
    }
    canonical
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostAllocationOptimizationManifestError {
    RootMismatch,
    UnresolvedFixedViewTransitions,
    StatisticsOverflow,
    NonCanonicalTransformationLedger,
    IdentityMismatch,
    ContentMismatch,
}

crate::decode_error_display! { PostAllocationOptimizationManifestError, "invalid post-allocation optimization manifest" }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostAllocationOptimizationManifestDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownStage(u8),
    UnknownArchitecture(u8),
    UnknownObjectFormat(u8),
    TargetLayoutOverflow,
    LengthOverflow,
    UnknownTransformationTag(u8),
    UnknownCompletionStatus(u8),
    UnknownSpillStatus(u8),
    UnknownUnavailableStatus(u8),
    IdentityMismatch,
    TrailingBytes,
}

crate::decode_error! { PostAllocationOptimizationManifestDecodeError, "invalid post-allocation manifest encoding" }

#[cfg(test)]
mod tests {
    use super::{
        PostAllocationManifestStage, PostAllocationOptimizationManifest,
        PostAllocationOptimizationManifestDecodeError, PostAllocationSelectedTransformation,
        PostAllocationSpillStatus, PostAllocationStatistics, PostAllocationUnavailableData,
    };
    use crate::{AllocationLegalityIdentity, AllocatorAvailabilityIdentity, RegisterHomeIdentity};
    use optimization_core::{
        PostAllocationOptimizationManifestIdentity, PreAllocationOptimizationCompletionIdentity,
        PrePhysicalOptimizationManifestIdentity, SelectedLoweringOptimizationCompletionIdentity,
    };
    use register_model::TargetRegisterEnvironmentIdentity;
    use selected_instructions::{
        CopiedCallOperandIdentity, CopyRemovalIdentity, FixedViewCopyIdentity, LiteralFoldIdentity,
        LiveRangeIdentity, LivenessIdentity, PressureRematerializationIdentity,
        SelectedInstructionPlanIdentity, StoredLoadForwardingIdentity,
    };
    use target::NativeTarget;

    type Mutation = fn(&mut PostAllocationOptimizationManifest);

    fn record() -> PostAllocationOptimizationManifest {
        let mut record = PostAllocationOptimizationManifest {
            identity: PostAllocationOptimizationManifestIdentity::from_canonical_bytes(b"pending"),
            stage: PostAllocationManifestStage::RegisterHomes,
            pre_physical: PrePhysicalOptimizationManifestIdentity::from_canonical_bytes(b"pre"),
            target: NativeTarget::linux_x64(),
            selected: SelectedInstructionPlanIdentity::from_canonical_bytes(b"selected"),
            selected_lowering_completion: None,
            pre_allocation_completion: None,
            selected_transformations: Vec::new(),
            liveness: LivenessIdentity::from_bytes([1; 32]),
            ranges: LiveRangeIdentity::from_bytes([2; 32]),
            legality: AllocationLegalityIdentity::from_bytes([3; 32]),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([4; 32]),
            allocator_availability: AllocatorAvailabilityIdentity::from_bytes([6; 32]),
            homes: RegisterHomeIdentity::from_bytes([5; 32]),
            spills: PostAllocationSpillStatus::NotRequiredForValidatedHomePlan,
            frame: PostAllocationUnavailableData::Unavailable,
            emission: PostAllocationUnavailableData::Unavailable,
            publication: PostAllocationUnavailableData::Unavailable,
            statistics: PostAllocationStatistics {
                functions: 1,
                assignments: 2,
                distinct_physical_views: 2,
                virtual_interferences: 1,
                fixed_view_transitions: 0,
            },
        };
        record.identity = record.recomputed_identity();
        record
    }

    #[test]
    fn executable_runtime_spill_manifest_binds_order_and_realized_storage() {
        let mut spilled = record();
        spilled.selected_transformations = vec![
            PostAllocationSelectedTransformation::RuntimeSpill(
                SelectedInstructionPlanIdentity::from_bytes([7; 32]),
            ),
            PostAllocationSelectedTransformation::RuntimeSpill(
                SelectedInstructionPlanIdentity::from_bytes([8; 32]),
            ),
        ];
        spilled.spills = PostAllocationSpillStatus::RealizedInSelectedProgram;
        spilled.identity = spilled.recomputed_identity();
        assert_eq!(
            PostAllocationOptimizationManifest::decode(&spilled.encode()),
            Ok(spilled.clone())
        );
        let mut reordered = spilled.clone();
        reordered.selected_transformations.reverse();
        assert_ne!(reordered.recomputed_identity(), spilled.identity);
        let mut missing_storage = spilled.clone();
        missing_storage.spills = PostAllocationSpillStatus::NotRequiredForValidatedHomePlan;
        assert_ne!(missing_storage.recomputed_identity(), spilled.identity);
    }

    #[test]
    fn runtime_split_manifest_round_trips_and_is_distinct_from_storage_steps() {
        let identity = SelectedInstructionPlanIdentity::from_bytes([9; 32]);
        let mut split = record();
        split.selected_transformations =
            vec![PostAllocationSelectedTransformation::RuntimeSplit(identity)];
        split.identity = split.recomputed_identity();
        assert_eq!(
            PostAllocationOptimizationManifest::decode(&split.encode()),
            Ok(split.clone())
        );
        for other in [
            PostAllocationSelectedTransformation::RuntimeSpill(identity),
            PostAllocationSelectedTransformation::RuntimeRematerialization(identity),
        ] {
            let mut relabeled = split.clone();
            relabeled.selected_transformations = vec![other];
            assert_ne!(relabeled.recomputed_identity(), split.identity);
        }
        assert!(split.render_text().contains("runtime-split"));
    }

    #[test]
    fn identity_binds_every_post_allocation_domain() {
        let baseline = record();
        assert_eq!(baseline.identity, baseline.recomputed_identity());
        let mutations: Vec<Mutation> = vec![
            |record| {
                record.pre_physical =
                    PrePhysicalOptimizationManifestIdentity::from_canonical_bytes(b"other")
            },
            |record| record.target = NativeTarget::linux_arm64(),
            |record| {
                record.selected = SelectedInstructionPlanIdentity::from_canonical_bytes(b"other")
            },
            |record| {
                record.selected_lowering_completion = Some(
                    SelectedLoweringOptimizationCompletionIdentity::from_canonical_bytes(
                        b"completed",
                    ),
                )
            },
            |record| {
                record.pre_allocation_completion = Some(
                    PreAllocationOptimizationCompletionIdentity::from_canonical_bytes(b"prealloc"),
                )
            },
            |record| {
                record.selected_transformations.push(
                    PostAllocationSelectedTransformation::FixedViewCopy(
                        FixedViewCopyIdentity::from_bytes([6; 32]),
                    ),
                )
            },
            |record| {
                record.selected_transformations.push(
                    PostAllocationSelectedTransformation::LiteralFold(
                        LiteralFoldIdentity::from_bytes([13; 32]),
                    ),
                )
            },
            |record| {
                record.selected_transformations.push(
                    PostAllocationSelectedTransformation::PressureRematerialization(
                        PressureRematerializationIdentity::from_bytes([14; 32]),
                    ),
                )
            },
            |record| {
                record.selected_transformations.push(
                    PostAllocationSelectedTransformation::CopyRemoval(
                        CopyRemovalIdentity::from_bytes([15; 32]),
                    ),
                )
            },
            |record| record.liveness = LivenessIdentity::from_bytes([7; 32]),
            |record| record.ranges = LiveRangeIdentity::from_bytes([8; 32]),
            |record| record.legality = AllocationLegalityIdentity::from_bytes([9; 32]),
            |record| {
                record.register_environment =
                    TargetRegisterEnvironmentIdentity::from_bytes([10; 32])
            },
            |record| {
                record.allocator_availability = AllocatorAvailabilityIdentity::from_bytes([12; 32])
            },
            |record| record.homes = RegisterHomeIdentity::from_bytes([11; 32]),
            |record| record.statistics.functions += 1,
            |record| record.statistics.assignments += 1,
            |record| record.statistics.distinct_physical_views += 1,
            |record| record.statistics.virtual_interferences += 1,
            |record| record.statistics.fixed_view_transitions += 1,
        ];
        for mutate in mutations {
            let mut changed = baseline.clone();
            mutate(&mut changed);
            assert_ne!(baseline.identity, changed.recomputed_identity());
        }
        let text = baseline.render_text();
        assert!(text.contains("spills: not required"));
        assert!(text.contains("publication: unavailable"));
        assert!(text.contains("functions: 1"));
        assert!(!text.contains("structural Unit functions:"));
        let mut rematerialized = baseline.clone();
        rematerialized.selected_transformations = vec![
            PostAllocationSelectedTransformation::PressureRematerialization(
                PressureRematerializationIdentity::from_bytes([14; 32]),
            ),
        ];
        rematerialized.identity = rematerialized.recomputed_identity();
        assert!(
            rematerialized
                .render_text()
                .contains("pressure-rematerialization")
        );
    }

    #[test]
    fn canonical_codec_round_trips_both_routes_and_rejects_corruption() {
        let direct = record();
        let encoded = direct.encode();
        assert_eq!(
            PostAllocationOptimizationManifest::decode(&encoded),
            Ok(direct)
        );

        let mut transformed = record();
        transformed.selected_lowering_completion = Some(
            SelectedLoweringOptimizationCompletionIdentity::from_canonical_bytes(b"completed"),
        );
        transformed.selected_transformations = vec![
            PostAllocationSelectedTransformation::FixedViewCopy(FixedViewCopyIdentity::from_bytes(
                [12; 32],
            )),
            PostAllocationSelectedTransformation::LiteralFold(LiteralFoldIdentity::from_bytes(
                [13; 32],
            )),
            PostAllocationSelectedTransformation::PressureRematerialization(
                PressureRematerializationIdentity::from_bytes([14; 32]),
            ),
            PostAllocationSelectedTransformation::CopiedCallOperand(
                CopiedCallOperandIdentity::from_bytes([16; 32]),
            ),
            PostAllocationSelectedTransformation::StoredLoadForwarding(
                StoredLoadForwardingIdentity::from_bytes([17; 32]),
            ),
        ];
        transformed.identity = transformed.recomputed_identity();
        assert_eq!(
            PostAllocationOptimizationManifest::decode(&transformed.encode()),
            Ok(transformed)
        );

        let mut identity_tamper = encoded.clone();
        identity_tamper[12] ^= 1;
        assert_eq!(
            PostAllocationOptimizationManifest::decode(&identity_tamper),
            Err(PostAllocationOptimizationManifestDecodeError::IdentityMismatch)
        );
        let mut structural_count_tamper = encoded.clone();
        let structural_count_offset = structural_count_tamper.len() - 5 * size_of::<u64>();
        structural_count_tamper[structural_count_offset] ^= 1;
        assert_eq!(
            PostAllocationOptimizationManifest::decode(&structural_count_tamper),
            Err(PostAllocationOptimizationManifestDecodeError::IdentityMismatch)
        );
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert_eq!(
            PostAllocationOptimizationManifest::decode(&trailing),
            Err(PostAllocationOptimizationManifestDecodeError::TrailingBytes)
        );
        assert_eq!(
            PostAllocationOptimizationManifest::decode(&encoded[..encoded.len() - 1]),
            Err(PostAllocationOptimizationManifestDecodeError::Truncated)
        );
        let mut wrong_magic = encoded.clone();
        wrong_magic[0] ^= 1;
        assert_eq!(
            PostAllocationOptimizationManifest::decode(&wrong_magic),
            Err(PostAllocationOptimizationManifestDecodeError::WrongMagic)
        );
        let mut wrong_version = encoded.clone();
        crate::codec_bytes::patch_encoded(&mut wrong_version, 8, 5_u32.to_le_bytes());
        assert_eq!(
            PostAllocationOptimizationManifest::decode(&wrong_version),
            Err(PostAllocationOptimizationManifestDecodeError::UnsupportedVersion(5))
        );
        let content_offset = 8 + 4 + 32;
        let mut unknown_architecture = encoded.clone();
        unknown_architecture[content_offset + 1 + 32] = 9;
        assert_eq!(
            PostAllocationOptimizationManifest::decode(&unknown_architecture),
            Err(PostAllocationOptimizationManifestDecodeError::UnknownArchitecture(9))
        );
        let mut one_transformation = record();
        one_transformation.selected_transformations =
            vec![PostAllocationSelectedTransformation::FixedViewCopy(
                FixedViewCopyIdentity::from_bytes([12; 32]),
            )];
        one_transformation.identity = one_transformation.recomputed_identity();
        let mut unknown_transformation = one_transformation.encode();
        let transformation_tag_offset = content_offset + 1 + 32 + 18 + 32 + 1 + 1 + 8;
        unknown_transformation[transformation_tag_offset] = 18;
        assert_eq!(
            PostAllocationOptimizationManifest::decode(&unknown_transformation),
            Err(PostAllocationOptimizationManifestDecodeError::UnknownTransformationTag(18))
        );
    }
}
