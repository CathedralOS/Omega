/*
Abstract-optimization context admission (`context`), optimized plan projection
sealing (`projection`), and pre-physical optimization manifest projection
(`prephysical_manifest`).
*/

pub(crate) use abstract_operations::AbstractOperation as O;
pub(crate) use std::collections::{BTreeMap, BTreeSet};

pub(crate) mod context {
    use optimization_unit::PsiOptimizationUnit;
    use optimization_unit_semantics::OptimizationUnitValidationError;

    pub(in crate::abstract_optimization) use crate::abstract_optimization::ranked_cycles_validation as ranked_cycles;

    /// Admits the runtime evidence and exact ranked cycles of the unit stage 03
    /// built. The admitted cycle roster served only the parked Psi rules, so the
    /// verdict is all that remains.
    pub(crate) fn validate_psi_context(
        input: &verified_optimization_input::VerifiedPsiOptimizationInput,
        unit: &PsiOptimizationUnit,
    ) -> Result<(), OptimizationUnitValidationError> {
        optimization_unit::validate_runtime_evidence_context(
            unit,
            input.plan().psi,
            input.context().module(),
        )
        .map_err(|reason| OptimizationUnitValidationError::InvalidRuntimeEvidence { reason })?;
        ranked_cycles::validate_exact_ranked_cycles(input, unit)
    }
}
pub(crate) mod prephysical_manifest {
    use optimization_core::PrePhysicalOptimizationManifestIdentity;
    use optimization_unit::{
        OptimizationManifestStage, OptimizationStructuralStatistics,
        PhysicalOptimizationDataStatus, PrePhysicalOptimizationManifest, PsiOptimizationUnit,
    };

    use crate::abstract_optimization::OptimizationRun;
    use crate::abstract_optimization::validation::ValidatedOptimizedAbstractPlanProjection;

    /// The manifest compares the optimized unit with the unit stage 03 built
    /// before any pass ran. The caller records that unit's identity and
    /// statistics once, before the session attaches ownership frontier facts.
    #[derive(Debug, Clone, Copy)]
    pub(crate) struct InitialUnitRecord {
        identity: optimization_core::OptimizationUnitIdentity,
        statistics: OptimizationStructuralStatistics,
    }

    impl InitialUnitRecord {
        pub(crate) fn of(
            unit: &PsiOptimizationUnit,
        ) -> Result<Self, PrePhysicalOptimizationManifestError> {
            Ok(Self {
                identity: unit.identity,
                statistics: structural_statistics(unit)?,
            })
        }
    }

    pub(crate) fn project_pre_physical_optimization_manifest(
        run: &OptimizationRun,
        initial: InitialUnitRecord,
        projection: ValidatedOptimizedAbstractPlanProjection,
    ) -> Result<ValidatedPrePhysicalOptimizationManifest, PrePhysicalOptimizationManifestError>
    {
        let final_unit = run.session().unit();
        let mut record = PrePhysicalOptimizationManifest {
            identity: PrePhysicalOptimizationManifestIdentity::from_canonical_bytes(b"pending"),
            stage: OptimizationManifestStage::PrePhysicalAbstractPlan,
            physical_data: PhysicalOptimizationDataStatus::UnavailableBeforePhysicalRealization,
            psi: final_unit.psi,
            work_schedule: final_unit.work_schedule,
            initial_unit: initial.identity,
            final_unit: final_unit.identity,
            projection: projection.identity(),
            selections: run.selections().clone(),
            psi_selections: run.psi_selections().clone(),
            budget_per_pass: run.budget_per_pass(),
            usage: run.usage(),
            decision_log: run.decisions().clone(),
            pass_manifests: run.pass_manifests().to_vec(),
            transformation_ledger: run.transformation_ledger().clone(),
            identity_bundle: run.identity_bundle(),
            source_statistics: initial.statistics,
            optimized_statistics: structural_statistics(final_unit)?,
        };
        record.identity = record.recomputed_identity();
        Ok(ValidatedPrePhysicalOptimizationManifest { record })
    }

    fn structural_statistics(
        unit: &PsiOptimizationUnit,
    ) -> Result<OptimizationStructuralStatistics, PrePhysicalOptimizationManifestError> {
        let count = |value: usize| {
            u64::try_from(value)
                .map_err(|_| PrePhysicalOptimizationManifestError::StructuralStatisticsOverflow)
        };
        Ok(OptimizationStructuralStatistics {
            functions: count(unit.functions.len())?,
            blocks: count(
                unit.functions
                    .iter()
                    .map(|function| function.blocks.len())
                    .sum(),
            )?,
            nodes: count(unit.blocks().map(|block| block.nodes.len()).sum())?,
            scalar_definitions: count(
                unit.functions
                    .iter()
                    .map(|function| {
                        function.parameters.len()
                            + function
                                .blocks
                                .iter()
                                .map(|block| {
                                    block.parameters.len()
                                        + block
                                            .nodes
                                            .iter()
                                            .map(|node| node.definitions.len())
                                            .sum::<usize>()
                                })
                                .sum::<usize>()
                    })
                    .sum(),
            )?,

            scalar_uses: count(unit.block_nodes().map(|node| node.uses.len()).sum())?,
            optimization_facts: count(
                unit.functions
                    .iter()
                    .map(|function| function.facts.len())
                    .sum(),
            )?,
            ownership_frontier_facts: count(unit.ownership_frontier_facts.len())?,
        })
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ValidatedPrePhysicalOptimizationManifest {
        record: PrePhysicalOptimizationManifest,
    }

    impl ValidatedPrePhysicalOptimizationManifest {
        pub const fn record(&self) -> &PrePhysicalOptimizationManifest {
            &self.record
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum PrePhysicalOptimizationManifestError {
        StructuralStatisticsOverflow,
    }

    failed_display!(
        PrePhysicalOptimizationManifestError,
        "invalid pre-physical optimization manifest"
    );
}
pub(crate) mod projection {
    use optimization_core::{
        OptimizationSelectionIdentity, OptimizationUnitIdentity, OptimizationValidatorIdentity,
        OptimizedAbstractPlanProjectionIdentity, TransformationLedgerIdentity,
    };
    use semantic_vocabulary::WorkScheduleIdentity;
    use terminal_psi::TerminalPsiIdentity;

    use crate::abstract_optimization::OptimizationRun;

    pub(crate) fn seal_optimized_abstract_plan_projection(
        run: &OptimizationRun,
    ) -> ValidatedOptimizedAbstractPlanProjection {
        let final_unit = run.session().unit();
        let ledger = run.transformation_ledger();
        ValidatedOptimizedAbstractPlanProjection {
            psi: final_unit.psi,
            work_schedule: final_unit.work_schedule,
            initial_unit: ledger.input(),
            final_unit: final_unit.identity,
            selections: run.selections().identity(),
            psi_selections: run.psi_selections().identity(),
            ledger: ledger.identity(),
            bundle: run.identity_bundle().identity(),
            validator: OptimizationValidatorIdentity::from_canonical_bytes(
                b"omega.validator.optimized-abstract-plan-projection.v33",
            ),
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ValidatedOptimizedAbstractPlanProjection {
        psi: TerminalPsiIdentity,
        work_schedule: WorkScheduleIdentity,
        initial_unit: OptimizationUnitIdentity,
        final_unit: OptimizationUnitIdentity,
        selections: OptimizationSelectionIdentity,
        psi_selections: OptimizationSelectionIdentity,
        ledger: TransformationLedgerIdentity,
        bundle: optimization_core::OptimizationIdentityBundleIdentity,
        validator: OptimizationValidatorIdentity,
    }

    impl ValidatedOptimizedAbstractPlanProjection {
        pub const fn psi(self) -> TerminalPsiIdentity {
            self.psi
        }

        pub const fn final_unit(self) -> OptimizationUnitIdentity {
            self.final_unit
        }

        pub fn identity(self) -> OptimizedAbstractPlanProjectionIdentity {
            let mut canonical = Vec::with_capacity(272);
            crate::abstract_optimization::codec_bytes::push_encoded(
                &mut canonical,
                self.psi.vocabulary_marker.get().to_le_bytes(),
            );
            canonical.extend_from_slice(self.psi.program_fingerprint.as_bytes());
            crate::abstract_optimization::codec_bytes::push_encoded(
                &mut canonical,
                self.work_schedule.marker().to_le_bytes(),
            );
            canonical.extend_from_slice(&self.initial_unit.bytes());
            canonical.extend_from_slice(&self.final_unit.bytes());
            canonical.extend_from_slice(&self.selections.bytes());
            canonical.extend_from_slice(&self.psi_selections.bytes());
            canonical.extend_from_slice(&self.ledger.bytes());
            canonical.extend_from_slice(&self.bundle.bytes());
            canonical.extend_from_slice(&self.validator.bytes());
            OptimizedAbstractPlanProjectionIdentity::from_canonical_bytes(&canonical)
        }
    }
}

pub(crate) use context::validate_psi_context;
pub use prephysical_manifest::ValidatedPrePhysicalOptimizationManifest;
pub(crate) use prephysical_manifest::{
    InitialUnitRecord, PrePhysicalOptimizationManifestError,
    project_pre_physical_optimization_manifest,
};
pub(crate) use projection::{
    ValidatedOptimizedAbstractPlanProjection, seal_optimized_abstract_plan_projection,
};
