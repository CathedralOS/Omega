//! Named physical routes selected by the exact build manifest.

mod allocation_recovery;
mod selected_phases;

pub(in crate::coordination::physical_pipeline) use allocation_recovery::stage_allocation_recovery_pipeline;
pub(in crate::coordination::physical_pipeline) use selected_phases::stage_non_allocation_recovery_physical_pipeline;
