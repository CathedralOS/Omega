//! Producer/replay computation join for x86 branch relaxation.
//!
//! Production and independent replay retain separate scans, inspection, work
//! charging, and reflow mechanics. This coordinator exposes those two routes
//! and gives both traces to the single canonical artifact finalizer.

mod artifact;
mod branch_inspection;
mod production;
mod reflow;
mod replay;
#[cfg(test)]
mod tests;
mod work;

use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::ValidatedPhysicalRegisterModel;

use crate::{ResolvedSelectedFunctionLayout, StagedOptimizedResolvedSelectedFormLayout};

use super::{
    error::OptimizedX86BranchRelaxationError,
    model::{
        StagedOptimizedX86BranchRelaxation, X86BranchRelaxationAction, X86BranchRelaxationAttempt,
    },
};

struct RelaxationTrace {
    usage: OptimizationWorkUsage,
    attempts: Vec<X86BranchRelaxationAttempt>,
    actions: Vec<X86BranchRelaxationAction>,
    functions: Vec<ResolvedSelectedFunctionLayout>,
}

pub(super) fn compute_relaxation(
    source: &StagedOptimizedResolvedSelectedFormLayout,
    physical: &ValidatedPhysicalRegisterModel,
    budget: OptimizationWorkBudget,
) -> Result<StagedOptimizedX86BranchRelaxation, OptimizedX86BranchRelaxationError> {
    let trace = production::compute_trace(source, physical, budget)?;
    artifact::finish_artifact(source, budget, trace)
}

pub(super) fn replay_relaxation(
    source: &StagedOptimizedResolvedSelectedFormLayout,
    physical: &ValidatedPhysicalRegisterModel,
    budget: OptimizationWorkBudget,
) -> Result<StagedOptimizedX86BranchRelaxation, OptimizedX86BranchRelaxationError> {
    let trace = replay::replay_trace(source, physical, budget)?;
    artifact::finish_artifact(source, budget, trace)
}
