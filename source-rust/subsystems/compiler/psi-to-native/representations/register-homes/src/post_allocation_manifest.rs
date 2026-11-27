/*
We define the in-process post-allocation optimization manifest
(`PostAllocationOptimizationManifest`), its append-only selected-transformation
ledger (`PostAllocationSelectedTransformation`), canonical SHA-256 identity
recomputation, and human-readable text rendering.
*/

use std::fmt::Write;

use optimization_core::{
    OptimizationSelectionIdentity, OptimizationUnitIdentity, OptimizationWorkBudget,
    PostAllocationOptimizationManifestIdentity, PreAllocationOptimizationCompletionIdentity,
    SelectedLoweringOptimizationCompletionIdentity,
};
use register_model::TargetRegisterEnvironmentIdentity;
use selected_instructions::{
    AddressFoldIdentity, ConstantBooleanIdentity, ConstantBranchIdentity,
    CopiedCallOperandIdentity, CopyRemovalIdentity, DeadCompareIdentity, EquivalentCompareIdentity,
    FixedViewCopyIdentity, LiteralFoldIdentity, PressureRematerializationIdentity,
    RedundantCompareIdentity, RedundantExtensionIdentity, SelectedInstructionPlan,
    StoredLoadForwardingIdentity,
};
use std::sync::Arc;
use target::NativeTarget;
use wire::digest_encoder::hex;

use crate::codec_bytes::push_encoded;
use crate::{AllocatorAvailabilityIdentity, RegisterHomeIdentity};

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

/// One rewrite in the ordered ledger. A rule rewrite records its receipt
/// identity. A runtime recovery step records only its kind: its receipt holds
/// nothing but the programs it read and wrote, and keeping every intermediate
/// program here would hold one function copy per step until the image is
/// written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PostAllocationSelectedTransformation {
    RuntimeSpill,
    RuntimeRematerialization,
    RuntimeSplit,
    FixedOccurrences,
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
    (
        runtime { $( $runtime_tag:literal => $runtime:ident as $runtime_kind:literal; )* }
        receipts { $( $tag:literal => $variant:ident as $kind:literal; )* }
    ) => {
        impl PostAllocationSelectedTransformation {
            pub fn wire_tag(&self) -> u8 {
                match self {
                    $( Self::$runtime => $runtime_tag, )*
                    $( Self::$variant(_) => $tag, )*
                }
            }

            /// The receipt digest of a rule rewrite; `None` for a runtime step.
            pub fn identity_bytes(&self) -> Option<[u8; 32]> {
                match self {
                    $( Self::$runtime => None, )*
                    $( Self::$variant(identity) => Some(identity.bytes()), )*
                }
            }

            pub(super) fn kind(&self) -> &'static str {
                match self {
                    $( Self::$runtime => $runtime_kind, )*
                    $( Self::$variant(_) => $kind, )*
                }
            }
        }
    };
}

selected_transformation_rows! {
    runtime {
        4 => RuntimeSpill as "runtime-spill";
        5 => RuntimeRematerialization as "runtime-rematerialization";
        14 => RuntimeSplit as "runtime-split";
        17 => FixedOccurrences as "fixed-occurrences";
    }
    receipts {
        1 => FixedViewCopy as "fixed-view-copy";
        2 => LiteralFold as "literal-fold";
        3 => PressureRematerialization as "pressure-rematerialization";
        6 => CopyRemoval as "copy-removal";
        7 => RedundantExtension as "redundant-extension";
        8 => AddressFold as "address-fold";
        9 => ConstantBoolean as "constant-boolean";
        10 => ConstantBranch as "constant-branch";
        11 => DeadCompare as "dead-compare";
        12 => RedundantCompare as "redundant-compare";
        13 => EquivalentCompare as "equivalent-compare";
        15 => CopiedCallOperand as "copied-call-operand";
        16 => StoredLoadForwarding as "stored-load-forwarding";
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostAllocationOptimizationManifest {
    pub identity: PostAllocationOptimizationManifestIdentity, // Canonical manifest digest
    pub stage: PostAllocationManifestStage,                   // Manifest pipeline stage
    pub optimization_unit: OptimizationUnitIdentity, // Unit the selected program was lowered from
    pub selection: OptimizationSelectionIdentity,    // Optimization selections it was lowered under
    pub budget_per_pass: OptimizationWorkBudget,     // Per-pass work budget it was lowered under
    pub target: NativeTarget,                        // Target architecture and format
    pub selected: Arc<SelectedInstructionPlan>,      // Final selected plan
    pub selected_lowering_completion: Option<SelectedLoweringOptimizationCompletionIdentity>, // Optional selected-lowering completion
    pub pre_allocation_completion: Option<PreAllocationOptimizationCompletionIdentity>, // Optional pre-allocation completion
    pub selected_transformations: Vec<PostAllocationSelectedTransformation>, // Ordered physical-form rewrites
    pub register_environment: TargetRegisterEnvironmentIdentity, // Register environment digest
    pub allocator_availability: AllocatorAvailabilityIdentity,   // Allocator availability digest
    pub homes: RegisterHomeIdentity,                             // Validated register-home digest
    pub spills: PostAllocationSpillStatus,                       // Spill realization status
    pub frame: PostAllocationUnavailableData, // Frame status (unavailable at this stage)
    pub emission: PostAllocationUnavailableData, // Emission status (unavailable at this stage)
    pub publication: PostAllocationUnavailableData, // Publication status (unavailable at this stage)
    pub statistics: PostAllocationStatistics,       // Summary statistics
}

impl PostAllocationOptimizationManifest {
    pub fn recomputed_identity(&self) -> PostAllocationOptimizationManifestIdentity {
        let mut canonical = Vec::new();
        canonical.extend_from_slice(b"omega.post-allocation-optimization-manifest.v10\0");
        canonical.extend_from_slice(&encode_manifest_content(self));
        PostAllocationOptimizationManifestIdentity::from_canonical_bytes(&canonical)
    }

    pub fn render_text(&self) -> String {
        let mut output = String::new();
        writeln!(output, "Omega post-allocation optimization manifest").unwrap();
        writeln!(output, "stage: validated register homes").unwrap();
        writeln!(output, "manifest identity: {}", hex(&self.identity.bytes())).unwrap();
        writeln!(
            output,
            "optimization unit: {}",
            hex(&self.optimization_unit.bytes())
        )
        .unwrap();
        writeln!(
            output,
            "optimization selections: {}",
            hex(&self.selection.bytes())
        )
        .unwrap();
        writeln!(
            output,
            "budget per pass: {}",
            hex(&self.budget_per_pass.encode())
        )
        .unwrap();
        writeln!(
            output,
            "target: {:?}/{:?}",
            self.target.architecture, self.target.object_format
        )
        .unwrap();
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
            match transformation.identity_bytes() {
                Some(identity) => writeln!(
                    output,
                    "selected transformation {index}: {} {}",
                    transformation.kind(),
                    hex(&identity)
                ),
                None => writeln!(
                    output,
                    "selected transformation {index}: {}",
                    transformation.kind()
                ),
            }
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
    canonical.extend_from_slice(&manifest.optimization_unit.bytes());
    canonical.extend_from_slice(&manifest.selection.bytes());
    canonical.extend_from_slice(&manifest.budget_per_pass.encode());
    canonical.extend_from_slice(&manifest.target.wire_identity_bytes());
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
        if let Some(identity) = transformation.identity_bytes() {
            canonical.extend_from_slice(&identity);
        }
    }
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

#[cfg(test)]
mod tests {
    use super::{
        PostAllocationManifestStage, PostAllocationOptimizationManifest,
        PostAllocationSelectedTransformation, PostAllocationSpillStatus, PostAllocationStatistics,
        PostAllocationUnavailableData,
    };
    use crate::{AllocatorAvailabilityIdentity, RegisterHomeIdentity};
    use optimization_core::{
        OptimizationSelectionIdentity, OptimizationUnitIdentity, OptimizationWorkBudget,
        PostAllocationOptimizationManifestIdentity, PreAllocationOptimizationCompletionIdentity,
        SelectedLoweringOptimizationCompletionIdentity,
    };
    use register_model::TargetRegisterEnvironmentIdentity;
    use selected_instructions::{
        CopyRemovalIdentity, FixedViewCopyIdentity, LiteralFoldIdentity,
        PressureRematerializationIdentity, test_support::placeholder_plan,
    };
    use target::NativeTarget;

    type Mutation = fn(&mut PostAllocationOptimizationManifest);

    fn record() -> PostAllocationOptimizationManifest {
        let mut record = PostAllocationOptimizationManifest {
            identity: PostAllocationOptimizationManifestIdentity::from_canonical_bytes(b"pending"),
            stage: PostAllocationManifestStage::RegisterHomes,
            optimization_unit: OptimizationUnitIdentity::from_canonical_bytes(b"unit"),
            selection: OptimizationSelectionIdentity::from_bytes([3; 32]),
            budget_per_pass: OptimizationWorkBudget::new(1, 1, 1, 1, 1).unwrap(),
            target: NativeTarget::linux_x64(),
            selected: placeholder_plan(1),
            selected_lowering_completion: None,
            pre_allocation_completion: None,
            selected_transformations: Vec::new(),
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
            PostAllocationSelectedTransformation::RuntimeSpill,
            PostAllocationSelectedTransformation::LiteralFold(LiteralFoldIdentity::from_bytes(
                [7; 32],
            )),
        ];
        spilled.spills = PostAllocationSpillStatus::RealizedInSelectedProgram;
        spilled.identity = spilled.recomputed_identity();
        let mut reordered = spilled.clone();
        reordered.selected_transformations.reverse();
        assert_ne!(reordered.recomputed_identity(), spilled.identity);
        let mut repeated = spilled.clone();
        repeated
            .selected_transformations
            .push(PostAllocationSelectedTransformation::RuntimeSpill);
        assert_ne!(repeated.recomputed_identity(), spilled.identity);
        let mut missing_storage = spilled.clone();
        missing_storage.spills = PostAllocationSpillStatus::NotRequiredForValidatedHomePlan;
        assert_ne!(missing_storage.recomputed_identity(), spilled.identity);
    }

    #[test]
    fn runtime_split_manifest_is_distinct_from_storage_steps() {
        let mut split = record();
        split.selected_transformations = vec![PostAllocationSelectedTransformation::RuntimeSplit];
        split.identity = split.recomputed_identity();
        for other in [
            PostAllocationSelectedTransformation::RuntimeSpill,
            PostAllocationSelectedTransformation::RuntimeRematerialization,
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
                record.optimization_unit = OptimizationUnitIdentity::from_canonical_bytes(b"other")
            },
            |record| record.selection = OptimizationSelectionIdentity::from_bytes([8; 32]),
            |record| record.budget_per_pass = OptimizationWorkBudget::new(2, 1, 1, 1, 1).unwrap(),
            |record| record.target = NativeTarget::linux_arm64(),
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
        // The final program is shared, not hashed; the record comparison sees it.
        let mut reselected = baseline.clone();
        reselected.selected = placeholder_plan(2);
        assert_ne!(reselected, baseline);
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
}
