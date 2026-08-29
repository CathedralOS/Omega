//! Named physical routes selected by the exact build manifest.

mod active_resident;
mod selected_phases;

pub(in crate::coordination::physical_pipeline) use active_resident::{
    stage_active_resident_rematerialization_live_ranges,
    stage_active_resident_rematerialization_pipeline,
};
pub(in crate::coordination::physical_pipeline) use selected_phases::stage_non_active_resident_rematerialization_physical_pipeline;
