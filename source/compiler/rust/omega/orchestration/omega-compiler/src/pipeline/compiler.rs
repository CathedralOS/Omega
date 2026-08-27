use crate::pipeline::PackageCompilationInputs;
use crate::pipeline::artifacts::{
    FinalPipelineObservation, remove_stale_phase_diagrams, write_backend_report,
    write_checked_snapshot, write_control_flow_snapshot, write_final_pipeline_observations,
    write_pipeline_index, write_resolved_snapshot, write_state_graph_snapshot,
    write_syntax_snapshot, write_typed_snapshot,
};
use crate::pipeline::boundary_report::BoundaryReportObservation;
use crate::pipeline::compile_options::{ArtifactEmissionPolicy, CompileOptions};
use crate::pipeline::compile_policy::{
    ExecutableTcbBuildPolicy, settle_compiler_executable_tcb_installation,
};
use crate::pipeline::compile_report::CompileReport;
use crate::pipeline::output::{LegacyCompilerOutputCustody, write_output};
use crate::pipeline::stages::{
    backend_plan_to_native_image_payload, checked_trees_to_state_graph,
    control_flow_to_backend_plan, source_files_to_syntax_trees, state_graph_to_control_flow,
    symbol_resolved_trees_to_typed_trees, syntax_trees_to_symbol_resolved_trees,
    typed_trees_to_checked_trees,
};
use crate::pipeline::timing::CompileTimings;
use omega_artifacts::build_backend_surface_report;
use omega_core::parallel::WorkerPool;
use psi_diagnostics::Diagnostic;
use std::sync::Arc;

const COMPILE_STACK_SIZE: usize = 256 * 1024 * 1024;

/// The semantic product requested from the production compiler pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestedCompileProduct {
    Check,
    TerminalArtifact,
    NativeArtifact,
    InstalledOutput,
}

impl RequestedCompileProduct {
    const fn from_legacy_write_output(write_output: bool) -> Self {
        if write_output {
            Self::InstalledOutput
        } else {
            Self::Check
        }
    }

    const fn installs_output(self) -> bool {
        matches!(self, Self::InstalledOutput)
    }
}

/// One typed production compiler invocation.
///
/// Test-only entry overrides and worker ceilings deliberately remain on the
/// separate harness functions below. This request owns the production policy
/// inputs that used to select distinct public orchestration paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileRequest {
    options: CompileOptions,
    requested_product: RequestedCompileProduct,
    executable_tcb_policy: ExecutableTcbBuildPolicy,
    artifact_policy: ArtifactEmissionPolicy,
    package_inputs: Option<PackageCompilationInputs>,
}

impl CompileRequest {
    pub fn new(options: CompileOptions) -> Self {
        let requested_product =
            RequestedCompileProduct::from_legacy_write_output(options.write_output);
        Self {
            options,
            requested_product,
            executable_tcb_policy: ExecutableTcbBuildPolicy::default(),
            artifact_policy: ArtifactEmissionPolicy::Full,
            package_inputs: None,
        }
    }

    pub fn with_executable_tcb_policy(
        mut self,
        executable_tcb_policy: ExecutableTcbBuildPolicy,
    ) -> Self {
        self.executable_tcb_policy = executable_tcb_policy;
        self
    }

    pub fn with_requested_product(mut self, requested_product: RequestedCompileProduct) -> Self {
        self.requested_product = requested_product;
        self
    }

    pub fn with_artifact_policy(mut self, artifact_policy: ArtifactEmissionPolicy) -> Self {
        self.artifact_policy = artifact_policy;
        self
    }

    pub fn with_package_inputs(mut self, package_inputs: PackageCompilationInputs) -> Self {
        self.package_inputs = Some(package_inputs);
        self
    }

    pub const fn options(&self) -> &CompileOptions {
        &self.options
    }

    pub const fn requested_product(&self) -> RequestedCompileProduct {
        self.requested_product
    }
}

/// Execute the single typed production compiler request.
pub fn compile(request: CompileRequest) -> Result<CompileReport, Vec<Diagnostic>> {
    run_on_compile_thread(move || Compiler::from_request(request).compile())
}

/// Explicitly test-only compiler controls. Entry overrides and worker ceilings
/// cannot enter [`CompileRequest`] or production compilation.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileHarnessRequest {
    options: CompileOptions,
    entry_machine_name: Option<String>,
    worker_count: Option<usize>,
    artifact_policy: ArtifactEmissionPolicy,
}

#[doc(hidden)]
impl CompileHarnessRequest {
    pub fn new(options: CompileOptions) -> Self {
        Self {
            options,
            entry_machine_name: None,
            worker_count: None,
            artifact_policy: ArtifactEmissionPolicy::Full,
        }
    }

    pub fn with_test_entry(mut self, entry_machine_name: impl Into<String>) -> Self {
        self.entry_machine_name = Some(entry_machine_name.into());
        self
    }

    pub fn with_worker_count(mut self, worker_count: usize) -> Self {
        self.worker_count = Some(worker_count.max(1));
        self
    }

    pub fn with_artifact_policy(mut self, artifact_policy: ArtifactEmissionPolicy) -> Self {
        self.artifact_policy = artifact_policy;
        self
    }
}

/// Test-harness seam for fixtures and outer schedulers. A corpus runner may
/// bound backend workers here to avoid multiplying its own parallel job count.
#[doc(hidden)]
pub fn compile_harness(request: CompileHarnessRequest) -> Result<CompileReport, Vec<Diagnostic>> {
    run_on_compile_thread(move || {
        let mut compiler = Compiler::with_executable_tcb_policy(
            request.options,
            ExecutableTcbBuildPolicy::default(),
        )
        .with_artifact_policy(request.artifact_policy);
        if let Some(entry_machine_name) = request.entry_machine_name {
            compiler = compiler.with_test_entry(entry_machine_name);
        }
        if let Some(worker_count) = request.worker_count {
            compiler = compiler.with_worker_count(worker_count);
        }
        compiler.compile()
    })
}

/// Run the whole pipeline on a thread with a large explicit stack. Recursive
/// parsing and representation walks must reach their explicit depth guards on
/// hosts whose default thread stacks are small. Pages commit lazily, and a
/// genuine compiler panic is resumed on the calling thread.
pub(super) fn run_on_compile_thread<T>(work: impl FnOnce() -> T + Send + 'static) -> T
where
    T: Send + 'static,
{
    std::thread::Builder::new()
        .name("omega-compile".to_owned())
        .stack_size(COMPILE_STACK_SIZE)
        .spawn(work)
        .expect("failed to spawn compiler thread")
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
}
pub struct Compiler {
    options: CompileOptions,
    requested_product: RequestedCompileProduct,
    executable_tcb_policy: ExecutableTcbBuildPolicy,
    test_entry_machine_name: Option<String>,
    worker_count: Option<usize>,
    artifact_policy: ArtifactEmissionPolicy,
    package_inputs: Option<PackageCompilationInputs>,
}

impl Compiler {
    fn from_request(request: CompileRequest) -> Self {
        Self {
            options: request.options,
            requested_product: request.requested_product,
            executable_tcb_policy: request.executable_tcb_policy,
            test_entry_machine_name: None,
            worker_count: None,
            artifact_policy: request.artifact_policy,
            package_inputs: request.package_inputs,
        }
    }

    pub fn with_executable_tcb_policy(
        options: CompileOptions,
        executable_tcb_policy: ExecutableTcbBuildPolicy,
    ) -> Self {
        let requested_product =
            RequestedCompileProduct::from_legacy_write_output(options.write_output);
        Self {
            options,
            requested_product,
            executable_tcb_policy,
            test_entry_machine_name: None,
            worker_count: None,
            artifact_policy: ArtifactEmissionPolicy::Full,
            package_inputs: None,
        }
    }

    fn with_test_entry(mut self, entry_machine_name: String) -> Self {
        self.test_entry_machine_name = Some(entry_machine_name);
        self
    }

    fn with_worker_count(mut self, worker_count: usize) -> Self {
        self.worker_count = Some(worker_count.max(1));
        self
    }

    fn with_artifact_policy(mut self, artifact_policy: ArtifactEmissionPolicy) -> Self {
        self.artifact_policy = artifact_policy;
        self
    }

    pub fn compile(self) -> Result<CompileReport, Vec<Diagnostic>> {
        match self.requested_product {
            RequestedCompileProduct::Check
            | RequestedCompileProduct::NativeArtifact
            | RequestedCompileProduct::InstalledOutput => {}
            RequestedCompileProduct::TerminalArtifact => {
                return Err(vec![Diagnostic::error(
                    "terminal-artifact requests are unavailable through the legacy checked-tree compiler route",
                )]);
            }
        }
        let installs_output = self.requested_product.installs_output();
        let retains_native_artifact =
            self.requested_product == RequestedCompileProduct::NativeArtifact;
        let requires_native_backend = installs_output || retains_native_artifact;
        let mut timings = CompileTimings::default();
        let emit_auxiliary_artifacts = self.artifact_policy.emits_auxiliary_artifacts();

        let (source_file_count, mut syntax) = source_files_to_syntax_trees(
            &self.options.root_path,
            self.options.target_name.as_deref(),
            self.package_inputs.as_ref(),
            &mut timings,
        )?;
        let evaluated = match self.package_inputs.as_ref() {
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
        syntax.syntax_trees = evaluated.syntax_trees;
        let placed_view_records = evaluated.placed_view_records;
        let plan_laid_records = evaluated.plan_laid_records;
        // TARGET-SCOPED MACHINES (fs portable-contract settle 2026-07-18):
        // the SELECTED target's `<target> machine` implementations become
        // ordinary machines; every other target's stay inert. Loud edges:
        // duplicate / missing implementations for the selected target.
        let target_default_machine_names =
            crate::pipeline::target_machines::filter_target_machines(
                &mut syntax.syntax_trees,
                self.options.target_name.as_deref(),
            )?;
        let build_source_id = syntax.build_source_id;
        if emit_auxiliary_artifacts {
            remove_stale_phase_diagrams(&self.options)?;
            write_pipeline_index(&self.options)?;
            write_syntax_snapshot(&self.options, &syntax)?;
        }
        let boundary_report = BoundaryReportObservation::capture(&syntax.syntax_trees);
        boundary_report.write_initial(&self.options, emit_auxiliary_artifacts)?;

        let resolved = syntax_trees_to_symbol_resolved_trees(syntax, &mut timings)?;
        if emit_auxiliary_artifacts {
            write_resolved_snapshot(&self.options, &resolved)?;
        }

        let mut typed = symbol_resolved_trees_to_typed_trees(resolved, &mut timings)?;
        match self.package_inputs.as_ref() {
            Some(package_inputs) => psi_build_time_evaluation::evaluate_pre_check_with_authority(
                &mut typed,
                &plan_laid_records,
                &placed_view_records,
                std::sync::Arc::new(package_inputs.clone()),
            ),
            None => psi_build_time_evaluation::evaluate_pre_check(
                &mut typed,
                &plan_laid_records,
                &placed_view_records,
            ),
        }?;
        let mut boundary_calling_plan_realizations =
            crate::pipeline::calling_policy_plans::compute_boundary_calling_plans(
                &mut typed,
                self.package_inputs.as_ref(),
            )?;
        // PDI3 selected operation/algebra authority is public type identity,
        // including for generic trust receipts emitted before checked
        // lowering. Bind it on the typed tree before snapshots and lockfile
        // fingerprints consume the declaration graph.
        psi_typed_trees_to_checked_trees::normalize_open_index_identities(&mut typed)?;
        if let Some(package_inputs) = self.package_inputs.as_ref() {
            crate::pipeline::package_declaration_admission::validate_authored_declaration_selections_before_build(
                &typed,
                package_inputs,
                &mut timings,
            )?;
        }
        // BUILD CONFIG (build_and_package_model.md): image facts from
        // build.omg's augmenting `build(b: &mut Build)` machine, evaluated at
        // build time. When present it is AUTHORITATIVE; the legacy in-source
        // `target { subsystem }` word is the fallback until its removal.
        let build_machine_filesystem_scope =
            crate::pipeline::build_config::BuildMachineFilesystemScope::for_root(
                &self.options.root_path,
                self.options.build_dir(),
                None,
            );
        let computed_build_config = crate::pipeline::build_config::compute_build_config(
            &typed,
            build_source_id,
            &build_machine_filesystem_scope,
        )?;
        crate::pipeline::build_config::reject_uncompiled_generated_sources(&computed_build_config)?;
        let build_evaluation_usage = computed_build_config.evaluation_usage;
        let build_observation_summary = computed_build_config.observation_summary;
        // Selected native publication is still fail-closed before a report can
        // be emitted. Retain the independently evaluated request at this seam.
        let _optimization_report_request = computed_build_config.optimization_report_request;
        let build_config = computed_build_config.config;
        let selected_program_entry = crate::pipeline::build_config::selected_program_entry_machine(
            &build_config,
            self.options.target_name.as_deref(),
        )?;
        let selected_program_entry_source_signature =
            if let Some(selected_program_entry) = selected_program_entry {
                Some(
                    crate::pipeline::build_config::validate_selected_program_entry_shape(
                        &typed,
                        selected_program_entry,
                    )?,
                )
            } else {
                None
            };
        let program_entry_realization = if let Some(selected_program_entry) = selected_program_entry
        {
            crate::pipeline::build_config::validate_selected_program_entry_calling_plan(
                &typed,
                selected_program_entry,
                &boundary_calling_plan_realizations,
            )?
        } else {
            None
        };
        let program_entry_boundary_plan = program_entry_realization
            .as_ref()
            .map(|realization| realization.semantic_boundary_entry_plan.clone());
        let entry_machine_name = selected_program_entry
            .map(|selected| selected.machine_name.to_owned())
            .or(self.test_entry_machine_name.clone());
        let target_provider_defaults =
            crate::pipeline::build_config::compute_target_provider_defaults(
                &typed,
                &target_default_machine_names,
            )?;
        let build_machine_present = typed.machines().iter().any(|machine| {
            crate::pipeline::build_config::is_build_machine(&typed, machine, build_source_id)
        });
        // ASM DISCHARGE v0 (privileged_effects_and_binary_trust): asm
        // intrinsics (`hlt`, port I/O) are permitted only in a FREESTANDING
        // boundary root. The gate lives here because it consumes a
        // BuildConfig fact the typed->checked validations never see.
        psi_typed_trees_to_checked_trees::validate_asm_discharge(
            &typed,
            build_config.freestanding,
        )?;
        if emit_auxiliary_artifacts {
            write_typed_snapshot(&self.options, &typed)?;
        }
        let provider_plans = crate::pipeline::provider_plans::derive_satisfies_plans(
            &typed,
            self.options.target_name.as_deref(),
        );
        let selected_native_target =
            omega_target::NativeTarget::from_omega_target_name(self.options.target_name.as_deref())
                .unwrap_or_else(|_| omega_target::NativeTarget::host());
        let adapter_diagnostics =
            crate::pipeline::provider_plans::validate_provider_plan_candidates(
                &typed,
                &provider_plans,
            );
        if !adapter_diagnostics.is_empty() {
            return Err(adapter_diagnostics);
        }
        let selected_provider_plans = crate::pipeline::provider_plans::select_provider_plans(
            &provider_plans,
            selected_native_target,
            &target_provider_defaults,
            &build_config.provider_selections,
        )?;
        crate::pipeline::provider_plans::validate_selected_synchronous_invocation_cycles(
            &typed,
            &selected_provider_plans,
        )?;
        let selected_provider_plan_facts =
            omega_effects::SelectedProviderPlanFacts::from_selected_plans(
                selected_provider_plans.clone(),
            )
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let prepared_trust_lock = crate::pipeline::trust_lockfile::prepare_trust_lockfile(
            &self.options,
            &typed,
            &build_config.grants,
            &provider_plans,
            &selected_provider_plan_facts,
            self.package_inputs.is_some(),
        )?;
        crate::pipeline::wire_report::write_wire_protocol_report(
            &self.options,
            &typed,
            &build_config.wire_compatibility_demands,
            emit_auxiliary_artifacts,
        )?;

        // Capture the selected provider's validated source calling plans
        // before typed ownership moves into checked lowering. The rows carry
        // them beside their mechanisms into the host-ABI/backend path.
        let external_binding_rows = super::provider_plans::extract_external_binding_rows(
            self.options.target_name.as_deref(),
            selected_native_target,
            &selected_provider_plans,
            &boundary_calling_plan_realizations,
            &typed,
        )?;

        let mut checked = typed_trees_to_checked_trees(typed, &mut timings)?;
        if let Some(package_inputs) = self.package_inputs.as_ref() {
            crate::pipeline::package_declaration_admission::validate_authored_declaration_selections(
                &checked.program,
                package_inputs,
            )?;
        }
        crate::pipeline::calling_policy_plans::close_outbound_callback_materializations(
            Arc::get_mut(&mut checked.program)
                .expect("checked program must be uniquely owned before callback closure"),
            &mut boundary_calling_plan_realizations,
            selected_native_target,
            self.package_inputs.as_ref(),
        )?;
        checked.callback_placements = Arc::from(
            crate::pipeline::calling_policy_plans::validate_nominal_callback_placement_bindings(
                &checked.program,
                &boundary_calling_plan_realizations,
            )?,
        );
        crate::pipeline::trust_lockfile::enforce_trust_lockfile(
            prepared_trust_lock,
            checked.program.as_ref(),
        )?;
        let executable_tcb_installation_authorization =
            settle_compiler_executable_tcb_installation(
                &mut checked,
                &provider_plans,
                selected_provider_plan_facts,
                &build_config.grants,
                &self.executable_tcb_policy,
            )?;
        checked.component_progress =
            crate::pipeline::component_progress::build_selected_component_progress_manifest(
                &checked.program,
                &checked.selected_provider_plans,
                selected_program_entry_source_signature
                    .as_ref()
                    .map(|source| {
                        crate::pipeline::component_progress::ExactComponentProgressRoot::new(
                            source.machine_symbol(),
                            source.normalized_callable_identity(),
                        )
                    }),
                entry_machine_name.as_deref(),
            )?
            .map(Arc::new);
        crate::pipeline::trust_report::write_trust_report(
            &self.options,
            &checked.program,
            &build_config.grants,
            &provider_plans,
            &checked.selected_provider_plans,
            &checked.accepted_template_classifications,
            emit_auxiliary_artifacts,
        )?;
        crate::pipeline::operator_adapter_dispatch::settle_selected_operator_adapter_dispatch(
            &mut checked.program,
            &checked.selected_provider_plans,
        )?;
        crate::pipeline::float_intrinsic_dispatch::settle_selected_float_intrinsic_dispatch(
            &mut checked.program,
            &checked.selected_provider_plans,
        )?;
        // PRV4 adapter dispatch (both engines, after checking): semantic facts
        // stay attached to the admitted boundary requirement, while execution
        // alone is redirected to the uniquely selected checked adapter.
        crate::pipeline::adapter_dispatch::settle_selected_boundary_adapter_dispatch(
            &mut checked.program,
            &checked.selected_provider_plans,
        )?;
        crate::pipeline::task_plans::settle_compiler_task_activation_plans(
            &mut checked,
            selected_native_target,
        )?;
        if emit_auxiliary_artifacts {
            write_checked_snapshot(
                &self.options,
                &checked.program,
                entry_machine_name.as_deref(),
                &checked.selected_provider_plans,
                &checked.task_activations,
                checked.component_progress.as_deref(),
            )?;
        }
        boundary_report.settle_with_capabilities(
            &self.options,
            &checked.program,
            emit_auxiliary_artifacts,
        )?;
        let backend_surface = (emit_auxiliary_artifacts && requires_native_backend)
            .then(|| build_backend_surface_report(&checked.program, entry_machine_name.as_deref()));

        // A check-only compilation with no selected runtime root ends at
        // checked semantics. Requiring an entry merely to produce the
        // frontend artifacts would turn `--check` into implicit execution
        // policy; callers that need native validation either select an exact
        // `ProgramEntry` or use the explicit legacy test-entry seam.
        if self.requested_product == RequestedCompileProduct::Check
            && (entry_machine_name.is_none() || !build_config.optimizations.is_empty())
        {
            write_final_pipeline_observations(
                &self.options,
                self.artifact_policy,
                FinalPipelineObservation::CheckedOnly,
            )?;
            return LegacyCompilerOutputCustody::unpublished()
                .into_compile_report(
                    self.options.root_path,
                    source_file_count,
                    None,
                    build_evaluation_usage,
                    build_observation_summary,
                )
                .map_err(|message| vec![Diagnostic::error(message)]);
        }

        if requires_native_backend {
            super::component_progress::reject_undischarged_build_bound_progress(
                checked.component_progress.as_deref(),
            )?;
        }

        crate::pipeline::optimization_gate::require_available_pipeline(
            &build_config.optimizations,
        )?;

        // Frontend-only compilation never submits work to the backend pool.
        // Construct it only after the checked-only exit so large validation
        // corpora do not spawn and join a host-sized thread set per source.
        let workers = self
            .worker_count
            .map_or_else(WorkerPool::with_available_parallelism, WorkerPool::new);
        let state_graph = checked_trees_to_state_graph(&checked, workers.handle(), &mut timings)?;
        if emit_auxiliary_artifacts {
            write_state_graph_snapshot(&self.options, &state_graph)?;
        }
        let control_flow = state_graph_to_control_flow(state_graph, &mut timings)?;
        if emit_auxiliary_artifacts {
            write_control_flow_snapshot(&self.options, &control_flow)?;
        }

        // Build image subsystem and freestanding trust independently. PE
        // consumes the subsystem metadata; other formats ignore it. The
        // freestanding flag selects an empty ambient host ABI baseline. Both
        // facts come from build.omg (build_and_package_model.md); the old
        // in-source `target { subsystem }` word is retired.
        let _ = build_machine_present;
        let (subsystem, freestanding) = (build_config.subsystem, build_config.freestanding);
        let program_storage_entry_provider = program_entry_realization
            .as_ref()
            .map(|realization| {
                crate::pipeline::provider_plans::optional_selected_external_root_provider_plan(
                    &checked.selected_provider_plans,
                    &realization.storage_entry.schema().trait_name,
                )
                .map_err(|diagnostic| vec![Diagnostic::error(diagnostic.to_string())])
            })
            .transpose()?
            .flatten();
        // Selected external leaves become the target's source-authored
        // platform surface.
        let mut backend = control_flow_to_backend_plan(
            checked,
            entry_machine_name.as_deref(),
            program_entry_boundary_plan,
            self.options.target_name.as_deref(),
            freestanding,
            &external_binding_rows,
            control_flow,
            workers.handle(),
            &mut timings,
        )?;
        let program_storage_entry = crate::pipeline::program_storage_entry::bind_compiler_generated_program_storage_entry_plan(
            program_entry_realization
                .as_ref()
                .map(|realization| &realization.storage_entry),
            selected_program_entry_source_signature.as_ref(),
            &backend.plan,
        )?;
        let mut program_storage_entry_bridge = crate::pipeline::program_storage_entry::bind_compiler_generated_program_storage_entry_native_bridge(
            program_storage_entry,
            program_storage_entry_provider,
            self.options.target_name.as_deref(),
            &mut backend.plan,
        )?;
        if requires_native_backend && emit_auxiliary_artifacts {
            write_backend_report(
                &self.options,
                backend_surface
                    .as_ref()
                    .expect("full output compilation must build its backend report surface"),
                &backend.plan,
            )?;
        }

        let (emission_plan, emitted) =
            backend_plan_to_native_image_payload(&backend, subsystem, &mut timings)?;

        if retains_native_artifact {
            let artifact = super::RetainedNativeArtifact::checked(emission_plan, emitted)
                .map_err(|message| vec![Diagnostic::error(message)])?;
            write_final_pipeline_observations(
                &self.options,
                self.artifact_policy,
                FinalPipelineObservation::RetainedNative {
                    backend: &backend.plan,
                    emission: artifact.emission_plan(),
                    storage_bridge: program_storage_entry_bridge.as_ref(),
                    timings: timings.as_slice(),
                },
            )?;
            return CompileReport::from_retained_native_artifact(
                self.options.root_path,
                source_file_count,
                artifact,
                program_storage_entry_bridge
                    .as_ref()
                    .map(|bridge| bridge.binding().clone()),
                program_storage_entry_bridge,
                build_evaluation_usage,
                build_observation_summary,
            )
            .map_err(|message| vec![Diagnostic::error(message)]);
        }

        let output = if installs_output {
            let written_output = write_output(
                &self.options,
                &executable_tcb_installation_authorization,
                emitted,
                &backend.plan.encoded_machine.semantics.boundaries.footprints,
                emit_auxiliary_artifacts,
                |checked_image| {
                    crate::pipeline::program_storage_entry::retain_compiler_generated_program_storage_entry_publication_evidence(
                        program_storage_entry_bridge.as_mut(),
                        &backend.plan,
                        checked_image,
                    )
                },
            )?;
            let output = LegacyCompilerOutputCustody::written(written_output);
            output
        } else {
            LegacyCompilerOutputCustody::unpublished()
        };

        let final_observation = if installs_output {
            FinalPipelineObservation::InstalledOutput {
                backend: &backend.plan,
                emission: &emission_plan,
                storage_bridge: program_storage_entry_bridge.as_ref(),
                output_path: output
                    .output_path()
                    .expect("written compiler output retains its exact destination"),
                timings: timings.as_slice(),
            }
        } else {
            FinalPipelineObservation::UnpublishedNative {
                backend: &backend.plan,
                emission: &emission_plan,
            }
        };
        write_final_pipeline_observations(&self.options, self.artifact_policy, final_observation)?;

        output
            .into_compile_report(
                self.options.root_path,
                source_file_count,
                program_storage_entry_bridge,
                build_evaluation_usage,
                build_observation_summary,
            )
            .map_err(|message| vec![Diagnostic::error(message)])
    }
}
