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
use package_compilation::BuildSourceCustody;
use package_compilation::CheckedSourceCustody;
use package_compilation::PackageCompilationInputs;
use provider_planning::ProviderSettlement;
use selected_dispatch::SelectedExecution;
use std::sync::Arc;

mod production_subject;
mod report;
mod terminal;

pub use report::{CheckedProductReport, CompileReportSettlement};
pub use terminal::{
    validate_lowered_ieee_float_comparison_custody, validate_lowered_integer_comparison_custody,
};

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
    pub providers: ProviderSettlement,
    /// Identity-bearing checked-tree product selection evidence, present only
    /// when the effective build selection named `CheckedTreeProductPruning`.
    /// `None` means the checked-tree phase ran as the identity boundary. The
    /// retained roots and pruned roster re-derive the exact product; they are
    /// not a reselection input.
    pub product_selection: Option<typed_trees_to_checked_trees::CheckedTreeProductSelection>,
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
///     checked.typed = Default::default();
/// }
/// ```
#[derive(Debug, Clone)]
pub struct CheckedCompilation {
    execution: CheckedCompilationInputs,
    sources: CheckedSourceCustody,
    timings: CompileTimings,
}

// Measurements are observations, not checked semantic identity. Each retained
// semantic owner derives equality so newly retained fields participate too.
impl PartialEq for CheckedCompilation {
    fn eq(&self, other: &Self) -> bool {
        self.execution == other.execution && self.sources == other.sources
    }
}

impl Eq for CheckedCompilation {}

impl CheckedCompilation {
    /// Seal supplied stage products against their exact consumed source closure.
    /// This binds custody, not package admission or native publication authority;
    /// consumers still independently reconstruct the supplied evidence.
    pub fn seal(
        execution: CheckedCompilationInputs,
        sources: BuildSourceCustody,
        package_inputs: Option<&PackageCompilationInputs>,
        timings: CompileTimings,
    ) -> Result<Self, Vec<Diagnostic>> {
        let sources = sources.seal(&execution.settled.program, package_inputs)?;
        Ok(Self {
            execution,
            sources,
            timings,
        })
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

    /// Provider-planning custody retained for package review, trust replay
    /// and native realization. Its presence publishes no package ABI or
    /// admission row.
    pub const fn providers(&self) -> &ProviderSettlement {
        &self.execution.providers
    }

    /// The exact physical/generated source closure sealed against this
    /// program, with its package subject when package-aware. Source IDs are
    /// private join coordinates and never enter canonical product bytes.
    pub const fn sources(&self) -> &CheckedSourceCustody {
        &self.sources
    }

    /// Reconstruct this product's trust obligations against explicit owner
    /// admissions, independently of report output. This grants neither package
    /// review authority nor native publication authority.
    pub fn admit_trust(
        &self,
        accepted: &[trust_model::TrustAdmission],
    ) -> Result<trust_model::TrustAdmissionSettlement, Vec<Diagnostic>> {
        trust_model::reconstruct_trust_settlement(
            self,
            &self.build().config.root_grant_selectors(),
            &self.providers().provider_plans,
            &self.execution().selected_provider_plan_facts,
            &self.execution().accepted_template_classifications,
            self.sources.package_subject().is_some(),
            accepted,
        )
    }

    /// Re-read every ordinary physical source path and require it to equal the
    /// bytes retained by the frontend. Generated sources are instead checked
    /// against their compiler-retained staged-output custody. Resolver
    /// orchestration calls this around its own whole-snapshot verification;
    /// hostile same-user races still require an OS isolation boundary.
    pub fn verify_current_source_consumption(&self) -> Result<(), Vec<Diagnostic>> {
        self.sources
            .verify_current_source_consumption(&self.execution.settled.program)
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

    pub fn into_program(self) -> CheckedTrees {
        Arc::try_unwrap(self.execution.settled.program).unwrap_or_else(|shared| (*shared).clone())
    }
}

impl std::ops::Deref for CheckedCompilation {
    type Target = CheckedTrees;

    fn deref(&self) -> &Self::Target {
        &self.execution.settled.program
    }
}
