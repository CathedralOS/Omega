/*
Target lowering receives the abstract program Stage 02 lowered, unchanged: the
receiving native lowerer does not rerun Psi passes
(`wiki/spec/terminal-psi/product.md#optimization-ordering`), and no optimization runs on abstract
operations. `OptimizedAbstractProgram::unoptimized` still issues the identity
records later stages bind to (the optimization bundle, the plan projection and
the pre-physical manifest), so those stages keep one record shape whether or
not an optimizer ever runs here again.

Each record is the one an empty pass run produced, byte for byte: an empty rule
set and decision log, a transformation ledger whose input and output are the
same unit, and identical source and optimized statistics. The archived native
optimizer that issued these records with passes is under
`_recycling/source-rust/subsystems/compiler/psi-to-native/pipeline/03_abstract-operations-to-abstract-operations/`.
*/

use std::sync::Arc;

use abstract_operations::AbstractOperationPlan;
use optimization_core::{
    BaselineDecisionLog, OptimizationExecutionPhase, OptimizationIdentityBundle,
    OptimizationRuleSetIdentity, OptimizationSelections, OptimizationValidatorIdentity,
    OptimizationWorkBudget, OptimizedAbstractPlanProjectionIdentity,
    PrePhysicalOptimizationManifestIdentity, TargetCostModelIdentity,
};

use crate::{
    OptimizationManifestStage, OptimizationRunUsage, OptimizationStructuralStatistics,
    OptimizedAbstractProgram, PhysicalOptimizationDataStatus, PrePhysicalOptimizationManifest,
    PsiOptimizationUnit, PsiTransformationLedger,
};

/// Why the abstract program cannot be handed to target lowering unoptimized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnoptimizedAbstractProgramError {
    /// The build selected an optimization for a phase that no native stage runs.
    /// Running without it would silently drop what the build asked for.
    UnconsumedOptimizationPhase(OptimizationExecutionPhase),
    /// The unit was built from another program than the plan it accompanies.
    UnitDetachedFromPlan,
    EmptyDecisionLog(optimization_core::BaselineDecisionLogDecodeError),
    EmptyTransformationLedger(crate::InvalidPsiTransformationLedger),
    StructuralStatisticsOverflow,
}

impl std::fmt::Display for UnoptimizedAbstractProgramError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "cannot publish the unoptimized abstract program: {self:?}"
        )
    }
}

impl std::error::Error for UnoptimizedAbstractProgramError {}

impl OptimizedAbstractProgram {
    pub fn unoptimized(
        plan: Arc<AbstractOperationPlan>,
        unit: PsiOptimizationUnit,
        selections: OptimizationSelections,
        budget_per_pass: OptimizationWorkBudget,
    ) -> Result<Self, UnoptimizedAbstractProgramError> {
        for phase in [
            OptimizationExecutionPhase::Psi,
            OptimizationExecutionPhase::AbstractOperations,
        ] {
            if !selections.for_phase(phase).is_empty() {
                return Err(UnoptimizedAbstractProgramError::UnconsumedOptimizationPhase(phase));
            }
        }
        if unit.psi != plan.psi || unit.entry != plan.entry {
            return Err(UnoptimizedAbstractProgramError::UnitDetachedFromPlan);
        }
        let psi_selections = selections.for_phase(OptimizationExecutionPhase::Psi);
        let rule_set = OptimizationRuleSetIdentity::from_ordered_rules(&[])
            .expect("an empty rule list has no duplicate rule");
        let decision_log = BaselineDecisionLog::concatenate(std::iter::empty())
            .map_err(UnoptimizedAbstractProgramError::EmptyDecisionLog)?;
        let ledger = PsiTransformationLedger::new(
            unit.psi,
            unit.work_schedule,
            unit.identity,
            unit.identity,
            Vec::new(),
        )
        .map_err(UnoptimizedAbstractProgramError::EmptyTransformationLedger)?;
        let identity_bundle = OptimizationIdentityBundle::new(
            selections.identity(),
            rule_set,
            TargetCostModelIdentity::from_canonical_bytes(
                b"omega.psi-baseline-structural-cost-model.v1",
            ),
            Some(decision_log.identity),
            None,
            ledger.identity(),
        );
        let projection = projection_identity(
            &unit,
            &selections,
            &psi_selections,
            &ledger,
            identity_bundle,
        );
        let statistics = structural_statistics(&unit)?;
        let mut pre_physical_manifest = PrePhysicalOptimizationManifest {
            identity: PrePhysicalOptimizationManifestIdentity::from_canonical_bytes(b"pending"),
            stage: OptimizationManifestStage::PrePhysicalAbstractPlan,
            physical_data: PhysicalOptimizationDataStatus::UnavailableBeforePhysicalRealization,
            psi: unit.psi,
            work_schedule: unit.work_schedule,
            initial_unit: unit.identity,
            final_unit: unit.identity,
            projection,
            selections: selections.clone(),
            psi_selections,
            budget_per_pass,
            usage: OptimizationRunUsage::default(),
            decision_log,
            pass_manifests: Vec::new(),
            transformation_ledger: ledger,
            identity_bundle,
            source_statistics: statistics,
            optimized_statistics: statistics,
        };
        pre_physical_manifest.identity = pre_physical_manifest.recomputed_identity();
        Ok(Self {
            plan,
            projection_psi: unit.psi,
            final_unit: unit.identity,
            unit,
            selections,
            budget_per_pass,
            identity_bundle,
            projection,
            pre_physical_manifest,
        })
    }
}

/// The projection identity binds the Terminal program, the unit before and after
/// optimization, both selections, the ledger and the bundle under one validator name.
fn projection_identity(
    unit: &PsiOptimizationUnit,
    selections: &OptimizationSelections,
    psi_selections: &OptimizationSelections,
    ledger: &PsiTransformationLedger,
    bundle: OptimizationIdentityBundle,
) -> OptimizedAbstractPlanProjectionIdentity {
    let validator = OptimizationValidatorIdentity::from_canonical_bytes(
        b"omega.validator.optimized-abstract-plan-projection.v33",
    );
    let mut canonical = Vec::with_capacity(272);
    crate::codec_bytes::push_encoded(
        &mut canonical,
        unit.psi.vocabulary_marker.get().to_le_bytes(),
    );
    canonical.extend_from_slice(unit.psi.program_fingerprint.as_bytes());
    crate::codec_bytes::push_encoded(&mut canonical, unit.work_schedule.marker().to_le_bytes());
    canonical.extend_from_slice(&ledger.input().bytes());
    canonical.extend_from_slice(&unit.identity.bytes());
    canonical.extend_from_slice(&selections.identity().bytes());
    canonical.extend_from_slice(&psi_selections.identity().bytes());
    canonical.extend_from_slice(&ledger.identity().bytes());
    canonical.extend_from_slice(&bundle.identity().bytes());
    canonical.extend_from_slice(&validator.bytes());
    OptimizedAbstractPlanProjectionIdentity::from_canonical_bytes(&canonical)
}

fn structural_statistics(
    unit: &PsiOptimizationUnit,
) -> Result<OptimizationStructuralStatistics, UnoptimizedAbstractProgramError> {
    let count = |value: usize| {
        u64::try_from(value)
            .map_err(|_| UnoptimizedAbstractProgramError::StructuralStatisticsOverflow)
    };
    let scalar_definitions = unit
        .functions
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
        .sum();
    Ok(OptimizationStructuralStatistics {
        functions: count(unit.functions.len())?,
        blocks: count(
            unit.functions
                .iter()
                .map(|function| function.blocks.len())
                .sum(),
        )?,
        nodes: count(unit.blocks().map(|block| block.nodes.len()).sum())?,
        scalar_definitions: count(scalar_definitions)?,
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
