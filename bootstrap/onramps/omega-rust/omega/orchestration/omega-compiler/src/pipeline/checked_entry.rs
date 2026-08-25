use crate::pipeline::PackageCompilationInputs;
use crate::pipeline::stages::{
    source_files_to_syntax_trees_for_engine, symbol_resolved_trees_to_typed_trees,
    syntax_trees_to_symbol_resolved_trees, typed_trees_to_checked_trees,
};
use crate::pipeline::timing::CompileTimings;
use psi_checked_trees::CheckedTrees;
use psi_diagnostics::Diagnostic;
use std::path::Path;
use std::sync::Arc;

/// Psi-checked semantics paired with the Omega-owned provider realization
/// selected for one engine run. The semantic program deliberately does not
/// retain target/provider installation state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedCompilation {
    program: CheckedTrees,
    package_identity: Option<psi_core::PackageKeyIdentity>,
    source_consumption_commitment: Option<super::PackageSourceConsumptionCommitment>,
    selected_target_profile: Option<omega_target::TargetProfile>,
    selected_native_target: Option<omega_target::NativeTarget>,
    selected_program_entry_machine: Option<String>,
    selected_build_machine_symbol: Option<psi_symbols::SymbolHandle>,
    selected_provider_plans: omega_effects::SelectedProviderPlanFacts,
    selected_provider_provenance: Vec<super::provider_plans::SelectedProviderReviewProvenance>,
    component_progress: Option<omega_effects::ComponentProgressManifest>,
    task_activations: omega_task_plans::TaskActivationPlanSet,
    callback_placements: Vec<omega_backend_plan::BoundNominalCallbackPlacement>,
    build_evaluation_usage: Option<super::build_config::BuildEvaluationUsage>,
    build_observation_summary: Option<super::build_config::BuildObservationSummary>,
    contract_entailment_stand_downs: Vec<psi_validation::ContractEntailmentStandDown>,
}

impl CheckedCompilation {
    /// Reconciled root package identity for package-aware compilation.
    /// Standalone compilation has no package identity.
    pub const fn package_identity(&self) -> Option<psi_core::PackageKeyIdentity> {
        self.package_identity
    }

    /// Canonical commitment to the exact source bytes consumed by this
    /// package-aware frontend run. Standalone compilation has no package-
    /// custody commitment.
    pub const fn source_consumption_commitment(
        &self,
    ) -> Option<super::PackageSourceConsumptionCommitment> {
        self.source_consumption_commitment
    }

    /// Re-read every physical source path and require it to equal the bytes
    /// retained by the frontend. Resolver orchestration calls this around its
    /// own whole-snapshot verification; hostile same-user races still require
    /// an OS isolation boundary.
    pub fn verify_current_source_consumption(&self) -> Result<(), Vec<Diagnostic>> {
        super::package_source_consumption::verify_current_files(&self.program)
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
        self.selected_program_entry_machine.as_deref()
    }

    /// Exact symbol of the uniquely selected build machine. No build machine
    /// is represented by `None`; callers must not rediscover one by name.
    pub const fn selected_build_machine_symbol(&self) -> Option<psi_symbols::SymbolHandle> {
        self.selected_build_machine_symbol
    }

    pub const fn selected_provider_plans(&self) -> &omega_effects::SelectedProviderPlanFacts {
        &self.selected_provider_plans
    }

    pub(super) fn selected_provider_provenance(
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

impl std::ops::DerefMut for CheckedCompilation {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.program
    }
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
    // Checked-only callers traverse the same recursive parser as deployment
    // compilation and must reach its explicit depth guard before the host
    // thread's smaller default stack can overflow.
    let root_path = root_path.to_owned();
    let target_name = target_name.map(str::to_owned);
    super::compiler::run_on_compile_thread(move || {
        compile_to_checked_inner(&root_path, target_name.as_deref(), None, None, None)
    })
}

/// Checked-only compilation using a complete reconciled package graph. This
/// is the evidence-producing frontend seam for package admission and never
/// consults dependency rows in downloaded `build.omg` files.
pub fn compile_to_checked_with_packages(
    root_path: &Path,
    target_name: Option<&str>,
    package_inputs: PackageCompilationInputs,
) -> Result<CheckedCompilation, Vec<Diagnostic>> {
    let root_path = root_path.to_owned();
    let target_name = target_name.map(str::to_owned);
    super::compiler::run_on_compile_thread(move || {
        compile_to_checked_inner(
            &root_path,
            target_name.as_deref(),
            Some(&package_inputs),
            None,
            None,
        )
    })
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
    let root_path = root_path.to_owned();
    let build_dir = build_dir.to_owned();
    let target_name = target_name.map(str::to_owned);
    super::compiler::run_on_compile_thread(move || {
        compile_to_checked_inner(
            &root_path,
            target_name.as_deref(),
            Some(&package_inputs),
            Some(&build_dir),
            None,
        )
    })
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
    let root_path = root_path.to_owned();
    let build_dir = build_dir.to_owned();
    let target_name = target_name.map(str::to_owned);
    super::compiler::run_on_compile_thread(move || {
        compile_to_checked_inner(
            &root_path,
            target_name.as_deref(),
            Some(&package_inputs),
            Some(&build_dir),
            Some(filesystem_sponsor),
        )
    })
}

fn compile_to_checked_inner(
    root_path: &Path,
    target_name: Option<&str>,
    package_inputs: Option<&PackageCompilationInputs>,
    build_dir: Option<&Path>,
    filesystem_sponsor: Option<psi_build_time_evaluation::BuildMachineFilesystemSponsor>,
) -> Result<CheckedCompilation, Vec<Diagnostic>> {
    let mut timings = CompileTimings::default();
    let package_identity = package_inputs.map(PackageCompilationInputs::root);

    // The interpreter keeps the abstract `boundary trait Gui` for its headless
    // provider; only the native-image pipeline substitutes target providers.
    let (_source_file_count, mut syntax) = source_files_to_syntax_trees_for_engine(
        root_path,
        target_name,
        false,
        package_inputs,
        &mut timings,
    )?;
    let evaluated = psi_build_time_evaluation::evaluate_pre_resolution_with_sources(
        syntax.syntax_trees,
        syntax.sources.clone(),
    )?;
    syntax.syntax_trees = evaluated.syntax_trees;
    let placed_view_records = evaluated.placed_view_records;
    let plan_laid_records = evaluated.plan_laid_records;
    // TARGET-SCOPED MACHINES -- exactly as the full `compile` pipeline does:
    // the interpreter runs the SELECTED target's implementations.
    let target_default_machine_names = crate::pipeline::target_machines::filter_target_machines(
        &mut syntax.syntax_trees,
        target_name,
    )?;
    let build_file_machine_names: Vec<String> = syntax
        .files
        .iter()
        .filter(|file| file.path.file_name().and_then(|name| name.to_str()) == Some("build.omg"))
        .flat_map(|file| file.root_items.iter())
        .filter_map(|handle| match syntax.syntax_trees.root_item(*handle) {
            psi_syntax_trees::item::Item::Machine(machine) => {
                Some(machine.name.as_str().to_owned())
            }
            _ => None,
        })
        .collect();
    let resolved = syntax_trees_to_symbol_resolved_trees(syntax, &mut timings)?;
    let mut typed = symbol_resolved_trees_to_typed_trees(resolved, &mut timings)?;
    psi_build_time_evaluation::evaluate_pre_check(
        &mut typed,
        &plan_laid_records,
        &placed_view_records,
    )?;
    let boundary_calling_plan_realizations =
        crate::pipeline::calling_policy_plans::compute_boundary_calling_plans(&mut typed)?;
    let build_machine_filesystem_scope =
        crate::pipeline::build_config::BuildMachineFilesystemScope::for_root(
            root_path,
            build_dir.map(Path::to_path_buf).unwrap_or_else(|| {
                root_path
                    .parent()
                    .filter(|parent| !parent.as_os_str().is_empty())
                    .map(|parent| parent.join("build"))
                    .unwrap_or_else(|| std::path::PathBuf::from("build"))
            }),
            filesystem_sponsor,
        );
    let computed_build_config = crate::pipeline::build_config::compute_build_config(
        &typed,
        &build_file_machine_names,
        &build_machine_filesystem_scope,
    )?;
    crate::pipeline::build_config::reject_uncompiled_generated_sources(&computed_build_config)?;
    let build_evaluation_usage = computed_build_config.evaluation_usage;
    let build_observation_summary = computed_build_config.observation_summary;
    let selected_build_machine_symbol = computed_build_config.selected_build_machine_symbol;
    let build_config = computed_build_config.config;
    // A semantic-only checked compilation has no selected target and therefore
    // no storage root. Authored bindings remain available in the evaluated
    // build configuration, but only an exact target selection may activate one
    // for interpreter or production execution.
    let selected_program_entry_source_signature = match target_name {
        Some(target_name) => crate::pipeline::build_config::selected_program_entry_machine(
            &build_config,
            Some(target_name),
        )?
        .map(|entry| {
            crate::pipeline::build_config::validate_selected_program_entry_shape(&typed, entry)
        })
        .transpose()?,
        None => None,
    };
    let selected_program_entry_machine = selected_program_entry_source_signature
        .as_ref()
        .map(|source| source.machine_name().to_owned());
    let target_provider_defaults = crate::pipeline::build_config::compute_target_provider_defaults(
        &typed,
        &target_default_machine_names,
    )?;
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
    let selected_target_profile = target_name
        .map(|target_name| omega_target::TargetProfile::from_omega_target_name(Some(target_name)))
        .transpose()
        .map_err(|diagnostic| vec![diagnostic])?;
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
    let (selected_provider_plan_facts, selected_provider_provenance) =
        crate::pipeline::provider_plans::selected_provider_plan_facts_with_provenance(
            &typed,
            selected_provider_plans,
        )?;
    let contract_entailment_stand_downs =
        psi_validation::collect_contract_entailment_stand_downs(&typed);
    let mut checked = typed_trees_to_checked_trees(typed, &mut timings)?;
    if let Some(package_inputs) = package_inputs {
        crate::pipeline::package_declaration_admission::validate_authored_declaration_selections(
            &checked.program,
            package_inputs,
        )?;
    }
    let callback_placements =
        crate::pipeline::calling_policy_plans::validate_nominal_callback_placement_bindings(
            &checked.program,
            &boundary_calling_plan_realizations,
        )?;
    let checked_program = Arc::get_mut(&mut checked.program)
        .expect("checked program must be uniquely owned before engine handoff");
    let selected_provider_plan_facts =
        crate::pipeline::provider_plans::bind_selected_provider_plan_facts(
            checked_program,
            &provider_plans,
            selected_provider_plan_facts,
            &build_config.grants,
        )?;
    let component_progress = selected_program_entry_source_signature
        .as_ref()
        .map(|source| {
            crate::pipeline::component_progress::build_component_progress_manifest(
                checked_program,
                &selected_provider_plan_facts,
                source.machine_symbol(),
                source.normalized_callable_identity().to_owned(),
            )
        })
        .transpose()?;
    crate::pipeline::operator_adapter_dispatch::rewrite_selected_operator_adapter_calls(
        checked_program,
        &selected_provider_plan_facts,
    )?;
    crate::pipeline::float_intrinsic_dispatch::rewrite_selected_float_intrinsic_calls(
        checked_program,
        &selected_provider_plan_facts,
    )?;
    // Preserve boundary-requirement proof/evidence at checking time, then
    // redirect only execution to the selected checked adapter.
    crate::pipeline::adapter_dispatch::rewrite_adapter_calls(
        &mut checked_program.typed,
        &selected_provider_plan_facts,
    )?;
    let task_activations = crate::pipeline::task_plans::elaborate_task_activation_plans(
        checked_program,
        &selected_provider_plan_facts,
        provider_selection_target,
    )?;

    // `typed_trees_to_checked_trees` wraps the program in an `Arc`; unwrap it for the
    // caller (this is the only owner at this point in the pipeline).
    let program = Arc::try_unwrap(checked.program).unwrap_or_else(|shared| (*shared).clone());
    let source_consumption_commitment = package_inputs
        .map(|inputs| super::package_source_consumption::derive(&program, inputs))
        .transpose()?;
    if source_consumption_commitment.is_some() {
        super::package_source_consumption::verify_current_files(&program)?;
    }
    Ok(CheckedCompilation {
        program,
        package_identity,
        source_consumption_commitment,
        selected_target_profile,
        selected_native_target,
        selected_program_entry_machine,
        selected_build_machine_symbol,
        selected_provider_plans: selected_provider_plan_facts,
        selected_provider_provenance,
        component_progress,
        task_activations,
        callback_placements,
        build_evaluation_usage,
        build_observation_summary,
        contract_entailment_stand_downs,
    })
}
