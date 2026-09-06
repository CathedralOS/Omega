use crate::StagedOptimizedX86BranchRelaxation;
use machine_code::ResolvedMachineLayout;
use optimization_core::{OptimizationPhaseSelections, OptimizationWorkBudget};
use std::sync::Arc;

/// Admission is private to this phase; retained raw data grant no authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLayoutOptimization {
    pub(super) current: Arc<ResolvedMachineLayout>,
    pub(super) selections: OptimizationPhaseSelections,
    pub(super) budget: OptimizationWorkBudget,
    pub(super) relaxation: Option<StagedOptimizedX86BranchRelaxation>,
}

impl ResolvedLayoutOptimization {
    pub fn layout(&self) -> &ResolvedMachineLayout {
        &self.current
    }
    pub fn shared_layout(&self) -> Arc<ResolvedMachineLayout> {
        Arc::clone(&self.current)
    }
    pub fn relaxation(&self) -> Option<&StagedOptimizedX86BranchRelaxation> {
        self.relaxation.as_ref()
    }
    pub fn selections(&self) -> &OptimizationPhaseSelections {
        &self.selections
    }
    pub const fn budget(&self) -> OptimizationWorkBudget {
        self.budget
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn relaxation_mut_for_test(&mut self) -> Option<&mut StagedOptimizedX86BranchRelaxation> {
        self.relaxation.as_mut()
    }
    #[cfg(any(test, feature = "test-support"))]
    pub fn current_program_mut_for_test(&mut self) -> &mut ResolvedMachineLayout {
        Arc::make_mut(&mut self.current)
    }
    #[cfg(any(test, feature = "test-support"))]
    pub fn substitute_shared_layout_for_test(&mut self, current: Arc<ResolvedMachineLayout>) {
        self.current = current;
    }
}
