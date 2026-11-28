use std::sync::Arc;

use register_homes::PostAllocationSelectedTransformation;
use selected_instructions::SelectedInstructionPlan;

/// What earlier selected-instruction rewrites did to the program a stage
/// receives: the program each completed Omega 04 phase ended with, and the
/// transformations they applied in order. Allocation copies it into its
/// manifest as evidence; it never chooses how a later stage runs.
/// Instruction selection's output carries the empty record. A validator
/// that kept a phase's run compares that run's output with the program
/// named here.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SelectedOptimizationRecord {
    pub selected_lowering_output: Option<Arc<SelectedInstructionPlan>>,
    pub pre_allocation_output: Option<Arc<SelectedInstructionPlan>>,
    pub transformations: Vec<PostAllocationSelectedTransformation>,
}

impl SelectedOptimizationRecord {
    /// Whether an Omega 04 rewrite phase produced the program.
    pub const fn is_rewritten(&self) -> bool {
        self.selected_lowering_output.is_some() || self.pre_allocation_output.is_some()
    }

    /// This record followed by transformations a later stage applied.
    pub fn followed_by(
        &self,
        later: &[PostAllocationSelectedTransformation],
    ) -> Vec<PostAllocationSelectedTransformation> {
        let mut transformations = Vec::with_capacity(self.transformations.len() + later.len());
        transformations.extend_from_slice(&self.transformations);
        transformations.extend_from_slice(later);
        transformations
    }
}
