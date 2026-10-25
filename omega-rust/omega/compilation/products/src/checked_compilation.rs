//! Checked compilation and the source custody sealed against it.
//!
//! The sealed record retains each stage product as the unit its producer
//! returned: selected execution, the shared evaluated Build, this
//! compilation's Build admission, provider settlement, checked-tree product
//! selection and source custody. Consumers borrow the unit they read; the
//! methods here are those units, the joins that span them, and the record's
//! measurement and ownership transfer.

use crate::{BuildAdmission, ComputedBuildConfig};
use artifacts::compile_timings::CompileTimings;
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use package_compilation::CheckedSourceCustody;
use provider_planning::ProviderSettlement;
use selected_dispatch::SelectedExecution;
use std::sync::Arc;

mod report;
mod terminal;

pub use report::{CheckedProductReport, ProductReportPlan, ProductReportRequest};

/// Invocation-local language judgments retained by the compiler session. Build
/// and Product are independent checked programs; neither judgment carries
/// provider selection, executor resources or consent.
#[derive(Debug, Clone, Default)]
pub struct RetainedCompilationChecks {
    pub build: Option<psi_compiler::RetainedCheck>,
    pub product: Option<psi_compiler::RetainedCheck>,
    /// Retained Build judgments made by sibling occurrences of this same
    /// invocation. Sibling rows scope different entries through projections
    /// that can still arrive equal; an exact-input match takes the earlier
    /// judgment rather than repeating the whole-program check it owns.
    /// The pool is invocation-local: rows never reuse a sibling's admission.
    pub sibling_build_checks: Vec<psi_compiler::RetainedCheck>,
    /// Retained product judgments from earlier targets of this same request.
    /// The single-slot `product` chain only reaches the immediately prior
    /// target; a target whose program diverged would orphan a later target
    /// that converges with an earlier one. An exact-input match takes that
    /// earlier judgment; authority admission still runs per invocation.
    pub sibling_product_checks: Vec<psi_compiler::RetainedCheck>,
}

/// Whole stage products supplied when sealing a checked compilation, each as
/// the record its producing owner returned. These inputs are not an admission
/// receipt. Sealing retains them immutably; downstream consumers still
/// independently reconstruct their evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedCompilationInputs {
    /// Checked semantics with target/entry custody and every selected-execution
    /// sidecar, produced by selected dispatch.
    pub settled: SelectedExecution,
    /// One immutable evaluated Build shared by target products and review clones.
    /// Authored choices stay distinct from the realized settlements beside it.
    pub build: Arc<ComputedBuildConfig>,
    pub build_admission: BuildAdmission,
    /// Source-owned defaults include unrequested deployments and share storage
    /// across realizations. They are not effects of the earlier Build execution.
    pub provider_defaults:
        Arc<[build_time_evaluation::provider_declarations::TargetProviderDefaults]>,
    pub providers: ProviderSettlement,
    /// Completed independent dependency products from this invocation. These
    /// retain source-free production inputs, not installed provider authority.
    pub independent_components: Vec<Arc<crate::IndependentComponentProduct>>,
    /// Identity-bearing checked-tree product selection evidence, present only
    /// when the effective build selection named `CheckedTreeProductPruning`.
    /// `None` means the checked-tree phase ran as the identity boundary. The
    /// retained roots and pruned roster re-derive the exact product; they are
    /// not a reselection input.
    pub product_selection: Option<psi_compiler::CheckedTreeProductSelection>,
}

/// Psi-checked semantics paired with the Omega-owned provider realization
/// selected for one engine run. The semantic program deliberately does not
/// retain target/provider installation state.
///
/// Clones share immutable program storage. Temporary review types belong to
/// projection-local trees, not to this evidence-bearing result. Consumers still
/// perform their independent reconstruction.
///
/// ```compile_fail
/// fn replace_program(checked: &mut compilation_products::CheckedCompilation) {
///     let _ = &mut checked.execution;
/// }
/// ```
#[derive(Debug, Clone)]
pub struct CheckedCompilation {
    source_completion_accounting: build_time_evaluation::SourceCompletionAccounting,
    execution: CheckedCompilationInputs,
    sources: Arc<CheckedSourceCustody>,
    timings: CompileTimings,
    /// The settled language judgments and their exact inputs, kept only when the
    /// request asked to retain it for a later compile of the same occurrence.
    settled_checks: RetainedCompilationChecks,
}

// Measurements and a retained judgment are observations, not checked semantic
// identity. Each retained semantic owner derives equality so newly retained
// fields participate too.
impl PartialEq for CheckedCompilation {
    fn eq(&self, other: &Self) -> bool {
        self.execution == other.execution && self.sources == other.sources
    }
}

impl Eq for CheckedCompilation {}

impl CheckedCompilation {
    /// Retain a target's settlement beside the common source closure. Source
    /// checking already sealed that closure; realization must not hash it again.
    /// This assembles products, not package admission or publication authority.
    pub fn from_settled_execution(
        execution: CheckedCompilationInputs,
        sources: Arc<CheckedSourceCustody>,
        timings: CompileTimings,
    ) -> Self {
        Self {
            source_completion_accounting: Default::default(),
            execution,
            sources,
            timings,
            settled_checks: RetainedCompilationChecks::default(),
        }
    }

    /// Retain authored source completion's actual invocation charges separately
    /// from optional source judgments and target/provider admission.
    pub fn with_source_completion_accounting(
        mut self,
        accounting: build_time_evaluation::SourceCompletionAccounting,
    ) -> Self {
        self.source_completion_accounting = accounting;
        self
    }

    pub fn source_completion_accounting(
        &self,
    ) -> &build_time_evaluation::SourceCompletionAccounting {
        &self.source_completion_accounting
    }

    /// Keep the settled checks this compilation ran (or took) for a later
    /// compile of the same occurrence to hand back.
    pub fn with_settled_checks(mut self, settled_checks: RetainedCompilationChecks) -> Self {
        self.settled_checks = settled_checks;
        self
    }

    /// The retained settled checks, empty when the request asked for none.
    pub const fn settled_checks(&self) -> &RetainedCompilationChecks {
        &self.settled_checks
    }
}

impl CheckedCompilation {
    /// The complete immutable result of selected execution: the checked
    /// program, target-profile and native-ABI selection, program entry, and
    /// selected provider evidence. Borrowing this unit grants no installation
    /// or publication authority.
    pub const fn execution(&self) -> &SelectedExecution {
        &self.execution.settled
    }

    /// The one evaluated Build retained by this compilation. Authored choices
    /// remain distinct from the selected execution and its independently
    /// reconstructed provider evidence.
    pub fn build(&self) -> &ComputedBuildConfig {
        &self.execution.build
    }

    /// This compilation's admitted Build facts: the application declaration,
    /// the evaluated build machine's identity and its restricted host requests.
    pub const fn build_admission(&self) -> &BuildAdmission {
        &self.execution.build_admission
    }

    /// Every authored Product default, retained before source root pruning.
    pub fn provider_defaults(
        &self,
    ) -> &[build_time_evaluation::provider_declarations::TargetProviderDefaults] {
        &self.execution.provider_defaults
    }

    /// Provider-planning custody retained for package review, trust replay
    /// and native realization. Its presence publishes no package ABI or
    /// admission row.
    pub const fn providers(&self) -> &ProviderSettlement {
        &self.execution.providers
    }

    pub fn independent_component_products(&self) -> &[Arc<crate::IndependentComponentProduct>] {
        &self.execution.independent_components
    }

    /// The exact physical/generated source closure sealed against this
    /// program, with its package subject when package-aware. Source IDs are
    /// private join coordinates and never enter canonical product bytes.
    pub fn sources(&self) -> &CheckedSourceCustody {
        &self.sources
    }

    /// Reconstruct this product's trust obligations against explicit owner
    /// admissions, independently of report output. This grants neither package
    /// review authority nor native publication authority.
    /// Resolve every root grant to its exact claim or provider plan; a grant
    /// that names nothing the program selects rejects.
    pub fn admit_trust(&self) -> Result<(), Vec<Diagnostic>> {
        trust_model::reconstruct_trust_obligations(
            &self.typed,
            self,
            &self.build().config.root_grant_selectors(),
            &self.providers().provider_plans,
            &self.execution().selected_provider_plan_facts,
            &self.execution().accepted_template_classifications,
            self.sources.package_subject().is_some(),
        )
        .map(drop)
    }

    /// Retain this package's own explicit generated-source handoffs as one
    /// compiler-issued bundle suitable for a later dependency compilation.
    /// The bundle is not admission and carries no filesystem authority.
    pub fn package_generated_source_bundle(
        &self,
    ) -> Result<package_compilation::PackageGeneratedSourceBundle, &'static str> {
        self.sources.package_generated_source_bundle(
            self.execution.settled.selected_target_profile,
            self.execution.build.generated_sources.clone(),
        )
    }

    pub const fn timings(&self) -> &CompileTimings {
        &self.timings
    }

    /// The measurement accumulator the compiler's later product legs record
    /// into, so stage timings survive inside the checked record until the
    /// report-facing leg prints them.
    pub fn timings_mut(&mut self) -> &mut CompileTimings {
        &mut self.timings
    }
}

impl std::ops::Deref for CheckedCompilation {
    type Target = CheckedTrees;

    fn deref(&self) -> &Self::Target {
        &self.execution.settled.program
    }
}
