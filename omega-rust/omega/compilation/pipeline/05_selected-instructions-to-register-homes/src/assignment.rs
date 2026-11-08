/*
We sequence the register-home staging routes invoked by `stage_register_allocation`:
baseline home custody (`baseline`), transformed homes after completed rewrite
runs (`transformed`), declared recovery routes (`recovery`), iterative physical
victim spilling and splitting (`runtime_spill`), and the `PostAllocationOptimizationManifest`
published by every route (`post_allocation_manifest`).
*/

macro_rules! assignment_context_accessors {
    () => {
        pub const fn homes(&self) -> &crate::ValidatedRegisterHomes {
            &self.homes
        }
        pub const fn post_allocation_manifest(
            &self,
        ) -> &crate::ValidatedPostAllocationOptimizationManifest {
            &self.manifest
        }
    };
}

macro_rules! admitted_stage_accessors {
    (const $vis:vis, $stage:ident $(. $step:ident())*) => {
        admitted_stage_accessors!(const $vis, $stage $(. $step())*, selections $stage $(. $step())*);
    };
    (const $vis:vis, $stage:ident $(. $step:ident())*, selections $selections:ident $(. $selection_step:ident())*) => {
        $vis const fn register_environment(
            &self,
        ) -> &register_environment::ValidatedTargetRegisterEnvironment {
            self.$stage$(.$step())*.register_environment()
        }
        $vis fn selections(&self) -> &optimization_core::OptimizationSelections {
            self.$selections$(.$selection_step())*.selections()
        }
        $vis fn budget_per_pass(&self) -> optimization_core::OptimizationWorkBudget {
            self.$stage$(.$step())*.budget_per_pass()
        }
    };
    (fn $vis:vis, $method:ident) => {
        $vis fn register_environment(
            &self,
        ) -> &register_environment::ValidatedTargetRegisterEnvironment {
            self.$method().register_environment()
        }
        $vis fn selections(&self) -> &optimization_core::OptimizationSelections {
            self.$method().selections()
        }
        $vis fn budget_per_pass(&self) -> optimization_core::OptimizationWorkBudget {
            self.$method().budget_per_pass()
        }
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostHomeCustodyReceipt<Source> {
    source: Source,
    homes: register_homes::RegisterHomeIdentity,
    post_allocation_manifest: optimization_core::PostAllocationOptimizationManifestIdentity,
    function_count: usize,
    assignment_count: usize,
}

impl<Source> PostHomeCustodyReceipt<Source> {
    pub(crate) fn new(
        source: Source,
        homes: crate::RegisterHomeValidationReceipt,
        post_allocation_manifest: optimization_core::PostAllocationOptimizationManifestIdentity,
    ) -> Self {
        Self {
            source,
            homes: homes.identity(),
            post_allocation_manifest,
            function_count: homes.function_count(),
            assignment_count: homes.assignment_count(),
        }
    }

    receipt_accessor_rows!(borrow PostHomeCustodyReceipt {
        source: &Source,
    });
    receipt_accessor_rows!(ref PostHomeCustodyReceipt {
        homes: register_homes::RegisterHomeIdentity,
        post_allocation_manifest: optimization_core::PostAllocationOptimizationManifestIdentity,
        function_count: usize,
        assignment_count: usize,
    });
}

pub(crate) mod baseline {
    pub(crate) mod construction {
        use crate::assign_register_homes;
        use crate::assignment::post_allocation_manifest::project_post_allocation_optimization_manifest;
        use register_homes::PostAllocationSelectedTransformation;

        use crate::StagedOptimizedSelectedReanalysis;
        use selected_instruction_analyses::StagedOptimizedAllocationLegality;

        use super::custody::custody_receipt;
        use super::{
            OptimizedPostCopyRegisterHomeCustodyError, OptimizedRegisterHomeCustodyError,
            StagedOptimizedRegisterHomes, StagedOptimizedRegisterHomesAfterFixedViewCopies,
        };

        pub(crate) fn stage_register_homes_with_assignment(
            legality: StagedOptimizedAllocationLegality,
            homes: crate::ValidatedRegisterHomes,
        ) -> Result<StagedOptimizedRegisterHomes, OptimizedRegisterHomeCustodyError> {
            let upstream = legality.custody();
            let manifest = project_post_allocation_optimization_manifest(
                upstream.manifest(),
                None,
                None,
                &[],
                legality.ranges(),
                legality.legality(),
                &homes,
            )
            .map_err(OptimizedRegisterHomeCustodyError::Manifest)?;
            let custody = custody_receipt(upstream, homes.receipt(), manifest.record().identity);
            Ok(StagedOptimizedRegisterHomes {
                legality,
                homes,
                manifest,
                custody,
            })
        }

        pub(crate) fn assign_optimized_register_homes_after_fixed_view_copies(
            reanalysis: &StagedOptimizedSelectedReanalysis,
        ) -> Result<crate::ValidatedRegisterHomes, crate::RegisterHomeError> {
            let environment = reanalysis.register_environment();
            assign_register_homes(reanalysis.legality(), reanalysis.ranges(), environment)
        }

        pub(crate) fn stage_register_homes_after_fixed_view_copies_with_assignment(
            reanalysis: StagedOptimizedSelectedReanalysis,
            homes: crate::ValidatedRegisterHomes,
        ) -> Result<
            StagedOptimizedRegisterHomesAfterFixedViewCopies,
            OptimizedPostCopyRegisterHomeCustodyError,
        > {
            let source = reanalysis.custody();
            let _environment = reanalysis.register_environment();
            let manifest = project_post_allocation_optimization_manifest(
                source.source().manifest(),
                None,
                None,
                &[PostAllocationSelectedTransformation::FixedViewCopy(
                    source.source().transformation(),
                )],
                reanalysis.ranges(),
                reanalysis.legality(),
                &homes,
            )
            .map_err(OptimizedPostCopyRegisterHomeCustodyError::Manifest)?;
            let custody = crate::assignment::PostHomeCustodyReceipt::new(
                source,
                homes.receipt(),
                manifest.record().identity,
            );
            Ok(StagedOptimizedRegisterHomesAfterFixedViewCopies {
                reanalysis,
                homes,
                manifest,
                custody,
            })
        }
    }
    pub(crate) mod custody {
        use crate::RegisterHomeValidationReceipt;
        use optimization_core::PostAllocationOptimizationManifestIdentity;

        use selected_instruction_analyses::StagedOptimizedAllocationLegalityCustodyReceipt;

        use super::StagedOptimizedRegisterHomeCustodyReceipt;

        pub(crate) fn custody_receipt(
            upstream: StagedOptimizedAllocationLegalityCustodyReceipt,
            homes: RegisterHomeValidationReceipt,
            manifest: PostAllocationOptimizationManifestIdentity,
        ) -> StagedOptimizedRegisterHomeCustodyReceipt {
            StagedOptimizedRegisterHomeCustodyReceipt {
                core: upstream.core(),
                allocator_availability: upstream.allocator_availability(),
                selected: upstream.selected(),
                liveness: upstream.liveness(),
                ranges: upstream.ranges(),
                legality: upstream.legality(),
                homes: homes.identity(),
                post_allocation_manifest: manifest,
                function_count: homes.function_count(),
                assignment_count: homes.assignment_count(),
            }
        }
    }

    use crate::StagedOptimizedSelectedReanalysisCustodyReceipt;
    use crate::{
        OptimizedSelectedReanalysisError, StagedOptimizedSelectedReanalysis,
        ValidatedFixedViewCopies,
    };
    use crate::{
        RegisterHomeError, RegisterHomeIdentity, ValidatedPostAllocationOptimizationManifest,
        ValidatedRegisterHomes,
    };
    use optimization_core::{
        OptimizationIdentityBundleIdentity, OptimizationUnitIdentity,
        OptimizedAbstractPlanProjectionIdentity, PostAllocationOptimizationManifestIdentity,
        PrePhysicalOptimizationManifestIdentity,
    };
    use register_homes::{
        AllocationLegalityIdentity, AllocatorAvailabilityIdentity,
        PostAllocationOptimizationManifestError,
    };
    use selected_instruction_analyses::{
        SelectedProgramRef, StagedOptimizedAllocationLegality, ValidatedAllocationLegality,
        ValidatedLiveRanges, ValidatedLiveness,
    };
    use selected_instructions::StagedOptimizedCustodyCore;
    use selected_instructions::{
        LiveRangeIdentity, LivenessIdentity, SelectedInstructionPlanIdentity,
    };
    use semantic_vocabulary::{MachineId, WorkScheduleIdentity};
    use terminal_psi::TerminalPsiIdentity;

    use super::PostHomeCustodyReceipt;

    pub(crate) use construction::{
        assign_optimized_register_homes_after_fixed_view_copies,
        stage_register_homes_after_fixed_view_copies_with_assignment,
        stage_register_homes_with_assignment,
    };

    #[derive(Debug)]
    pub(crate) struct StagedOptimizedRegisterHomes {
        legality: StagedOptimizedAllocationLegality,
        homes: ValidatedRegisterHomes,
        manifest: ValidatedPostAllocationOptimizationManifest,
        custody: StagedOptimizedRegisterHomeCustodyReceipt,
    }

    impl StagedOptimizedRegisterHomes {
        pub fn selected(&self) -> SelectedProgramRef<'_> {
            SelectedProgramRef::new(self.legality.selected())
        }
        admitted_stage_accessors!(const pub, legality);
        pub const fn liveness(&self) -> &ValidatedLiveness {
            self.legality.liveness()
        }
        pub const fn ranges(&self) -> &ValidatedLiveRanges {
            self.legality.ranges()
        }
        pub const fn legality(&self) -> &ValidatedAllocationLegality {
            self.legality.legality()
        }
        assignment_context_accessors!();
        receipt_accessor_rows!(ref StagedOptimizedRegisterHomes {
            custody: StagedOptimizedRegisterHomeCustodyReceipt,
        });
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct StagedOptimizedRegisterHomeCustodyReceipt {
        core: StagedOptimizedCustodyCore,
        allocator_availability: AllocatorAvailabilityIdentity,
        selected: SelectedInstructionPlanIdentity,
        liveness: LivenessIdentity,
        ranges: LiveRangeIdentity,
        legality: AllocationLegalityIdentity,
        homes: RegisterHomeIdentity,
        post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
        function_count: usize,
        assignment_count: usize,
    }

    impl StagedOptimizedRegisterHomeCustodyReceipt {
        selected_instructions::staging_custody_core_accessors!();

        receipt_accessor_rows!(StagedOptimizedRegisterHomeCustodyReceipt {
            allocator_availability: AllocatorAvailabilityIdentity,
            selected: SelectedInstructionPlanIdentity,
            liveness: LivenessIdentity,
            ranges: LiveRangeIdentity,
            legality: AllocationLegalityIdentity,
            homes: RegisterHomeIdentity,
            post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
            function_count: usize,
            assignment_count: usize,
        });
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum OptimizedRegisterHomeCustodyError {
        Assignment(RegisterHomeError),
        Revalidation(RegisterHomeError),
        Manifest(PostAllocationOptimizationManifestError),
        ReceiptMismatch,
    }

    stage_error_display!(
        OptimizedRegisterHomeCustodyError,
        "optimized register-home staging failed"
    );

    #[derive(Debug)]
    pub(crate) struct StagedOptimizedRegisterHomesAfterFixedViewCopies {
        reanalysis: StagedOptimizedSelectedReanalysis,
        homes: ValidatedRegisterHomes,
        manifest: ValidatedPostAllocationOptimizationManifest,
        custody: StagedOptimizedPostCopyRegisterHomeCustodyReceipt,
    }

    impl StagedOptimizedRegisterHomesAfterFixedViewCopies {
        pub const fn selected(&self) -> &ValidatedFixedViewCopies {
            self.reanalysis.transformation_stage().copies()
        }
        admitted_stage_accessors!(const pub, reanalysis);
        pub const fn liveness(&self) -> &ValidatedLiveness {
            self.reanalysis.liveness()
        }
        pub const fn ranges(&self) -> &ValidatedLiveRanges {
            self.reanalysis.ranges()
        }
        pub const fn legality(&self) -> &ValidatedAllocationLegality {
            self.reanalysis.legality()
        }
        assignment_context_accessors!();
        receipt_accessor_rows!(ref StagedOptimizedRegisterHomesAfterFixedViewCopies {
            custody: StagedOptimizedPostCopyRegisterHomeCustodyReceipt,
        });
    }

    pub type StagedOptimizedPostCopyRegisterHomeCustodyReceipt =
        PostHomeCustodyReceipt<StagedOptimizedSelectedReanalysisCustodyReceipt>;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum OptimizedPostCopyRegisterHomeCustodyError {
        UpstreamReanalysis(OptimizedSelectedReanalysisError),
        Assignment(RegisterHomeError),
        Revalidation(RegisterHomeError),
        Manifest(PostAllocationOptimizationManifestError),
        ReceiptMismatch,
    }

    stage_error_display!(
        OptimizedPostCopyRegisterHomeCustodyError,
        "optimized post-copy register-home staging failed"
    );
}
pub(crate) use crate::home_assignment;
pub(crate) mod post_allocation_manifest {
    pub(crate) mod reconstruction {
        use std::collections::BTreeSet;

        use optimization_core::{
            PostAllocationOptimizationManifestIdentity,
            PreAllocationOptimizationCompletionIdentity, PrePhysicalOptimizationManifestIdentity,
            SelectedLoweringOptimizationCompletionIdentity,
        };

        use crate::ValidatedRegisterHomes;
        use selected_instruction_analyses::{ValidatedAllocationLegality, ValidatedLiveRanges};

        use super::{
            PostAllocationManifestStage, PostAllocationOptimizationManifest,
            PostAllocationOptimizationManifestError, PostAllocationSelectedTransformation,
            PostAllocationSpillStatus, PostAllocationStatistics, PostAllocationUnavailableData,
        };

        pub(crate) fn expected_record(
            pre_physical: PrePhysicalOptimizationManifestIdentity,
            selected_lowering_completion: Option<SelectedLoweringOptimizationCompletionIdentity>,
            pre_allocation_completion: Option<PreAllocationOptimizationCompletionIdentity>,
            selected_transformations: &[PostAllocationSelectedTransformation],
            ranges: &ValidatedLiveRanges,
            legality: &ValidatedAllocationLegality,
            homes: &ValidatedRegisterHomes,
        ) -> Result<PostAllocationOptimizationManifest, PostAllocationOptimizationManifestError>
        {
            let mut unique_transformations = BTreeSet::new();
            if selected_transformations.iter().any(|transformation| {
                let key = match transformation {
                    PostAllocationSelectedTransformation::FixedViewCopy(identity) => {
                        (1_u8, identity.bytes())
                    }
                    PostAllocationSelectedTransformation::LiteralFold(identity) => {
                        (2_u8, identity.bytes())
                    }
                    PostAllocationSelectedTransformation::PressureRematerialization(identity) => {
                        (3_u8, identity.bytes())
                    }
                    PostAllocationSelectedTransformation::RuntimeSpill(identity) => {
                        (4_u8, identity.bytes())
                    }
                    PostAllocationSelectedTransformation::RuntimeRematerialization(identity) => {
                        (5_u8, identity.bytes())
                    }
                    PostAllocationSelectedTransformation::CopyRemoval(identity) => {
                        (6_u8, identity.bytes())
                    }
                    PostAllocationSelectedTransformation::RedundantExtension(identity) => {
                        (7_u8, identity.bytes())
                    }
                    PostAllocationSelectedTransformation::AddressFold(identity) => {
                        (8_u8, identity.bytes())
                    }
                    PostAllocationSelectedTransformation::ConstantBoolean(identity) => {
                        (9_u8, identity.bytes())
                    }
                    PostAllocationSelectedTransformation::ConstantBranch(identity) => {
                        (10_u8, identity.bytes())
                    }
                    PostAllocationSelectedTransformation::DeadCompare(identity) => {
                        (11_u8, identity.bytes())
                    }
                    PostAllocationSelectedTransformation::RedundantCompare(identity) => {
                        (12_u8, identity.bytes())
                    }
                    PostAllocationSelectedTransformation::EquivalentCompare(identity) => {
                        (13_u8, identity.bytes())
                    }
                    PostAllocationSelectedTransformation::FixedOccurrences(identity) => {
                        (17_u8, identity.bytes())
                    }
                    PostAllocationSelectedTransformation::RuntimeSplit(identity) => {
                        (14_u8, identity.bytes())
                    }
                    PostAllocationSelectedTransformation::CopiedCallOperand(identity) => {
                        (15_u8, identity.bytes())
                    }
                    PostAllocationSelectedTransformation::StoredLoadForwarding(identity) => {
                        (16_u8, identity.bytes())
                    }
                };
                !unique_transformations.insert(key)
            }) {
                return Err(
                    PostAllocationOptimizationManifestError::NonCanonicalTransformationLedger,
                );
            }
            if legality.receipt().ranges() != ranges.receipt().identity()
                || homes.receipt().legality() != legality.receipt().identity()
                || homes.receipt().ranges() != ranges.receipt().identity()
                || homes.receipt().register_environment()
                    != legality.receipt().register_environment()
                || homes.receipt().allocator_availability()
                    != legality.receipt().allocator_availability()
                || homes.plan().functions.len() != ranges.plan().functions.len()
                || homes.plan().functions.len() != legality.plan().functions.len()
            {
                return Err(PostAllocationOptimizationManifestError::RootMismatch);
            }
            let transition_count = legality
                .plan()
                .virtual_registers()
                .map(|register| register.entry_transitions.len())
                .sum::<usize>();
            if transition_count != 0 {
                return Err(
                    PostAllocationOptimizationManifestError::UnresolvedFixedViewTransitions,
                );
            }
            let distinct_views = homes
                .plan()
                .functions
                .iter()
                .flat_map(|function| &function.assignments)
                .map(|assignment| assignment.view)
                .collect::<BTreeSet<_>>()
                .len();
            let interference_count = ranges
                .plan()
                .functions
                .iter()
                .map(|function| function.interference.len())
                .sum::<usize>();
            let statistics = PostAllocationStatistics {
                functions: count(homes.plan().functions.len())?,
                assignments: count(homes.receipt().assignment_count())?,
                distinct_physical_views: count(distinct_views)?,
                virtual_interferences: count(interference_count)?,
                fixed_view_transitions: 0,
            };
            let mut record = PostAllocationOptimizationManifest {
                identity: PostAllocationOptimizationManifestIdentity::from_canonical_bytes(
                    b"pending",
                ),
                stage: PostAllocationManifestStage::RegisterHomes,
                pre_physical,
                target: ranges.plan().target,
                selected: ranges.plan().selected,
                selected_lowering_completion,
                pre_allocation_completion,
                selected_transformations: selected_transformations.to_vec(),
                liveness: ranges.receipt().liveness(),
                ranges: ranges.receipt().identity(),
                legality: legality.receipt().identity(),
                register_environment: legality.receipt().register_environment(),
                allocator_availability: legality.receipt().allocator_availability(),
                homes: homes.receipt().identity(),
                spills: if selected_transformations.iter().any(|transformation| {
                    matches!(
                        transformation,
                        PostAllocationSelectedTransformation::RuntimeSpill(_)
                    )
                }) {
                    PostAllocationSpillStatus::RealizedInSelectedProgram
                } else {
                    PostAllocationSpillStatus::NotRequiredForValidatedHomePlan
                },
                frame: PostAllocationUnavailableData::Unavailable,
                emission: PostAllocationUnavailableData::Unavailable,
                publication: PostAllocationUnavailableData::Unavailable,
                statistics,
            };
            record.identity = record.recomputed_identity();
            Ok(record)
        }

        fn count(value: usize) -> Result<u64, PostAllocationOptimizationManifestError> {
            u64::try_from(value)
                .map_err(|_| PostAllocationOptimizationManifestError::StatisticsOverflow)
        }
    }

    use optimization_core::{
        PreAllocationOptimizationCompletionIdentity, PrePhysicalOptimizationManifestIdentity,
        SelectedLoweringOptimizationCompletionIdentity,
    };

    use crate::ValidatedRegisterHomes;
    use selected_instruction_analyses::{ValidatedAllocationLegality, ValidatedLiveRanges};

    pub use register_homes::post_allocation_manifest::{
        PostAllocationManifestStage, PostAllocationOptimizationManifest,
        PostAllocationOptimizationManifestDecodeError, PostAllocationOptimizationManifestError,
        PostAllocationSelectedTransformation, PostAllocationSpillStatus, PostAllocationStatistics,
        PostAllocationUnavailableData,
    };

    pub(crate) fn project_post_allocation_optimization_manifest(
        pre_physical: PrePhysicalOptimizationManifestIdentity,
        selected_lowering_completion: Option<SelectedLoweringOptimizationCompletionIdentity>,
        pre_allocation_completion: Option<PreAllocationOptimizationCompletionIdentity>,
        selected_transformations: &[PostAllocationSelectedTransformation],
        ranges: &ValidatedLiveRanges,
        legality: &ValidatedAllocationLegality,
        homes: &ValidatedRegisterHomes,
    ) -> Result<ValidatedPostAllocationOptimizationManifest, PostAllocationOptimizationManifestError>
    {
        let record = reconstruction::expected_record(
            pre_physical,
            selected_lowering_completion,
            pre_allocation_completion,
            selected_transformations,
            ranges,
            legality,
            homes,
        )?;
        Ok(ValidatedPostAllocationOptimizationManifest { record })
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ValidatedPostAllocationOptimizationManifest {
        record: PostAllocationOptimizationManifest,
    }

    impl ValidatedPostAllocationOptimizationManifest {
        receipt_accessor_rows!(borrow ValidatedPostAllocationOptimizationManifest {
            record: &PostAllocationOptimizationManifest,
        });
    }
}
pub(crate) mod recovery {
    use crate::FixedViewCopyPolicy;
    use crate::{PreAllocationResult, SelectedLoweringResult};
    use crate::{
        PressureRematerializationPolicy, StagedOptimizedSelectedReanalysis,
        probe_optimized_fixed_precolored_segment_homes,
        stage_optimized_fixed_precolored_segment_homes, stage_optimized_fixed_view_copies,
        stage_optimized_selected_reanalysis,
    };
    use crate::{
        RegisterAllocationError, RetainedAllocation,
        stage_optimized_active_resident_rematerialization_pressure,
    };
    use register_homes::{RecoveryClassificationPolicy, SpillChoicePolicy};
    use selected_instruction_analyses::{
        StagedOptimizedAllocationLegality, StagedOptimizedLiveRanges,
        stage_optimized_allocation_legality_for_active_resident_immediate_u64_multi_use_rematerialization_v1,
    };

    fn fixed_view_reanalysis(
        legality: StagedOptimizedAllocationLegality,
        policy: FixedViewCopyPolicy,
    ) -> Result<StagedOptimizedSelectedReanalysis, RegisterAllocationError> {
        let budget = legality.budget_per_pass();
        let segments = stage_optimized_fixed_precolored_segment_homes(legality, budget)
            .map_err(RegisterAllocationError::FixedSegments)?;
        let copies = stage_optimized_fixed_view_copies(segments, policy, budget)
            .map_err(RegisterAllocationError::FixedViewCopies)?;
        stage_optimized_selected_reanalysis(copies).map_err(RegisterAllocationError::Reanalysis)
    }

    fn fixed_view_allocation(
        legality: StagedOptimizedAllocationLegality,
        policy: FixedViewCopyPolicy,
    ) -> Result<RetainedAllocation, RegisterAllocationError> {
        if policy == FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1 {
            let budget = legality.budget_per_pass();
            if probe_optimized_fixed_precolored_segment_homes(&legality, budget)
                .err()
                .and_then(|error| error.capacity_decline())
                .is_some()
            {
                return RetainedAllocation::try_from(
                    crate::assignment::runtime_spill::recover_after_declined_fixed_view_probe(
                        legality, policy,
                    )
                    .map_err(RegisterAllocationError::RuntimeSpill)?,
                )
                .map_err(RegisterAllocationError::Admission);
            }
        }
        let reanalysis = fixed_view_reanalysis(legality, policy)?;
        match crate::assignment::baseline::assign_optimized_register_homes_after_fixed_view_copies(
            &reanalysis,
        ) {
            Ok(homes) => RetainedAllocation::try_from(
                crate::assignment::baseline::stage_register_homes_after_fixed_view_copies_with_assignment(
                    reanalysis,
                    homes,
                )
                .map_err(RegisterAllocationError::FixedViewHomes)?,
            )
            .map_err(RegisterAllocationError::Admission),
            Err(error) if error.admits_runtime_recovery() => RetainedAllocation::try_from(
                crate::assignment::runtime_spill::recover_after_fixed_view_copies(reanalysis)
                    .map_err(RegisterAllocationError::RuntimeSpill)?,
            )
            .map_err(RegisterAllocationError::Admission),
            Err(error) => Err(RegisterAllocationError::FixedViewHomes(
                crate::OptimizedPostCopyRegisterHomeCustodyError::Assignment(error),
            )),
        }
    }

    pub(crate) fn stage_shared_entry_fixed_view_register_allocation(
        legality: StagedOptimizedAllocationLegality,
    ) -> Result<RetainedAllocation, RegisterAllocationError> {
        fixed_view_allocation(
            legality,
            FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1,
        )
    }

    pub(crate) fn stage_active_resident_register_allocation(
        ranges: StagedOptimizedLiveRanges,
    ) -> Result<RetainedAllocation, RegisterAllocationError> {
        let budget = ranges.budget_per_pass();
        let legality = stage_optimized_allocation_legality_for_active_resident_immediate_u64_multi_use_rematerialization_v1(ranges).map_err(RegisterAllocationError::Legality)?;
        let pressure = stage_optimized_active_resident_rematerialization_pressure(
            legality,
            SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
            RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
            PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
            budget,
        ).map_err(RegisterAllocationError::Rematerialization)?;
        let environment = pressure.source().register_environment();
        match crate::assign_register_homes(pressure.legality(), pressure.ranges(), environment) {
            Ok(homes) => RetainedAllocation::try_from(
                crate::rewrites::complete_optimized_active_resident_rematerialization(
                    pressure, homes,
                )
                .map_err(RegisterAllocationError::Rematerialization)?,
            )
            .map_err(RegisterAllocationError::Admission),
            Err(error) if error.admits_runtime_recovery() => RetainedAllocation::try_from(
                crate::assignment::runtime_spill::recover_after_active_resident_rematerialization(
                    pressure,
                )
                .map_err(RegisterAllocationError::RuntimeSpill)?,
            )
            .map_err(RegisterAllocationError::Admission),
            Err(error) => Err(RegisterAllocationError::Rematerialization(
                crate::OptimizedActiveResidentRematerializationError::Homes(error),
            )),
        }
    }

    pub(crate) fn stage_selected_lowering_register_allocation(
        run: SelectedLoweringResult,
    ) -> Result<RetainedAllocation, RegisterAllocationError> {
        let mut home_reuse =
            crate::assignment::home_assignment::compute::FunctionHomeReuse::default();
        match crate::assignment::transformed::assign_optimized_register_homes_after_selected_lowering(
            &run,
            Some(&mut home_reuse),
        ) {
            Ok(homes) => RetainedAllocation::try_from(
                crate::assignment::transformed::stage_optimized_register_homes_after_selected_lowering_with_assignment(run, homes)
                    .map_err(RegisterAllocationError::TransformedHomes)?,
            )
            .map_err(RegisterAllocationError::Admission),
            Err(error) if error.admits_runtime_recovery() => RetainedAllocation::try_from(
                crate::assignment::runtime_spill::recover_after_selected_lowering_with_reuse(
                    run, home_reuse,
                )
                .map_err(RegisterAllocationError::RuntimeSpill)?,
            )
            .map_err(RegisterAllocationError::Admission),
            Err(error) => Err(RegisterAllocationError::TransformedHomes(
                crate::OptimizedPostSelectedLoweringHomeCustodyError::Assignment(error),
            )),
        }
    }

    pub(crate) fn stage_pre_allocation_register_allocation(
        run: PreAllocationResult,
    ) -> Result<RetainedAllocation, RegisterAllocationError> {
        let mut home_reuse =
            crate::assignment::home_assignment::compute::FunctionHomeReuse::default();
        match crate::assignment::transformed::assign_optimized_register_homes_after_pre_allocation(
            &run,
            Some(&mut home_reuse),
        ) {
            Ok(homes) => RetainedAllocation::try_from(
                crate::assignment::transformed::stage_optimized_register_homes_after_pre_allocation_with_assignment(run, homes)
                    .map_err(RegisterAllocationError::PreAllocationHomes)?,
            )
            .map_err(RegisterAllocationError::Admission),
            Err(error) if error.admits_runtime_recovery() => RetainedAllocation::try_from(
                crate::assignment::runtime_spill::recover_after_pre_allocation_with_reuse(
                    run, home_reuse,
                )
                .map_err(RegisterAllocationError::RuntimeSpill)?,
            )
            .map_err(RegisterAllocationError::Admission),
            Err(error) => Err(RegisterAllocationError::PreAllocationHomes(
                crate::OptimizedPostPreAllocationHomeCustodyError::Assignment(error),
            )),
        }
    }
}
pub(crate) mod runtime_spill {
    pub(crate) mod recovery {
        use super::{
            RuntimeSpillAllocation, RuntimeSpillAllocationError, RuntimeSpillFacts,
            RuntimeSpillManifestBase, RuntimeSpillSource, RuntimeSpillStep,
            RuntimeSpillStepRewrite,
        };
        use crate::assignment::home_assignment::compute::FunctionHomeReuse;
        use crate::{
            FixedViewCopyPolicy, ValidatedRuntimeSpill, ValidatedRuntimeSplit,
            ValidatedSelectedAnalysis,
        };
        use crate::{PreAllocationResult, SelectedLoweringResult};
        use crate::{
            RuntimeSpillError, RuntimeSpillSpanPolicy, StagedOptimizedSelectedReanalysis,
            materialize_selected_fixed_occurrences, rematerialize_selected_runtime_value,
            spill_selected_runtime_value, spill_selected_runtime_value_with_crossing_status,
            split_selected_runtime_value,
        };
        use optimization_core::OptimizationWorkBudget;
        use register_homes::PostAllocationSelectedTransformation;
        use selected_instruction_analyses::SelectedProgramView;
        use selected_instruction_analyses::{
            RecoveryReanalysisError, SelectedProgramRef, StagedOptimizedAllocationLegality,
            UnsealedRecoveryFacts, ValidatedAllocationLegality, ValidatedAllocatorAvailability,
            ValidatedLiveRanges, ValidatedLiveness,
        };
        use selected_instructions::{
            FunctionLiveRanges, SelectedInstructionPlan, VirtualRegisterId, VirtualRegisterOrigin,
        };

        pub(crate) fn assign_source_reusing(
            source: &StagedOptimizedAllocationLegality,
            reuse: &mut FunctionHomeReuse,
        ) -> Result<crate::ValidatedRegisterHomes, crate::RegisterHomeError> {
            assign_reusing(
                source.register_environment(),
                source.ranges(),
                source.legality(),
                Some(reuse),
            )
        }

        fn assign_reusing(
            environment: &register_environment::ValidatedTargetRegisterEnvironment,
            ranges: &ValidatedLiveRanges,
            legality: &ValidatedAllocationLegality,
            reuse: Option<&mut FunctionHomeReuse>,
        ) -> Result<crate::ValidatedRegisterHomes, crate::RegisterHomeError> {
            crate::assignment::home_assignment::assign_register_homes_reusing(
                legality,
                ranges,
                environment,
                reuse,
            )
        }

        fn reanalyze_unsealed(
            environment: &register_environment::ValidatedTargetRegisterEnvironment,
            availability: &ValidatedAllocatorAvailability,
            previous: &impl ValidatedSelectedAnalysis,
            current_facts: &UnsealedRecoveryFacts,
            selected: &impl ValidatedSelectedAnalysis,
        ) -> Result<UnsealedRecoveryFacts, RuntimeSpillAllocationError> {
            current_facts
                .reanalyze_changed_functions(
                    previous,
                    selected,
                    availability,
                    environment.physical(),
                    environment.reservations(),
                )
                .map_err(|error| match error {
                    RecoveryReanalysisError::Liveness(error) => {
                        RuntimeSpillAllocationError::Liveness(error)
                    }
                    RecoveryReanalysisError::Ranges(error) => {
                        RuntimeSpillAllocationError::Ranges(error)
                    }
                    RecoveryReanalysisError::Legality(error) => {
                        RuntimeSpillAllocationError::Legality(error)
                    }
                })
        }

        #[allow(clippy::too_many_arguments)]
        fn seal_recovery_facts(
            environment: &register_environment::ValidatedTargetRegisterEnvironment,
            availability: &ValidatedAllocatorAvailability,
            anchor_selected: &impl ValidatedSelectedAnalysis,
            anchor_liveness: &ValidatedLiveness,
            anchor_ranges: &ValidatedLiveRanges,
            anchor_legality: &ValidatedAllocationLegality,
            unsealed: UnsealedRecoveryFacts,
            selected: &impl ValidatedSelectedAnalysis,
        ) -> Result<RuntimeSpillFacts, RuntimeSpillAllocationError> {
            let (liveness, ranges, legality) = unsealed
                .seal(
                    anchor_selected,
                    anchor_liveness,
                    anchor_ranges,
                    anchor_legality,
                    selected,
                    availability,
                    environment.identity(),
                )
                .map_err(RuntimeSpillAllocationError::Legality)?;
            Ok(RuntimeSpillFacts {
                liveness,
                ranges,
                legality,
            })
        }

        struct SpillPressureProbe {
            rewrite: ValidatedRuntimeSpill,
            facts: UnsealedRecoveryFacts,
            failure: crate::RegisterHomeError,
        }

        impl SpillPressureProbe {
            fn reuse_for(
                self,
                bounded: &ValidatedRuntimeSpill,
            ) -> Option<(UnsealedRecoveryFacts, crate::RegisterHomeError)> {
                (self.rewrite.receipt() == bounded.receipt()
                    && self.rewrite.transformed() == bounded.transformed())
                .then_some((self.facts, self.failure))
            }
        }

        pub(crate) fn candidates(
            plan: &SelectedInstructionPlan,
            function: usize,
        ) -> Vec<(usize, VirtualRegisterId)> {
            plan.functions
                .get(function)
                .into_iter()
                .flat_map(|body| {
                    body.virtual_registers
                        .iter()
                        .filter(|register| is_recovery_candidate(body, register))
                        .map(move |register| (function, register.id))
                })
                .collect()
        }

        fn is_recovery_candidate(
            body: &selected_instructions::SelectedFunction,
            register: &selected_instructions::VirtualRegister,
        ) -> bool {
            matches!(
                register.origin,
                VirtualRegisterOrigin::InstructionResult { .. }
                    | VirtualRegisterOrigin::BlockParameter { .. }
                    | VirtualRegisterOrigin::StructuralObservation { .. }
                    | VirtualRegisterOrigin::AbiTransport { .. }
            ) || body.is_entry_register(register)
        }

        pub(crate) fn transformations(
            prefix: &[PostAllocationSelectedTransformation],
            recorded: &[PostAllocationSelectedTransformation],
        ) -> Vec<PostAllocationSelectedTransformation> {
            prefix.iter().chain(recorded).cloned().collect()
        }

        pub(crate) struct BatchFunctionIndex {
            components: crate::assignment::same_home::SameHomeComponents,
            neighbor_offsets: Vec<u32>,
            neighbor_targets: Vec<VirtualRegisterId>,
            live_points: Vec<u64>,
        }

        impl BatchFunctionIndex {
            pub(crate) fn new(ranges: &FunctionLiveRanges) -> Self {
                let components =
                    crate::assignment::same_home::SameHomeComponents::from_ranges(ranges);
                let max_reg_id = ranges
                    .virtual_registers
                    .iter()
                    .map(|r| r.virtual_register.0 as usize)
                    .chain(
                        ranges
                            .interference
                            .iter()
                            .flat_map(|p| [p.lower.0 as usize, p.higher.0 as usize]),
                    )
                    .max()
                    .unwrap_or(0);
                let mut counts = vec![0u32; max_reg_id + 1];
                for pair in &ranges.interference {
                    counts[pair.lower.0 as usize] += 1;
                    counts[pair.higher.0 as usize] += 1;
                }
                let mut neighbor_offsets = vec![0u32; max_reg_id + 2];
                for i in 0..=max_reg_id {
                    neighbor_offsets[i + 1] = neighbor_offsets[i] + counts[i];
                }
                let mut cursors = neighbor_offsets.clone();
                let mut neighbor_targets =
                    vec![VirtualRegisterId(0); neighbor_offsets[max_reg_id + 1] as usize];
                for pair in &ranges.interference {
                    let l_idx = pair.lower.0 as usize;
                    let h_idx = pair.higher.0 as usize;
                    let lc = &mut cursors[l_idx];
                    neighbor_targets[*lc as usize] = pair.higher;
                    *lc += 1;
                    let hc = &mut cursors[h_idx];
                    neighbor_targets[*hc as usize] = pair.lower;
                    *hc += 1;
                }
                for reg_idx in 0..=max_reg_id {
                    let start = neighbor_offsets[reg_idx] as usize;
                    let end = neighbor_offsets[reg_idx + 1] as usize;
                    let slice = &mut neighbor_targets[start..end];
                    if slice.len() > 1 && slice.windows(2).any(|w| w[0] > w[1]) {
                        slice.sort_unstable();
                    }
                }
                let mut live_points = vec![0u64; max_reg_id + 1];
                for range in &ranges.virtual_registers {
                    let points: u64 = range
                        .fragments
                        .iter()
                        .map(|fragment| u64::from(fragment.end.0 - fragment.start.0))
                        .sum();
                    live_points[range.virtual_register.0 as usize] = points;
                }
                Self {
                    components,
                    neighbor_offsets,
                    neighbor_targets,
                    live_points,
                }
            }

            #[inline]
            fn register_neighbors(&self, register: VirtualRegisterId) -> &[VirtualRegisterId] {
                let idx = register.0 as usize;
                if idx + 1 >= self.neighbor_offsets.len() {
                    return &[];
                }
                let start = self.neighbor_offsets[idx] as usize;
                let end = self.neighbor_offsets[idx + 1] as usize;
                &self.neighbor_targets[start..end]
            }
        }

        pub(crate) struct FailedDomain<'ranges> {
            function: usize,
            ranges: &'ranges FunctionLiveRanges,
            index: Option<&'ranges BatchFunctionIndex>,
            members: Vec<VirtualRegisterId>,
            components: Option<crate::assignment::same_home::SameHomeComponents>,
            pressure: Vec<(VirtualRegisterId, usize)>,
            direct_pressure: Vec<(VirtualRegisterId, usize)>,
            intrinsic: bool,
        }

        impl<'ranges> FailedDomain<'ranges> {
            fn components(&self) -> &crate::assignment::same_home::SameHomeComponents {
                if let Some(index) = self.index {
                    &index.components
                } else {
                    self.components.as_ref().expect(
                        "FailedDomain carries either BatchFunctionIndex or owned components",
                    )
                }
            }

            pub(crate) fn of_indexed(
                failure: &crate::RegisterHomeError,
                range_functions: &'ranges [std::sync::Arc<FunctionLiveRanges>],
                batch_indices: &'ranges std::collections::BTreeMap<usize, BatchFunctionIndex>,
            ) -> Option<Self> {
                let (function, register, intrinsic) = match failure {
                    crate::RegisterHomeError::NoCompatibleHome { function, register } => {
                        (*function, *register, false)
                    }
                    crate::RegisterHomeError::NoCommonCandidate { function, register }
                    | crate::RegisterHomeError::UnresolvedEntryTransitions {
                        function,
                        register,
                        ..
                    } => (*function, *register, true),
                    _ => return None,
                };
                let function_ranges = range_functions.get(function)?;
                let mut domain = if let Some(index) = batch_indices.get(&function) {
                    Self::in_function_indexed(
                        function,
                        function_ranges,
                        index,
                        VirtualRegisterId(register),
                    )
                } else {
                    Self::in_function(function, function_ranges, VirtualRegisterId(register))
                };
                if intrinsic {
                    domain.members = vec![VirtualRegisterId(register)];
                    domain.direct_pressure = vec![(VirtualRegisterId(register), 1)];
                    domain.intrinsic = true;
                }
                Some(domain)
            }

            pub(crate) fn in_function_indexed(
                function: usize,
                ranges: &'ranges FunctionLiveRanges,
                index: &'ranges BatchFunctionIndex,
                leader: VirtualRegisterId,
            ) -> Self {
                let members = index.components.members(leader);
                if let [member] = members.as_slice() {
                    let neighbors = index.register_neighbors(*member);
                    let split = neighbors.partition_point(|&r| r < *member);
                    let mut direct_pressure = Vec::with_capacity(neighbors.len() + 1);
                    for &r in &neighbors[..split] {
                        direct_pressure.push((r, 1));
                    }
                    direct_pressure.push((*member, 1));
                    for &r in &neighbors[split..] {
                        direct_pressure.push((r, 1));
                    }
                    let pressure = if neighbors
                        .iter()
                        .all(|&other| index.components.leader(other) == other)
                    {
                        neighbors.iter().map(|&r| (r, 1)).collect()
                    } else {
                        let mut covered: Vec<VirtualRegisterId> = neighbors
                            .iter()
                            .map(|&other| index.components.leader(other))
                            .collect();
                        covered.sort_unstable();
                        covered.dedup();
                        covered.into_iter().map(|c| (c, 1)).collect()
                    };
                    return Self {
                        function,
                        ranges,
                        index: Some(index),
                        members,
                        components: None,
                        pressure,
                        direct_pressure,
                        intrinsic: false,
                    };
                }
                let mut covered = Vec::new();
                let mut direct_covered = Vec::new();
                for &member in &members {
                    direct_covered.push((member, member));
                    for &other in index.register_neighbors(member) {
                        covered.push((index.components.leader(other), member));
                        direct_covered.push((other, member));
                    }
                }
                covered.sort_unstable();
                covered.dedup();
                let pressure = region_counts(covered);
                direct_covered.sort_unstable();
                direct_covered.dedup();
                let direct_pressure = region_counts(direct_covered);
                Self {
                    function,
                    ranges,
                    index: Some(index),
                    members,
                    components: None,
                    pressure,
                    direct_pressure,
                    intrinsic: false,
                }
            }

            pub(crate) fn in_function(
                function: usize,
                ranges: &'ranges FunctionLiveRanges,
                leader: VirtualRegisterId,
            ) -> Self {
                let components =
                    crate::assignment::same_home::SameHomeComponents::from_ranges(ranges);
                let members = components.members(leader);
                let mut covered = Vec::new();
                let mut direct_covered = Vec::new();
                for &member in &members {
                    direct_covered.push((member, member));
                }
                for pair in &ranges.interference {
                    for (member, other) in [(pair.lower, pair.higher), (pair.higher, pair.lower)] {
                        if members.binary_search(&member).is_ok() {
                            covered.push((components.leader(other), member));
                            direct_covered.push((other, member));
                        }
                    }
                }
                covered.sort_unstable();
                covered.dedup();
                let pressure = region_counts(covered);
                direct_covered.sort_unstable();
                direct_covered.dedup();
                let direct_pressure = region_counts(direct_covered);
                Self {
                    function,
                    ranges,
                    index: None,
                    members,
                    components: Some(components),
                    pressure,
                    direct_pressure,
                    intrinsic: false,
                }
            }

            #[cfg(test)]
            pub(crate) fn relief(&self, function: usize, register: VirtualRegisterId) -> usize {
                if function != self.function {
                    return 0;
                }
                if self.members.binary_search(&register).is_ok() {
                    return 1;
                }
                if self.intrinsic {
                    return 0;
                }
                self.pressure
                    .binary_search_by_key(&self.components().leader(register), |(component, _)| {
                        *component
                    })
                    .map_or(0, |position| self.pressure[position].1)
            }

            fn retain_unrelieved(
                &self,
                victim: VirtualRegisterId,
                pending: &mut Vec<crate::RegisterHomeError>,
            ) {
                if pending.is_empty() {
                    return;
                }
                let components = self.components();
                let component = components.leader(victim);
                if self.members.binary_search(&victim).is_ok() {
                    {
                        pending.retain(|failure| match failure {
                            crate::RegisterHomeError::NoCompatibleHome { function, register } => {
                                *function == self.function
                                    && components.leader(VirtualRegisterId(*register)) != component
                            }
                            _ => false,
                        });
                    };
                    return;
                }
                if let Some(index) = self.index {
                    if !components.has_multiple_members(component) {
                        let neighbors = index.register_neighbors(component);
                        if neighbors
                            .iter()
                            .all(|&other| components.leader(other) == other)
                        {
                            pending.retain(|failure| match failure {
                                crate::RegisterHomeError::NoCompatibleHome {
                                    function,
                                    register,
                                } => {
                                    if *function != self.function {
                                        return false;
                                    }
                                    let leader = components.leader(VirtualRegisterId(*register));
                                    leader != component && neighbors.binary_search(&leader).is_err()
                                }
                                _ => false,
                            });
                            return;
                        }
                    }
                    let mut relieved = vec![component];
                    for member in components.members(component) {
                        for &other in index.register_neighbors(member) {
                            relieved.push(components.leader(other));
                        }
                    }
                    relieved.sort_unstable();
                    relieved.dedup();
                    pending.retain(|failure| match failure {
                        crate::RegisterHomeError::NoCompatibleHome { function, register } => {
                            *function == self.function
                                && relieved
                                    .binary_search(&components.leader(VirtualRegisterId(*register)))
                                    .is_err()
                        }
                        _ => false,
                    });
                    return;
                }
                let mut relieved = vec![component];
                for pair in &self.ranges.interference {
                    let lower = components.leader(pair.lower);
                    let higher = components.leader(pair.higher);
                    if lower == component {
                        relieved.push(higher);
                    }
                    if higher == component {
                        relieved.push(lower);
                    }
                }
                relieved.sort_unstable();
                relieved.dedup();
                pending.retain(|failure| match failure {
                    crate::RegisterHomeError::NoCompatibleHome { function, register } => {
                        *function == self.function
                            && relieved
                                .binary_search(&components.leader(VirtualRegisterId(*register)))
                                .is_err()
                    }
                    _ => false,
                });
            }

            pub(crate) fn function(&self) -> usize {
                self.function
            }

            #[cfg(test)]
            fn direct_relief(&self, register: VirtualRegisterId) -> usize {
                self.direct_pressure
                    .binary_search_by_key(&register, |&(reg, _)| reg)
                    .map_or(0, |pos| self.direct_pressure[pos].1)
            }

            fn visit_overlapping_registers_with_direct_relief(
                &self,
                mut f: impl FnMut(VirtualRegisterId, usize),
            ) {
                for &(reg, count) in &self.direct_pressure {
                    f(reg, count);
                }
                if !self.intrinsic {
                    let components = self.components();
                    for &(component, _) in &self.pressure {
                        if components.has_multiple_members(component) {
                            components.visit_members(component, |member| {
                                if self
                                    .direct_pressure
                                    .binary_search_by_key(&member, |&(r, _)| r)
                                    .is_err()
                                {
                                    f(member, 0);
                                }
                            });
                        }
                    }
                }
            }

            #[inline]
            fn live_points_of(&self, register: VirtualRegisterId) -> u64 {
                if let Some(index) = self.index {
                    index
                        .live_points
                        .get(register.0 as usize)
                        .copied()
                        .unwrap_or(0)
                } else {
                    self.ranges
                        .virtual_registers
                        .iter()
                        .find(|range| range.virtual_register == register)
                        .map_or(0, |range| {
                            range
                                .fragments
                                .iter()
                                .map(|fragment| u64::from(fragment.end.0 - fragment.start.0))
                                .sum()
                        })
                }
            }

            #[cfg(test)]
            fn spill_priority(&self, function: usize, register: VirtualRegisterId) -> (usize, u64) {
                (
                    if function == self.function {
                        self.direct_relief(register)
                    } else {
                        0
                    },
                    self.live_points_of(register),
                )
            }
        }

        fn region_counts(
            pairs: Vec<(VirtualRegisterId, VirtualRegisterId)>,
        ) -> Vec<(VirtualRegisterId, usize)> {
            let mut counts: Vec<(VirtualRegisterId, usize)> = Vec::new();
            for (register, _) in pairs {
                if let Some((previous, count)) = counts.last_mut()
                    && *previous == register
                {
                    *count += 1;
                } else {
                    counts.push((register, 1));
                }
            }
            counts
        }

        #[cfg(test)]
        fn candidate_position<Priority: Ord + Copy>(
            failure: &crate::RegisterHomeError,
            roster: &[(usize, VirtualRegisterId)],
            mut overlaps: impl FnMut(&(usize, VirtualRegisterId)) -> bool,
            relief: impl Fn(&(usize, VirtualRegisterId)) -> Priority,
        ) -> Option<usize> {
            match failure {
                crate::RegisterHomeError::NoCompatibleHome { .. } => {}
                crate::RegisterHomeError::NoCommonCandidate { function, register }
                | crate::RegisterHomeError::UnresolvedEntryTransitions {
                    function, register, ..
                } => {
                    return roster.iter().position(|candidate| {
                        *candidate == (*function, VirtualRegisterId(*register))
                    });
                }
                _ => return None,
            };
            roster
                .iter()
                .enumerate()
                .filter(|(_, candidate)| overlaps(candidate))
                .map(|(position, candidate)| (position, relief(candidate)))
                .min_by_key(|(position, relief)| (std::cmp::Reverse(*relief), *position))
                .map(|(position, _)| position)
        }

        struct FunctionRoster {
            function: usize,
            entries: Vec<(usize, VirtualRegisterId)>,
            pos_by_reg: Vec<u32>,
            split_inadmissible_bits: Vec<u64>,
        }

        impl FunctionRoster {
            fn new(function: usize, entries: Vec<(usize, VirtualRegisterId)>) -> Self {
                let max_reg = entries
                    .iter()
                    .map(|&(_, reg)| reg.0 as usize)
                    .max()
                    .unwrap_or(0);
                let mut pos_by_reg = vec![u32::MAX; max_reg + 1];
                for (idx, &(_, reg)) in entries.iter().enumerate() {
                    pos_by_reg[reg.0 as usize] = idx as u32;
                }
                Self {
                    function,
                    entries,
                    pos_by_reg,
                    split_inadmissible_bits: Vec::new(),
                }
            }

            #[inline]
            fn active_position(&self, register: VirtualRegisterId) -> Option<u32> {
                self.pos_by_reg
                    .get(register.0 as usize)
                    .copied()
                    .filter(|&pos| pos != u32::MAX)
            }

            #[inline]
            fn remove_register(&mut self, register: VirtualRegisterId) {
                if let Some(slot) = self.pos_by_reg.get_mut(register.0 as usize) {
                    *slot = u32::MAX;
                }
            }

            #[inline]
            fn is_split_inadmissible(&self, register: VirtualRegisterId) -> bool {
                let word = (register.0 as usize) >> 6;
                self.split_inadmissible_bits
                    .get(word)
                    .is_some_and(|&bits| (bits & (1u64 << (register.0 & 63))) != 0)
            }

            #[inline]
            fn mark_split_inadmissible(&mut self, register: VirtualRegisterId) {
                let word = (register.0 as usize) >> 6;
                if word >= self.split_inadmissible_bits.len() {
                    self.split_inadmissible_bits.resize(word + 1, 0);
                }
                self.split_inadmissible_bits[word] |= 1u64 << (register.0 & 63);
            }

            fn extend(&mut self, iter: impl IntoIterator<Item = (usize, VirtualRegisterId)>) {
                for (func, reg) in iter {
                    let pos = self.entries.len() as u32;
                    self.entries.push((func, reg));
                    let reg_idx = reg.0 as usize;
                    if reg_idx >= self.pos_by_reg.len() {
                        self.pos_by_reg.resize(reg_idx + 1, u32::MAX);
                    }
                    self.pos_by_reg[reg_idx] = pos;
                }
            }
        }

        fn select_roster_candidate(
            failure: &crate::RegisterHomeError,
            roster: &FunctionRoster,
            domain: Option<&FailedDomain<'_>>,
        ) -> Option<(usize, VirtualRegisterId)> {
            match failure {
                crate::RegisterHomeError::NoCompatibleHome { .. } => {}
                crate::RegisterHomeError::NoCommonCandidate { function, register }
                | crate::RegisterHomeError::UnresolvedEntryTransitions {
                    function, register, ..
                } => {
                    if *function != roster.function {
                        return None;
                    }
                    let pos = roster.active_position(VirtualRegisterId(*register))?;
                    return Some(roster.entries[pos as usize]);
                }
                _ => return None,
            };
            let domain = domain?;
            if domain.function() != roster.function {
                return None;
            }
            if let Some(&register) = domain
                .members
                .iter()
                .find(|&&register| roster.active_position(register).is_some())
            {
                return Some((roster.function, register));
            }
            let mut best: Option<((std::cmp::Reverse<(usize, u64)>, u32), VirtualRegisterId)> =
                None;
            domain.visit_overlapping_registers_with_direct_relief(|reg, direct_relief| {
                if let Some(pos) = roster.active_position(reg) {
                    let key = (
                        std::cmp::Reverse((direct_relief, domain.live_points_of(reg))),
                        pos,
                    );
                    if best.is_none_or(|(best_key, _)| key < best_key) {
                        best = Some((key, reg));
                    }
                }
            });
            best.map(|(_, reg)| (roster.function, reg))
        }

        #[allow(clippy::too_many_arguments)]
        fn admissible_split_indexed(
            domain: Option<&FailedDomain<'_>>,
            roster: &mut FunctionRoster,
            selected: &impl ValidatedSelectedAnalysis,
            environment: &register_environment::ValidatedTargetRegisterEnvironment,
            budget: OptimizationWorkBudget,
        ) -> Result<
            Option<((usize, VirtualRegisterId), ValidatedRuntimeSplit)>,
            RuntimeSpillAllocationError,
        > {
            let Some(domain) = domain else {
                return Ok(None);
            };
            if domain.function() != roster.function {
                return Ok(None);
            }
            let plan = selected.selected_plan();
            let can_prefilter = plan.target == environment.target()
                && environment
                    .constraint(environment.selected_keys().copy_i64)
                    .is_some();
            let function_body = if can_prefilter {
                plan.functions.get(domain.function())
            } else {
                None
            };
            let mut ordered: Vec<((std::cmp::Reverse<usize>, u32), (usize, VirtualRegisterId))> =
                Vec::new();
            domain.visit_overlapping_registers_with_direct_relief(|reg, direct_relief| {
                let Some(pos) = roster.active_position(reg) else {
                    return;
                };
                if roster.is_split_inadmissible(reg) {
                    return;
                }
                if let Some(body) = function_body
                    && !body.blocks.has_fixed_definition(reg)
                {
                    roster.mark_split_inadmissible(reg);
                    return;
                }
                ordered.push((
                    (std::cmp::Reverse(direct_relief), pos),
                    (roster.function, reg),
                ));
            });
            if ordered.is_empty() {
                return Ok(None);
            }
            ordered.sort_unstable_by_key(|&(key, _)| key);
            for (_, (function, register)) in ordered {
                match split_selected_runtime_value(
                    selected,
                    function,
                    register,
                    environment,
                    budget,
                ) {
                    Ok(split) => return Ok(Some(((function, register), split))),
                    Err(error) if error.is_inadmissible() => {
                        roster.mark_split_inadmissible(register);
                    }
                    Err(error) => return Err(RuntimeSpillAllocationError::Split(error)),
                }
            }
            Ok(None)
        }

        pub(crate) fn recover_with_reuse(
            source: StagedOptimizedAllocationLegality,
            home_reuse: FunctionHomeReuse,
        ) -> Result<RuntimeSpillAllocation, RuntimeSpillAllocationError> {
            recover_over(RuntimeSpillSource::Legality(source), home_reuse)
        }

        pub(crate) fn recover_after_fixed_view_copies(
            reanalysis: StagedOptimizedSelectedReanalysis,
        ) -> Result<RuntimeSpillAllocation, RuntimeSpillAllocationError> {
            recover_over(
                RuntimeSpillSource::FixedViewCopies(reanalysis),
                FunctionHomeReuse::default(),
            )
        }

        pub(crate) fn recover_after_declined_fixed_view_probe(
            legality: StagedOptimizedAllocationLegality,
            policy: FixedViewCopyPolicy,
        ) -> Result<RuntimeSpillAllocation, RuntimeSpillAllocationError> {
            recover_over(
                RuntimeSpillSource::DeclinedFixedView { legality, policy },
                FunctionHomeReuse::default(),
            )
        }

        pub(crate) fn recover_after_active_resident_rematerialization(
            pressure: crate::StagedOptimizedActiveResidentRematerializationPressure,
        ) -> Result<RuntimeSpillAllocation, RuntimeSpillAllocationError> {
            recover_over(
                RuntimeSpillSource::ActiveResidentRematerialization(pressure),
                FunctionHomeReuse::default(),
            )
        }

        pub(crate) fn recover_after_selected_lowering_with_reuse(
            run: SelectedLoweringResult,
            home_reuse: FunctionHomeReuse,
        ) -> Result<RuntimeSpillAllocation, RuntimeSpillAllocationError> {
            recover_over(RuntimeSpillSource::SelectedLowering(run), home_reuse)
        }

        pub(crate) fn recover_after_pre_allocation_with_reuse(
            run: PreAllocationResult,
            home_reuse: FunctionHomeReuse,
        ) -> Result<RuntimeSpillAllocation, RuntimeSpillAllocationError> {
            recover_over(RuntimeSpillSource::PreAllocation(run), home_reuse)
        }

        fn project_recovery_manifest(
            base: &RuntimeSpillManifestBase,
            recorded: &[PostAllocationSelectedTransformation],
            ranges: &ValidatedLiveRanges,
            legality: &ValidatedAllocationLegality,
            homes: &crate::ValidatedRegisterHomes,
        ) -> Result<crate::ValidatedPostAllocationOptimizationManifest, RuntimeSpillAllocationError>
        {
            crate::assignment::post_allocation_manifest::project_post_allocation_optimization_manifest(
                base.pre_physical,
                base.selected_lowering_completion,
                base.pre_allocation_completion,
                &transformations(&base.prefix, recorded),
                ranges,
                legality,
                homes,
            )
            .map_err(RuntimeSpillAllocationError::Manifest)
        }

        fn publish_recovery(
            source: RuntimeSpillSource,
            rewrite: RuntimeSpillStepRewrite,
            facts: UnsealedRecoveryFacts,
            home_reuse: &mut FunctionHomeReuse,
            recorded: &[PostAllocationSelectedTransformation],
        ) -> Result<RuntimeSpillAllocation, RuntimeSpillAllocationError> {
            let resolved = source.resolve();
            let environment = resolved.legality_stage.register_environment();
            let availability = resolved.legality_stage.allocator_availability();
            let facts = seal_recovery_facts(
                environment,
                availability,
                &resolved.base,
                resolved.liveness,
                resolved.ranges,
                resolved.legality,
                facts,
                &rewrite.selected(),
            )?;
            let homes = assign_reusing(
                environment,
                &facts.ranges,
                &facts.legality,
                Some(home_reuse),
            )
            .map_err(RuntimeSpillAllocationError::Homes)?;
            let manifest = project_recovery_manifest(
                &resolved.manifest,
                recorded,
                &facts.ranges,
                &facts.legality,
                &homes,
            )?;
            Ok(RuntimeSpillAllocation {
                source,
                last: RuntimeSpillStep { rewrite },
                facts,
                homes,
                manifest,
            })
        }

        fn recover_over(
            source: RuntimeSpillSource,
            mut home_reuse: FunctionHomeReuse,
        ) -> Result<RuntimeSpillAllocation, RuntimeSpillAllocationError> {
            let resolved = source.resolve();
            let environment = resolved.legality_stage.register_environment();
            let availability = resolved.legality_stage.allocator_availability();
            let budget = resolved.legality_stage.budget_per_pass();
            let rewrite =
                materialize_selected_fixed_occurrences(&resolved.base, environment, budget)
                    .map(RuntimeSpillStepRewrite::FixedOccurrences)
                    .map_err(RuntimeSpillAllocationError::FixedOccurrences)?;
            let initial_facts = {
                let base_facts = UnsealedRecoveryFacts::from_validated(
                    resolved.liveness,
                    resolved.ranges,
                    resolved.legality,
                );
                reanalyze_unsealed(
                    environment,
                    availability,
                    &resolved.base,
                    &base_facts,
                    &rewrite.selected(),
                )
            }?;
            let mut recorded = vec![rewrite.transformation()];
            let mut failure = match home_reuse.probe_all(
                environment.identity(),
                initial_facts.legality_functions(),
                initial_facts.range_functions(),
                environment.physical(),
            ) {
                Ok(()) => {
                    return publish_recovery(
                        source,
                        rewrite,
                        initial_facts,
                        &mut home_reuse,
                        &recorded,
                    );
                }
                Err(error) if error.admits_runtime_recovery() => error,
                Err(error) => return Err(RuntimeSpillAllocationError::Homes(error)),
            };
            let mut current = rewrite;
            let mut analysis_source = current.clone();
            let mut pending_pressure = home_reuse.take_pressure();
            pending_pressure.reverse();
            pending_pressure.pop();
            let mut current_facts = initial_facts;
            let mut batch_indices: std::collections::BTreeMap<usize, BatchFunctionIndex> =
                std::collections::BTreeMap::new();
            let mut rosters: std::collections::BTreeMap<usize, FunctionRoster> =
                std::collections::BTreeMap::new();
            loop {
                let selected = current.selected();
                let failure_function = match &failure {
                    crate::RegisterHomeError::NoCompatibleHome { function, .. }
                    | crate::RegisterHomeError::NoCommonCandidate { function, .. }
                    | crate::RegisterHomeError::UnresolvedEntryTransitions { function, .. } => {
                        Some(*function)
                    }
                    _ => None,
                };
                if let Some(func_idx) = failure_function
                    && !batch_indices.contains_key(&func_idx)
                    && let Some(func_ranges) = current_facts.range_functions().get(func_idx)
                {
                    batch_indices.insert(func_idx, BatchFunctionIndex::new(func_ranges));
                }
                let domain = FailedDomain::of_indexed(
                    &failure,
                    current_facts.range_functions(),
                    &batch_indices,
                );
                let Some(pressured) = domain.as_ref().map(FailedDomain::function).or({
                    if let crate::RegisterHomeError::NoCommonCandidate { function, .. } = &failure {
                        Some(*function)
                    } else {
                        None
                    }
                }) else {
                    break;
                };
                let old_vreg_len = selected
                    .plan()
                    .functions
                    .get(pressured)
                    .map_or(0, |body| body.virtual_registers.len());
                let roster: &mut FunctionRoster = rosters.entry(pressured).or_insert_with(|| {
                    FunctionRoster::new(pressured, candidates(selected.plan(), pressured))
                });
                let split = admissible_split_indexed(
                    domain.as_ref(),
                    roster,
                    &selected,
                    environment,
                    budget,
                )?;
                let before_use = if split.is_none() {
                    if let crate::RegisterHomeError::NoCommonCandidate { function, register } =
                        &failure
                    {
                        match ValidatedRuntimeSplit::before_fixed_use(
                            &selected,
                            *function,
                            VirtualRegisterId(*register),
                            environment,
                            budget,
                        ) {
                            Ok(split) => Some(split),
                            Err(error) if error.is_inadmissible() => None,
                            Err(error) => return Err(RuntimeSpillAllocationError::Split(error)),
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };
                let mut failed_crossing = None;
                let rewritten_candidate;
                let rewrite = if let Some(split) = before_use {
                    rewritten_candidate =
                        if let crate::RegisterHomeError::NoCommonCandidate { function, register } =
                            failure
                        {
                            (function, VirtualRegisterId(register))
                        } else {
                            unreachable!("before-use split belongs to an intrinsic failure")
                        };
                    RuntimeSpillStepRewrite::Split(split)
                } else if let Some((candidate, split)) = split {
                    rewritten_candidate = candidate;
                    roster.remove_register(candidate.1);
                    if let Some(body) = split.transformed().functions.get(pressured)
                        && let Some(appended) = body.virtual_registers.get(old_vreg_len..)
                    {
                        roster.extend(
                            appended
                                .iter()
                                .filter(|register| is_recovery_candidate(body, register))
                                .map(|register| (pressured, register.id)),
                        );
                    }
                    RuntimeSpillStepRewrite::Split(split)
                } else {
                    let Some((function, register)) =
                        select_roster_candidate(&failure, roster, domain.as_ref())
                    else {
                        if selected.selected_identity()
                            != analysis_source.selected().selected_identity()
                        {
                            drop(domain);
                            batch_indices.clear();
                            let unsealed = reanalyze_unsealed(
                                environment,
                                availability,
                                &analysis_source.selected(),
                                &current_facts,
                                &selected,
                            )?;
                            match home_reuse.probe_all(
                                environment.identity(),
                                unsealed.legality_functions(),
                                unsealed.range_functions(),
                                environment.physical(),
                            ) {
                                Ok(()) => {
                                    return publish_recovery(
                                        source,
                                        current,
                                        unsealed,
                                        &mut home_reuse,
                                        &recorded,
                                    );
                                }
                                Err(error) if error.admits_runtime_recovery() => {
                                    failure = error;
                                    current_facts = unsealed;
                                    analysis_source = current.clone();
                                    pending_pressure = home_reuse.take_pressure();
                                    pending_pressure.reverse();
                                    pending_pressure.pop();
                                    continue;
                                }
                                Err(error) => {
                                    return Err(RuntimeSpillAllocationError::Homes(error));
                                }
                            }
                        }
                        break;
                    };
                    roster.remove_register(register);
                    rewritten_candidate = (function, register);

                    match rematerialize_selected_runtime_value(
                        &selected,
                        function,
                        register,
                        environment,
                        budget,
                    ) {
                        Ok(rewrite) => RuntimeSpillStepRewrite::Rematerialization(rewrite),
                        Err(_) => {
                            let mut bounded_already = None;
                            if pending_pressure.is_empty() {
                                match spill_selected_runtime_value_with_crossing_status(
                                    &selected,
                                    function,
                                    register,
                                    environment,
                                    budget,
                                    RuntimeSpillSpanPolicy::UnitWriteCrossing,
                                ) {
                                    Ok((crossing, false)) => {
                                        bounded_already = Some(crossing);
                                    }
                                    Ok((crossing, true)) => {
                                        let probe = reanalyze_unsealed(
                                            environment,
                                            availability,
                                            &analysis_source.selected(),
                                            &current_facts,
                                            &SelectedProgramRef::new(&crossing),
                                        )?;
                                        match home_reuse.probe_all(
                                            environment.identity(),
                                            probe.legality_functions(),
                                            probe.range_functions(),
                                            environment.physical(),
                                        ) {
                                            Ok(()) => {
                                                let rewrite =
                                                    RuntimeSpillStepRewrite::Spill(crossing);
                                                recorded.push(rewrite.transformation());
                                                return publish_recovery(
                                                    source,
                                                    rewrite,
                                                    probe,
                                                    &mut home_reuse,
                                                    &recorded,
                                                );
                                            }
                                            Err(error) if error.admits_runtime_recovery() => {
                                                failed_crossing = Some(SpillPressureProbe {
                                                    rewrite: crossing,
                                                    facts: probe,
                                                    failure: error,
                                                });
                                            }
                                            Err(error) => {
                                                return Err(RuntimeSpillAllocationError::Homes(
                                                    error,
                                                ));
                                            }
                                        }
                                    }
                                    Err(_) => {}
                                }
                            }
                            if let Some(rewrite) = bounded_already {
                                RuntimeSpillStepRewrite::Spill(rewrite)
                            } else {
                                match spill_selected_runtime_value(
                                    &selected,
                                    function,
                                    register,
                                    environment,
                                    budget,
                                ) {
                                    Ok(rewrite) => RuntimeSpillStepRewrite::Spill(rewrite),
                                    Err(error) if inadmissible(&error) => continue,
                                    Err(error) => {
                                        return Err(RuntimeSpillAllocationError::Rewrite(error));
                                    }
                                }
                            }
                        }
                    }
                };
                if let Some(domain) = &domain {
                    domain.retain_unrelieved(rewritten_candidate.1, &mut pending_pressure);
                } else {
                    pending_pressure.clear();
                }
                if let Some(next_failure) = pending_pressure.pop() {
                    recorded.push(rewrite.transformation());
                    current = rewrite;
                    failure = next_failure;
                    continue;
                }
                drop(domain);
                batch_indices.clear();
                let reused_pressure = match &rewrite {
                    RuntimeSpillStepRewrite::Spill(bounded) => {
                        failed_crossing.and_then(|probe| probe.reuse_for(bounded))
                    }
                    _ => None,
                };
                let (unsealed, probe_result) = if let Some((unsealed, pressure)) = reused_pressure {
                    (unsealed, Err(pressure))
                } else {
                    let unsealed = reanalyze_unsealed(
                        environment,
                        availability,
                        &analysis_source.selected(),
                        &current_facts,
                        &rewrite.selected(),
                    )?;
                    let probe_result = home_reuse.probe_all(
                        environment.identity(),
                        unsealed.legality_functions(),
                        unsealed.range_functions(),
                        environment.physical(),
                    );
                    (unsealed, probe_result)
                };
                recorded.push(rewrite.transformation());
                match probe_result {
                    Ok(()) => {
                        return publish_recovery(
                            source,
                            rewrite,
                            unsealed,
                            &mut home_reuse,
                            &recorded,
                        );
                    }
                    Err(error) if error.admits_runtime_recovery() => {
                        failure = error;
                        current_facts = unsealed;
                        current = rewrite;
                        analysis_source = current.clone();
                        pending_pressure = home_reuse.take_pressure();
                        pending_pressure.reverse();
                        pending_pressure.pop();
                    }
                    Err(error) => return Err(RuntimeSpillAllocationError::Homes(error)),
                }
            }
            Err(RuntimeSpillAllocationError::Homes(failure))
        }

        #[cfg(test)]
        pub(crate) mod candidate_tests {
            use super::{FailedDomain, candidate_position};
            use crate::RegisterHomeError;
            use register_model::RegisterClassId;
            use selected_instructions::{
                DistinctUseDefTie, EdgeRegisterTransfer, FunctionLiveRanges, LiveRangePoint,
                LivenessPosition, SelectedBlockId, SelectedInstructionId, VirtualInterference,
                VirtualRegisterId,
            };
            use semantic_vocabulary::{EdgeId, MachineId};

            fn pressure(function: usize, register: u32) -> RegisterHomeError {
                RegisterHomeError::NoCompatibleHome { function, register }
            }

            #[test]
            fn pressure_ranking_filters_other_functions_and_preserves_equal_score_order() {
                let roster = [
                    (1, VirtualRegisterId(9)),
                    (2, VirtualRegisterId(3)),
                    (2, VirtualRegisterId(9)),
                ];
                assert_eq!(
                    candidate_position(
                        &pressure(2, 9),
                        &roster,
                        |(function, _)| *function == 2,
                        |_| 0
                    ),
                    Some(1)
                );
                assert_eq!(
                    candidate_position(
                        &pressure(2, 9),
                        &roster[..2],
                        |(function, _)| *function == 2,
                        |_| 0
                    ),
                    Some(1)
                );
                assert_eq!(
                    candidate_position(
                        &pressure(2, 9),
                        &roster,
                        |(function, _)| *function == 2,
                        |(_, register)| (1, if register.0 == 3 { 100 } else { 1 })
                    ),
                    Some(1)
                );
            }

            #[test]
            fn incompatible_interval_homes_choose_an_admitted_recovery_candidate() {
                let roster = [(2, VirtualRegisterId(9))];
                assert_eq!(
                    candidate_position(
                        &RegisterHomeError::NoCommonCandidate {
                            function: 2,
                            register: 9
                        },
                        &roster,
                        |_| true,
                        |_| 0
                    ),
                    Some(0)
                );
                assert_eq!(
                    candidate_position(
                        &RegisterHomeError::UnknownOrIncompatibleView {
                            function: 2,
                            register: 9,
                            view: 0
                        },
                        &roster,
                        |_| true,
                        |_| 0
                    ),
                    None
                );
            }

            #[test]
            fn empty_common_domain_can_only_rewrite_the_failed_value() {
                let failure = RegisterHomeError::NoCommonCandidate {
                    function: 2,
                    register: 9,
                };
                let roster = [
                    (2, VirtualRegisterId(7)),
                    (1, VirtualRegisterId(9)),
                    (2, VirtualRegisterId(9)),
                ];
                assert_eq!(
                    candidate_position(&failure, &roster, |_| true, |_| 100),
                    Some(2)
                );
                assert_eq!(
                    candidate_position(&failure, &roster[..2], |_| true, |_| 100),
                    None
                );
                let ranges = split_ranges(&[(9, 10)], &[], &[(9, 7)]);
                let mut domain = FailedDomain::in_function(2, &ranges, VirtualRegisterId(9));
                domain.members = vec![VirtualRegisterId(9)];
                domain.intrinsic = true;
                assert_eq!(domain.relief(2, VirtualRegisterId(9)), 1);
                assert_eq!(domain.relief(2, VirtualRegisterId(7)), 0);
                assert_eq!(domain.relief(2, VirtualRegisterId(10)), 0);
            }

            fn tie(use_register: u32, def_register: u32) -> DistinctUseDefTie {
                DistinctUseDefTie {
                    block: SelectedBlockId(0),
                    position: LivenessPosition(use_register),
                    instruction: SelectedInstructionId(use_register),
                    use_operand: 0,
                    use_virtual_register: VirtualRegisterId(use_register),
                    use_point: LiveRangePoint(use_register),
                    def_operand: 1,
                    def_virtual_register: VirtualRegisterId(def_register),
                    def_point: LiveRangePoint(def_register),
                    class: RegisterClassId(0),
                }
            }

            fn transfer(argument: u32, parameter: u32) -> EdgeRegisterTransfer {
                EdgeRegisterTransfer {
                    source: SelectedBlockId(0),
                    target: SelectedBlockId(1),
                    psi_edge: EdgeId::new(u64::from(argument) + 1).unwrap(),
                    argument: VirtualRegisterId(argument),
                    parameter: VirtualRegisterId(parameter),
                    class: RegisterClassId(0),
                }
            }

            fn split_ranges(
                ties: &[(u32, u32)],
                transfers: &[(u32, u32)],
                interference: &[(u32, u32)],
            ) -> FunctionLiveRanges {
                FunctionLiveRanges {
                    machine: MachineId::new(1).unwrap(),
                    block_domains: Vec::new(),
                    virtual_registers: Vec::new(),
                    tied_pairs: ties
                        .iter()
                        .map(|(used, defined)| tie(*used, *defined))
                        .collect(),
                    edge_transfers: transfers
                        .iter()
                        .map(|(argument, parameter)| transfer(*argument, *parameter))
                        .collect(),
                    copy_affinities: Vec::new(),
                    early_clobbers: Vec::new(),
                    architectural_units: Vec::new(),
                    interference: interference
                        .iter()
                        .map(|(lower, higher)| VirtualInterference {
                            lower: VirtualRegisterId(*lower),
                            higher: VirtualRegisterId(*higher),
                        })
                        .collect(),
                }
            }

            #[test]
            fn pressure_coalesces_across_tied_split_points() {
                let ranges = split_ranges(&[(0, 1)], &[], &[(1, 7)]);
                let domain = FailedDomain::in_function(0, &ranges, VirtualRegisterId(0));
                assert!(domain.relief(0, VirtualRegisterId(7)) > 0);
                assert_eq!(domain.relief(0, VirtualRegisterId(5)), 0);
                assert_eq!(domain.relief(1, VirtualRegisterId(7)), 0);

                let ranges = split_ranges(&[(0, 1), (7, 8)], &[(8, 9)], &[(1, 7), (1, 8)]);
                let domain = FailedDomain::in_function(0, &ranges, VirtualRegisterId(0));
                assert_eq!(domain.relief(0, VirtualRegisterId(9)), 1);
                assert_eq!(domain.relief(0, VirtualRegisterId(7)), 1);
                assert_eq!(domain.relief(0, VirtualRegisterId(5)), 0);
            }

            #[test]
            fn direct_pressure_precedes_longer_peers_without_excluding_them() {
                use selected_instructions::{LiveRangeFragment, VirtualLiveRange};
                let mut ranges = split_ranges(&[], &[(8, 9)], &[(0, 7), (0, 8)]);
                for (register, length) in [(7, 2), (9, 200), (10, 2)] {
                    ranges.virtual_registers.push(VirtualLiveRange {
                        virtual_register: VirtualRegisterId(register),
                        class: RegisterClassId(0),
                        occurrences: Vec::new(),
                        fixed_constraints: Vec::new(),
                        fragments: vec![LiveRangeFragment {
                            block: SelectedBlockId(0),
                            start: LiveRangePoint(0),
                            end: LiveRangePoint(length),
                        }],
                        edge_connectors: Vec::new(),
                    });
                }
                ranges.interference.push(VirtualInterference {
                    lower: VirtualRegisterId(0),
                    higher: VirtualRegisterId(10),
                });
                let domain = FailedDomain::in_function(0, &ranges, VirtualRegisterId(0));
                let roster = [
                    (0, VirtualRegisterId(9)),
                    (0, VirtualRegisterId(7)),
                    (0, VirtualRegisterId(10)),
                ];
                let select = |roster: &[(usize, VirtualRegisterId)]| {
                    candidate_position(
                        &pressure(0, 0),
                        roster,
                        |(function, register)| domain.relief(*function, *register) > 0,
                        |(function, register)| domain.spill_priority(*function, *register),
                    )
                };
                assert_eq!(select(&roster), Some(1));
                assert_eq!(select(&roster[..1]), Some(0));

                let ranges = split_ranges(&[(9, 1), (1, 7)], &[(4, 2)], &[]);
                let components =
                    crate::assignment::same_home::SameHomeComponents::from_ranges(&ranges);
                let roster = [
                    VirtualRegisterId(7),
                    VirtualRegisterId(4),
                    VirtualRegisterId(9),
                    VirtualRegisterId(1),
                    VirtualRegisterId(2),
                ];
                let positions: Vec<_> = roster
                    .iter()
                    .map(|member| {
                        roster
                            .iter()
                            .position(|candidate| *candidate == components.leader(*member))
                    })
                    .collect();
                assert_eq!(positions, [Some(2), Some(1), Some(2), Some(2), Some(1)]);
            }

            #[test]
            fn relief_counts_membership_alongside_covered_siblings() {
                let ranges = split_ranges(&[(0, 1)], &[(1, 2)], &[(0, 9), (1, 9), (2, 9)]);
                let domain = FailedDomain::in_function(0, &ranges, VirtualRegisterId(0));
                assert_eq!(domain.relief(0, VirtualRegisterId(9)), 3);
                assert_eq!(domain.relief(0, VirtualRegisterId(1)), 1);
                assert_eq!(domain.relief(0, VirtualRegisterId(2)), 1);
            }

            #[test]
            fn pressure_batch_defers_shared_and_interfering_regions_but_keeps_disjoint_ones() {
                let ranges = split_ranges(&[(0, 1), (7, 8)], &[(8, 9)], &[(1, 7), (9, 12)]);
                let domain = FailedDomain::in_function(2, &ranges, VirtualRegisterId(0));
                let mut pending = vec![
                    pressure(2, 1),
                    pressure(2, 8),
                    pressure(2, 12),
                    pressure(2, 20),
                ];
                domain.retain_unrelieved(VirtualRegisterId(9), &mut pending);
                assert_eq!(pending, vec![pressure(2, 20)]);
                domain.retain_unrelieved(VirtualRegisterId(7), &mut pending);
                assert_eq!(pending, vec![pressure(2, 20)]);
            }

            #[test]
            fn rewriting_unplaced_component_retains_interfering_pressure_in_order() {
                use super::BatchFunctionIndex;
                let ranges = split_ranges(&[(0, 1), (7, 8)], &[], &[(1, 7)]);
                let index = BatchFunctionIndex::new(&ranges);
                let domains = [
                    FailedDomain::in_function(2, &ranges, VirtualRegisterId(0)),
                    FailedDomain::in_function_indexed(2, &ranges, &index, VirtualRegisterId(0)),
                ];
                for domain in domains {
                    let mut pending = vec![
                        pressure(2, 7),
                        pressure(2, 1),
                        pressure(3, 20),
                        pressure(2, 8),
                        pressure(2, 20),
                    ];
                    domain.retain_unrelieved(VirtualRegisterId(1), &mut pending);
                    assert_eq!(
                        pending,
                        vec![pressure(2, 7), pressure(2, 8), pressure(2, 20)]
                    );
                }
            }

            #[test]
            fn unplaced_component_precedes_neighbors_and_falls_back_after_retirement() {
                use super::{FunctionRoster, select_roster_candidate};
                let ranges = split_ranges(&[(9, 10)], &[], &[(7, 9)]);
                let domain = FailedDomain::in_function(2, &ranges, VirtualRegisterId(9));
                let mut roster = FunctionRoster::new(
                    2,
                    vec![
                        (2, VirtualRegisterId(7)),
                        (2, VirtualRegisterId(9)),
                        (2, VirtualRegisterId(10)),
                    ],
                );
                for register in [9, 10, 7] {
                    assert_eq!(
                        select_roster_candidate(&pressure(2, 9), &roster, Some(&domain)),
                        Some((2, VirtualRegisterId(register)))
                    );
                    roster.remove_register(VirtualRegisterId(register));
                }
                assert_eq!(
                    select_roster_candidate(&pressure(2, 9), &roster, Some(&domain)),
                    None
                );
            }
        }

        fn inadmissible(error: &RuntimeSpillError) -> bool {
            matches!(
                error,
                RuntimeSpillError::UnsupportedValue
                    | RuntimeSpillError::UnsupportedUse
                    | RuntimeSpillError::UnsupportedControlFlow
            )
        }
    }

    use crate::{
        FixedOccurrencesError, RuntimeRematerializationError, RuntimeSpillError, RuntimeSplitError,
        StagedOptimizedSelectedReanalysis,
    };
    use crate::{
        FixedViewCopyPolicy, ValidatedFixedOccurrences, ValidatedRuntimeRematerialization,
        ValidatedRuntimeSpill, ValidatedRuntimeSplit,
    };
    use crate::{PreAllocationResult, SelectedLoweringResult};
    use crate::{ValidatedPostAllocationOptimizationManifest, ValidatedRegisterHomes};
    use optimization_core::{
        PreAllocationOptimizationCompletionIdentity, PrePhysicalOptimizationManifestIdentity,
        SelectedLoweringOptimizationCompletionIdentity,
    };
    pub(crate) use recovery::{
        assign_source_reusing, recover_after_active_resident_rematerialization,
        recover_after_declined_fixed_view_probe, recover_after_fixed_view_copies,
        recover_after_pre_allocation_with_reuse, recover_after_selected_lowering_with_reuse,
        recover_with_reuse,
    };
    use register_homes::{
        PostAllocationOptimizationManifestError, PostAllocationSelectedTransformation,
    };
    use selected_instruction_analyses::{
        AllocationLegalityError, LiveRangeError, LivenessError, SelectedProgramRef,
        StagedOptimizedAllocationLegality, ValidatedAllocationLegality, ValidatedLiveRanges,
        ValidatedLiveness,
    };
    use selected_instructions::SelectedInstructionPlan;

    #[derive(Debug)]
    pub(crate) struct RuntimeSpillAllocation {
        pub(crate) source: RuntimeSpillSource,
        pub(crate) last: RuntimeSpillStep,
        pub(crate) facts: RuntimeSpillFacts,
        pub(crate) homes: ValidatedRegisterHomes,
        pub(crate) manifest: ValidatedPostAllocationOptimizationManifest,
    }

    impl RuntimeSpillAllocation {
        pub(crate) fn recovery_prefix_selection(&self) -> Option<optimization_core::Optimization> {
            self.source.recovery_prefix_selection()
        }
    }

    #[derive(Debug)]
    pub(crate) enum RuntimeSpillSource {
        Legality(StagedOptimizedAllocationLegality),
        DeclinedFixedView {
            legality: StagedOptimizedAllocationLegality,
            policy: FixedViewCopyPolicy,
        },
        FixedViewCopies(StagedOptimizedSelectedReanalysis),
        ActiveResidentRematerialization(
            crate::StagedOptimizedActiveResidentRematerializationPressure,
        ),
        SelectedLowering(SelectedLoweringResult),
        PreAllocation(PreAllocationResult),
    }

    #[derive(Debug)]
    pub(crate) struct RuntimeSpillManifestBase {
        pub(crate) pre_physical: PrePhysicalOptimizationManifestIdentity,
        pub(crate) selected_lowering_completion:
            Option<SelectedLoweringOptimizationCompletionIdentity>,
        pub(crate) pre_allocation_completion: Option<PreAllocationOptimizationCompletionIdentity>,
        pub(crate) prefix: Vec<PostAllocationSelectedTransformation>,
    }

    pub(crate) struct ResolvedSpillSource<'source> {
        pub(crate) legality_stage: &'source StagedOptimizedAllocationLegality,
        pub(crate) base: SelectedProgramRef<'source>,
        pub(crate) liveness: &'source ValidatedLiveness,
        pub(crate) ranges: &'source ValidatedLiveRanges,
        pub(crate) legality: &'source ValidatedAllocationLegality,
        pub(crate) manifest: RuntimeSpillManifestBase,
    }

    impl RuntimeSpillSource {
        fn legality_stage(&self) -> &StagedOptimizedAllocationLegality {
            match self {
                Self::Legality(source) => source,
                Self::DeclinedFixedView { legality, .. } => legality,
                Self::FixedViewCopies(reanalysis) => {
                    reanalysis.transformation_stage().source_legality_stage()
                }
                Self::ActiveResidentRematerialization(pressure) => pressure.source(),
                Self::SelectedLowering(run) => run.source_legality_stage(),
                Self::PreAllocation(run) => run.source_legality_stage(),
            }
        }

        admitted_stage_accessors!(fn pub(crate), legality_stage);

        pub(crate) fn recovery_prefix_selection(&self) -> Option<optimization_core::Optimization> {
            match self {
                Self::ActiveResidentRematerialization(_) => {
                    Some(optimization_core::Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1)
                }
                Self::DeclinedFixedView { policy, .. } => declared_shared_entry_selection(*policy),
                Self::FixedViewCopies(reanalysis) => {
                    declared_shared_entry_selection(reanalysis.custody().source().policy())
                }
                Self::Legality(_) | Self::SelectedLowering(_) | Self::PreAllocation(_) => None,
            }
        }

        pub(crate) fn resolve(&self) -> ResolvedSpillSource<'_> {
            match self {
                Self::Legality(legality) | Self::DeclinedFixedView { legality, .. } => {
                    ResolvedSpillSource {
                        legality_stage: legality,
                        base: SelectedProgramRef::new(legality.selected()),
                        liveness: legality.liveness(),
                        ranges: legality.ranges(),
                        legality: legality.legality(),
                        manifest: RuntimeSpillManifestBase {
                            pre_physical: legality.custody().manifest(),
                            selected_lowering_completion: None,
                            pre_allocation_completion: None,
                            prefix: Vec::new(),
                        },
                    }
                }
                Self::FixedViewCopies(reanalysis) => {
                    let upstream = reanalysis.custody();
                    ResolvedSpillSource {
                        legality_stage: reanalysis.transformation_stage().source_legality_stage(),
                        base: SelectedProgramRef::new(reanalysis.transformation_stage().copies()),
                        liveness: reanalysis.liveness(),
                        ranges: reanalysis.ranges(),
                        legality: reanalysis.legality(),
                        manifest: RuntimeSpillManifestBase {
                            pre_physical: upstream.source().manifest(),
                            selected_lowering_completion: None,
                            pre_allocation_completion: None,
                            prefix: vec![PostAllocationSelectedTransformation::FixedViewCopy(
                                upstream.source().transformation(),
                            )],
                        },
                    }
                }
                Self::ActiveResidentRematerialization(pressure) => {
                    let upstream = pressure.custody();
                    ResolvedSpillSource {
                        legality_stage: pressure.source(),
                        base: SelectedProgramRef::new(pressure.rematerialization()),
                        liveness: pressure.liveness(),
                        ranges: pressure.ranges(),
                        legality: pressure.legality(),
                        manifest: RuntimeSpillManifestBase {
                            pre_physical: upstream.source().manifest(),
                            selected_lowering_completion: None,
                            pre_allocation_completion: None,
                            prefix: vec![
                                PostAllocationSelectedTransformation::PressureRematerialization(
                                    upstream.rematerialization(),
                                ),
                            ],
                        },
                    }
                }
                Self::SelectedLowering(run) => {
                    let upstream = run.ledger();
                    ResolvedSpillSource {
                        legality_stage: run.source_legality_stage(),
                        base: run.current(),
                        liveness: run.liveness(),
                        ranges: run.ranges(),
                        legality: run.legality(),
                        manifest: RuntimeSpillManifestBase {
                            pre_physical: upstream.source().manifest(),
                            selected_lowering_completion: Some(upstream.identity()),
                            pre_allocation_completion: None,
                            prefix: upstream.transformations().to_vec(),
                        },
                    }
                }
                Self::PreAllocation(run) => {
                    let upstream = run.ledger();
                    ResolvedSpillSource {
                        legality_stage: run.source_legality_stage(),
                        base: run.current(),
                        liveness: run.liveness(),
                        ranges: run.ranges(),
                        legality: run.legality(),
                        manifest: RuntimeSpillManifestBase {
                            pre_physical: upstream.source().manifest(),
                            selected_lowering_completion: None,
                            pre_allocation_completion: Some(upstream.identity()),
                            prefix: upstream.transformations().to_vec(),
                        },
                    }
                }
            }
        }
    }

    fn declared_shared_entry_selection(
        policy: FixedViewCopyPolicy,
    ) -> Option<optimization_core::Optimization> {
        (policy == FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1).then_some(
            optimization_core::Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
        )
    }

    #[derive(Debug)]
    pub(crate) struct RuntimeSpillStep {
        pub(crate) rewrite: RuntimeSpillStepRewrite,
    }

    #[derive(Debug, Clone)]
    pub(crate) enum RuntimeSpillStepRewrite {
        FixedOccurrences(ValidatedFixedOccurrences),
        Split(ValidatedRuntimeSplit),
        Spill(ValidatedRuntimeSpill),
        Rematerialization(ValidatedRuntimeRematerialization),
    }

    impl RuntimeSpillStepRewrite {
        pub(crate) fn transformation(&self) -> PostAllocationSelectedTransformation {
            match self {
                Self::FixedOccurrences(rewrite) => {
                    PostAllocationSelectedTransformation::FixedOccurrences(
                        rewrite.receipt().transformed_selected(),
                    )
                }
                Self::Split(rewrite) => PostAllocationSelectedTransformation::RuntimeSplit(
                    rewrite.receipt().transformed_selected(),
                ),
                Self::Spill(rewrite) => PostAllocationSelectedTransformation::RuntimeSpill(
                    rewrite.receipt().transformed_selected(),
                ),
                Self::Rematerialization(rewrite) => {
                    PostAllocationSelectedTransformation::RuntimeRematerialization(
                        rewrite.receipt().transformed_selected(),
                    )
                }
            }
        }

        pub(crate) fn transformed(&self) -> &SelectedInstructionPlan {
            match self {
                Self::FixedOccurrences(rewrite) => rewrite.transformed(),
                Self::Split(rewrite) => rewrite.transformed(),
                Self::Spill(rewrite) => rewrite.transformed(),
                Self::Rematerialization(rewrite) => rewrite.transformed(),
            }
        }

        pub(crate) fn selected(&self) -> SelectedProgramRef<'_> {
            match self {
                Self::FixedOccurrences(rewrite) => SelectedProgramRef::new(rewrite),
                Self::Split(rewrite) => SelectedProgramRef::new(rewrite),
                Self::Spill(rewrite) => SelectedProgramRef::new(rewrite),
                Self::Rematerialization(rewrite) => SelectedProgramRef::new(rewrite),
            }
        }
    }

    #[derive(Debug)]
    pub(crate) struct RuntimeSpillFacts {
        pub(crate) liveness: ValidatedLiveness,
        pub(crate) ranges: ValidatedLiveRanges,
        pub(crate) legality: ValidatedAllocationLegality,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum RuntimeSpillAllocationError {
        FixedOccurrences(FixedOccurrencesError),
        Rewrite(RuntimeSpillError),
        Rematerialization(RuntimeRematerializationError),
        Split(RuntimeSplitError),
        Liveness(LivenessError),
        Ranges(LiveRangeError),
        Legality(AllocationLegalityError),
        Homes(crate::RegisterHomeError),
        Manifest(PostAllocationOptimizationManifestError),
        RecoveryNotRequired,
    }

    stage_error_display!(
        RuntimeSpillAllocationError,
        "runtime spill allocation failed"
    );
}
pub(crate) mod same_home {
    use selected_instructions::{FunctionLiveRanges, VirtualRegisterId};

    pub(crate) struct SameHomeComponents {
        registers: Vec<VirtualRegisterId>,
        root_by_id: Vec<u32>,
        member_offsets: Vec<u32>,
        member_targets: Vec<VirtualRegisterId>,
    }

    impl SameHomeComponents {
        pub(crate) fn from_ranges(ranges: &FunctionLiveRanges) -> Self {
            let relations = || {
                ranges
                    .tied_pairs
                    .iter()
                    .map(|tie| (tie.use_virtual_register, tie.def_virtual_register))
                    .chain(
                        ranges
                            .edge_transfers
                            .iter()
                            .map(|edge| (edge.argument, edge.parameter)),
                    )
            };
            let mut registers: Vec<_> = relations()
                .flat_map(|(left, right)| [left, right])
                .collect();
            registers.sort_unstable();
            registers.dedup();
            let mut roots: Vec<_> = (0..registers.len()).collect();
            for (left, right) in relations() {
                let left = registers.partition_point(|register| *register < left);
                let right = registers.partition_point(|register| *register < right);
                let left = root(&roots, left);
                let right = root(&roots, right);
                roots[right] = left;
            }
            let max_id = registers.last().map_or(0, |reg| reg.0 as usize + 1);
            let mut root_by_id = vec![u32::MAX; max_id];
            let mut counts = vec![0u32; registers.len()];
            for position in 0..roots.len() {
                let r = root(&roots, position);
                roots[position] = r;
                root_by_id[registers[position].0 as usize] = r as u32;
                counts[r] += 1;
            }
            let mut member_offsets = vec![0u32; registers.len() + 1];
            for i in 0..registers.len() {
                member_offsets[i + 1] = member_offsets[i] + counts[i];
            }
            let mut cursors = member_offsets.clone();
            let mut member_targets = vec![VirtualRegisterId(0); registers.len()];
            for (position, &reg) in registers.iter().enumerate() {
                let r = roots[position];
                let cursor = &mut cursors[r];
                member_targets[*cursor as usize] = reg;
                *cursor += 1;
            }
            Self {
                registers,
                root_by_id,
                member_offsets,
                member_targets,
            }
        }

        #[inline]
        fn row(&self, register: VirtualRegisterId) -> Option<usize> {
            self.root_by_id
                .get(register.0 as usize)
                .filter(|&&r| r != u32::MAX)
                .map(|&r| r as usize)
        }

        #[inline]
        pub(crate) fn leader(&self, register: VirtualRegisterId) -> VirtualRegisterId {
            self.row(register).map_or(register, |r| self.registers[r])
        }

        #[inline]
        pub(crate) fn has_multiple_members(&self, register: VirtualRegisterId) -> bool {
            let Some(r) = self.row(register) else {
                return false;
            };
            self.member_offsets[r + 1] - self.member_offsets[r] > 1
        }

        pub(crate) fn members(&self, register: VirtualRegisterId) -> Vec<VirtualRegisterId> {
            let Some(r) = self.row(register) else {
                return vec![register];
            };
            let start = self.member_offsets[r] as usize;
            let end = self.member_offsets[r + 1] as usize;
            self.member_targets[start..end].to_vec()
        }

        #[inline]
        pub(crate) fn visit_members(
            &self,
            register: VirtualRegisterId,
            mut f: impl FnMut(VirtualRegisterId),
        ) {
            let Some(r) = self.row(register) else {
                f(register);
                return;
            };
            let start = self.member_offsets[r] as usize;
            let end = self.member_offsets[r + 1] as usize;
            for &member in &self.member_targets[start..end] {
                f(member);
            }
        }
    }

    fn root(parents: &[usize], mut position: usize) -> usize {
        while parents[position] != position {
            position = parents[position];
        }
        position
    }
}
pub(crate) mod transformed {
    pub(crate) mod construction {
        use crate::assignment::post_allocation_manifest::project_post_allocation_optimization_manifest;
        use crate::{RegisterHomeError, ValidatedRegisterHomes};

        use crate::{
            PreAllocationLedger, PreAllocationResult, SelectedLoweringLedger,
            SelectedLoweringResult,
        };

        use super::projection::{
            pre_allocation_final_analysis, pre_allocation_transformations,
            selected_lowering_final_analysis, selected_lowering_transformations,
        };
        use super::{
            OptimizedPostPreAllocationHomeCustodyError,
            OptimizedPostSelectedLoweringHomeCustodyError,
            StagedOptimizedRegisterHomesAfterPreAllocation,
            StagedOptimizedRegisterHomesAfterSelectedLowering,
        };

        pub(crate) fn assign_register_homes_after_pre_allocation(
            run: &PreAllocationResult,
            reuse: Option<&mut crate::assignment::home_assignment::compute::FunctionHomeReuse>,
        ) -> Result<ValidatedRegisterHomes, RegisterHomeError> {
            let (ranges, legality) = pre_allocation_final_analysis(run);
            let environment = run.source_legality_stage().register_environment();
            crate::assignment::home_assignment::assign_register_homes_reusing(
                legality,
                ranges,
                environment,
                reuse,
            )
        }

        pub(crate) fn stage_optimized_register_homes_after_pre_allocation_with_assignment(
            run: PreAllocationResult,
            homes: ValidatedRegisterHomes,
        ) -> Result<
            StagedOptimizedRegisterHomesAfterPreAllocation,
            OptimizedPostPreAllocationHomeCustodyError,
        > {
            let source = run.ledger().clone();
            finish_register_homes_after_pre_allocation(run, source, homes)
        }

        fn finish_register_homes_after_pre_allocation(
            run: PreAllocationResult,
            source: PreAllocationLedger,
            homes: ValidatedRegisterHomes,
        ) -> Result<
            StagedOptimizedRegisterHomesAfterPreAllocation,
            OptimizedPostPreAllocationHomeCustodyError,
        > {
            let (ranges, legality) = pre_allocation_final_analysis(&run);
            let transformations = pre_allocation_transformations(&source);
            let manifest = project_post_allocation_optimization_manifest(
                source.source().manifest(),
                None,
                Some(source.identity()),
                &transformations,
                ranges,
                legality,
                &homes,
            )
            .map_err(OptimizedPostPreAllocationHomeCustodyError::Manifest)?;
            let custody = crate::assignment::PostHomeCustodyReceipt::new(
                source,
                homes.receipt(),
                manifest.record().identity,
            );
            Ok(StagedOptimizedRegisterHomesAfterPreAllocation {
                run,
                homes,
                manifest,
                custody,
            })
        }

        pub(crate) fn assign_register_homes_after_selected_lowering(
            run: &SelectedLoweringResult,
            reuse: Option<&mut crate::assignment::home_assignment::compute::FunctionHomeReuse>,
        ) -> Result<ValidatedRegisterHomes, RegisterHomeError> {
            let (ranges, legality) = selected_lowering_final_analysis(run);
            let environment = run.source_legality_stage().register_environment();
            crate::assignment::home_assignment::assign_register_homes_reusing(
                legality,
                ranges,
                environment,
                reuse,
            )
        }

        pub(crate) fn stage_optimized_register_homes_after_selected_lowering_with_assignment(
            run: SelectedLoweringResult,
            homes: ValidatedRegisterHomes,
        ) -> Result<
            StagedOptimizedRegisterHomesAfterSelectedLowering,
            OptimizedPostSelectedLoweringHomeCustodyError,
        > {
            let source = run.ledger().clone();
            finish_register_homes_after_selected_lowering(run, source, homes)
        }

        fn finish_register_homes_after_selected_lowering(
            run: SelectedLoweringResult,
            source: SelectedLoweringLedger,
            homes: ValidatedRegisterHomes,
        ) -> Result<
            StagedOptimizedRegisterHomesAfterSelectedLowering,
            OptimizedPostSelectedLoweringHomeCustodyError,
        > {
            let (ranges, legality) = selected_lowering_final_analysis(&run);
            let transformations = selected_lowering_transformations(&source);
            let manifest = project_post_allocation_optimization_manifest(
                source.source().manifest(),
                Some(source.identity()),
                None,
                &transformations,
                ranges,
                legality,
                &homes,
            )
            .map_err(OptimizedPostSelectedLoweringHomeCustodyError::Manifest)?;
            let custody = crate::assignment::PostHomeCustodyReceipt::new(
                source,
                homes.receipt(),
                manifest.record().identity,
            );
            Ok(StagedOptimizedRegisterHomesAfterSelectedLowering {
                run,
                homes,
                manifest,
                custody,
            })
        }
    }
    pub(crate) mod projection {
        use register_homes::PostAllocationSelectedTransformation;

        use crate::{
            PreAllocationLedger, PreAllocationResult, SelectedLoweringLedger,
            SelectedLoweringResult,
        };
        use selected_instruction_analyses::{ValidatedAllocationLegality, ValidatedLiveRanges};

        pub(crate) fn selected_lowering_final_analysis(
            run: &SelectedLoweringResult,
        ) -> (&ValidatedLiveRanges, &ValidatedAllocationLegality) {
            (run.ranges(), run.legality())
        }

        pub(crate) fn pre_allocation_final_analysis(
            run: &PreAllocationResult,
        ) -> (&ValidatedLiveRanges, &ValidatedAllocationLegality) {
            (run.ranges(), run.legality())
        }

        pub(crate) fn pre_allocation_transformations(
            source: &PreAllocationLedger,
        ) -> Vec<PostAllocationSelectedTransformation> {
            source.transformations().to_vec()
        }

        pub(crate) fn selected_lowering_transformations(
            source: &SelectedLoweringLedger,
        ) -> Vec<PostAllocationSelectedTransformation> {
            source.transformations().to_vec()
        }
    }

    use crate::{
        PreAllocationLedger, PreAllocationResult, SelectedLoweringLedger, SelectedLoweringResult,
    };
    use crate::{
        RegisterHomeError, ValidatedPostAllocationOptimizationManifest, ValidatedRegisterHomes,
    };
    use register_homes::PostAllocationOptimizationManifestError;
    use selected_instruction_analyses::{
        SelectedProgramRef, ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedLiveness,
    };

    use super::PostHomeCustodyReceipt;

    pub(crate) use construction::{
        assign_register_homes_after_pre_allocation as assign_optimized_register_homes_after_pre_allocation,
        assign_register_homes_after_selected_lowering as assign_optimized_register_homes_after_selected_lowering,
        stage_optimized_register_homes_after_pre_allocation_with_assignment,
        stage_optimized_register_homes_after_selected_lowering_with_assignment,
    };

    macro_rules! source_evidence_accessors {
        ($field:ident) => {
            pub const fn register_environment(
                &self,
            ) -> &register_environment::ValidatedTargetRegisterEnvironment {
                self.$field.source_legality_stage().register_environment()
            }
            pub fn selections(&self) -> &optimization_core::OptimizationSelections {
                self.$field.source_legality_stage().selections()
            }
            pub fn budget_per_pass(&self) -> optimization_core::OptimizationWorkBudget {
                self.$field.source_legality_stage().budget_per_pass()
            }
            assignment_context_accessors!();
        };
    }

    #[derive(Debug)]
    pub(crate) struct StagedOptimizedRegisterHomesAfterSelectedLowering {
        run: SelectedLoweringResult,
        homes: ValidatedRegisterHomes,
        manifest: ValidatedPostAllocationOptimizationManifest,
        custody: StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
    }

    impl StagedOptimizedRegisterHomesAfterSelectedLowering {
        pub fn selected(&self) -> SelectedProgramRef<'_> {
            self.run.current()
        }
        pub fn liveness(&self) -> &ValidatedLiveness {
            self.run.liveness()
        }
        pub fn ranges(&self) -> &ValidatedLiveRanges {
            self.run.ranges()
        }
        pub fn legality(&self) -> &ValidatedAllocationLegality {
            self.run.legality()
        }
        source_evidence_accessors!(run);
        receipt_accessor_rows!(borrow StagedOptimizedRegisterHomesAfterSelectedLowering {
            custody: &StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
        });
    }

    pub type StagedOptimizedPostSelectedLoweringHomeCustodyReceipt =
        PostHomeCustodyReceipt<SelectedLoweringLedger>;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum OptimizedPostSelectedLoweringHomeCustodyError {
        Assignment(RegisterHomeError),
        Manifest(PostAllocationOptimizationManifestError),
        ReceiptMismatch,
    }

    stage_error_display!(
        OptimizedPostSelectedLoweringHomeCustodyError,
        "optimized post-selected-lowering home staging failed"
    );

    #[derive(Debug)]
    pub(crate) struct StagedOptimizedRegisterHomesAfterPreAllocation {
        run: PreAllocationResult,
        homes: ValidatedRegisterHomes,
        manifest: ValidatedPostAllocationOptimizationManifest,
        custody: StagedOptimizedPostPreAllocationHomeCustodyReceipt,
    }

    impl StagedOptimizedRegisterHomesAfterPreAllocation {
        pub fn selected(&self) -> SelectedProgramRef<'_> {
            self.run.current()
        }
        pub fn liveness(&self) -> &ValidatedLiveness {
            self.run.liveness()
        }
        pub fn ranges(&self) -> &ValidatedLiveRanges {
            self.run.ranges()
        }
        pub fn legality(&self) -> &ValidatedAllocationLegality {
            self.run.legality()
        }
        source_evidence_accessors!(run);
        receipt_accessor_rows!(borrow StagedOptimizedRegisterHomesAfterPreAllocation {
            custody: &StagedOptimizedPostPreAllocationHomeCustodyReceipt,
        });
    }

    pub type StagedOptimizedPostPreAllocationHomeCustodyReceipt =
        PostHomeCustodyReceipt<PreAllocationLedger>;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum OptimizedPostPreAllocationHomeCustodyError {
        Assignment(RegisterHomeError),
        Manifest(PostAllocationOptimizationManifestError),
        ReceiptMismatch,
    }

    stage_error_display!(
        OptimizedPostPreAllocationHomeCustodyError,
        "optimized post-pre-allocation home staging failed"
    );
}
