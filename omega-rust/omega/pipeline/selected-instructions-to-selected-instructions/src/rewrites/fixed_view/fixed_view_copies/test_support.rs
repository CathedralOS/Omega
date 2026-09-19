use super::model::StagedOptimizedFixedViewCopies;
use crate::FixedViewCopyPolicy;

fn other_fixed_view_copy_policy(policy: FixedViewCopyPolicy) -> FixedViewCopyPolicy {
    match policy {
        FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1 => {
            FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1
        }
        FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1 => {
            FixedViewCopyPolicy::ImmediateBeforeFixedUseV1
        }
        FixedViewCopyPolicy::ImmediateBeforeFixedUseV1 => {
            FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1
        }
    }
}

/// One substitutable field of [`StagedOptimizedFixedViewCopyCustodyReceipt`](super::StagedOptimizedFixedViewCopyCustodyReceipt). The custody matrix
/// substitutes exactly one field per leg so a rejection attributes to that
/// claim alone. Every field is representable in memory; the receipt has no
/// wire form, so no field is canonical-encoding-closed. The independent
/// checker is `validate_optimized_fixed_view_copy_custody`: it replays the
/// copy plan against the retained segment-home stage and rejects the
/// substitution.
#[derive(Debug, Clone, Copy)]
pub enum OptimizedFixedViewCopyCustodyFieldForTest {
    Psi,
    Target,
    Entry,
    Optimization,
    Projection,
    Manifest,
    OptimizationUnit,
    FuelSchedule,
    RegisterEnvironment,
    AllocatorAvailability,
    SourceSelected,
    SourceLiveness,
    SourceRanges,
    SourceLegality,
    FixedIntervals,
    SplitRequirements,
    SegmentHomes,
    Transformation,
    TransformedSelected,
    Policy,
    Usage,
    FunctionCount,
    CopyCount,
}

impl StagedOptimizedFixedViewCopies {
    /// Mutate only retained receipt facts; the donor grants no new authority.
    /// Flat fields take fixed alternates that cannot equal any honest value;
    /// `donor` is retained for signature uniformity with nested-receipt
    /// families and is intentionally unused here.
    pub fn corrupt_custody_for_test(
        &mut self,
        field: OptimizedFixedViewCopyCustodyFieldForTest,
        _donor: &Self,
    ) {
        match field {
            OptimizedFixedViewCopyCustodyFieldForTest::Psi => {
                self.custody.psi.program_fingerprint =
                    terminal_psi::SemanticFingerprint::from_bytes([0xa1; 32]);
            }
            OptimizedFixedViewCopyCustodyFieldForTest::Target => {
                self.custody.target = match self.custody.target.architecture {
                    target::Architecture::X86_64 => target::NativeTarget::linux_arm64(),
                    _ => target::NativeTarget::linux_x64(),
                };
            }
            OptimizedFixedViewCopyCustodyFieldForTest::Entry => {
                self.custody.entry =
                    semantic_vocabulary::MachineId::new(0x5a17).expect("nonzero machine id");
            }
            OptimizedFixedViewCopyCustodyFieldForTest::Optimization => {
                self.custody.optimization =
                    optimization_core::OptimizationIdentityBundleIdentity::from_bytes([0xa2; 32]);
            }
            OptimizedFixedViewCopyCustodyFieldForTest::Projection => {
                self.custody.projection =
                    optimization_core::OptimizedAbstractPlanProjectionIdentity::from_bytes(
                        [0xa3; 32],
                    );
            }
            OptimizedFixedViewCopyCustodyFieldForTest::Manifest => {
                self.custody.manifest =
                    optimization_core::PrePhysicalOptimizationManifestIdentity::from_bytes(
                        [0xa4; 32],
                    );
            }
            OptimizedFixedViewCopyCustodyFieldForTest::OptimizationUnit => {
                self.custody.optimization_unit =
                    optimization_core::OptimizationUnitIdentity::from_bytes([0xa5; 32]);
            }
            OptimizedFixedViewCopyCustodyFieldForTest::FuelSchedule => {
                self.custody.fuel_schedule = semantic_vocabulary::FuelScheduleIdentity::new(0x5afe)
                    .expect("nonzero fuel schedule marker");
            }
            OptimizedFixedViewCopyCustodyFieldForTest::RegisterEnvironment => {
                self.custody.register_environment =
                    register_model::TargetRegisterEnvironmentIdentity::from_bytes([0xa6; 32]);
            }
            OptimizedFixedViewCopyCustodyFieldForTest::AllocatorAvailability => {
                self.custody.allocator_availability =
                    crate::AllocatorAvailabilityIdentity::from_bytes([0xb0; 32]);
            }
            OptimizedFixedViewCopyCustodyFieldForTest::SourceSelected => {
                self.custody.source_selected =
                    selected_instructions::SelectedInstructionPlanIdentity::from_bytes([0xa9; 32]);
            }
            OptimizedFixedViewCopyCustodyFieldForTest::SourceLiveness => {
                self.custody.source_liveness =
                    selected_instructions::LivenessIdentity::from_bytes([0xaa; 32]);
            }
            OptimizedFixedViewCopyCustodyFieldForTest::SourceRanges => {
                self.custody.source_ranges =
                    selected_instructions::LiveRangeIdentity::from_bytes([0xab; 32]);
            }
            OptimizedFixedViewCopyCustodyFieldForTest::SourceLegality => {
                self.custody.source_legality =
                    crate::AllocationLegalityIdentity::from_bytes([0xb1; 32]);
            }
            OptimizedFixedViewCopyCustodyFieldForTest::FixedIntervals => {
                self.custody.fixed_intervals =
                    crate::FixedPrecoloredIntervalPlanIdentity::from_bytes([0xb7; 32]);
            }
            OptimizedFixedViewCopyCustodyFieldForTest::SplitRequirements => {
                self.custody.split_requirements =
                    crate::FixedPrecoloredSplitRequirementPlanIdentity::from_bytes([0xb8; 32]);
            }
            OptimizedFixedViewCopyCustodyFieldForTest::SegmentHomes => {
                self.custody.segment_homes =
                    crate::FixedPrecoloredSegmentHomePlanIdentity::from_bytes([0xb9; 32]);
            }
            OptimizedFixedViewCopyCustodyFieldForTest::Transformation => {
                self.custody.transformation = crate::FixedViewCopyIdentity::from_bytes([0xba; 32]);
            }
            OptimizedFixedViewCopyCustodyFieldForTest::TransformedSelected => {
                self.custody.transformed_selected =
                    selected_instructions::SelectedInstructionPlanIdentity::from_bytes([0xac; 32]);
            }
            OptimizedFixedViewCopyCustodyFieldForTest::Policy => {
                self.custody.policy = other_fixed_view_copy_policy(self.custody.policy);
            }
            OptimizedFixedViewCopyCustodyFieldForTest::Usage => {
                self.custody.usage.iterations += 1;
            }
            OptimizedFixedViewCopyCustodyFieldForTest::FunctionCount => {
                self.custody.function_count += 1;
            }
            OptimizedFixedViewCopyCustodyFieldForTest::CopyCount => {
                self.custody.copy_count += 1;
            }
        }
    }
}
