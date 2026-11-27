/*
Target lowering receives the abstract program Stage 02 lowered, unchanged: the
receiving native lowerer does not rerun Psi passes
(`wiki/spec/terminal-psi/product.md#optimization-ordering`), and no optimization
runs on abstract operations. `OptimizedAbstractProgram::unoptimized` pairs the
plan with its unit and issues the three identities stages 04 to 08 and image
emission compare: the optimization identity, the plan projection and the
pre-physical manifest identity.

Each identity digests only what it binds, because nothing runs between the
unit and target lowering: the selections, the Terminal program and the unit,
and the per-pass budget. The archived native optimizer that issued them from a
rule set, decision log and transformation ledger is under
`_recycling/source-rust/subsystems/compiler/psi-to-native/pipeline/03_abstract-operations-to-abstract-operations/`.
*/

use std::sync::Arc;

use abstract_operations::AbstractOperationPlan;
use optimization_core::{
    OptimizationExecutionPhase, OptimizationIdentityBundleIdentity, OptimizationSelections,
    OptimizationWorkBudget, OptimizedAbstractPlanProjectionIdentity,
    PrePhysicalOptimizationManifestIdentity,
};

use crate::{OptimizedAbstractProgram, PsiOptimizationUnit};

/// Why the abstract program cannot be handed to target lowering unoptimized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnoptimizedAbstractProgramError {
    /// The build selected an optimization for a phase that no native stage runs.
    /// Running without it would silently drop what the build asked for.
    UnconsumedOptimizationPhase(OptimizationExecutionPhase),
    /// The unit was built from another program than the plan it accompanies.
    UnitDetachedFromPlan,
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
        let psi = OptimizationExecutionPhase::Psi;
        if !selections.for_phase(psi).is_empty() {
            return Err(UnoptimizedAbstractProgramError::UnconsumedOptimizationPhase(psi));
        }
        if unit.psi != plan.psi || unit.entry != plan.entry {
            return Err(UnoptimizedAbstractProgramError::UnitDetachedFromPlan);
        }
        let optimization = OptimizationIdentityBundleIdentity::from_canonical_bytes(
            &selections.identity().bytes(),
        );
        let projection = projection_identity(&unit, &selections);
        let pre_physical_manifest = manifest_identity(projection, &selections, budget_per_pass);
        Ok(Self {
            plan,
            unit,
            selections,
            budget_per_pass,
            optimization,
            projection,
            pre_physical_manifest,
        })
    }
}

/// The projection binds the Terminal program, its work schedule and the unit
/// target lowering reads to the selections it was lowered under.
fn projection_identity(
    unit: &PsiOptimizationUnit,
    selections: &OptimizationSelections,
) -> OptimizedAbstractPlanProjectionIdentity {
    let mut canonical = Vec::with_capacity(102);
    canonical.extend_from_slice(&unit.psi.vocabulary_marker.get().to_le_bytes());
    canonical.extend_from_slice(unit.psi.program_fingerprint.as_bytes());
    canonical.extend_from_slice(&unit.work_schedule.marker().to_le_bytes());
    canonical.extend_from_slice(&unit.identity.bytes());
    canonical.extend_from_slice(&selections.identity().bytes());
    OptimizedAbstractPlanProjectionIdentity::from_canonical_bytes(&canonical)
}

fn manifest_identity(
    projection: OptimizedAbstractPlanProjectionIdentity,
    selections: &OptimizationSelections,
    budget_per_pass: OptimizationWorkBudget,
) -> PrePhysicalOptimizationManifestIdentity {
    let mut canonical = Vec::with_capacity(96);
    canonical.extend_from_slice(&projection.bytes());
    canonical.extend_from_slice(&selections.identity().bytes());
    canonical.extend_from_slice(&budget_per_pass.encode());
    PrePhysicalOptimizationManifestIdentity::from_canonical_bytes(&canonical)
}
