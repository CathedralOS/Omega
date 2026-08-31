use crate::pipeline::PackageCompilationInputs;
use crate::pipeline::phase_transitions::{
    SelectedExecutionSettlementInput, TypedToCheckedSettlementInput, settle_selected_execution,
    symbol_resolved_trees_to_typed_trees, syntax_trees_to_symbol_resolved_trees,
    typed_trees_to_checked_trees,
};
use crate::pipeline::source_assembly::source_files_to_syntax_trees_for_engine;
use crate::pipeline::timing::CompileTimings;
use psi_checked_trees::CheckedTrees;
use psi_diagnostics::Diagnostic;
use std::path::Path;
use std::sync::Arc;

/// Psi-checked semantics paired with the Omega-owned provider realization
/// selected for one engine run. The semantic program deliberately does not
/// retain target/provider installation state.
#[derive(Debug, Clone)]
pub struct CheckedCompilation {
    program: CheckedTrees,
    source_file_count: usize,
    subsystem: u16,
    package_subject: Option<omega_package_compilation::PackageCompilationSubject>,
    exact_toolchain_sources: Vec<(psi_source::SourceId, [u8; 32])>,
    generated_source_custody: Vec<(
        psi_source::SourceId,
        omega_build_output::PackageGeneratedSource,
    )>,
    own_generated_sources: Vec<omega_build_output::PackageGeneratedSource>,
    selected_target_profile: Option<omega_target::TargetProfile>,
    selected_native_target: Option<omega_target::NativeTarget>,
    selected_program_entry: Option<omega_build_evaluation::SelectedCompilerProgramEntry>,
    selected_build_machine_symbol: Option<psi_symbols::SymbolHandle>,
    selected_build_machine_identity: Option<String>,
    opaque_representation_selections:
        Vec<omega_representation_planning::OpaqueRepresentationSelection>,
    boundary_calling_plan_realizations:
        Vec<omega_provider_planning::calling_policy_plans::BoundaryCallingPlanRealization>,
    optimization_selections: omega_optimization_core::OptimizationSelections,
    optimization_selection_identity: omega_optimization_core::OptimizationSelectionIdentity,
    optimization_report: omega_optimization_pipeline::OptimizationReportRequest,
    selected_provider_plans: omega_effects::SelectedProviderPlanFacts,
    selected_provider_grants: Vec<omega_trust_model::ResolvedAuthoredSelectedProviderGrant>,
    provider_plans: Vec<omega_effects::provider_plan::ProviderPlan>,
    external_binding_rows: Vec<omega_calling_conventions::ExternalBindingRow>,
    root_grants: Vec<String>,
    accepted_template_classifications: omega_trust_model::AcceptedTemplateClassifications,
    selected_provider_provenance: Vec<super::provider_plans::SelectedProviderReviewProvenance>,
    component_progress: Option<omega_effects::ComponentProgressManifest>,
    task_activations: omega_task_plans::TaskActivationPlanSet,
    callback_placements: Vec<omega_backend_plan::BoundNominalCallbackPlacement>,
    build_evaluation_usage: Option<super::build_config::BuildEvaluationUsage>,
    build_observation_summary: Option<super::build_config::BuildObservationSummary>,
    contract_entailment_stand_downs: Vec<psi_validation::ContractEntailmentStandDown>,
    timings: CompileTimings,
}

// Timing measurements are nondeterministic observations of a compilation,
// not part of checked semantic identity. Keep the public semantic equality
// contract while deliberately excluding `timings`.
impl PartialEq for CheckedCompilation {
    fn eq(&self, other: &Self) -> bool {
        self.program == other.program
            && self.source_file_count == other.source_file_count
            && self.subsystem == other.subsystem
            && self.package_subject == other.package_subject
            && self.exact_toolchain_sources == other.exact_toolchain_sources
            && self.generated_source_custody == other.generated_source_custody
            && self.own_generated_sources == other.own_generated_sources
            && self.selected_target_profile == other.selected_target_profile
            && self.selected_native_target == other.selected_native_target
            && self.selected_program_entry == other.selected_program_entry
            && self.selected_build_machine_symbol == other.selected_build_machine_symbol
            && self.selected_build_machine_identity == other.selected_build_machine_identity
            && self.opaque_representation_selections == other.opaque_representation_selections
            && self.boundary_calling_plan_realizations == other.boundary_calling_plan_realizations
            && self.optimization_selections == other.optimization_selections
            && self.optimization_selection_identity == other.optimization_selection_identity
            && self.optimization_report == other.optimization_report
            && self.selected_provider_plans == other.selected_provider_plans
            && self.selected_provider_grants == other.selected_provider_grants
            && self.provider_plans == other.provider_plans
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
    pub const fn package_identity(&self) -> Option<psi_core::PackageKeyIdentity> {
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

    /// Canonical package/source subject derived from the final checked source
    /// closure. Standalone compilation has no package subject.
    pub const fn package_compilation_subject(
        &self,
    ) -> Option<&omega_package_compilation::PackageCompilationSubject> {
        self.package_subject.as_ref()
    }

    /// Compiler-validated exact source owners used only while projecting
    /// package-review structural type identity. Source IDs are private join
    /// coordinates and never enter canonical review bytes.
    #[doc(hidden)]
    pub fn exact_toolchain_sources(&self) -> &[(psi_source::SourceId, [u8; 32])] {
        &self.exact_toolchain_sources
    }

    /// Re-read every ordinary physical source path and require it to equal the
    /// bytes retained by the frontend. Generated sources are instead checked
    /// against their compiler-retained staged-output custody. Resolver
    /// orchestration calls this around its own whole-snapshot verification;
    /// hostile same-user races still require an OS isolation boundary.
    pub fn verify_current_source_consumption(&self) -> Result<(), Vec<Diagnostic>> {
        omega_package_compilation::verify_current_files(
            &self.program,
            &self.generated_source_custody,
        )
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
    pub const fn selected_native_target(&self) -> Option<omega_target::NativeTarget> {
        self.selected_native_target
    }

    /// Exact deployment-policy target selected for this checked compilation.
    /// This remains distinct when profiles share a native ABI, notably
    /// `windows_x64` and `uefi_x64`.
    pub const fn selected_target_profile(&self) -> Option<omega_target::TargetProfile> {
        self.selected_target_profile
    }

    /// Exact target-owned `ProgramEntry` choice retained by Omega, if this
    /// checked-only compilation had one. Pure semantic checking is entry-
    /// agnostic; an execution caller must not infer a machine from its name.
    pub fn selected_program_entry_machine(&self) -> Option<&str> {
        self.selected_program_entry
            .as_ref()
            .map(omega_build_evaluation::SelectedCompilerProgramEntry::machine_name)
    }

    /// Complete build-owned `ProgramEntry` settlement captured while typed
    /// declarations and evaluated calling plans were still available. The
    /// source signature and optional target calling plans remain one custody
    /// object; downstream stages must not reconstruct either from the retained
    /// machine name.
    pub const fn selected_program_entry(
        &self,
    ) -> Option<&omega_build_evaluation::SelectedCompilerProgramEntry> {
        self.selected_program_entry.as_ref()
    }

    /// Exact symbol of the uniquely selected build machine. No build machine
    /// is represented by `None`; callers must not rediscover one by name.
    pub const fn selected_build_machine_symbol(&self) -> Option<psi_symbols::SymbolHandle> {
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
    ) -> &[omega_representation_planning::OpaqueRepresentationSelection] {
        &self.opaque_representation_selections
    }

    /// Exact validated boundary calling-plan realizations retained while the
    /// typed declaration graph and selected opaque representations still
    /// coexisted. This is compiler custody for downstream reconstruction; its
    /// presence does not itself publish a package ABI or admission row.
    pub fn boundary_calling_plan_realizations(
        &self,
    ) -> &[omega_provider_planning::calling_policy_plans::BoundaryCallingPlanRealization] {
        &self.boundary_calling_plan_realizations
    }

    /// Exact named optimizations selected by the authoritative root build.
    /// Empty retains the ordinary, optimizer-free compilation path.
    pub const fn optimization_selections(
        &self,
    ) -> &omega_optimization_core::OptimizationSelections {
        &self.optimization_selections
    }

    /// Domain-separated identity of the exact canonical selected set. This is
    /// retained independently so later cache, replay, and artifact boundaries
    /// never have to rediscover an optimization input from build syntax.
    pub const fn optimization_selection_identity(
        &self,
    ) -> omega_optimization_core::OptimizationSelectionIdentity {
        self.optimization_selection_identity
    }

    /// Auxiliary report projection requested by the authoritative root build.
    /// This remains independent of the exact transformation selection.
    pub const fn optimization_report_request(
        &self,
    ) -> omega_optimization_pipeline::OptimizationReportRequest {
        self.optimization_report
    }

    pub const fn selected_provider_plans(&self) -> &omega_effects::SelectedProviderPlanFacts {
        &self.selected_provider_plans
    }

    /// Exact `build.omg` grants resolved to retained selected provider plans.
    pub fn selected_provider_grants(
        &self,
    ) -> &[omega_trust_model::ResolvedAuthoredSelectedProviderGrant] {
        &self.selected_provider_grants
    }

    #[doc(hidden)]
    pub fn provider_plans(&self) -> &[omega_effects::provider_plan::ProviderPlan] {
        &self.provider_plans
    }

    /// Exact selected normalized-import bindings and their evaluated calling
    /// plans. These rows are derived before typed trees are consumed and remain
    /// the only source of normalized foreign-locator custody in native
    /// realization; other provider mechanisms keep their specialized lanes.
    pub fn external_binding_rows(&self) -> &[omega_calling_conventions::ExternalBindingRow] {
        &self.external_binding_rows
    }

    #[doc(hidden)]
    pub fn root_grants(&self) -> &[String] {
        &self.root_grants
    }

    #[doc(hidden)]
    pub const fn accepted_template_classifications(
        &self,
    ) -> &omega_trust_model::AcceptedTemplateClassifications {
        &self.accepted_template_classifications
    }

    #[doc(hidden)]
    pub fn selected_provider_provenance(
        &self,
    ) -> &[super::provider_plans::SelectedProviderReviewProvenance] {
        &self.selected_provider_provenance
    }

    pub const fn component_progress(&self) -> Option<&omega_effects::ComponentProgressManifest> {
        self.component_progress.as_ref()
    }

    pub const fn task_activations(&self) -> &omega_task_plans::TaskActivationPlanSet {
        &self.task_activations
    }

    /// Exact target-owned callback recipes joined to their checked nominal
    /// use sites. An execution engine must consume these plans rather than
    /// derive placement from the semantic tree.
    pub fn callback_placements(&self) -> &[omega_backend_plan::BoundNominalCallbackPlacement] {
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
    pub fn contract_entailment_stand_downs(
        &self,
    ) -> &[psi_validation::ContractEntailmentStandDown] {
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
    filesystem_sponsor: Option<psi_build_time_evaluation::BuildMachineFilesystemSponsor>,
    evaluation_sponsor: Option<psi_build_time_evaluation::BuildEvaluationSponsor>,
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
    filesystem_sponsor: psi_build_time_evaluation::BuildMachineFilesystemSponsor,
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
    filesystem_sponsor: psi_build_time_evaluation::BuildMachineFilesystemSponsor,
    evaluation_sponsor: psi_build_time_evaluation::BuildEvaluationSponsor,
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
    compile_to_checked_inner(
        &options.root_path,
        options.target_name.as_deref(),
        package_inputs,
        Some(&options.build_dir()),
        None,
        None,
    )
}

struct CheckedFrontend {
    typed: psi_typed_trees::TypedTrees,
    selected_target_machine_declarations:
        crate::pipeline::target_machines::SelectedTargetMachineDeclarations,
    build_source_id: Option<psi_source::SourceId>,
}

fn lower_checked_frontend(
    mut syntax: crate::pipeline::source_assembly::AssembledSyntax,
    target_name: Option<&str>,
    package_inputs: Option<&PackageCompilationInputs>,
    timings: &mut CompileTimings,
) -> Result<CheckedFrontend, Vec<Diagnostic>> {
    let evaluated = match package_inputs {
        Some(package_inputs) => {
            psi_build_time_evaluation::evaluate_pre_resolution_with_sources_and_authority(
                syntax.syntax_trees,
                syntax.sources.clone(),
                std::sync::Arc::new(package_inputs.clone()),
            )
        }
        None => psi_build_time_evaluation::evaluate_pre_resolution_with_sources(
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
    let mut typed = symbol_resolved_trees_to_typed_trees(resolved, timings)?;
    pre_check.evaluate(&mut typed)?;
    // Build evaluation consumes this coherent private typed stage before the
    // final checked-tree lowering. Bind trait-valued parameter-field calls now
    // so the evaluator receives the same exact requirement identity that the
    // checker will subsequently validate and retain.
    psi_validation::resolve_dynamic_call_targets(&mut typed)?;
    Ok(CheckedFrontend {
        typed,
        selected_target_machine_declarations,
        build_source_id,
    })
}

fn compile_to_checked_inner(
    root_path: &Path,
    target_name: Option<&str>,
    package_inputs: Option<&PackageCompilationInputs>,
    build_dir: Option<&Path>,
    filesystem_sponsor: Option<psi_build_time_evaluation::BuildMachineFilesystemSponsor>,
    evaluation_sponsor: Option<psi_build_time_evaluation::BuildEvaluationSponsor>,
) -> Result<CheckedCompilation, Vec<Diagnostic>> {
    compile_to_checked_inner_with_replay(
        root_path,
        target_name,
        package_inputs,
        build_dir,
        filesystem_sponsor,
        evaluation_sponsor,
        None,
    )
}

fn compile_to_checked_inner_with_replay(
    root_path: &Path,
    target_name: Option<&str>,
    package_inputs: Option<&PackageCompilationInputs>,
    build_dir: Option<&Path>,
    filesystem_sponsor: Option<psi_build_time_evaluation::BuildMachineFilesystemSponsor>,
    evaluation_sponsor: Option<psi_build_time_evaluation::BuildEvaluationSponsor>,
    replay_record: Option<&super::ReviewOnlyBuildFilesystemReplayRecord>,
) -> Result<CheckedCompilation, Vec<Diagnostic>> {
    let mut timings = CompileTimings::default();
    let selected_target_profile = target_name
        .map(|target_name| omega_target::TargetProfile::from_omega_target_name(Some(target_name)))
        .transpose()
        .map_err(|diagnostic| vec![diagnostic])?;
    // CLI aliases end at request admission. Every source, build, provider, and
    // artifact consumer below observes only the catalog's canonical spelling.
    let target_name = selected_target_profile.map(omega_target::TargetProfile::target_name);

    let (mut source_file_count, syntax) = source_files_to_syntax_trees_for_engine(
        root_path,
        target_name,
        package_inputs,
        &mut timings,
    )?;
    let mut generated_source_custody = syntax.generated_source_custody.clone();
    let frozen_syntax = syntax.clone();
    let mut frontend = lower_checked_frontend(syntax, target_name, package_inputs, &mut timings)?;
    if let Some(package_inputs) = package_inputs {
        crate::pipeline::package_declaration_admission::validate_authored_declaration_selections_before_build(
            &frontend.typed,
            package_inputs,
            &mut timings,
        )?;
    }
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
    let computed_build_config = crate::pipeline::build_config::compute_build_config(
        &frontend.typed,
        frontend.build_source_id,
        &build_machine_filesystem_scope,
        evaluation_sponsor.as_ref(),
        selected_target_profile,
    )?;
    let prepass_build_identity = computed_build_config
        .selected_build_machine_symbol
        .map(|symbol| -> Result<_, Vec<Diagnostic>> {
            let source_span = frontend
                .typed
                .symbols
                .symbol_source_span(symbol)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "selected build machine has no exact authored source occurrence",
                    )]
                })?;
            let name = frontend
                .typed
                .machines()
                .iter()
                .find(|machine| machine.symbol == symbol)
                .map(|machine| machine.name.as_str().to_owned())
                .ok_or_else(|| vec![Diagnostic::error("selected build machine disappeared")])?;
            Ok((source_span, name))
        })
        .transpose()?;
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
        let mut final_syntax = frozen_syntax;
        let retained = crate::pipeline::source_assembly::append_retained_generated_sources(
            &mut final_syntax,
            package_root,
            Some(package_inputs.root()),
            &computed_build_config.generated_sources,
        )?;
        source_file_count = source_file_count
            .checked_add(retained.len())
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "generated package source count exceeds the compiler range",
                )]
            })?;
        generated_source_custody.extend(retained);
        frontend = lower_checked_frontend(
            final_syntax,
            target_name,
            Some(package_inputs),
            &mut timings,
        )?;
        let Some((source_span, name)) = prepass_build_identity else {
            return Err(vec![Diagnostic::error(
                "generated-source handoff has no selected build machine to rebind",
            )]);
        };
        let matching = frontend
            .typed
            .machines()
            .iter()
            .filter(|machine| {
                machine.name.as_str() == name
                    && frontend.typed.symbols.symbol_source_span(machine.symbol)
                        == Some(source_span)
            })
            .map(|machine| machine.symbol)
            .collect::<Vec<_>>();
        let [selected] = matching.as_slice() else {
            return Err(vec![Diagnostic::error(
                "final compilation could not exactly rebind the build machine executed by the frozen prepass",
            )]);
        };
        Some(*selected)
    };
    let CheckedFrontend {
        mut typed,
        selected_target_machine_declarations,
        ..
    } = frontend;
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
        .map(omega_target::TargetProfile::native_target)
        .unwrap_or_else(omega_target::NativeTarget::host);
    let mut boundary_calling_plan_realizations =
        crate::pipeline::calling_policy_plans::compute_boundary_calling_plans(
            &mut typed,
            selected_native_target,
            &build_config.opaque_representation_selections,
            package_inputs,
        )?;
    let opaque_representation_selections = build_config.opaque_representation_selections.clone();
    let subsystem = build_config.subsystem;
    let optimization_selections = build_config.optimizations.clone();
    let optimization_selection_identity = optimization_selections.identity();
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
    let selected_program_entry = crate::pipeline::build_config::select_compiler_program_entry(
        &typed,
        &build_config,
        selected_target_profile,
        &boundary_calling_plan_realizations,
    )?;
    let target_provider_defaults =
        selected_target_machine_declarations.settle_provider_defaults(&typed)?;
    // PRV4 provider selection mirrors the native pipeline: candidates remain
    // separate by provider type and only the uniquely covering candidate may
    // rewrite adapter calls in the interpreter program.
    let derived_provider_plans =
        crate::pipeline::provider_plans::derive_satisfies_plans_with_provenance(
            &typed,
            target_name,
        );
    let provider_plans = derived_provider_plans
        .iter()
        .map(|derived| derived.plan.clone())
        .collect::<Vec<_>>();
    let selected_native_target =
        selected_target_profile.map(omega_target::TargetProfile::native_target);
    let provider_selection_target =
        selected_native_target.unwrap_or_else(omega_target::NativeTarget::host);
    let diagnostics =
        crate::pipeline::provider_plans::validate_provider_plan_candidates(&typed, &provider_plans);
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
    let selected_semantic_plans = selected_provider_plans
        .iter()
        .map(|selected| selected.derived.plan.clone())
        .collect::<Vec<_>>();
    crate::pipeline::provider_plans::validate_selected_synchronous_invocation_cycles(
        &typed,
        &selected_semantic_plans,
    )?;
    let external_binding_rows =
        omega_provider_planning::plans::extract_normalized_import_binding_rows(
            target_name,
            provider_selection_target,
            &selected_semantic_plans,
            &boundary_calling_plan_realizations,
            &typed,
        )?;
    let (selected_provider_plan_facts, selected_provider_provenance) =
        crate::pipeline::provider_plans::selected_provider_plan_facts_with_provenance(
            &typed,
            selected_provider_plans,
        )?;
    let root_grants = build_config
        .grants
        .iter()
        .map(|grant| grant.selector.clone())
        .collect::<Vec<_>>();
    if package_inputs.is_some() {
        omega_trust_model::reject_package_non_provider_grants(
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
            boundary_calling_plan_realizations: &mut boundary_calling_plan_realizations,
            provider_plans: &provider_plans,
            selected_provider_plan_facts,
            root_grants: &root_grants,
            authored_root_grants: &build_config.grants,
        },
    )?;
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
        },
    )?;

    // `typed_trees_to_checked_trees` wraps the program in an `Arc`; unwrap it for the
    // caller (this is the only owner at this point in the pipeline).
    let program = Arc::try_unwrap(selected_execution_settlement.program)
        .unwrap_or_else(|shared| (*shared).clone());
    let package_subject = package_inputs
        .map(|inputs| {
            omega_package_compilation::derive_package_compilation_subject(
                &program,
                inputs,
                &generated_source_custody,
            )
        })
        .transpose()?;
    let exact_toolchain_sources = package_inputs
        .is_some()
        .then(|| omega_package_compilation::toolchain_source_identities(&program))
        .transpose()?
        .unwrap_or_default();
    if package_subject.is_some() {
        omega_package_compilation::verify_current_files(&program, &generated_source_custody)?;
    }
    if let Some(package_inputs) = package_inputs {
        package_inputs.validate_canonical_source_metadata()?;
    }
    Ok(CheckedCompilation {
        program,
        source_file_count,
        subsystem,
        package_subject,
        exact_toolchain_sources,
        generated_source_custody,
        own_generated_sources,
        selected_target_profile,
        selected_native_target,
        selected_program_entry,
        selected_build_machine_symbol,
        selected_build_machine_identity,
        opaque_representation_selections,
        boundary_calling_plan_realizations,
        optimization_selections,
        optimization_selection_identity,
        optimization_report,
        selected_provider_plans: selected_execution_settlement.selected_provider_plan_facts,
        selected_provider_grants: selected_execution_settlement.selected_provider_grants,
        provider_plans,
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
