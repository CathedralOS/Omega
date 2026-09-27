//! Optimizer module role: stage group. Compiler-facing entrances that select and report complete pipeline routes.

pub(crate) mod physical_pipeline;
mod report;

pub use physical_pipeline::{
    OptimizedVerifiedPhysicalPipelineError, stage_optimized_verified_physical_pipeline,
};
pub use report::{
    OptimizationPipelineReport, optimization_pipeline_report,
    optimization_pipeline_report_from_object_artifact,
    optimization_pipeline_report_from_ordinary_callable_entry,
};
