//! Optimizer module role: stage group. Named physical routes selected by the exact build manifest.

mod composition;
mod current_allocation;
mod selected_phases;

pub(crate) use composition::{
    ResolvedPhysicalPhaseComposition, ResolvedRealizationPlan, resolve_physical_phase_composition,
};
pub(in crate::native_pipeline::physical_pipeline) use current_allocation::stage_current_allocation_function_relative_pipeline;
pub(in crate::native_pipeline::physical_pipeline) use selected_phases::realize_allocated_program;
