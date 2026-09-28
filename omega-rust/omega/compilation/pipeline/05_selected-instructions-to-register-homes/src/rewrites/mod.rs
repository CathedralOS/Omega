//! Optimizer module role: stage group. Selected-instruction rewrites and retained replay evidence.

mod rematerialization;

pub(crate) use rematerialization::complete_optimized_active_resident_rematerialization;
pub use rematerialization::{
    OptimizedActiveResidentRematerializationError, StagedOptimizedActiveResidentRematerialization,
    StagedOptimizedActiveResidentRematerializationCustodyReceipt,
    StagedOptimizedActiveResidentRematerializationPressure,
    StagedOptimizedActiveResidentRematerializationPressureCustodyReceipt,
    stage_optimized_active_resident_rematerialization,
    stage_optimized_active_resident_rematerialization_pressure,
    validate_optimized_active_resident_rematerialization,
    validate_optimized_active_resident_rematerialization_pressure,
};
