use crate::pipeline::PackageCompilationInputs;
use crate::pipeline::artifacts::{
    remove_stale_phase_diagrams, write_backend_report, write_checked_snapshot,
    write_control_flow_snapshot, write_emission_plan, write_pipeline_index, write_pipeline_shell,
    write_program_storage_entry_snapshot, write_resolved_snapshot, write_state_graph_snapshot,
    write_syntax_snapshot, write_timings, write_typed_snapshot,
};
use crate::pipeline::boundary_report::{
    write_boundary_report, write_boundary_report_with_capabilities,
};
use crate::pipeline::compile_options::{ArtifactEmissionPolicy, CompileOptions};
use crate::pipeline::compile_policy::{
    ExecutableTcbBuildPolicy, ExecutableTcbInstallationAuthorization,
};
use crate::pipeline::compile_report::CompileReport;
use crate::pipeline::output::write_output;
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

/// Extract bodyless external leaves into the calling-convention rows consumed
/// by the freestanding ABI builder.
fn extract_external_binding_rows(
    selected_target: Option<&str>,
    native_target: omega_target::NativeTarget,
    selected_plans: &[omega_effects::provider_plan::ProviderPlan],
    boundary_calling_plan_realizations: &[
        crate::pipeline::calling_policy_plans::BoundaryCallingPlanRealization
    ],
    typed: &psi_typed_trees::TypedTrees,
) -> Result<Vec<omega_calling_conventions::ExternalBindingRow>, Vec<Diagnostic>> {
    use omega_calling_conventions::{CallingPolicy, ExternalBindingKind, ExternalBindingRow};
    use omega_effects::provider_plan::ProviderBinding;

    let mut rows = Vec::new();
    // The selected ProviderPlan set is the immutable normalization boundary.
    // Do not rescan source `via` clauses after selection: doing so would create
    // a second binding authority beside the retained typed identity.
    for plan in selected_plans {
        for row in &plan.rows {
            let binding = match &row.binding {
                ProviderBinding::Import { locator } => ExternalBindingKind::Import {
                    locator: locator.clone(),
                },
                ProviderBinding::StringBackedImportBootstrap { library, symbol } => {
                    ExternalBindingKind::StringBackedImportBootstrap {
                        module: library.clone(),
                        symbol: symbol.clone(),
                    }
                }
                ProviderBinding::Syscall { number } => {
                    ExternalBindingKind::Syscall { number: *number }
                }
                ProviderBinding::CompilerIntrinsic { machine } => {
                    ExternalBindingKind::CompilerIntrinsic {
                        machine: machine.clone(),
                    }
                }
                ProviderBinding::VtableSlot { index } => {
                    ExternalBindingKind::VtableSlot { index: *index }
                }
                ProviderBinding::VtableField { field, .. } => ExternalBindingKind::VtableField {
                    field: field.clone(),
                },
                ProviderBinding::TableFunction { field, .. } => {
                    ExternalBindingKind::TableFunction {
                        field: field.clone(),
                    }
                }
                ProviderBinding::CheckedAdapter { .. } => continue,
            };
            let boundary_entry_plan = selected_source_boundary_entry_plan(
                typed,
                boundary_calling_plan_realizations,
                plan,
                &plan.schema.trait_name,
                &row.method,
                &row.requirement_identity,
            )
            .map_err(|diagnostic| vec![diagnostic])?;
            // Boundary operators are consumed by checked operator dispatch,
            // where the exact selected ProviderPlan realization is retained
            // on the operator-use fact. They are not host ABI calls and must
            // not be reinterpreted as platform-call catalog entries merely
            // because their selected realization is compiler-owned.
            if matches!(&binding, ExternalBindingKind::CompilerIntrinsic { .. })
                && typed.operators().iter().any(|operator| {
                    operator.is_boundary
                        && psi_typed_trees::operator::boundary_operator_requirement_identity(
                            typed, operator,
                        ) == plan.schema.trait_name
                })
            {
                continue;
            }
            let compatibility_policy = match &binding {
                ExternalBindingKind::CompilerIntrinsic { .. } => None,
                ExternalBindingKind::Syscall { .. } => {
                    match (native_target.object_format, native_target.architecture) {
                        (omega_target::ObjectFormat::Elf, omega_target::Architecture::X86_64) => {
                            Some(CallingPolicy::LinuxSyscallX86_64)
                        }
                        (omega_target::ObjectFormat::Elf, omega_target::Architecture::Aarch64) => {
                            Some(CallingPolicy::LinuxSyscallAarch64)
                        }
                        _ => None,
                    }
                }
                _ => Some(CallingPolicy::native_for_target(native_target)),
            };
            let boundary_entry_plan = match (boundary_entry_plan, compatibility_policy) {
                (Some(plan), _) => Some(plan),
                (None, Some(policy)) => crate::pipeline::calling_policy_plans::evaluate_compatibility_boundary_entry_plan(
                    typed,
                    &plan.schema.trait_name,
                    &row.method,
                    &row.requirement_identity,
                    policy,
                    usize::from(matches!(&binding, ExternalBindingKind::TableFunction { .. })),
                )
                .map_err(|reason| {
                    vec![Diagnostic::error(format!(
                        "cannot evaluate compatibility calling plan for `{}::{}`: {reason}",
                        plan.schema.trait_name, row.method
                    ))]
                })?,
                (None, None) => None,
            };
            rows.push(ExternalBindingRow {
                target_name: if plan.target.is_empty() {
                    selected_target.unwrap_or("cross_platform_cli").to_owned()
                } else {
                    plan.target.clone()
                },
                trait_name: plan.schema.trait_name.clone(),
                method: row.method.clone(),
                requirement_identity: row.requirement_identity.clone(),
                table_type: plan.provider_type.clone(),
                boundary_entry_plan,
                binding,
            });
        }
    }
    Ok(rows)
}

/// Resolve implementation evidence only through the provider candidate that
/// selection admitted. The public provider/schema identity carries the
/// canonical fingerprint; the typed program retains the corresponding plan
/// internally so lowering never has to rediscover or re-run policy source.
fn selected_source_boundary_entry_plan(
    typed: &psi_typed_trees::TypedTrees,
    boundary_calling_plan_realizations: &[
        crate::pipeline::calling_policy_plans::BoundaryCallingPlanRealization
    ],
    plan: &omega_effects::provider_plan::ProviderPlan,
    trait_name: &str,
    method_name: &str,
    requirement_identity: &str,
) -> Result<Option<omega_calling_conventions::BoundaryEntryPlan>, Diagnostic> {
    if plan.name.is_empty() {
        return Err(Diagnostic::error(
            "selected source boundary entry plan has an empty ProviderPlan name",
        ));
    }
    let provider_plan_name = plan.name.as_str();
    if plan.schema.trait_name != trait_name {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry ProviderPlan `{}` serves schema `{}`, not exact requested schema `{trait_name}`",
            plan.name, plan.schema.trait_name
        )));
    }
    if requirement_identity.is_empty() {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry `{trait_name}::{method_name}` has an empty exact requirement overload identity"
        )));
    }

    let matching_methods = plan
        .schema
        .methods
        .iter()
        .filter(|method| {
            method.name == method_name && method.requirement_identity == requirement_identity
        })
        .collect::<Vec<_>>();
    let [method] = matching_methods.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry ProviderPlan `{provider_plan_name}` binds {} exact schema methods for `{trait_name}::{method_name}` / `{requirement_identity}`",
            matching_methods.len()
        )));
    };
    if method.requirement_owner.is_empty() {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry ProviderPlan `{provider_plan_name}` method `{method_name}` has an empty exact requirement owner"
        )));
    }

    let schema_operators = typed
        .operators()
        .iter()
        .filter(|operator| {
            operator.is_boundary
                && psi_typed_trees::operator::boundary_operator_requirement_identity(
                    typed, operator,
                ) == plan.schema.trait_name
        })
        .collect::<Vec<_>>();
    if !schema_operators.is_empty() {
        let [operator] = schema_operators.as_slice() else {
            return Err(Diagnostic::error(format!(
                "selected source boundary entry schema `{}` resolves to {} exact typed boundary operators",
                plan.schema.trait_name,
                schema_operators.len()
            )));
        };
        let operator_identity =
            psi_typed_trees::operator::boundary_operator_requirement_identity(typed, operator);
        if method.name != "realize"
            || method.requirement_owner != operator_identity
            || method.requirement_identity != operator_identity
        {
            return Err(Diagnostic::error(format!(
                "selected source boundary entry ProviderPlan `{provider_plan_name}` does not bind exact boundary operator `{operator_identity}`"
            )));
        }
        return match method.calling_plan_fingerprint {
            None => Ok(None),
            Some(_) => Err(Diagnostic::error(format!(
                "selected source boundary operator `{operator_identity}` retains a trait calling-plan fingerprint"
            ))),
        };
    }

    let schema_owners = typed
        .traits()
        .iter()
        .filter(|definition| {
            definition.is_boundary && definition.name.as_str() == plan.schema.trait_name
        })
        .collect::<Vec<_>>();
    let [schema_owner] = schema_owners.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry schema `{}` resolves to {} exact typed boundary traits",
            plan.schema.trait_name,
            schema_owners.len()
        )));
    };

    let requirement_owners = typed
        .traits()
        .iter()
        .filter(|definition| definition.name.as_str() == method.requirement_owner)
        .collect::<Vec<_>>();
    let [requirement_owner] = requirement_owners.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry requirement owner `{}` resolves to {} exact typed traits",
            method.requirement_owner,
            requirement_owners.len()
        )));
    };
    if !requirement_owner.is_boundary {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry requirement owner `{}` is not an exact boundary trait",
            method.requirement_owner
        )));
    }

    let matching_signatures = typed
        .trait_machine_signatures(requirement_owner)
        .iter()
        .filter(|signature| {
            signature.name.as_str() == method_name
                && typed
                    .normalized_trait_requirement_overload_identity(requirement_owner, signature)
                    .identity()
                    == requirement_identity
        })
        .collect::<Vec<_>>();
    let [signature] = matching_signatures.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry requirement `{}` binds {} exact typed signatures for `{method_name}` / `{requirement_identity}`",
            method.requirement_owner,
            matching_signatures.len()
        )));
    };

    let Some(fingerprint) = method.calling_plan_fingerprint else {
        return Ok(None);
    };
    if fingerprint == 0 {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry `{trait_name}::{method_name}` has a zero calling-plan fingerprint"
        )));
    }

    let matching_realizations = boundary_calling_plan_realizations
        .iter()
        .filter(|realization| {
            realization.fingerprint == fingerprint
                && realization.boundary_trait == schema_owner.symbol
                && realization.requirement_machine == signature.symbol
        })
        .collect::<Vec<_>>();
    let [realization] = matching_realizations.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry `{trait_name}::{method_name}` / `{requirement_identity}` resolves to {} exact calling-plan realizations for fingerprint 0x{fingerprint:016x}",
            matching_realizations.len()
        )));
    };

    Ok(Some(realization.boundary_entry_plan.clone()))
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
        write_boundary_report(
            &self.options,
            &syntax.syntax_trees,
            emit_auxiliary_artifacts,
        )?;
        let syntax_trees = syntax.syntax_trees.clone();

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
        let generic_accepted_template_fingerprints =
            crate::pipeline::trust_report::GenericAcceptedTemplateFingerprints::capture(&typed);
        crate::pipeline::wire_report::write_wire_protocol_report(
            &self.options,
            &typed,
            &build_config.wire_compatibility_demands,
            emit_auxiliary_artifacts,
        )?;

        // Capture the selected provider's validated source calling plans
        // before typed ownership moves into checked lowering. The rows carry
        // them beside their mechanisms into the host-ABI/backend path.
        let external_binding_rows = extract_external_binding_rows(
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
        let selected_provider_plan_facts =
            crate::pipeline::provider_plans::bind_selected_provider_plan_facts(
                Arc::get_mut(&mut checked.program)
                    .expect("checked program must be uniquely owned before backend fan-out"),
                &provider_plans,
                selected_provider_plan_facts,
                &build_config.grants,
            )?
            .with_opaque_executable_admissions(
                self.executable_tcb_policy
                    .opaque_executable_admissions
                    .iter()
                    .cloned(),
            )
            .map_err(|reason| {
                vec![Diagnostic::error(format!(
                    "executable TCB admission rejected: {reason}"
                ))]
            })?;
        let executable_tcb_installation_authorization =
            ExecutableTcbInstallationAuthorization::bind(
                &selected_provider_plan_facts,
                self.executable_tcb_policy.profile.as_ref(),
            )?;
        checked.selected_provider_plans = Arc::new(selected_provider_plan_facts);
        checked.component_progress =
            if let Some(source) = selected_program_entry_source_signature.as_ref() {
                Some(Arc::new(
                    crate::pipeline::component_progress::build_component_progress_manifest(
                        &checked.program,
                        &checked.selected_provider_plans,
                        source.machine_symbol(),
                        source.normalized_callable_identity().to_owned(),
                    )?,
                ))
            } else if let Some(entry_name) = entry_machine_name.as_deref() {
                let matches = checked
                    .program
                    .machines()
                    .iter()
                    .filter(|machine| machine.name.as_str() == entry_name)
                    .collect::<Vec<_>>();
                let [entry] = matches.as_slice() else {
                    return Err(vec![Diagnostic::error(format!(
                        "selected test entry `{entry_name}` resolves to {} checked machines",
                        matches.len()
                    ))]);
                };
                let identity = checked
                    .program
                    .normalized_machine_overload_identity(entry)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(format!(
                            "selected test entry `{entry_name}` has no normalized callable identity"
                        ))]
                    })?
                    .identity();
                Some(Arc::new(
                    crate::pipeline::component_progress::build_component_progress_manifest(
                        &checked.program,
                        &checked.selected_provider_plans,
                        entry.symbol,
                        identity,
                    )?,
                ))
            } else {
                None
            };
        crate::pipeline::trust_report::write_trust_report(
            &self.options,
            &checked.program,
            &build_config.grants,
            &provider_plans,
            &checked.selected_provider_plans,
            &generic_accepted_template_fingerprints,
            emit_auxiliary_artifacts,
        )?;
        crate::pipeline::operator_adapter_dispatch::rewrite_selected_operator_adapter_calls(
            Arc::get_mut(&mut checked.program)
                .expect("checked program must be uniquely owned before backend fan-out"),
            &checked.selected_provider_plans,
        )?;
        crate::pipeline::float_intrinsic_dispatch::rewrite_selected_float_intrinsic_calls(
            Arc::get_mut(&mut checked.program)
                .expect("checked program must be uniquely owned before backend fan-out"),
            &checked.selected_provider_plans,
        )?;
        // PRV4 adapter dispatch (both engines, after checking): semantic facts
        // stay attached to the admitted boundary requirement, while execution
        // alone is redirected to the uniquely selected checked adapter.
        crate::pipeline::adapter_dispatch::rewrite_adapter_calls(
            &mut Arc::get_mut(&mut checked.program)
                .expect("checked program must be uniquely owned before backend fan-out")
                .typed,
            &checked.selected_provider_plans,
        )?;
        let task_activations = crate::pipeline::task_plans::elaborate_task_activation_plans(
            Arc::get_mut(&mut checked.program)
                .expect("checked program must be uniquely owned before backend fan-out"),
            &checked.selected_provider_plans,
            selected_native_target,
        )?;
        checked.task_activations = Arc::new(task_activations);
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
        write_boundary_report_with_capabilities(
            &self.options,
            &syntax_trees,
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
            if emit_auxiliary_artifacts {
                write_pipeline_shell(&self.options)?;
            }
            return CompileReport::checked(
                self.options.root_path,
                source_file_count,
                false,
                super::CompileOutputKind::CheckOnly,
                None,
                None,
                None,
                None,
                build_evaluation_usage,
                build_observation_summary,
            )
            .map_err(|message| vec![Diagnostic::error(message)]);
        }

        if requires_native_backend {
            reject_undischarged_build_bound_progress(checked.component_progress.as_deref())?;
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
        let program_storage_entry = program_entry_realization
            .as_ref()
            .map(|realization| {
                let source_signature = selected_program_entry_source_signature
                    .as_ref()
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "selected program-storage entry lost its checked source signature before backend binding",
                        )]
                    })?;
                let plan = backend.plan.entry_boundary_plan.as_ref().ok_or_else(|| {
                    vec![Diagnostic::error(
                        "selected program-storage entry lost its retained calling plan before backend binding",
                    )]
                })?;
                crate::pipeline::program_storage_entry::bind_generated_program_storage_entry_plan(
                    &realization.storage_entry,
                    plan,
                    &backend.plan.runtime_storage,
                    &backend.plan.layouts,
                    backend.plan.entry_key,
                    source_signature,
                )
                .map_err(|diagnostic| vec![Diagnostic::error(diagnostic.to_string())])
            })
            .transpose()?;
        let bind_bridge = |binding: crate::pipeline::ProgramStorageEntryPlanBinding,
                           backend: &crate::pipeline::stages::BackendPlanningSurface,
                           selected_provider| {
            if binding.source_signature().is_none() {
                return Err(vec![Diagnostic::error(
                    "compiler-generated program-storage binding lost its checked source signature",
                )]);
            }
            if binding.physical_contract().is_none() {
                return Err(vec![Diagnostic::error(
                    "compiler-generated UEFI program-storage binding lost its distinct physical entry contract",
                )]);
            }
            crate::pipeline::program_storage_entry::bind_emitted_program_storage_entry_native_bridge(
                    binding,
                    selected_provider,
                    self.options
                        .target_name
                        .clone()
                        .unwrap_or_else(|| "host".to_owned()),
                    &backend.plan.object,
                    &backend.plan.encoded_machine,
                    backend.plan.entry_key,
                    backend
                        .plan
                        .encoded_machine
                        .semantics
                        .boundaries
                        .footprints
                        .boundary_contract_fingerprint,
                    backend.plan.entry_machine_name().to_owned(),
                    backend.plan.entry_state_name().to_owned(),
                )
                .map_err(|diagnostic| vec![Diagnostic::error(diagnostic.to_string())])
        };
        let preview_program_storage_entry_bridge = program_storage_entry
            .as_ref()
            .map(|binding| {
                bind_bridge(
                    binding.clone(),
                    &backend,
                    program_storage_entry_provider.clone(),
                )
            })
            .transpose()?;
        if let Some(template) = preview_program_storage_entry_bridge
            .as_ref()
            .and_then(|bridge| bridge.wrapper_body_template())
        {
            crate::pipeline::program_storage_wrapper_body::insert_and_validate_program_storage_entry_wrapper(
                template,
                &mut backend.plan,
            )
            .map_err(|diagnostic| vec![Diagnostic::error(diagnostic.to_string())])?;
        }
        let mut program_storage_entry_bridge = if let Some(binding) = program_storage_entry {
            Some(bind_bridge(
                binding,
                &backend,
                program_storage_entry_provider,
            )?)
        } else {
            None
        };
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
            if emit_auxiliary_artifacts {
                if let Some(bridge) = &program_storage_entry_bridge {
                    write_program_storage_entry_snapshot(&self.options, bridge)?;
                }
                write_emission_plan(&self.options, &backend.plan, artifact.emission_plan(), None)?;
                write_timings(&self.options, timings.as_slice())?;
                write_pipeline_shell(&self.options)?;
            }
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

        let (output_kind, executable_publication, app_bundle_publication) = if installs_output {
            let written_output = write_output(
                &self.options,
                &executable_tcb_installation_authorization,
                emitted,
                &backend.plan.encoded_machine.semantics.boundaries.footprints,
                emit_auxiliary_artifacts,
                |checked_image| {
                    let Some(bridge) = &mut program_storage_entry_bridge else {
                        return Ok(());
                    };
                    if bridge.wrapper_body_template().is_none() {
                        if bridge.is_receiver_bound_without_wrapper_template() {
                            return Ok(());
                        }
                        return Err(vec![Diagnostic::error(
                            "native program-storage publication lost its receiver-free wrapper template without an exact receiver-bound continuation",
                        )]);
                    }
                    let checked_image = checked_image.ok_or_else(|| {
                        vec![Diagnostic::error(
                            "program-storage entry target emitted no checked executable image",
                        )]
                    })?;
                    let evidence = crate::pipeline::program_storage_wrapper_evidence::bind_final_program_storage_entry_wrapper_evidence(
                        bridge,
                        &backend.plan,
                        checked_image,
                    )
                    .map_err(|diagnostic| vec![Diagnostic::error(diagnostic.to_string())])?;
                    bridge
                        .retain_emitted_wrapper_evidence(evidence)
                        .map_err(|diagnostic| vec![Diagnostic::error(diagnostic.to_string())])
                },
            )?;
            let (output_path, output_kind, executable_publication, app_bundle_publication) =
                written_output.into_report_parts();
            if emit_auxiliary_artifacts {
                if let Some(bridge) = &program_storage_entry_bridge {
                    write_program_storage_entry_snapshot(&self.options, bridge)?;
                }
                write_emission_plan(
                    &self.options,
                    &backend.plan,
                    &emission_plan,
                    Some(output_path.as_path()),
                )?;
                write_timings(&self.options, timings.as_slice())?;
            }
            (output_kind, executable_publication, app_bundle_publication)
        } else if emit_auxiliary_artifacts {
            write_emission_plan(&self.options, &backend.plan, &emission_plan, None)?;
            (super::CompileOutputKind::CheckOnly, None, None)
        } else {
            (super::CompileOutputKind::CheckOnly, None, None)
        };

        if emit_auxiliary_artifacts {
            write_pipeline_shell(&self.options)?;
        }

        CompileReport::checked(
            self.options.root_path,
            source_file_count,
            installs_output,
            output_kind,
            executable_publication,
            app_bundle_publication,
            program_storage_entry_bridge
                .as_ref()
                .map(|bridge| bridge.binding().clone()),
            program_storage_entry_bridge,
            build_evaluation_usage,
            build_observation_summary,
        )
        .map_err(|message| vec![Diagnostic::error(message)])
    }
}

/// TPR6 fail-closed admission seam. Checked lowering and the component
/// manifest now preserve exact provider-receiver demands, but a selected
/// provider plan is not itself an establishment receipt. Native/final
/// composition must stop here until the installation occurrence + admitted
/// receipt carrier can discharge each exact row.
fn reject_undischarged_build_bound_progress(
    manifest: Option<&omega_effects::ComponentProgressManifest>,
) -> Result<(), Vec<Diagnostic>> {
    let Some(manifest) = manifest else {
        return Ok(());
    };
    let demands = manifest.pending();
    if demands.is_empty() {
        return Ok(());
    }
    Err(demands
        .iter()
        .map(|demand| {
            Diagnostic::error(format!(
                "final composition cannot discharge build-bound progress demand `{}` requiring profile `{}` at checked call {}:{}; the exact installed provider occurrence and admitted establishment receipt must be bound before native lowering",
                demand.requirement_identity,
                demand.profile_identity,
                demand.statement_ordinal,
                demand.call_ordinal,
            ))
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::{extract_external_binding_rows, selected_source_boundary_entry_plan};
    use crate::pipeline::calling_policy_plans::BoundaryCallingPlanRealization;
    use omega_calling_conventions::{
        BoundaryEntryPlan, CallSignature, CallingPolicy, evaluate_ordinary_boundary_entry_plan,
    };
    use omega_effects::provider_plan::{
        ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod, ServiceSchema,
    };
    use psi_symbols::SymbolHandle;
    use psi_typed_trees::TypedTrees;
    use psi_typed_trees::name::Identifier;
    use psi_typed_trees::signature::StateSignature;
    use psi_typed_trees::trait_definition::TraitDefinition;

    const PLAN_NAME: &str = "selected::source::plan";
    const SCHEMA_NAME: &str = "platform::RootService";
    const REQUIREMENT_NAME: &str = "core::RootService";
    const METHOD_NAME: &str = "enter";

    struct Fixture {
        typed: TypedTrees,
        plans: Vec<ProviderPlan>,
        realizations: Vec<BoundaryCallingPlanRealization>,
        requirement_identity: String,
        expected: BoundaryEntryPlan,
    }

    fn symbol(index: u32) -> SymbolHandle {
        SymbolHandle::from_arena_index(index)
    }

    fn fixture(inherited_owner: bool) -> Fixture {
        fixture_with_inventory(inherited_owner, 1, 1, true)
    }

    fn fixture_with_inventory(
        inherited_owner: bool,
        schema_owner_count: u32,
        signature_count: u32,
        requirement_owner_is_boundary: bool,
    ) -> Fixture {
        let requirement_owner_name = if inherited_owner {
            REQUIREMENT_NAME
        } else {
            SCHEMA_NAME
        };
        let requirement_owner_symbol = symbol(10);
        let requirement_symbol = symbol(20);
        let mut typed = TypedTrees::default();

        let mut requirement_owner = TraitDefinition {
            symbol: requirement_owner_symbol,
            is_boundary: requirement_owner_is_boundary,
            name: Identifier::generated(requirement_owner_name),
            ..Default::default()
        };
        for _ in 0..signature_count {
            typed.push_trait_machine_signature(
                &mut requirement_owner,
                StateSignature {
                    symbol: requirement_symbol,
                    name: Identifier::generated(METHOD_NAME),
                    ..Default::default()
                },
            );
        }
        typed.push_trait_definition(requirement_owner);

        let schema_owner_symbol = if inherited_owner {
            symbol(30)
        } else {
            requirement_owner_symbol
        };
        if inherited_owner {
            for offset in 0..schema_owner_count {
                typed.push_trait_definition(TraitDefinition {
                    symbol: if offset == 0 {
                        schema_owner_symbol
                    } else {
                        symbol(30 + offset)
                    },
                    is_boundary: true,
                    name: Identifier::generated(SCHEMA_NAME),
                    ..Default::default()
                });
            }
        } else {
            for offset in 1..schema_owner_count {
                typed.push_trait_definition(TraitDefinition {
                    symbol: symbol(30 + offset),
                    is_boundary: true,
                    name: Identifier::generated(SCHEMA_NAME),
                    ..Default::default()
                });
            }
        }

        let requirement_identity = {
            let owner = typed
                .traits()
                .iter()
                .find(|owner| owner.symbol == requirement_owner_symbol)
                .expect("requirement owner");
            let signature = typed
                .trait_machine_signatures(owner)
                .first()
                .expect("requirement signature");
            typed
                .normalized_trait_requirement_overload_identity(owner, signature)
                .identity()
        };
        let validated = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::MicrosoftX64,
            &CallSignature::default(),
        )
        .expect("empty ordinary boundary plan");
        let fingerprint = validated.contract_fingerprint();
        let expected = validated.plan().clone();
        let method = ServiceMethod {
            name: METHOD_NAME.to_owned(),
            requirement_owner: requirement_owner_name.to_owned(),
            requirement_identity: requirement_identity.clone(),
            calling_plan_fingerprint: Some(fingerprint),
            ..Default::default()
        };
        let plan = ProviderPlan {
            name: PLAN_NAME.to_owned(),
            schema: ServiceSchema {
                trait_name: SCHEMA_NAME.to_owned(),
                trait_package_identity: None,
                methods: vec![method],
            },
            ..Default::default()
        };
        let realization = BoundaryCallingPlanRealization {
            boundary_trait: schema_owner_symbol,
            boundary_arguments: Vec::new(),
            requirement_machine: requirement_symbol,
            fingerprint,
            boundary_entry_plan: expected.clone(),
            callback_binders: Vec::new(),
            callback_demands: Vec::new(),
            callback_context_closed: false,
            policy_machine: String::new(),
            relationship_span: psi_source::SourceSpan::default(),
            native_parameters: Vec::new(),
            materialized_signature:
                crate::pipeline::calling_policy_plans::materialized_boundary_signature_from_abi(
                    &CallSignature::default(),
                )
                .unwrap(),
        };
        Fixture {
            typed,
            plans: vec![plan],
            realizations: vec![realization],
            requirement_identity,
            expected,
        }
    }

    fn resolve(fixture: &Fixture, schema_name: &str) -> Result<Option<BoundaryEntryPlan>, String> {
        let plan = fixture
            .plans
            .first()
            .ok_or_else(|| "missing resolved provider plan".to_owned())?;
        selected_source_boundary_entry_plan(
            &fixture.typed,
            &fixture.realizations,
            plan,
            schema_name,
            METHOD_NAME,
            &fixture.requirement_identity,
        )
        .map_err(|diagnostic| diagnostic.message)
    }

    #[test]
    fn external_abi_rows_derive_from_the_selected_provider_plan() {
        let mut fixture = fixture(false);
        fixture.plans[0].target = "retained-target".to_owned();
        fixture.plans[0].provider_type = "RetainedProvider".to_owned();
        fixture.plans[0].rows.push(ProviderPlanRow {
            method: METHOD_NAME.to_owned(),
            requirement_identity: fixture.requirement_identity.clone(),
            binding: ProviderBinding::StringBackedImportBootstrap {
                library: "retained-library".to_owned(),
                symbol: "retained-symbol".to_owned(),
            },
        });

        let rows = extract_external_binding_rows(
            None,
            omega_target::NativeTarget::host(),
            &fixture.plans,
            &fixture.realizations,
            &fixture.typed,
        )
        .expect("selected provider binding should produce one ABI row");
        let [row] = rows.as_slice() else {
            panic!("one selected external ABI row")
        };

        assert_eq!(row.target_name, "retained-target");
        assert_eq!(row.trait_name, SCHEMA_NAME);
        assert_eq!(row.method, METHOD_NAME);
        assert_eq!(row.requirement_identity, fixture.requirement_identity);
        assert_eq!(row.table_type, "RetainedProvider");
        assert_eq!(row.boundary_entry_plan, Some(fixture.expected));
        assert_eq!(
            row.binding,
            omega_calling_conventions::ExternalBindingKind::StringBackedImportBootstrap {
                module: "retained-library".to_owned(),
                symbol: "retained-symbol".to_owned(),
            }
        );
    }

    #[test]
    fn normalized_locator_survives_provider_selection_and_host_abi_bridge_atomically() {
        let mut fixture = fixture(false);
        fixture.plans[0].target = "windows_x64".to_owned();
        let locator = omega_effects::normalize_foreign_locator(
            omega_effects::ForeignLocatorCandidate::PeByOrdinal {
                library: b"opaque\xff.dll".to_vec(),
                ordinal: 17,
            },
            omega_target::TargetProfile::WindowsX64,
        )
        .expect("valid normalized PE-by-ordinal locator");
        fixture.plans[0].rows.push(ProviderPlanRow {
            method: METHOD_NAME.to_owned(),
            requirement_identity: fixture.requirement_identity.clone(),
            binding: ProviderBinding::Import {
                locator: locator.clone(),
            },
        });

        let rows = extract_external_binding_rows(
            Some("windows_x64"),
            omega_target::NativeTarget::windows_x64(),
            &fixture.plans,
            &fixture.realizations,
            &fixture.typed,
        )
        .expect("normalized locator should cross the compiler ABI bridge");
        assert!(matches!(
            rows.as_slice(),
            [omega_calling_conventions::ExternalBindingRow {
                binding: omega_calling_conventions::ExternalBindingKind::Import {
                    locator: retained,
                },
                ..
            }] if retained == &locator
        ));

        let mut host_abi = omega_calling_conventions::build_host_abi_plan(
            omega_target::NativeTarget::windows_x64(),
        );
        omega_calling_conventions::merge_external_binding_rows(&mut host_abi, &rows)
            .expect("normalized locator should enter the host ABI plan");
        assert!(host_abi.bindings.iter().any(|(_, binding)| matches!(
            &binding.mechanism,
            omega_calling_conventions::HostBindingMechanism::Import {
                locator: omega_calling_conventions::HostImportLocator::Normalized(retained),
            } if retained == &locator
        )));
    }

    #[test]
    fn selected_source_boundary_entry_plan_accepts_exact_direct_and_inherited_owners() {
        for inherited in [false, true] {
            let fixture = fixture(inherited);
            assert_eq!(
                resolve(&fixture, SCHEMA_NAME),
                Ok(Some(fixture.expected.clone()))
            );
        }
    }

    #[test]
    fn selected_source_boundary_entry_plan_accepts_exact_operator_custody_without_trait_abi() {
        let source = r#"
            data CheckedMath {}
            boundary operator CheckedMath::offset_zero(value: i32) -> i32;

            data CheckedMathProvider {}
            machine CheckedMathProvider::offset_zero_impl(input: i32) -> i32
            satisfies CheckedMath::offset_zero
            {
                transition { _ -> (input) }
            }
        "#;
        let tokens = psi_source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .expect("tokenize operator custody fixture");
        let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens)
            .expect("parse operator custody fixture");
        let resolved = psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
            .expect("resolve operator custody fixture");
        let typed =
            psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                .expect("type operator custody fixture");
        let mut plans = crate::pipeline::provider_plans::derive_satisfies_plans(&typed, None);
        let plan_index = plans
            .iter()
            .position(|plan| {
                plan.schema
                    .trait_name
                    .starts_with("operator::CheckedMath::offset_zero")
            })
            .expect("exact operator provider plan");
        let method = plans[plan_index].schema.methods[0].clone();

        assert_eq!(
            selected_source_boundary_entry_plan(
                &typed,
                &[],
                &plans[plan_index],
                &plans[plan_index].schema.trait_name,
                &method.name,
                &method.requirement_identity,
            )
            .expect("exact boundary operator custody"),
            None,
            "operator dispatch owns its selected realization; it is not a trait ABI call",
        );

        plans[plan_index].schema.methods[0].requirement_owner = "other::Owner".to_owned();
        let error = selected_source_boundary_entry_plan(
            &typed,
            &[],
            &plans[plan_index],
            &plans[plan_index].schema.trait_name,
            &method.name,
            &method.requirement_identity,
        )
        .expect_err("operator requirement-owner drift must reject");
        assert!(
            error
                .message
                .contains("does not bind exact boundary operator")
        );
    }

    #[test]
    fn selected_source_boundary_entry_plan_rejects_plan_and_schema_drift_exactly() {
        enum Drift {
            EmptyPlanName,
            SchemaName,
            EmptyRequirementIdentity,
            MissingMethod,
            DuplicateMethod,
            EmptyRequirementOwner,
        }
        let cases = [
            (Drift::EmptyPlanName, "empty ProviderPlan name"),
            (Drift::SchemaName, "not exact requested schema"),
            (
                Drift::EmptyRequirementIdentity,
                "empty exact requirement overload identity",
            ),
            (Drift::MissingMethod, "binds 0 exact schema methods"),
            (Drift::DuplicateMethod, "binds 2 exact schema methods"),
            (
                Drift::EmptyRequirementOwner,
                "empty exact requirement owner",
            ),
        ];

        for (drift, expected) in cases {
            let mut fixture = fixture(false);
            let mut schema_name = SCHEMA_NAME;
            match drift {
                Drift::EmptyPlanName => fixture.plans[0].name.clear(),
                Drift::SchemaName => schema_name = "other::RootService",
                Drift::EmptyRequirementIdentity => fixture.requirement_identity.clear(),
                Drift::MissingMethod => {
                    fixture.plans[0].schema.methods[0].requirement_identity = "other".to_owned()
                }
                Drift::DuplicateMethod => {
                    let duplicate = fixture.plans[0].schema.methods[0].clone();
                    fixture.plans[0].schema.methods.push(duplicate);
                }
                Drift::EmptyRequirementOwner => {
                    fixture.plans[0].schema.methods[0].requirement_owner.clear()
                }
            }
            let error = resolve(&fixture, schema_name)
                .expect_err("selected source authority drift must reject");
            assert!(
                error.contains(expected),
                "expected `{expected}` in `{error}`"
            );
        }
    }

    #[test]
    fn selected_source_boundary_entry_plan_rejects_typed_custody_drift_exactly() {
        let duplicate_schema = fixture_with_inventory(true, 2, 1, true);
        assert!(
            resolve(&duplicate_schema, SCHEMA_NAME)
                .expect_err("duplicate schema owner")
                .contains("resolves to 2 exact typed boundary traits")
        );

        let duplicate_signature = fixture_with_inventory(true, 1, 2, true);
        assert!(
            resolve(&duplicate_signature, SCHEMA_NAME)
                .expect_err("duplicate signature")
                .contains("binds 2 exact typed signatures")
        );

        let mut duplicate_requirement_owner = fixture(true);
        duplicate_requirement_owner
            .typed
            .push_trait_definition(TraitDefinition {
                symbol: symbol(50),
                is_boundary: true,
                name: Identifier::generated(REQUIREMENT_NAME),
                ..Default::default()
            });
        assert!(
            resolve(&duplicate_requirement_owner, SCHEMA_NAME)
                .expect_err("duplicate requirement owner")
                .contains("resolves to 2 exact typed traits")
        );

        let mut missing_owner = fixture(true);
        missing_owner.plans[0].schema.methods[0].requirement_owner = "missing::Owner".to_owned();
        assert!(
            resolve(&missing_owner, SCHEMA_NAME)
                .expect_err("missing requirement owner")
                .contains("resolves to 0 exact typed traits")
        );

        let mut missing_signature = fixture(true);
        missing_signature
            .typed
            .push_trait_definition(TraitDefinition {
                symbol: symbol(51),
                is_boundary: true,
                name: Identifier::generated("empty::Owner"),
                ..Default::default()
            });
        missing_signature.plans[0].schema.methods[0].requirement_owner = "empty::Owner".to_owned();
        assert!(
            resolve(&missing_signature, SCHEMA_NAME)
                .expect_err("missing typed signature")
                .contains("binds 0 exact typed signatures")
        );

        let non_boundary = fixture_with_inventory(true, 1, 1, false);
        assert!(
            resolve(&non_boundary, SCHEMA_NAME)
                .expect_err("non-boundary requirement owner")
                .contains("is not an exact boundary trait")
        );

        let mut missing_schema = fixture(true);
        missing_schema.plans[0].schema.trait_name = "missing::Schema".to_owned();
        assert!(
            resolve(&missing_schema, "missing::Schema")
                .expect_err("missing schema owner")
                .contains("resolves to 0 exact typed boundary traits")
        );

        let mut cross_owner = fixture(true);
        let mut other_owner = TraitDefinition {
            symbol: symbol(60),
            is_boundary: true,
            name: Identifier::generated("other::Owner"),
            ..Default::default()
        };
        cross_owner.typed.push_trait_machine_signature(
            &mut other_owner,
            StateSignature {
                symbol: symbol(61),
                name: Identifier::generated(METHOD_NAME),
                ..Default::default()
            },
        );
        cross_owner.typed.push_trait_definition(other_owner);
        let other_identity = {
            let owner = cross_owner
                .typed
                .traits()
                .iter()
                .find(|owner| owner.symbol == symbol(60))
                .expect("other owner");
            cross_owner
                .typed
                .normalized_trait_requirement_overload_identity(
                    owner,
                    &cross_owner.typed.trait_machine_signatures(owner)[0],
                )
                .identity()
        };
        cross_owner.plans[0].schema.methods[0].requirement_owner = "other::Owner".to_owned();
        cross_owner.plans[0].schema.methods[0].requirement_identity = other_identity.clone();
        cross_owner.requirement_identity = other_identity;
        assert!(
            resolve(&cross_owner, SCHEMA_NAME)
                .expect_err("cross-owner realization")
                .contains("resolves to 0 exact calling-plan realizations")
        );
    }

    #[test]
    fn selected_source_boundary_entry_plan_rejects_realization_drift_exactly() {
        enum Drift {
            Missing,
            Duplicate,
            Fingerprint,
            SchemaOwner,
            Requirement,
            ZeroFingerprint,
        }
        let cases = [
            (
                Drift::Missing,
                "resolves to 0 exact calling-plan realizations",
            ),
            (
                Drift::Duplicate,
                "resolves to 2 exact calling-plan realizations",
            ),
            (
                Drift::Fingerprint,
                "resolves to 0 exact calling-plan realizations",
            ),
            (
                Drift::SchemaOwner,
                "resolves to 0 exact calling-plan realizations",
            ),
            (
                Drift::Requirement,
                "resolves to 0 exact calling-plan realizations",
            ),
            (Drift::ZeroFingerprint, "zero calling-plan fingerprint"),
        ];
        for (drift, expected) in cases {
            let mut fixture = fixture(true);
            match drift {
                Drift::Missing => fixture.realizations.clear(),
                Drift::Duplicate => fixture.realizations.push(fixture.realizations[0].clone()),
                Drift::Fingerprint => fixture.realizations[0].fingerprint ^= 1,
                Drift::SchemaOwner => fixture.realizations[0].boundary_trait = symbol(90),
                Drift::Requirement => fixture.realizations[0].requirement_machine = symbol(91),
                Drift::ZeroFingerprint => {
                    fixture.plans[0].schema.methods[0].calling_plan_fingerprint = Some(0)
                }
            }
            let error = resolve(&fixture, SCHEMA_NAME).expect_err("realization drift must reject");
            assert!(
                error.contains(expected),
                "expected `{expected}` in `{error}`"
            );
        }
    }

    #[test]
    fn selected_source_boundary_entry_plan_allows_none_only_for_exact_absent_fingerprint() {
        let mut fixture = fixture(true);
        fixture.plans[0].schema.methods[0].calling_plan_fingerprint = None;
        fixture.realizations.clear();
        assert_eq!(resolve(&fixture, SCHEMA_NAME), Ok(None));

        fixture.plans[0].schema.methods[0].requirement_owner = "missing::Owner".to_owned();
        assert!(
            resolve(&fixture, SCHEMA_NAME)
                .expect_err("missing owner cannot enter compatibility fallback")
                .contains("resolves to 0 exact typed traits")
        );
    }
}
