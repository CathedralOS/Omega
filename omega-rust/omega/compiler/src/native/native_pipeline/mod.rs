//! Optimizer module role: stage group. Compiler-facing entrances that select and report complete pipeline routes.

mod abstract_operation_optimization;
pub(crate) mod physical_pipeline;
mod report;

pub use abstract_operation_optimization::{
    EmptyOptimizationSelections, ExplicitOptimizationRequest, OptimizationPipelineError,
    OptimizationPipelineRequest, compiler_baseline_request_v1, optimize_artifact_sections,
    optimize_verified_abstract_input,
};
pub use physical_pipeline::{
    OptimizedVerifiedPhysicalPipelineError, StagedOptimizedVerifiedPhysicalPipeline,
    stage_optimized_verified_physical_pipeline,
};
pub use report::{
    OptimizationPipelineReport, optimization_pipeline_report,
    optimization_pipeline_report_from_object_artifact,
    optimization_pipeline_report_from_ordinary_callable_entry,
};
