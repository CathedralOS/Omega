//! Optimizer module role: stage group. Named physical routes selected by the exact build manifest.

mod allocation_recovery;
mod composition;
mod identity;
mod selected_phases;

pub(in crate::coordination::physical_pipeline) use allocation_recovery::stage_allocation_recovery_pipeline;
pub(crate) use composition::{
    ResolvedPhysicalPhaseComposition, ResolvedRealizationPlan, resolve_physical_phase_composition,
};
pub(in crate::coordination::physical_pipeline) use identity::stage_identity_function_relative_pipeline;
pub(in crate::coordination::physical_pipeline) use selected_phases::stage_allocation_and_realization;
