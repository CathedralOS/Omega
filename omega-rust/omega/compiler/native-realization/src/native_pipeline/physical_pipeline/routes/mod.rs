//! Optimizer module role: stage group. Named physical routes selected by the exact build manifest.

mod allocation_recovery;
mod composition;
mod identity;
mod selected_phases;

pub(in crate::native_pipeline::physical_pipeline) use allocation_recovery::realize_recovered_allocation;
pub(crate) use composition::{
    ResolvedPhysicalPhaseComposition, ResolvedRealizationPlan, resolve_physical_phase_composition,
};
pub(in crate::native_pipeline::physical_pipeline) use identity::stage_identity_function_relative_pipeline;
pub(in crate::native_pipeline::physical_pipeline) use selected_phases::realize_allocated_program;
