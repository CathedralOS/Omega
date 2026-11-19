// Completed Build and Product source continuations (`SettledBuildSource`,
// `SettledPackageBuild`, `BuiltTypedProgram`, `PreparedBuild`,
// `PreparedProductSource` and `CheckedProductSource`).
// Build executes once for an invocation and Product completion consumes its
// pending bodies and generated sources once before target settlement starts.

use crate::{BuildAdmission, ComputedBuildConfig};
use artifacts::compile_timings::CompileTimings;
use package_compilation::BuildSourceCustody;
use std::sync::Arc;

pub struct SettledBuildSource {
    pub execution_contract: package_compilation::build_execution::BuildExecutionContract,
    pub parsed: crate::PreparedCheckedSource,
    pub frontier: crate::TypedCompilationFrontier,
    pub evaluated: crate::EvaluatedBuild,
    pub package_inputs: Option<package_compilation::PackageCompilationInputs>,
    pub original_assembly: package_compilation::SourceAssemblyInputs,
    pub build_execution_profile: Option<target::TargetProfile>,
    pub timings: CompileTimings,
    pub build_elapsed_micros: u128,
}

pub struct SettledPackageBuild {
    pub root: std::path::PathBuf,
    pub target: target::TargetProfile,
    pub inputs: package_compilation::PackageCompilationInputs,
    pub rollback: optimization_requests::OptimizationRollback,
    pub source: SettledBuildSource,
}

impl SettledPackageBuild {
    pub fn fork(&mut self) -> Self {
        Self {
            root: self.root.clone(),
            target: self.target,
            inputs: self.inputs.clone(),
            rollback: self.rollback.clone(),
            source: SettledBuildSource {
                execution_contract: self.source.execution_contract.clone(),
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

#[derive(Clone)]
pub struct BuiltTypedProgram {
    /// Historical Build costs stay separate from Product value evidence.
    pub build_accounting: source_compiler::evaluation::SourceCompletionAccounting,
    pub anonymous_source: Option<package_compilation::AnonymousSourceCapture>,
    pub preparation: source_compiler::SourcePreparation,
    pub source: source_compiler::SourceContinuation,
    pub base_sources: Arc<source_compiler::source_inputs::SourceMap>,
    pub computed_build_config: Arc<ComputedBuildConfig>,
    pub admission: BuildAdmission,
}

#[derive(Clone)]
pub struct PreparedBuild {
    pub built: BuiltTypedProgram,
    pub build_sources: BuildSourceCustody,
    pub timings: CompileTimings,
}

#[derive(Clone)]
pub struct PreparedProductSource {
    pub build_accounting: source_compiler::evaluation::SourceCompletionAccounting,
    pub source_completion_accounting: source_compiler::evaluation::SourceCompletionAccounting,
    pub typed: source_compiler::source_state::TypedTrees,
    pub computed_build_config: Arc<ComputedBuildConfig>,
    pub admission: BuildAdmission,
    pub build_sources: BuildSourceCustody,
    pub timings: CompileTimings,
}

#[derive(Clone)]
pub struct CheckedProductSource {
    pub optimizations: optimization_selections::OptimizationSelections,
    pub source_completion_accounting: source_compiler::evaluation::SourceCompletionAccounting,
    pub program: Arc<source_compiler::source_state::FrozenTypedTrees>,
    pub provider_inventory:
        Arc<source_compiler::source_state::provider_declarations::PreparedProviderInventory>,
    pub terminal_publication: Arc<source_compiler::SourcePublication>,
    pub computed_build_config: Arc<ComputedBuildConfig>,
    pub admission: BuildAdmission,
    pub sources: Arc<package_compilation::CheckedSourceCustody>,
    pub timings: CompileTimings,
}

impl CheckedProductSource {
    pub fn shares_terminal_publication(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.program, &other.program)
            && self
                .terminal_publication
                .roster()
                .ok()
                .zip(other.terminal_publication.roster().ok())
                .is_some_and(|(left, right)| left.inspection_root == right.inspection_root)
            && self
                .terminal_publication
                .optimizations()
                .eq(other.terminal_publication.optimizations())
            && self
                .computed_build_config
                .config
                .same_source_choices(&other.computed_build_config.config)
            && self.sources == other.sources
    }

    pub fn adopt_derived_code(&mut self, canonical: &Self) {
        debug_assert!(canonical.shares_terminal_publication(self));
        self.terminal_publication = Arc::clone(&canonical.terminal_publication);
    }
}
