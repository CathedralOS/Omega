use crate::pipeline::PackageCompilationInputs;
use crate::pipeline::phase_transitions::{
    SelectedExecutionSettlementInput, TypedToCheckedSettlementInput,
    resolve_seeded_syntax_extension, settle_selected_execution,
    symbol_resolved_trees_to_seeded_base, syntax_trees_to_symbol_resolved_trees,
    type_seeded_extension, typed_trees_to_checked_trees,
};
use crate::pipeline::source_assembly::ImmutableSourceParseCheckpoint;
use crate::pipeline::timing::CompileTimings;
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use std::path::Path;
use std::sync::Arc;

/// Psi-checked semantics paired with the Omega-owned provider realization
/// selected for one engine run. The semantic program deliberately does not
/// retain target/provider installation state.
#[derive(Debug, Clone)]
pub struct CheckedCompilation {
    program: CheckedTrees,
    dispatch_source_edits: selected_dispatch::SelectedDispatchSourceEdits,
    source_file_count: usize,
    subsystem: u16,
    package_subject: Option<package_compilation::PackageCompilationSubject>,
    resolved_semantic_bindings: Vec<selected_dispatch::ResolvedAcceptedSemanticBinding>,
    base_source_consumption_commitment: Option<super::PackageSourceConsumptionCommitment>,
    exact_toolchain_sources: Vec<(source::SourceId, [u8; 32])>,
    generated_source_custody: Vec<(source::SourceId, build_output::PackageGeneratedSource)>,
    own_generated_sources: Vec<build_output::PackageGeneratedSource>,
    selected_target_profile: Option<target::TargetProfile>,
    selected_native_target: Option<target::NativeTarget>,
    x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
    x86_scalar_fma_plan_associations:
        Vec<super::x86_fma_plan_association::CheckedX86ScalarFmaPlanAssociation>,
    selected_program_entry: Option<build_evaluation::SelectedCompilerProgramEntry>,
    selected_build_machine_symbol: Option<symbols::SymbolHandle>,
    selected_build_machine_identity: Option<String>,
    opaque_representation_selections: Vec<representation_planning::OpaqueRepresentationSelection>,
    boundary_calling_plan_realizations:
        Vec<provider_planning::calling_policy_plans::BoundaryCallingPlanRealization>,
    optimization: super::optimization::checked_handoff::CheckedOptimizationHandoff,
    selected_provider_plans: effects::SelectedProviderPlanFacts,
    selected_provider_grants: Vec<trust_model::ResolvedAuthoredSelectedProviderGrant>,
    provider_plans: Vec<effects::provider_plan::ProviderPlan>,
    evaluated_via_bindings: provider_planning::evaluated_via_bindings::EvaluatedViaBindingTable,
    external_binding_rows: Vec<calling_conventions::ExternalBindingRow>,
    root_grants: Vec<String>,
    accepted_template_classifications: trust_model::AcceptedTemplateClassifications,
    selected_provider_provenance: Vec<super::provider_plans::SelectedProviderReviewProvenance>,
    component_progress: Option<effects::ComponentProgressManifest>,
    task_activations: task_plans::TaskActivationPlanSet,
    callback_placements: Vec<backend_plan::BoundNominalCallbackPlacement>,
    build_evaluation_usage: Option<super::build_config::BuildEvaluationUsage>,
    build_observation_summary: Option<super::build_config::BuildObservationSummary>,
    contract_entailment_stand_downs: Vec<validation::ContractEntailmentStandDown>,
    timings: CompileTimings,
}

// Timing measurements are nondeterministic observations of a compilation,
// not part of checked semantic identity. Keep the public semantic equality
// contract while deliberately excluding `timings`.
impl PartialEq for CheckedCompilation {
    fn eq(&self, other: &Self) -> bool {
        self.program == other.program
            && self.dispatch_source_edits == other.dispatch_source_edits
            && self.source_file_count == other.source_file_count
            && self.subsystem == other.subsystem
            && self.package_subject == other.package_subject
            && self.resolved_semantic_bindings == other.resolved_semantic_bindings
            && self.base_source_consumption_commitment == other.base_source_consumption_commitment
            && self.exact_toolchain_sources == other.exact_toolchain_sources
            && self.generated_source_custody == other.generated_source_custody
            && self.own_generated_sources == other.own_generated_sources
            && self.selected_target_profile == other.selected_target_profile
            && self.selected_native_target == other.selected_native_target
            && self.x86_scalar_fma_provider == other.x86_scalar_fma_provider
            && self.x86_scalar_fma_plan_associations == other.x86_scalar_fma_plan_associations
            && self.selected_program_entry == other.selected_program_entry
            && self.selected_build_machine_symbol == other.selected_build_machine_symbol
            && self.selected_build_machine_identity == other.selected_build_machine_identity
            && self.opaque_representation_selections == other.opaque_representation_selections
            && self.boundary_calling_plan_realizations == other.boundary_calling_plan_realizations
            && self.optimization == other.optimization
            && self.selected_provider_plans == other.selected_provider_plans
            && self.selected_provider_grants == other.selected_provider_grants
            && self.provider_plans == other.provider_plans
            && self.evaluated_via_bindings == other.evaluated_via_bindings
            && self.external_binding_rows == other.external_binding_rows
            && self.root_grants == other.root_grants
            && self.accepted_template_classifications == other.accepted_template_classifications
            && self.selected_provider_provenance == other.selected_provider_provenance
            && self.component_progress == other.component_progress
            && self.task_activations == other.task_activations
            && self.callback_placements == other.callback_placements
            && self.build_evaluation_usage == other.build_evaluation_usage
            && self.build_observation_summary == other.build_observation_summary
            && self.contract_entailment_stand_downs == other.contract_entailment_stand_downs
    }
}

impl Eq for CheckedCompilation {}

impl CheckedCompilation {
    /// Restore only exact selected-dispatch edits after checking their settled
    /// operand/type graphs. This is not a pre-specialization or source-text view.
    pub fn pre_selected_dispatch_source_trees(
        &self,
    ) -> Result<std::borrow::Cow<'_, typed_trees::TypedTrees>, Vec<Diagnostic>> {
        self.dispatch_source_edits.source_trees(&self.program.typed)
    }

    /// Exact physical/generated source count consumed by this checked run.
    pub const fn source_file_count(&self) -> usize {
        self.source_file_count
    }

    /// Exact image subsystem selected by the owning build configuration.
    pub const fn subsystem(&self) -> u16 {
        self.subsystem
    }

    /// Reconciled root package identity for package-aware compilation.
    /// Standalone compilation has no package identity.
    pub const fn package_identity(&self) -> Option<semantic_vocabulary::PackageKeyIdentity> {
        match &self.package_subject {
            Some(subject) => Some(subject.root()),
            None => None,
        }
    }

    /// Exact source-path-free dependency closure consumed by package-aware
    /// compilation. Standalone compilation has no package closure.
    pub const fn dependency_closure(&self) -> Option<&super::PackageDependencyClosure> {
        match &self.package_subject {
            Some(subject) => Some(subject.dependency_closure()),
            None => None,
        }
    }

    /// Canonical commitment to the exact source bytes consumed by this
    /// package-aware frontend run. Standalone compilation has no package-
    /// custody commitment.
    pub const fn source_consumption_commitment(
        &self,
    ) -> Option<super::PackageSourceConsumptionCommitment> {
        match &self.package_subject {
            Some(subject) => Some(subject.source_consumption_commitment()),
            None => None,
        }
    }

    /// Canonical commitment to the exact package-aware source closure admitted
    /// before this activation executed its selected build machine. Own
    /// generated source is absent; imported dependency-generated bundles are
    /// already part of this base.
    pub const fn base_source_consumption_commitment(
        &self,
    ) -> Option<super::PackageSourceConsumptionCommitment> {
        self.base_source_consumption_commitment
    }

    /// Canonical package/source subject derived from the final checked source
    /// closure. Standalone compilation has no package subject.
    pub const fn package_compilation_subject(
        &self,
    ) -> Option<&package_compilation::PackageCompilationSubject> {
        self.package_subject.as_ref()
    }

    #[doc(hidden)]
    pub fn resolved_semantic_binding(
        &self,
        role: package_compilation::AcceptedSemanticBindingRole,
    ) -> Option<&selected_dispatch::ResolvedAcceptedSemanticBinding> {
        self.resolved_semantic_bindings
            .iter()
            .find(|binding| binding.role() == role)
    }

    /// Exact consumer-policy semantic bindings that were resolved and
    /// consumed by this checked package compilation. Unconsumed or stale input
    /// bindings reject before a compilation can expose this set.
    #[doc(hidden)]
    pub fn resolved_semantic_bindings(
        &self,
    ) -> impl ExactSizeIterator<Item = &package_compilation::AcceptedSemanticBinding> {
        self.resolved_semantic_bindings
            .iter()
            .map(selected_dispatch::ResolvedAcceptedSemanticBinding::accepted)
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
        let matches = self
            .program
            .typed
            .traits()
            .iter()
            .filter(|definition| {
                definition.is_boundary
                    && self
                        .program
                        .typed
                        .symbols
                        .symbol_package_identity(definition.symbol)
                        == Some(package)
                    && self
                        .program
                        .typed
                        .symbols
                        .display_path(definition.symbol, "::")
                        == declaration_path
            })
            .filter_map(|definition| {
                provider_planning::service_schema::from_typed(&self.program.typed, definition)
            })
            .collect::<Vec<_>>();
        let [schema] = matches.as_slice() else {
            return Err(Diagnostic::error(format!(
                "semantic-binding candidate {:?} resolved to {} exact package-owned boundary schemas instead of one",
                role,
                matches.len(),
            )));
        };
        package_compilation::AcceptedSemanticBinding::new_service(
            role,
            package,
            declaration_path,
            package_compilation::accepted_service_schema_digest(role, schema),
        )
        .map_err(Diagnostic::error)
    }

    /// Compiler-validated exact source owners used while projecting
    /// source-free structural and nominal identity. Source IDs are private
    /// join coordinates and never enter canonical product or review bytes.
    #[doc(hidden)]
    pub fn exact_toolchain_sources(&self) -> &[(source::SourceId, [u8; 32])] {
        &self.exact_toolchain_sources
    }

    /// Re-read every ordinary physical source path and require it to equal the
    /// bytes retained by the frontend. Generated sources are instead checked
    /// against their compiler-retained staged-output custody. Resolver
    /// orchestration calls this around its own whole-snapshot verification;
    /// hostile same-user races still require an OS isolation boundary.
    pub fn verify_current_source_consumption(&self) -> Result<(), Vec<Diagnostic>> {
        package_compilation::verify_current_files(&self.program, &self.generated_source_custody)
    }

    /// Retain this package's own explicit generated-source handoffs as one
    /// compiler-issued bundle suitable for a later dependency compilation.
    /// The bundle is not admission and carries no filesystem authority.
    pub fn package_generated_source_bundle(
        &self,
    ) -> Result<super::PackageGeneratedSourceBundle, &'static str> {
        let package = self
            .package_identity()
            .ok_or("generated-source bundles require package-aware compilation")?;
        let target = self
            .selected_target_profile
            .ok_or("generated-source bundles require one selected target")?;
        let dependency_closure = self
            .dependency_closure()
            .cloned()
            .ok_or("generated-source bundles require one dependency closure")?;
        let source_consumption_commitment = self
            .source_consumption_commitment()
            .ok_or("generated-source bundles require source-consumption custody")?;
        Ok(super::PackageGeneratedSourceBundle::from_checked(
            package,
            target,
            dependency_closure,
            source_consumption_commitment,
            self.own_generated_sources.clone(),
        ))
    }

    /// Exact native target selected for this checked compilation. Semantic-only
    /// checking has no selected target and therefore cannot be staged.
    pub const fn selected_native_target(&self) -> Option<target::NativeTarget> {
        self.selected_native_target
    }

    /// Exact deployment-policy target selected for this checked compilation.
    /// This remains distinct when profiles share a native ABI, notably
    /// `windows_x64` and `uefi_x64`.
    pub const fn selected_target_profile(&self) -> Option<target::TargetProfile> {
        self.selected_target_profile
    }

    /// Exact build-selected x86 scalar FMA admission for this compilation.
    /// Absence preserves the generic SSE2 baseline. The retained provider
    /// carries the canonical semantic cancellation-vector admission but does
    /// not claim native differential execution.
    pub const fn x86_scalar_fma_provider(&self) -> Option<target::AdmittedX86ScalarFmaProvider> {
        self.x86_scalar_fma_provider
    }

    /// Exact source-selected nearest-FMA plans joined to this compilation's
    /// admitted x86 provider. Empty means the source demanded no x86 FMA;
    /// admission alone never fabricates source demand or execution evidence.
    pub fn x86_scalar_fma_plan_associations(
        &self,
    ) -> &[super::x86_fma_plan_association::CheckedX86ScalarFmaPlanAssociation] {
        &self.x86_scalar_fma_plan_associations
    }

    /// Exact target-owned `ProgramEntry` choice retained by Omega, if this
    /// checked-only compilation had one. Pure semantic checking is entry-
    /// agnostic; an execution caller must not infer a machine from its name.
    pub fn selected_program_entry_machine(&self) -> Option<&str> {
        self.selected_program_entry
            .as_ref()
            .map(build_evaluation::SelectedCompilerProgramEntry::machine_name)
    }

    /// Complete build-owned `ProgramEntry` settlement captured while typed
    /// declarations and evaluated calling plans were still available. The
    /// source signature and optional target calling plans remain one custody
    /// object; downstream stages must not reconstruct either from the retained
    /// machine name.
    pub const fn selected_program_entry(
        &self,
    ) -> Option<&build_evaluation::SelectedCompilerProgramEntry> {
        self.selected_program_entry.as_ref()
    }

    /// Exact symbol of the uniquely selected build machine. No build machine
    /// is represented by `None`; callers must not rediscover one by name.
    pub const fn selected_build_machine_symbol(&self) -> Option<symbols::SymbolHandle> {
        self.selected_build_machine_symbol
    }

    /// Canonical semantic identity of the build machine actually evaluated.
    /// This is derived while the final typed declarations remain available;
    /// downstream custody must not rediscover it by short name.
    pub fn selected_build_machine_identity(&self) -> Option<&str> {
        self.selected_build_machine_identity.as_deref()
    }

    /// Complete compiler-validated opaque-representation selections harvested
    /// from the authoritative build machine. Unused selections remain here as
    /// activation policy; this custody does not imply a by-value demand or a
    /// physical ABI commitment.
    pub fn opaque_representation_selections(
        &self,
    ) -> &[representation_planning::OpaqueRepresentationSelection] {
        &self.opaque_representation_selections
    }

    /// Exact validated boundary calling-plan realizations retained while the
    /// typed declaration graph and selected opaque representations still
    /// coexisted. This is compiler custody for downstream reconstruction; its
    /// presence does not itself publish a package ABI or admission row.
    pub fn boundary_calling_plan_realizations(
        &self,
    ) -> &[provider_planning::calling_policy_plans::BoundaryCallingPlanRealization] {
        &self.boundary_calling_plan_realizations
    }

    /// Exact named optimizations selected by the authoritative root build.
    /// Empty executes each applicable optimization phase as a validated
    /// identity transformation; it does not select an optimizer-free path.
    pub const fn optimization_selections(&self) -> &optimization_core::OptimizationSelections {
        self.optimization.selections()
    }

    /// Domain-separated identity of the exact canonical selected set. This is
    /// retained independently so later cache, replay, and artifact boundaries
    /// never have to rediscover an optimization input from build syntax.
    pub const fn optimization_selection_identity(
        &self,
    ) -> optimization_core::OptimizationSelectionIdentity {
        self.optimization.selection_identity()
    }

    /// Auxiliary report projection requested by the authoritative root build.
    /// This remains independent of the exact transformation selection.
    pub const fn optimization_report_request(
        &self,
    ) -> optimization_core::OptimizationReportRequest {
        self.optimization.report()
    }

    pub const fn selected_provider_plans(&self) -> &effects::SelectedProviderPlanFacts {
        &self.selected_provider_plans
    }

    /// Exact `build.omg` grants resolved to retained selected provider plans.
    pub fn selected_provider_grants(
        &self,
    ) -> &[trust_model::ResolvedAuthoredSelectedProviderGrant] {
        &self.selected_provider_grants
    }

    #[doc(hidden)]
    pub fn provider_plans(&self) -> &[effects::provider_plan::ProviderPlan] {
        &self.provider_plans
    }

    /// Complete exact ordinary-`via` evaluation population, including leaves
    /// not selected for this executable. Package review consumes this table;
    /// backend lowering consumes only the evaluated imports installed in
    /// provider plans.
    #[doc(hidden)]
    pub const fn evaluated_via_bindings(
        &self,
    ) -> &provider_planning::evaluated_via_bindings::EvaluatedViaBindingTable {
        &self.evaluated_via_bindings
    }

    /// Exact selected normalized-import bindings and their evaluated calling
    /// plans. These rows are derived before typed trees are consumed and remain
    /// the only source of normalized foreign-locator custody in native
    /// realization; other provider mechanisms keep their specialized lanes.
    pub fn external_binding_rows(&self) -> &[calling_conventions::ExternalBindingRow] {
        &self.external_binding_rows
    }

    #[doc(hidden)]
    pub fn root_grants(&self) -> &[String] {
        &self.root_grants
    }

    #[doc(hidden)]
    pub const fn accepted_template_classifications(
        &self,
    ) -> &trust_model::AcceptedTemplateClassifications {
        &self.accepted_template_classifications
    }

    #[doc(hidden)]
    pub fn selected_provider_provenance(
        &self,
    ) -> &[super::provider_plans::SelectedProviderReviewProvenance] {
        &self.selected_provider_provenance
    }

    pub const fn component_progress(&self) -> Option<&effects::ComponentProgressManifest> {
        self.component_progress.as_ref()
    }

    pub const fn task_activations(&self) -> &task_plans::TaskActivationPlanSet {
        &self.task_activations
    }

    /// Exact target-owned callback recipes joined to their checked nominal
    /// use sites. An execution engine must consume these plans rather than
    /// derive placement from the semantic tree.
    pub fn callback_placements(&self) -> &[backend_plan::BoundNominalCallbackPlacement] {
        &self.callback_placements
    }

    pub const fn build_evaluation_usage(
        &self,
    ) -> Option<super::build_config::BuildEvaluationUsage> {
        self.build_evaluation_usage
    }

    /// Exact selected build-machine observation ceiling and realized class.
    /// This execution evidence remains separate from package capability/API
    /// comparison bytes.
    pub const fn build_observation_summary(
        &self,
    ) -> Option<&super::build_config::BuildObservationSummary> {
        self.build_observation_summary.as_ref()
    }

    /// Exact compiler-owned coordinates of checked implementation claims that
    /// ordinary validation deliberately left unjudged. Package review rejects
    /// any row until a later-discharge ledger exists.
    pub fn contract_entailment_stand_downs(&self) -> &[validation::ContractEntailmentStandDown] {
        &self.contract_entailment_stand_downs
    }

    pub(super) const fn timings(&self) -> &CompileTimings {
        &self.timings
    }

    pub fn into_program(self) -> CheckedTrees {
        self.program
    }
}

impl std::ops::Deref for CheckedCompilation {
    type Target = CheckedTrees;

    fn deref(&self) -> &Self::Target {
        &self.program
    }
}

// Compatibility for corruption-oriented integration tests. Production
// projection code must use the explicit scratch seam above. Remove this once
// validator tests construct malformed `CheckedTrees` directly.
impl std::ops::DerefMut for CheckedCompilation {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.program
    }
}

/// One owned request for the checked-Psi frontend. Public compatibility
/// helpers differ only in how they populate this request; execution and option
/// pairing have one implementation.
struct CheckedCompileRequest {
    root_path: std::path::PathBuf,
    target_name: Option<String>,
    package_inputs: Option<PackageCompilationInputs>,
    build_dir: Option<std::path::PathBuf>,
    filesystem_sponsor: Option<build_time_evaluation::BuildMachineFilesystemSponsor>,
    evaluation_sponsor: Option<build_time_evaluation::BuildEvaluationSponsor>,
    replay_record: Option<super::ReviewOnlyBuildFilesystemReplayRecord>,
}

impl CheckedCompileRequest {
    fn new(root_path: &Path, target_name: Option<&str>) -> Self {
        Self {
            root_path: root_path.to_owned(),
            target_name: target_name.map(str::to_owned),
            package_inputs: None,
            build_dir: None,
            filesystem_sponsor: None,
            evaluation_sponsor: None,
            replay_record: None,
        }
    }
}

/// Target-independent source and parse work retained for one or more exact
/// checked children.
///
/// The checkpoint owns no child authority. Each child must independently
/// supply package inputs, build staging, and exact target selection; source
/// assembly rejects a child whose immutable package source projection differs
/// from the one prepared here.
/// Clones share parsed storage; consuming the sole checkpoint moves that storage
/// into its child instead of copying the syntax trees.
#[derive(Clone)]
pub(crate) struct PreparedCheckedSource {
    root_path: std::path::PathBuf,
    source_checkpoint: ImmutableSourceParseCheckpoint,
    shared_timings: CompileTimings,
}

struct CheckedChildExecution<'a> {
    selected_target_profile: Option<target::TargetProfile>,
    package_inputs: Option<&'a PackageCompilationInputs>,
    build_dir: Option<&'a Path>,
    filesystem_sponsor: Option<build_time_evaluation::BuildMachineFilesystemSponsor>,
    evaluation_sponsor: Option<build_time_evaluation::BuildEvaluationSponsor>,
    replay_record: Option<&'a super::ReviewOnlyBuildFilesystemReplayRecord>,
}

impl CheckedChildExecution<'_> {
    #[cfg(test)]
    fn exact_target(selected_target_profile: target::TargetProfile) -> Self {
        Self {
            selected_target_profile: Some(selected_target_profile),
            package_inputs: None,
            build_dir: None,
            filesystem_sponsor: None,
            evaluation_sponsor: None,
            replay_record: None,
        }
    }
}

impl PreparedCheckedSource {
    pub(crate) fn prepare(
        root_path: &Path,
        package_inputs: Option<&PackageCompilationInputs>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut shared_timings = CompileTimings::default();
        let source_checkpoint = ImmutableSourceParseCheckpoint::prepare(
            root_path,
            package_inputs,
            &mut shared_timings,
        )?;
        Ok(Self {
            root_path: root_path.to_owned(),
            source_checkpoint,
            shared_timings,
        })
    }

    pub(crate) fn compile_for_terminal(
        self,
        options: &super::CompileOptions,
        package_inputs: Option<&PackageCompilationInputs>,
    ) -> Result<CheckedCompilation, Vec<Diagnostic>> {
        if options.root_path != self.root_path {
            return Err(vec![Diagnostic::error(
                "checked child compilation root does not match its prepared source checkpoint",
            )]);
        }
        let selected_target_profile = options
            .target_name
            .as_deref()
            .map(|target_name| target::TargetProfile::from_omega_target_name(Some(target_name)))
            .transpose()
            .map_err(|diagnostic| vec![diagnostic])?;
        let build_dir = options.build_dir();
        self.compile_child_with_replay(CheckedChildExecution {
            selected_target_profile,
            package_inputs,
            build_dir: Some(&build_dir),
            filesystem_sponsor: None,
            evaluation_sponsor: None,
            replay_record: None,
        })
    }

    fn compile_child_with_replay(
        self,
        child: CheckedChildExecution<'_>,
    ) -> Result<CheckedCompilation, Vec<Diagnostic>> {
        let target_name = child
            .selected_target_profile
            .map(target::TargetProfile::target_name);
        let mut timings = self.shared_timings;
        let (source_file_count, syntax) = match target_name {
            Some(target_name) => self
                .source_checkpoint
                .for_exact_target(target_name, child.package_inputs)?
                .assemble(&mut timings)?,
            None => self
                .source_checkpoint
                .assemble_targetless(child.package_inputs, &mut timings)?,
        };
        compile_assembled_checked_child(&self.root_path, child, source_file_count, syntax, timings)
    }
}

fn execute_checked_request(
    request: CheckedCompileRequest,
) -> Result<CheckedCompilation, Vec<Diagnostic>> {
    crate::compiler::execution::run_on_compile_thread(move || {
        compile_to_checked_inner_with_replay(
            &request.root_path,
            request.target_name.as_deref(),
            request.package_inputs.as_ref(),
            request.build_dir.as_deref(),
            request.filesystem_sponsor,
            request.evaluation_sponsor,
            request.replay_record.as_ref(),
        )
    })
}

/// Runs ONLY the four frontend stages (lex/parse -> symbol resolution -> typing ->
/// checking) and returns the in-memory `CheckedTrees` program. No backend lowering,
/// no file output. The Psi checked-tree interpreter evaluates this transitional
/// representation as a differential oracle for the native backend while terminal-Psi
/// coverage grows.
pub fn compile_to_checked(
    root_path: &Path,
    target_name: Option<&str>,
) -> Result<CheckedCompilation, Vec<Diagnostic>> {
    execute_checked_request(CheckedCompileRequest::new(root_path, target_name))
}

/// Checked-only compilation using a complete reconciled package graph. This
/// is the evidence-producing frontend seam for package admission and never
/// consults dependency rows in downloaded `build.omg` files.
pub fn compile_to_checked_with_packages(
    root_path: &Path,
    target_name: Option<&str>,
    package_inputs: PackageCompilationInputs,
) -> Result<CheckedCompilation, Vec<Diagnostic>> {
    let mut request = CheckedCompileRequest::new(root_path, target_name);
    request.package_inputs = Some(package_inputs);
    execute_checked_request(request)
}

/// Checked-only compilation whose build machine reconsumes one compiler-owned
/// bounded filesystem replay record. The replay installs no host filesystem
/// provider; authored build inputs and the complete event stream must match.
pub fn compile_to_checked_with_replay_record(
    root_path: &Path,
    target_name: Option<&str>,
    replay_record: super::ReviewOnlyBuildFilesystemReplayRecord,
) -> Result<CheckedCompilation, Vec<Diagnostic>> {
    let mut request = CheckedCompileRequest::new(root_path, target_name);
    request.replay_record = Some(replay_record);
    execute_checked_request(request)
}

/// Package-aware checked compilation whose build machine reconsumes one
/// compiler-owned bounded filesystem replay record without host authority.
pub fn compile_to_checked_with_packages_and_replay_record(
    root_path: &Path,
    target_name: Option<&str>,
    package_inputs: PackageCompilationInputs,
    replay_record: super::ReviewOnlyBuildFilesystemReplayRecord,
) -> Result<CheckedCompilation, Vec<Diagnostic>> {
    let mut request = CheckedCompileRequest::new(root_path, target_name);
    request.package_inputs = Some(package_inputs);
    request.replay_record = Some(replay_record);
    execute_checked_request(request)
}

/// Package-aware checked compilation with a caller-owned writable build root.
/// Resolver snapshots remain immutable; package admission and build execution
/// must stage outputs in separate custody.
pub fn compile_to_checked_with_packages_in_build_dir(
    root_path: &Path,
    build_dir: &Path,
    target_name: Option<&str>,
    package_inputs: PackageCompilationInputs,
) -> Result<CheckedCompilation, Vec<Diagnostic>> {
    let mut request = CheckedCompileRequest::new(root_path, target_name);
    request.package_inputs = Some(package_inputs);
    request.build_dir = Some(build_dir.to_owned());
    execute_checked_request(request)
}

/// Package-aware checked compilation whose build machine consumes one
/// caller-owned staging sponsor shared across a complete review session.
pub fn compile_to_checked_with_packages_in_sponsored_build_dir(
    root_path: &Path,
    build_dir: &Path,
    target_name: Option<&str>,
    package_inputs: PackageCompilationInputs,
    filesystem_sponsor: build_time_evaluation::BuildMachineFilesystemSponsor,
) -> Result<CheckedCompilation, Vec<Diagnostic>> {
    let mut request = CheckedCompileRequest::new(root_path, target_name);
    request.package_inputs = Some(package_inputs);
    request.build_dir = Some(build_dir.to_owned());
    request.filesystem_sponsor = Some(filesystem_sponsor);
    execute_checked_request(request)
}

/// Package-aware checked compilation with both compiler-owned review-session
/// accounts. Filesystem custody and deterministic evaluator work remain
/// separate resources with separate claims.
pub fn compile_to_checked_with_packages_in_sponsored_build_session(
    root_path: &Path,
    build_dir: &Path,
    target_name: Option<&str>,
    package_inputs: PackageCompilationInputs,
    filesystem_sponsor: build_time_evaluation::BuildMachineFilesystemSponsor,
    evaluation_sponsor: build_time_evaluation::BuildEvaluationSponsor,
) -> Result<CheckedCompilation, Vec<Diagnostic>> {
    let mut request = CheckedCompileRequest::new(root_path, target_name);
    request.package_inputs = Some(package_inputs);
    request.build_dir = Some(build_dir.to_owned());
    request.filesystem_sponsor = Some(filesystem_sponsor);
    request.evaluation_sponsor = Some(evaluation_sponsor);
    execute_checked_request(request)
}

/// Run the ordinary checked frontend for the typed terminal-component handoff
/// without consuming its caller-owned request or deployment authority.
pub(crate) fn compile_to_checked_for_terminal(
    options: &super::CompileOptions,
    package_inputs: Option<&PackageCompilationInputs>,
) -> Result<CheckedCompilation, Vec<Diagnostic>> {
    let prepared = PreparedCheckedSource::prepare(&options.root_path, package_inputs)?;
    prepared.compile_for_terminal(options, package_inputs)
}

struct CheckedFrontend {
    typing: CheckedFrontendTyping,
    selected_target_machine_declarations:
        crate::pipeline::target_machines::SelectedTargetMachineDeclarations,
    build_source_id: Option<source::SourceId>,
}

enum CheckedFrontendTyping {
    Continuable(symbol_resolved_trees_to_typed_trees::SeededTypingBase),
    Complete(typed_trees::TypedTrees),
}

impl CheckedFrontendTyping {
    fn typed(&self) -> &typed_trees::TypedTrees {
        match self {
            Self::Continuable(base) => base.typed(),
            Self::Complete(typed) => typed,
        }
    }

    fn into_typed(self) -> typed_trees::TypedTrees {
        match self {
            Self::Continuable(base) => base.into_typed(),
            Self::Complete(typed) => typed,
        }
    }
}

impl CheckedFrontend {
    fn typed(&self) -> &typed_trees::TypedTrees {
        self.typing.typed()
    }
}

/// One activation-local D18 checkpoint admitted before any authored build code
/// executes.
///
/// The coherent base frontend, exact prepared build projection, reach and
/// authority verdicts, package declaration verdict, and exact base source map
/// needed to bind a generated extension stay coupled across execution.
struct AdmittedBuildCheckpoint {
    frontend: CheckedFrontend,
    admitted_build: crate::pipeline::build_config::AdmittedBuildProgram,
    package_authority_verdict:
        Option<crate::pipeline::package_declaration_admission::AuthoredDeclarationAuthorityVerdict>,
    base_sources: Arc<source::SourceMap>,
}

struct ExecutedBuildCheckpoint {
    frontend: CheckedFrontend,
    computed_build_config: crate::pipeline::build_config::ComputedBuildConfig,
    package_authority_verdict:
        Option<crate::pipeline::package_declaration_admission::AuthoredDeclarationAuthorityVerdict>,
    base_sources: Arc<source::SourceMap>,
}

impl AdmittedBuildCheckpoint {
    fn execute(self) -> Result<ExecutedBuildCheckpoint, Vec<Diagnostic>> {
        let selected_build_symbol = self.admitted_build.selected_build_machine_symbol();
        let computed_build_config = self.admitted_build.execute()?;
        if computed_build_config.selected_build_machine_symbol != selected_build_symbol {
            return Err(vec![Diagnostic::error(
                "build execution returned a selected symbol different from its admitted checkpoint",
            )]);
        }
        Ok(ExecutedBuildCheckpoint {
            frontend: self.frontend,
            computed_build_config,
            package_authority_verdict: self.package_authority_verdict,
            base_sources: self.base_sources,
        })
    }
}

fn lower_checked_frontend(
    mut syntax: crate::pipeline::source_assembly::AssembledSyntax,
    target_name: Option<&str>,
    package_inputs: Option<&PackageCompilationInputs>,
    timings: &mut CompileTimings,
) -> Result<CheckedFrontend, Vec<Diagnostic>> {
    let evaluated = match package_inputs {
        Some(package_inputs) => {
            build_time_evaluation::evaluate_pre_resolution_with_sources_and_authority(
                syntax.syntax_trees,
                syntax.sources.clone(),
                std::sync::Arc::new(package_inputs.clone()),
            )
        }
        None => build_time_evaluation::evaluate_pre_resolution_with_sources(
            syntax.syntax_trees,
            syntax.sources.clone(),
        ),
    }?;
    let (syntax_trees, pre_check) = evaluated.into_syntax_and_pre_check();
    syntax.syntax_trees = syntax_trees;
    let selected_target_machine_declarations =
        crate::pipeline::target_machines::filter_target_machines(
            &mut syntax.syntax_trees,
            target_name,
        )?;
    let build_source_id = syntax.build_source_id;
    let resolved = syntax_trees_to_symbol_resolved_trees(syntax, timings)?;
    let mut typing_base = symbol_resolved_trees_to_seeded_base(resolved, timings)?;
    pre_check.evaluate(typing_base.typed_mut())?;
    // Build evaluation consumes this coherent private typed stage before the
    // final checked-tree lowering. Bind trait-valued parameter-field calls now
    // so the evaluator receives the same exact requirement identity that the
    // checker will subsequently validate and retain.
    validation::resolve_dynamic_call_targets(typing_base.typed_mut())?;
    Ok(CheckedFrontend {
        typing: CheckedFrontendTyping::Continuable(typing_base),
        selected_target_machine_declarations,
        build_source_id,
    })
}

fn try_seeded_extension(
    base: symbol_resolved_trees_to_typed_trees::SeededTypingBase,
    base_sources: &Arc<source::SourceMap>,
    extension: crate::pipeline::source_assembly::RetainedGeneratedSyntaxExtension,
    selected_target_machine_declarations:
        crate::pipeline::target_machines::SelectedTargetMachineDeclarations,
    target_name: Option<&str>,
    package_inputs: Option<&PackageCompilationInputs>,
    timings: &mut CompileTimings,
) -> Result<
    (
        typed_trees::TypedTrees,
        crate::pipeline::target_machines::SelectedTargetMachineDeclarations,
    ),
    Vec<Diagnostic>,
> {
    let retained_prefix = base.typed().clone();
    let mut wire_schema_frontier = retained_prefix.wire_schemas().len();
    let (extension_units, sources) = extension.into_pre_resolution_inputs(base_sources)?;
    let mut extension_syntax = syntax_trees::SyntaxTrees::new(source::SourceId(base_sources.len()));
    let mut pre_checks = Vec::with_capacity(extension_units.len());
    for unit in extension_units {
        let evaluated = match package_inputs {
            Some(package_inputs) => {
                build_time_evaluation::evaluate_pre_resolution_with_sources_and_authority(
                    unit,
                    sources.clone(),
                    Arc::new(package_inputs.clone()),
                )
            }
            None => {
                build_time_evaluation::evaluate_pre_resolution_with_sources(unit, sources.clone())
            }
        }?;
        let (unit, pre_check) = evaluated.into_syntax_and_pre_check();
        extension_syntax.extend_from(&unit);
        pre_checks.push(pre_check);
    }
    let selected_target_machine_declarations = selected_target_machine_declarations
        .filter_generated_extension(&mut extension_syntax, target_name)?;
    let seeded = resolve_seeded_syntax_extension(
        base.resolved_base_for_extension(),
        &extension_syntax,
        sources,
        timings,
    )?;
    let rebased = seeded
        .rebase_authored_selections_for_typed_continuation(
            base.typed().authored_declaration_selections(),
        )
        .map_err(|(_, error)| {
            vec![Diagnostic::error(format!(
                "generated-source authored-selection suffix could not join the retained typed base: {error:?}"
            ))]
        })?;
    let mut typed = match type_seeded_extension(rebased, base, timings) {
        Ok(typed) => Ok(typed),
        Err((
            _,
            symbol_resolved_trees_to_typed_trees::SeededContinuationError::Lowering(
                diagnostic,
            ),
        )) => Err(vec![diagnostic]),
        Err((
            _,
            symbol_resolved_trees_to_typed_trees::SeededContinuationError::UnsupportedExtensionShape,
        )) => Err(vec![Diagnostic::error(
            "generated source uses a declaration shape not yet supported by retained-checkpoint continuation; reconstructing a second frontend is forbidden",
        )]),
        Err((_, error)) => Err(vec![Diagnostic::error(format!(
            "generated-source continuation violated its retained-base invariant: {error:?}"
        ))]),
    }?;
    for pre_check in pre_checks {
        pre_check.evaluate_extension(&mut typed, wire_schema_frontier)?;
        wire_schema_frontier = typed.wire_schemas().len();
    }
    if !symbol_resolved_trees_to_typed_trees::retained_typed_base_is_exact_prefix(
        &retained_prefix,
        &typed,
    ) {
        return Err(vec![Diagnostic::error(
            "generated-source pre-check evaluation changed the retained typed base",
        )]);
    }
    Ok((typed, selected_target_machine_declarations))
}

fn compile_to_checked_inner_with_replay(
    root_path: &Path,
    target_name: Option<&str>,
    package_inputs: Option<&PackageCompilationInputs>,
    build_dir: Option<&Path>,
    filesystem_sponsor: Option<build_time_evaluation::BuildMachineFilesystemSponsor>,
    evaluation_sponsor: Option<build_time_evaluation::BuildEvaluationSponsor>,
    replay_record: Option<&super::ReviewOnlyBuildFilesystemReplayRecord>,
) -> Result<CheckedCompilation, Vec<Diagnostic>> {
    let selected_target_profile = target_name
        .map(|target_name| target::TargetProfile::from_omega_target_name(Some(target_name)))
        .transpose()
        .map_err(|diagnostic| vec![diagnostic])?;
    let prepared = PreparedCheckedSource::prepare(root_path, package_inputs)?;
    prepared.compile_child_with_replay(CheckedChildExecution {
        selected_target_profile,
        package_inputs,
        build_dir,
        filesystem_sponsor,
        evaluation_sponsor,
        replay_record,
    })
}

fn compile_assembled_checked_child(
    root_path: &Path,
    child: CheckedChildExecution<'_>,
    mut source_file_count: usize,
    syntax: crate::pipeline::source_assembly::AssembledSyntax,
    mut timings: CompileTimings,
) -> Result<CheckedCompilation, Vec<Diagnostic>> {
    let CheckedChildExecution {
        selected_target_profile,
        package_inputs,
        build_dir,
        filesystem_sponsor,
        evaluation_sponsor,
        replay_record,
    } = child;
    // CLI aliases end at request admission. Every source, build, provider, and
    // artifact consumer below observes only the catalog's canonical spelling.
    let target_name = selected_target_profile.map(target::TargetProfile::target_name);
    let mut generated_source_custody = syntax.generated_source_custody.clone();
    let base_sources = syntax.sources.clone();
    let mut frontend = lower_checked_frontend(syntax, target_name, package_inputs, &mut timings)?;
    let package_authority_verdict = if let Some(package_inputs) = package_inputs {
        Some(crate::pipeline::package_declaration_admission::validate_authored_declaration_selections_before_build(
            frontend.typed(),
            package_inputs,
            &generated_source_custody,
            &mut timings,
        )?)
    } else {
        None
    };
    if let Some(replay_record) = replay_record {
        let expected_source_metadata = package_inputs
            .map(|inputs| {
                inputs
                    .canonical_source_metadata(inputs.root())
                    .map(|metadata| {
                        crate::pipeline::build_config::BuildCanonicalSourceMetadataIdentity::new(
                            metadata.policy_version(),
                            *metadata.source_content_commitment(),
                        )
                    })
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "package-aware filesystem replay requires canonical Source metadata",
                        )]
                    })
            })
            .transpose()?;
        if replay_record.canonical_source_metadata_identity() != expected_source_metadata {
            return Err(vec![Diagnostic::error(
                "build filesystem replay record does not match the current canonical Source metadata identity",
            )]);
        }
    }
    let filesystem_replay = replay_record
        .map(|record| {
            super::build_replay_record::rehydrate_review_only_build_filesystem_replay_record(
                record,
                super::BuildFilesystemReplayRecordLimits::new(
                    record.canonical_bytes().len(),
                    4_096,
                ),
            )
            .map_err(|error| {
                vec![Diagnostic::error(format!(
                    "could not reopen build filesystem replay record: {error}"
                ))]
            })
        })
        .transpose()?;
    let build_dir = build_dir.map(Path::to_path_buf).unwrap_or_else(|| {
        root_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(|parent| parent.join("build"))
            .unwrap_or_else(|| std::path::PathBuf::from("build"))
    });
    let mut build_machine_filesystem_scope = if let Some(inputs) = package_inputs {
        crate::pipeline::build_config::BuildMachineFilesystemScope::for_package_root(
            inputs
                .package_root(inputs.root())
                .expect("validated package inputs retain their root")
                .to_path_buf(),
            build_dir,
            filesystem_sponsor,
            inputs.canonical_source_metadata(inputs.root()).cloned(),
        )
    } else {
        crate::pipeline::build_config::BuildMachineFilesystemScope::for_root(
            root_path,
            build_dir,
            filesystem_sponsor,
        )
    };
    if let Some(filesystem_replay) = filesystem_replay {
        build_machine_filesystem_scope =
            build_machine_filesystem_scope.with_replay(filesystem_replay);
    }
    let admitted_build = crate::pipeline::build_config::admit_build_program(
        frontend.typed(),
        frontend.build_source_id,
        &build_machine_filesystem_scope,
        evaluation_sponsor.as_ref(),
        selected_target_profile,
    )?;
    let ExecutedBuildCheckpoint {
        frontend: executed_frontend,
        computed_build_config,
        package_authority_verdict,
        base_sources,
    } = (AdmittedBuildCheckpoint {
        frontend,
        admitted_build,
        package_authority_verdict,
        base_sources,
    })
    .execute()?;
    frontend = executed_frontend;
    let own_generated_sources = computed_build_config.generated_sources.clone();
    let selected_build_machine_symbol = if computed_build_config.generated_sources.is_empty() {
        computed_build_config.selected_build_machine_symbol
    } else {
        let package_inputs = package_inputs.ok_or_else(|| {
            vec![Diagnostic::error(
                "generated-source final compilation requires package-aware source custody",
            )]
        })?;
        let package_root = package_inputs
            .package_root(package_inputs.root())
            .expect("validated package inputs retain their root package");
        let extension = crate::pipeline::source_assembly::retain_generated_syntax_extension(
            &base_sources,
            package_root,
            Some(package_inputs.root()),
            &computed_build_config.generated_sources,
        )?;
        source_file_count = source_file_count
            .checked_add(extension.source_count())
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "generated package source count exceeds the compiler range",
                )]
            })?;
        generated_source_custody.extend(extension.generated_source_custody().iter().cloned());
        let CheckedFrontend {
            typing,
            selected_target_machine_declarations,
            build_source_id,
        } = frontend;
        let CheckedFrontendTyping::Continuable(typing_base) = typing else {
            return Err(vec![Diagnostic::error(
                "generated-source continuation lost its retained frontend base",
            )]);
        };
        let (typed, selected_target_machine_declarations) = try_seeded_extension(
            typing_base,
            &base_sources,
            extension,
            selected_target_machine_declarations,
            target_name,
            Some(package_inputs),
            &mut timings,
        )?;
        frontend = CheckedFrontend {
            typing: CheckedFrontendTyping::Complete(typed),
            selected_target_machine_declarations,
            build_source_id,
        };
        computed_build_config.selected_build_machine_symbol
    };
    let CheckedFrontend {
        typing,
        selected_target_machine_declarations,
        ..
    } = frontend;
    let mut typed = typing.into_typed();
    let selected_build_machine_identity = selected_build_machine_symbol
        .map(|selected| {
            let machine = typed
                .machines()
                .iter()
                .find(|machine| machine.symbol == selected)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "selected build machine disappeared before final checking",
                    )]
                })?;
            typed
                .normalized_machine_overload_identity(machine)
                .map(|identity| identity.identity().to_owned())
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "selected build machine has no canonical callable identity",
                    )]
                })
        })
        .transpose()?;
    let build_evaluation_usage = computed_build_config.evaluation_usage;
    let build_observation_summary = computed_build_config.observation_summary;
    let optimization_report = computed_build_config.optimization_report_request;
    let build_config = computed_build_config.config;
    let selected_native_target = selected_target_profile
        .map(target::TargetProfile::native_target)
        .unwrap_or_else(target::NativeTarget::host);
    let mut boundary_calling_plan_realizations =
        crate::pipeline::calling_policy_plans::compute_boundary_calling_plans(
            &mut typed,
            selected_native_target,
            &build_config.opaque_representation_selections,
            package_inputs,
        )?;
    let opaque_representation_selections = build_config.opaque_representation_selections.clone();
    let x86_scalar_fma_provider = build_config.x86_scalar_fma_provider;
    let subsystem = build_config.subsystem;
    let optimization = super::optimization::checked_handoff::CheckedOptimizationHandoff::retain(
        build_config.optimizations.clone(),
        optimization_report,
    );
    // Compatibility demands are semantic checks, not report-mode behavior.
    // Validate them on the canonical checked route even when no auxiliary
    // artifact writer is requested by the outer compiler coordinator.
    crate::pipeline::reporting::wire::validate_wire_protocol(
        &typed,
        &build_config.wire_compatibility_demands,
    )?;
    // A semantic-only checked compilation has no selected target and therefore
    // no storage root. Authored bindings remain available in the evaluated
    // build configuration, but only an exact target selection may activate one
    // for interpreter or production execution.
    let mut selected_program_entry = crate::pipeline::build_config::select_compiler_program_entry(
        &typed,
        &build_config,
        selected_target_profile,
        &boundary_calling_plan_realizations,
        package_inputs.and_then(|inputs| {
            inputs.accepted_semantic_binding(
                package_compilation::AcceptedSemanticBindingRole::UefiX64ProgramEntry,
            )
        }),
    )?;
    let settled_target_machines =
        selected_target_machine_declarations.settle_provider_defaults(&typed)?;
    let target_provider_defaults = settled_target_machines.provider_defaults;
    // PRV4 provider selection mirrors the native pipeline: candidates remain
    // separate by provider type and only the uniquely covering candidate may
    // rewrite adapter calls in the interpreter program.
    let evaluated_via_bindings = provider_planning::evaluated_via_bindings::evaluate_via_bindings(
        &typed,
        selected_target_profile,
        package_inputs,
    )?;
    let derived_provider_plans =
        crate::pipeline::provider_plans::derive_satisfies_plans_with_evaluated_bindings_and_target_machine_origins(
            &typed,
            target_name,
            &evaluated_via_bindings,
            &settled_target_machines.origins,
        )?;
    let provider_plans = derived_provider_plans
        .iter()
        .map(|derived| derived.plan.clone())
        .collect::<Vec<_>>();
    let selected_native_target = selected_target_profile.map(target::TargetProfile::native_target);
    let provider_selection_target =
        selected_native_target.unwrap_or_else(target::NativeTarget::host);
    let diagnostics = crate::pipeline::provider_plans::validate_derived_provider_plan_candidates(
        &typed,
        &evaluated_via_bindings,
        &derived_provider_plans,
    );
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let selected_provider_plans =
        crate::pipeline::provider_plans::select_provider_plans_with_provenance(
            &derived_provider_plans,
            provider_selection_target,
            &target_provider_defaults,
            &build_config.provider_selections,
        )?;
    let mut fused_service_erasures = Vec::new();
    for selected in &selected_provider_plans {
        let composition_mode = selected
            .selected_by
            .composition_mode()
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        if composition_mode != provider_planning::CompositionMode::Fused {
            continue;
        }
        let requirement = selected.derived.provenance.schema.symbol();
        if typed
            .traits()
            .iter()
            .any(|definition| definition.is_boundary && definition.symbol == requirement)
        {
            fused_service_erasures.push(
                typed_trees::typed_trees::FusedServiceErasureAuthorization {
                    requirement,
                    provider_plan_digest: *selected.derived.plan.identity_digest().as_bytes(),
                },
            );
        }
    }
    typed
        .bind_fused_service_erasures(fused_service_erasures)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let selected_semantic_plans = selected_provider_plans
        .iter()
        .map(|selected| selected.derived.plan.clone())
        .collect::<Vec<_>>();
    crate::pipeline::provider_plans::validate_selected_synchronous_invocation_cycles(
        &typed,
        &selected_semantic_plans,
    )?;
    let external_binding_rows = provider_planning::plans::extract_normalized_import_binding_rows(
        target_name,
        provider_selection_target,
        &selected_semantic_plans,
        &boundary_calling_plan_realizations,
        &typed,
    )?;
    let (selected_provider_plan_facts, selected_provider_provenance) =
        crate::pipeline::provider_plans::selected_provider_plan_facts_with_provenance(
            &typed,
            &evaluated_via_bindings,
            selected_provider_plans,
        )?;
    let root_grants = build_config
        .grants
        .iter()
        .map(|grant| grant.selector.clone())
        .collect::<Vec<_>>();
    if package_inputs.is_some() {
        trust_model::reject_package_non_provider_grants(
            &typed,
            &root_grants,
            &provider_plans,
            &selected_provider_plan_facts,
        )?;
    }
    let checked = typed_trees_to_checked_trees(
        typed,
        &mut timings,
        TypedToCheckedSettlementInput {
            native_target: selected_native_target,
            package_inputs,
            selected_build_machine: selected_build_machine_symbol,
            boundary_calling_plan_realizations: &mut boundary_calling_plan_realizations,
            opaque_representation_selections: &opaque_representation_selections,
            provider_plans: &provider_plans,
            selected_provider_plan_facts,
            root_grants: &root_grants,
            authored_root_grants: &build_config.grants,
        },
    )?;
    if let Some(package_inputs) = package_inputs {
        crate::pipeline::package_declaration_admission::validate_authored_declaration_selections(
            &checked.program,
            package_inputs,
        )?;
    }
    let exact_component_progress_root = selected_program_entry.as_ref().map(|entry| {
        let source = entry.source_signature();
        crate::pipeline::component_progress::ExactComponentProgressRoot::new(
            source.machine_symbol(),
            source.normalized_callable_identity(),
        )
    });
    let selected_execution_settlement = settle_selected_execution(
        checked,
        SelectedExecutionSettlementInput {
            exact_component_progress_root,
            provider_selection_target,
            selected_target_profile,
            selected_provider_provenance,
            opaque_representation_selections: &opaque_representation_selections,
            accepted_console_binding: package_inputs.and_then(|inputs| {
                inputs.accepted_semantic_binding(
                    package_compilation::AcceptedSemanticBindingRole::ConsoleExitProcessI32,
                )
            }),
            accepted_filesystem_binding: package_inputs.and_then(|inputs| {
                inputs.accepted_semantic_binding(
                    package_compilation::AcceptedSemanticBindingRole::FilesystemHostService,
                )
            }),
            accepted_uefi_binding: package_inputs.and_then(|inputs| {
                inputs.accepted_semantic_binding(
                    package_compilation::AcceptedSemanticBindingRole::UefiX64ProgramEntry,
                )
            }),
        },
    )?;
    let x86_scalar_fma_plan_associations =
        super::x86_fma_plan_association::bind_checked_x86_scalar_fma_plan_associations(
            &selected_execution_settlement.program,
            &selected_execution_settlement.selected_provider_plan_facts,
            &selected_execution_settlement.selected_provider_provenance,
            x86_scalar_fma_provider,
            selected_target_profile,
        )?;

    if let Some(entry) = selected_program_entry.as_mut() {
        let establishments = selected_dispatch::derive_fused_program_entry_establishments(
            &selected_execution_settlement.program,
            entry.source_signature(),
            &selected_execution_settlement.selected_provider_provenance,
        )?;
        entry
            .bind_fused_service_establishments(establishments)
            .map_err(|message| vec![Diagnostic::error(message)])?;
    }

    // `typed_trees_to_checked_trees` wraps the program in an `Arc`; unwrap it for the
    // caller (this is the only owner at this point in the pipeline).
    let program = Arc::try_unwrap(selected_execution_settlement.program)
        .unwrap_or_else(|shared| (*shared).clone());
    let package_subject = package_inputs
        .map(|inputs| {
            package_compilation::derive_package_compilation_subject(
                &program,
                inputs,
                &generated_source_custody,
            )
        })
        .transpose()?;
    let exact_toolchain_sources = package_compilation::toolchain_source_identities(&program)?;
    if package_subject.is_some() {
        package_compilation::verify_current_files(&program, &generated_source_custody)?;
    }
    if let Some(package_inputs) = package_inputs {
        package_inputs.validate_canonical_source_metadata()?;
    }
    Ok(CheckedCompilation {
        program,
        dispatch_source_edits: selected_execution_settlement.dispatch_source_edits,
        source_file_count,
        subsystem,
        package_subject,
        resolved_semantic_bindings: selected_execution_settlement.resolved_semantic_bindings,
        base_source_consumption_commitment: package_authority_verdict
            .as_ref()
            .map(|verdict| verdict.base_source_consumption_commitment()),
        exact_toolchain_sources,
        generated_source_custody,
        own_generated_sources,
        selected_target_profile,
        selected_native_target,
        x86_scalar_fma_provider,
        x86_scalar_fma_plan_associations,
        selected_program_entry,
        selected_build_machine_symbol,
        selected_build_machine_identity,
        opaque_representation_selections,
        boundary_calling_plan_realizations,
        optimization,
        selected_provider_plans: selected_execution_settlement.selected_provider_plan_facts,
        selected_provider_grants: selected_execution_settlement.selected_provider_grants,
        provider_plans,
        evaluated_via_bindings,
        external_binding_rows,
        root_grants,
        accepted_template_classifications: selected_execution_settlement
            .accepted_template_classifications,
        selected_provider_provenance: selected_execution_settlement.selected_provider_provenance,
        component_progress: selected_execution_settlement.component_progress,
        task_activations: selected_execution_settlement.task_activations,
        callback_placements: selected_execution_settlement.callback_placements,
        build_evaluation_usage,
        build_observation_summary,
        contract_entailment_stand_downs: selected_execution_settlement
            .contract_entailment_stand_downs,
        timings,
    })
}

#[cfg(test)]
mod continuation_tests {
    use super::{CheckedChildExecution, PreparedCheckedSource};
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_PREPARED_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct PreparedFixture {
        root: std::path::PathBuf,
        main: std::path::PathBuf,
    }

    impl PreparedFixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "omega-prepared-checked-source-{}-{}",
                std::process::id(),
                NEXT_PREPARED_FIXTURE.fetch_add(1, Ordering::Relaxed),
            ));
            fs::create_dir(&root).expect("create prepared checked-source fixture");
            let main = root.join("main.omg");
            fs::write(&main, "const ANSWER: u32 = 42;\n")
                .expect("write prepared checked-source main");
            fs::write(
                root.join("build.omg"),
                r#"machine build(builder: &mut Build) {
    builder.application("prepared-checked-source");
    transition builder.target {
        TargetProfile::WindowsX86_64 -> windows(builder)
        _ -> other(builder)
    }
    state windows(builder: &mut Build) {
        builder.subsystem = Subsystem::Gui;
    }
    state other(builder: &mut Build) {
        builder.subsystem = Subsystem::Console;
    }
}
"#,
            )
            .expect("write prepared checked-source build");
            Self { root, main }
        }
    }

    impl Drop for PreparedFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn prepared_source_checkpoint_preserves_standalone_child_identity_and_siblings() {
        let fixture = PreparedFixture::new();
        let standalone = super::compile_to_checked(&fixture.main, Some("windows_x86_64"))
            .expect("standalone Windows child should compile");
        let main = fixture.main.clone();
        let (windows, linux, windows_again) =
            crate::compiler::execution::run_on_compile_thread(move || {
                let prepared = PreparedCheckedSource::prepare(&main, None)
                    .expect("prepare checked source checkpoint");
                let windows = prepared
                    .clone()
                    .compile_child_with_replay(CheckedChildExecution::exact_target(
                        target::TargetProfile::WindowsX64,
                    ))
                    .expect("compile Windows child from prepared source");
                let linux = prepared
                    .clone()
                    .compile_child_with_replay(CheckedChildExecution::exact_target(
                        target::TargetProfile::LinuxX64,
                    ))
                    .expect("compile Linux child from prepared source");
                let windows_again = prepared
                    .compile_child_with_replay(CheckedChildExecution::exact_target(
                        target::TargetProfile::WindowsX64,
                    ))
                    .expect("recompile Windows child after sibling");
                Ok((windows, linux, windows_again))
            })
            .expect("spawn prepared source compiler thread");

        assert_eq!(standalone, windows);
        assert_eq!(windows, windows_again);
        assert_eq!(windows.subsystem(), 2);
        assert_eq!(
            linux.selected_target_profile(),
            Some(target::TargetProfile::LinuxX64),
        );
        assert_ne!(linux.subsystem(), windows.subsystem());
    }
}
