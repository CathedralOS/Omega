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
    pub source: psi_compiler::SourceContinuation,
    pub base_sources: Arc<source::SourceMap>,
    pub computed_build_config: Arc<ComputedBuildConfig>,
    pub admission: BuildAdmission,
    /// The exact successful Build language judgment, only when requested for
    /// another review pass. This is not an execution or authority checkpoint.
    pub retained_build_check: Option<psi_compiler::RetainedCheck>,
}

/// One completed continuation with its source custody and inclusive timing ladder.
#[derive(Clone)]
pub struct PreparedBuild {
    pub built: BuiltTypedProgram,
    pub build_sources: BuildSourceCustody,
    pub timings: CompileTimings,
}

/// Complete authored and generated Product source, consumed once by Psi checking.
/// Realization never receives a mutable copy of this source continuation.
pub struct PreparedProductSource {
    pub source: Arc<psi_compiler::PreparedSource>,
    pub computed_build_config: Arc<ComputedBuildConfig>,
    pub admission: BuildAdmission,
    pub build_sources: BuildSourceCustody,
    pub timings: CompileTimings,
    pub retained_build_check: Option<psi_compiler::RetainedCheck>,
}

/// One source judgment shared by every realization in the invocation.
/// The source checker produces this before any target provider is selected.
#[derive(Clone)]
pub struct CheckedProductSource {
    pub program: Arc<checked_trees::CheckedTrees>,
    /// All authored Product defaults, shared unchanged across target settlement.
    pub provider_defaults:
        Arc<[build_time_evaluation::provider_declarations::TargetProviderDefaults]>,
    pub computed_build_config: Arc<ComputedBuildConfig>,
    pub admission: BuildAdmission,
    pub sources: Arc<package_compilation::CheckedSourceCustody>,
    pub timings: CompileTimings,
    pub accepted_template_classifications: trust_model::AcceptedTemplateClassifications,
    pub retained_checks: crate::RetainedCompilationChecks,
    pub product_selection: Option<psi_compiler::CheckedTreeProductSelection>,
    /// The deferred constant evaluation each target closure resumes under its
    /// selected rows. Empty when the program retains no constant recipe.
    pub constant_recipe_continuations: Vec<build_time_evaluation::PreCheckEvaluation>,
    /// The typed program those continuations fold on: the authored source
    /// before executable preparation, where boundary token uses are still
    /// their authored occurrences. Present only beside continuations.
    pub constant_recipe_source: Option<Arc<typed_trees::TypedTrees>>,
}
