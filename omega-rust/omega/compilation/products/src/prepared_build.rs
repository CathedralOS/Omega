//! Completed Build and Product source continuations.
//!
//! Build executes once for an invocation. Product completion consumes its pending
//! bodies and generated sources once before target settlement starts. Neither
//! carrier authorizes execution or repeats a stage on behalf of its consumer.

use crate::{BuildAdmission, ComputedBuildConfig};
use artifacts::compile_timings::CompileTimings;
use package_compilation::BuildSourceCustody;
use std::sync::Arc;

/// Evaluated Build and the retained frontend that an actual Product demand
/// completes. Pending authored bodies and generated sources grant no execution.
#[derive(Clone)]
pub struct BuiltTypedProgram {
    pub typing: symbol_resolved_trees_to_typed_trees::SeededTypingBase,
    pub pending_bodies: syntax_trees_to_symbol_resolved_trees::DeferredResolvedFrontier,
    pub base_sources: Arc<source::SourceMap>,
    pub pending_pre_checks: Vec<build_time_evaluation::PreCheckEvaluation>,
    pub computed_build_config: Arc<ComputedBuildConfig>,
    pub admission: BuildAdmission,
    /// The exact successful Build language judgment, only when requested for
    /// another review pass. This is not an execution or authority checkpoint.
    pub retained_build_check: Option<typed_trees_to_checked_trees::RetainedCheck>,
}

/// One completed continuation with its source custody and inclusive timing ladder.
#[derive(Clone)]
pub struct PreparedBuild {
    pub built: BuiltTypedProgram,
    pub build_sources: BuildSourceCustody,
    pub timings: CompileTimings,
}

/// Complete authored and generated Product source, independent of realization.
/// Target settlement still mutates typed metadata and therefore consumes its own
/// copy; it must not execute Build or complete source bodies again. Moving those
/// remaining mutations behind Terminal permits sharing checked storage instead.
#[derive(Clone)]
pub struct PreparedProductSource {
    pub typed: typed_trees::TypedTrees,
    pub pending_pre_checks: Vec<build_time_evaluation::PreCheckEvaluation>,
    pub computed_build_config: Arc<ComputedBuildConfig>,
    pub admission: BuildAdmission,
    pub build_sources: BuildSourceCustody,
    pub timings: CompileTimings,
    pub retained_build_check: Option<typed_trees_to_checked_trees::RetainedCheck>,
}
