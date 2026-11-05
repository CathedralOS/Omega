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
    pub execution_contract: package_compilation::build_execution::BuildExecutionContract,
    pub parsed: crate::PreparedCheckedSource,
    pub frontier: crate::TypedCompilationFrontier,
    pub evaluated: crate::EvaluatedBuild,
    pub package_inputs: Option<package_compilation::PackageCompilationInputs>,
    pub original_assembly: package_compilation::SourceAssemblyInputs,
    pub build_execution_profile: Option<target::TargetProfile>,
    pub toolchain_provider_defaults: Option<package_compilation::CompletedProviderDefaults>,
    pub timings: CompileTimings,
    pub build_elapsed_micros: u128,
}

/// The invocation's fixed source and execution custody beside Product defaults.
pub struct SettledPackageBuild {
    pub root: std::path::PathBuf,
    pub target: target::TargetProfile,
    pub inputs: package_compilation::PackageCompilationInputs,
    pub rollback: optimization_core::OptimizationRollback,
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
            source: SettledBuildSource {
                execution_contract: self.source.execution_contract.clone(),
                parsed: self.source.parsed.fork(),
                frontier: self.source.frontier.clone(),
                evaluated: self.source.evaluated.clone(),
                package_inputs: self.source.package_inputs.clone(),
                original_assembly: self.source.original_assembly.clone(),
                build_execution_profile: self.source.build_execution_profile,
                toolchain_provider_defaults: self.source.toolchain_provider_defaults.clone(),
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
    /// The source assembly that produced this Product declaration context,
    /// separate from the input capture consumed by its earlier Build.
    pub anonymous_source: Option<package_compilation::AnonymousSourceCapture>,
    pub preparation: crate::SourcePreparation,
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
    /// Real logical charges for authored completion, retained on both cold and
    /// reused paths. Generated admission and Build charges remain independent.
    pub source_completion_accounting: build_time_evaluation::SourceCompletionAccounting,
    pub typed: typed_trees::TypedTrees,
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
    pub source_completion_accounting: build_time_evaluation::SourceCompletionAccounting,
    pub program: Arc<checked_trees::CheckedTrees>,
    /// Complete source demand is shared across targets without forcing fallible
    /// external declarations or treating nominal references as invocations.
    pub provider_inventory: Arc<psi_compiler::PreparedProviderInventory>,
    /// Build-native declaration formation is complete before portable publication.
    /// Targets attach Calling to these exact surfaces without reopening source.
    pub native_bindings: Arc<provider_planning::native_bindings::PreparedNativeBuildBindings>,
    /// One portable publication for both compile-time policies and final
    /// Product applications. The holder carries no source or target selection.
    pub terminal_publication: Arc<crate::ProductTerminalPublication>,
    /// Calling declarations and exact policy entries in that Product artifact.
    pub calling_program: Arc<
        std::sync::OnceLock<
            Result<
                Arc<provider_planning::calling_policy_plans::CallingProgram>,
                Vec<diagnostics::Diagnostic>,
            >,
        >,
    >,
    /// Checks shared by target binding, captured before Terminal publication.
    pub binding_declarations:
        Arc<std::sync::OnceLock<Arc<selected_dispatch::PreparedBindingDeclarations>>>,
    /// All authored Product defaults, shared unchanged across target settlement.
    pub provider_defaults:
        Arc<[build_time_evaluation::provider_declarations::TargetProviderDefaults]>,
    pub computed_build_config: Arc<ComputedBuildConfig>,
    pub admission: BuildAdmission,
    pub sources: Arc<package_compilation::CheckedSourceCustody>,
    pub timings: CompileTimings,
    pub retained_checks: crate::RetainedCompilationChecks,
    pub product_selection: Option<psi_compiler::CheckedTreeProductSelection>,
}

impl CheckedProductSource {
    /// Whether both judgments feed the same inputs to portable publication:
    /// the shared program, equal source custody, equal Build source choices,
    /// equal provider defaults and the same fixed Psi projection. Target-scoped
    /// admissions and semantic bindings are realization inputs, never read by
    /// the producing operation.
    pub fn shares_terminal_publication(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.program, &other.program)
            && self
                .terminal_publication
                .optimizations()
                .eq(other.terminal_publication.optimizations())
            && self
                .computed_build_config
                .config
                .same_source_choices(&other.computed_build_config.config)
            && self.provider_defaults == other.provider_defaults
            && self.sources == other.sources
    }

    /// Adopt a sibling's derived-code owners so the invocation produces its
    /// portable artifact, policy catalog and external declarations once. Only
    /// meaningful for `shares_terminal_publication`; source judgment, target
    /// admission, receipts and timings stay with this checked product.
    pub fn adopt_derived_code(&mut self, canonical: &Self) {
        debug_assert!(canonical.shares_terminal_publication(self));
        self.terminal_publication = Arc::clone(&canonical.terminal_publication);
        self.calling_program = Arc::clone(&canonical.calling_program);
        self.binding_declarations = Arc::clone(&canonical.binding_declarations);
    }
}

/// Compare source request identities without exposing their trees. A weak owner
/// keeps its allocation identity distinct even after source has been dropped.
#[derive(Clone, Debug)]
pub struct ProductSourceIdentity(std::sync::Weak<checked_trees::CheckedTrees>);

impl ProductSourceIdentity {
    pub fn new(program: &Arc<checked_trees::CheckedTrees>) -> Self {
        Self(Arc::downgrade(program))
    }
    pub fn same_source(&self, other: &Self) -> bool {
        std::sync::Weak::ptr_eq(&self.0, &other.0)
    }
}
