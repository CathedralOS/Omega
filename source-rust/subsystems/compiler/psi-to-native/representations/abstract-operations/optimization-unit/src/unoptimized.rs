/*
Target lowering receives the abstract program Stage 02 lowered, unchanged: the
receiving native lowerer does not rerun Psi passes
(`wiki/spec/terminal-psi/product.md#optimization-ordering`), and no optimization
runs on abstract operations. `OptimizedAbstractProgram::unoptimized` pairs the
plan with its unit, the build's selections and the per-pass budget.

Later stages compare those inputs directly: the unit identity, which already
digests the Terminal program and its work schedule, the selection identity and
the budget. The archived native optimizer, which issued separate optimization,
projection and manifest identities from a rule set, decision log and
transformation ledger, is under
`_recycling/source-rust/subsystems/compiler/psi-to-native/pipeline/03_abstract-operations-to-abstract-operations/`.
*/

use std::sync::Arc;

use abstract_operations::AbstractOperationPlan;
use optimization_core::{
    OptimizationExecutionPhase, OptimizationSelections, OptimizationWorkBudget,
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
        Ok(Self {
            plan,
            unit,
            selections,
            budget_per_pass,
        })
    }
}
