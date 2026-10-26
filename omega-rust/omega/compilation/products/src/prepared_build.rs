//! Completed Build and Product source continuations.
//!
//! Build executes once for an invocation. Product completion consumes its pending
//! bodies and generated sources once before target settlement starts. Neither
//! carrier authorizes execution or repeats a stage on behalf of its consumer.

use crate::{BuildAdmission, ComputedBuildConfig};
use artifacts::compile_timings::CompileTimings;
use package_compilation::BuildSourceCustody;
use std::sync::Arc;

/// Settled Build output before any deferred Product context is prepared.
/// The parse prefix supplies fresh contexts; the typed frontier is reusable
/// only for its own assembly inputs or its still-deferred Product source.
pub struct SettledBuildSource {
    pub parsed: crate::PreparedCheckedSource,
    pub frontier: crate::TypedCompilationFrontier,
    pub evaluated: crate::EvaluatedBuild,
    pub package_inputs: Option<package_compilation::PackageCompilationInputs>,
    pub original_assembly: package_compilation::SourceAssemblyInputs,
    pub build_execution_profile: Option<target::TargetProfile>,
    pub timings: CompileTimings,
    pub build_elapsed_micros: u128,
}

/// The invocation's fixed source and execution custody beside Product defaults.
pub struct SettledPackageBuild {
    pub root: std::path::PathBuf,
    pub target: target::TargetProfile,
    pub inputs: package_compilation::PackageCompilationInputs,
    pub rollback: optimization_core::OptimizationRollback,
    pub permit_unsettled_fused_service_fields: bool,
    pub source: SettledBuildSource,
}

impl SettledPackageBuild {
    /// Fork only for a real sibling Product. The final consumer can move the
    /// original source/storage instead; no execution custody is duplicated.
    pub fn fork(&mut self) -> Self {
        Self {
            root: self.root.clone(),
            target: self.target,
            inputs: self.inputs.clone(),
            rollback: self.rollback.clone(),
            permit_unsettled_fused_service_fields: self.permit_unsettled_fused_service_fields,
            source: SettledBuildSource {
                parsed: self.source.parsed.fork(),
                frontier: self.source.frontier.clone(),
                evaluated: self.source.evaluated.clone(),
                package_inputs: self.source.package_inputs.clone(),
                original_assembly: self.source.original_assembly.clone(),
                build_execution_profile: self.source.build_execution_profile,
                timings: self.source.timings.clone(),
                build_elapsed_micros: self.source.build_elapsed_micros,
            },
        }
    }
}

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
    pub typed: typed_trees::TypedTrees,
    pub pending_pre_checks: Vec<build_time_evaluation::PreCheckEvaluation>,
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
