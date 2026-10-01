//! Checked compilation and the source custody sealed against it.

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
    /// The complete immutable result of selected execution. Consumers read the
    /// producing owner's fields instead of a parallel compiler-facing vocabulary.
    /// Borrowing this unit grants no installation or publication authority.
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
            self.package_identity().is_some(),
            accepted,
        )
    }

    /// Canonical boundary calls already live in this checked program.
    pub fn terminal_production_trees(&self) -> &CheckedTrees {
        &self.execution.settled.program
    }

    /// Exact physical/generated source count consumed by this checked run.
    pub const fn source_file_count(&self) -> usize {
        self.sources.source_file_count()
    }

    pub fn project_native_application_metadata(&self) -> target::NativeApplicationMetadata {
        self.execution.build_admission.native_application_metadata(
            &self.execution.build.config.settings,
            self.selected_target_profile(),
        )
    }

    /// The validated `builder.artifact_only()` intent retained at source
    /// custody. `true` only for an application build that declared the
    /// modifier; package/workspace builds and missing build roots return
    /// `false`.
    pub const fn application_artifact_only(&self) -> bool {
        self.execution.build_admission.artifact_only()
    }

    /// Reconciled root package identity for package-aware compilation.
    /// Standalone compilation has no package identity.
    pub const fn package_identity(&self) -> Option<semantic_vocabulary::PackageKeyIdentity> {
        self.sources.package_identity()
    }

    /// Exact source-path-free dependency closure consumed by package-aware
    /// compilation. Standalone compilation has no package closure.
    pub const fn dependency_closure(
        &self,
    ) -> Option<&package_compilation::PackageDependencyClosure> {
        self.sources.dependency_closure()
    }

    /// Canonical commitment to the exact source bytes consumed by this
    /// package-aware frontend run. Standalone compilation has no package-
    /// custody commitment.
    pub const fn source_consumption_commitment(
        &self,
    ) -> Option<package_compilation::PackageSourceConsumptionCommitment> {
        self.sources.source_consumption_commitment()
    }

    /// Canonical commitment to the exact package-aware source closure admitted
    /// before this activation executed its selected build machine. Own
    /// generated source is absent; imported dependency-generated bundles are
    /// already part of this base.
    pub const fn base_source_consumption_commitment(
        &self,
    ) -> Option<package_compilation::PackageSourceConsumptionCommitment> {
        self.sources.base_source_consumption_commitment()
    }

    /// Canonical package/source subject derived from the final checked source
    /// closure. Standalone compilation has no package subject.
    pub const fn package_compilation_subject(
        &self,
    ) -> Option<&package_compilation::PackageCompilationSubject> {
        self.sources.package_subject()
    }

    #[doc(hidden)]
    pub fn resolved_semantic_binding(
        &self,
        role: package_compilation::AcceptedSemanticBindingRole,
    ) -> Option<&selected_dispatch::ResolvedAcceptedSemanticBinding> {
        self.execution.settled.resolved_semantic_binding(role)
    }

    /// Exact consumer-policy semantic bindings that were resolved and
    /// consumed by this checked package compilation. Unconsumed or stale input
    /// bindings reject before a compilation can expose this set.
    #[doc(hidden)]
    pub fn resolved_semantic_bindings(
        &self,
    ) -> impl ExactSizeIterator<Item = &package_compilation::AcceptedSemanticBinding> {
        self.execution.settled.accepted_semantic_bindings()
    }

    /// Retain permission rows from the exact semantic bindings this checked
    /// product consumed. Proposal construction and accepted-policy admission
    /// project from the checked source independently, never from each other.
    pub fn package_terminal_authority_permissions(
        &self,
    ) -> Vec<effects::ServiceTerminalAuthorityPermission> {
        self.execution.settled.terminal_authority_permissions()
    }

    /// Project and validate this checked package's accepted Terminal permission
    /// rows before native proposal production. These are package evidence, not
    /// the independently supplied receiver-admission policy.
    pub fn accepted_terminal_authority_permission_policy(
        &self,
    ) -> Result<
        terminal_psi_to_abstract_operations::provider_admission::TerminalAuthorityPermissionPolicy,
        Vec<Diagnostic>,
    > {
        use terminal_psi_to_abstract_operations::provider_admission::{
            terminal_authority_permission_policy_with_rows,
            terminal_authority_permissions::validate_package_terminal_authority_permission_custody,
        };
        let permissions = self.package_terminal_authority_permissions();
        validate_package_terminal_authority_permission_custody(permissions.iter())?;
        terminal_authority_permission_policy_with_rows(permissions).map_err(|error| {
            vec![Diagnostic::error(format!(
                "checked package terminal-authority permissions are invalid: {error:?}"
            ))]
        })
    }

    /// Construct a non-authoritative review candidate for one exact package-
    /// owned requirement-only boundary. Readable role nomination happens
    /// outside the compiler; this contributes only checked owner, nominal, and
    /// normalized-schema coordinates for a required bound replay.
    #[doc(hidden)]
    pub fn candidate_service_binding(
        &self,
        role: package_compilation::AcceptedSemanticBindingRole,
        package: semantic_vocabulary::PackageKeyIdentity,
        declaration_path: &str,
    ) -> Result<package_compilation::AcceptedSemanticBinding, Diagnostic> {
        self.execution
            .settled
            .candidate_service_binding(role, package, declaration_path)
    }

    /// Compiler-validated exact source owners used while projecting
    /// source-free structural and nominal identity. Source IDs are private
    /// join coordinates and never enter canonical product or review bytes.
    #[doc(hidden)]
    pub fn exact_toolchain_sources(&self) -> &[(source::SourceId, [u8; 32])] {
        self.sources.exact_toolchain_sources()
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

    /// Exact native target selected for this checked compilation. Semantic-only
    /// checking has no selected target and therefore cannot be staged.
    pub fn selected_native_target(&self) -> Option<target::NativeTarget> {
        self.execution.settled.selected_native_target()
    }

    /// Exact deployment-policy target selected for this checked compilation.
    /// This remains distinct when profiles share a native ABI, notably
    /// `windows_x64` and `uefi_x64`.
    pub const fn selected_target_profile(&self) -> Option<target::TargetProfile> {
        self.execution.settled.selected_target_profile
    }

    /// Exact target-owned `ProgramEntry` choice retained by Omega, if this
    /// checked-only compilation had one. Pure semantic checking is entry-
    /// agnostic; an execution caller must not infer a machine from its name.
    pub fn selected_program_entry_machine(&self) -> Option<&str> {
        self.execution
            .settled
            .selected_program_entry
            .as_ref()
            .map(provider_planning::program_entry::SelectedCompilerProgramEntry::machine_name)
    }

    /// Complete build-owned `ProgramEntry` settlement captured while typed
    /// declarations and evaluated calling plans were still available. The
    /// source signature and optional target calling plans remain one custody
    /// object; downstream stages must not reconstruct either from the retained
    /// machine name.
    pub const fn selected_program_entry(
        &self,
    ) -> Option<&provider_planning::program_entry::SelectedCompilerProgramEntry> {
        self.execution.settled.selected_program_entry.as_ref()
    }

    /// Whether this compile's selected program entry tolerated a `Binding`
    /// field whose Fused provider was never selected. Only a discovery pass
    /// may carry such a result: a strict recompile must reject the same
    /// program, so a compile answering `true` is not interchangeable with the
    /// result a pass without the tolerance would produce.
    pub fn tolerated_unsettled_service_fields(&self) -> bool {
        self.execution.settled.tolerated_unsettled_service_fields()
    }

    /// Canonical semantic identity of the build machine actually evaluated.
    /// This is derived while the final typed declarations remain available;
    /// downstream custody must not rediscover it by short name.
    pub fn selected_build_machine_identity(&self) -> Option<&str> {
        self.execution
            .build_admission
            .selected_build_machine_identity
            .as_deref()
    }

    /// Exact validated boundary calling-plan realizations retained while the
    /// typed declaration graph and selected opaque representations still
    /// coexisted. This is compiler custody for downstream reconstruction; its
    /// presence does not itself publish a package ABI or admission row.
    pub fn boundary_calling_plan_realizations(
        &self,
    ) -> &[provider_planning::calling_policy_plans::BoundaryCallingPlanRealization] {
        &self.execution.providers.boundary_calling_plan_realizations
    }

    /// Identity-bearing evidence of the checked-tree product selection this
    /// compilation executed, present only when the effective build selection
    /// named `CheckedTreeProductPruning`. `None` means the checked-tree phase
    /// ran as the identity boundary and the complete checked program remains
    /// the product. The retained roots and pruned roster re-derive the exact
    /// product this compilation produced; they are not a reselection input.
    pub const fn checked_tree_product_selection(
        &self,
    ) -> Option<&typed_trees_to_checked_trees::CheckedTreeProductSelection> {
        self.execution.product_selection.as_ref()
    }

    #[doc(hidden)]
    pub fn provider_plans(&self) -> &[effects::provider_plan::ProviderPlan] {
        &self.execution.providers.provider_plans
    }

    /// Complete exact ordinary-`via` evaluation population, including leaves
    /// not selected for this executable. Package review consumes this table;
    /// backend lowering consumes only the evaluated imports installed in
    /// provider plans.
    #[doc(hidden)]
    pub const fn evaluated_via_bindings(
        &self,
    ) -> &provider_planning::evaluated_via_bindings::EvaluatedViaBindingTable {
        &self.execution.providers.evaluated_via_bindings
    }

    /// Exact selected normalized-import bindings and their evaluated calling
    /// plans. These rows are derived before typed trees are consumed and remain
    /// the only source of normalized foreign-locator custody in native
    /// realization; other provider mechanisms keep their specialized lanes.
    pub fn external_binding_rows(&self) -> &[calling_conventions::ExternalBindingRow] {
        &self.execution.providers.external_binding_rows
    }

    pub fn build_observation_identity(
        &self,
    ) -> Option<package_compilation::BuildObservationIdentity> {
        self.build().observation_identity()
    }

    /// Select the completed-file product from this checked execution. Compiler
    /// and package production share this projection; neither interprets the
    /// settlement roster or stages the build again.
    pub fn completed_build_outputs(
        &self,
    ) -> Result<Option<crate::RetainedBuildOutputs>, Vec<Diagnostic>> {
        self.build().completed_outputs()
    }

    /// The normalized restricted build-host requests this compilation's
    /// admitted build activation asked of the host before it executed
    /// (wiki/spec/packages/acceptance.md#restricted-build-acceptance). This
    /// is admission intent, not execution evidence: it names the restricted
    /// operation, logical resource roots, bounds, and activation profiles in
    /// package vocabulary, never host paths or live grants.
    pub fn restricted_build_requests(
        &self,
    ) -> &[package_compilation::build_execution::RestrictedBuildRequest] {
        &self.execution.build_admission.restricted_build_requests
    }

    /// The component descriptions this package compilation attached and
    /// provider settlement verified for its `Independent` provider
    /// selections, in package-identity order. Standalone compilations and
    /// compilations without attached descriptions retain an empty roster.
    /// Re-verification under the build's admission profile replays the same
    /// admission settlement performed.
    pub fn independent_component_descriptions(
        &self,
    ) -> &[package_compilation::IndependentComponentDescription] {
        &self.execution.providers.independent_component_descriptions
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

    /// Boundaries demanded by the selected ProgramEntry's receiver
    /// `Binding<R>` fields, including those nested in record fields. A
    /// ProgramEntry is root-bound rather than a package callable, so its
    /// service usage never surfaces in callable reach rows: the receiver's
    /// carrier requirements are the consumer's own nomination surface.
    /// `selected_dispatch` owns the traversal, and establishment reads the
    /// same one, so every field nominated here is a field establishment
    /// later requires a selected Fused provider for.
    pub fn selected_program_entry_service_requirements(&self) -> Vec<symbols::SymbolHandle> {
        self.execution.settled.program_entry_service_requirements()
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
