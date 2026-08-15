use crate::pipeline::artifacts::{
    remove_stale_phase_diagrams, write_backend_report, write_checked_snapshot,
    write_control_flow_snapshot, write_emission_plan, write_pipeline_index, write_pipeline_shell,
    write_program_storage_entry_snapshot, write_resolved_snapshot, write_state_graph_snapshot,
    write_syntax_snapshot, write_timings, write_typed_snapshot,
};
use crate::pipeline::boundary_report::{
    write_boundary_report, write_boundary_report_with_capabilities,
};
use crate::pipeline::compile_options::CompileOptions;
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

pub fn compile(options: CompileOptions) -> Result<CompileReport, Vec<Diagnostic>> {
    compile_with_policy(options, ExecutableTcbBuildPolicy::default())
}

/// Legacy native-test seam while semantic fixtures migrate to target-owned
/// `ProgramEntry` roots. Production callers must use [`compile`].
#[doc(hidden)]
pub fn compile_with_test_entry(
    options: CompileOptions,
    entry_machine_name: impl Into<String>,
) -> Result<CompileReport, Vec<Diagnostic>> {
    let entry_machine_name = entry_machine_name.into();
    run_on_compile_thread(move || {
        Compiler::with_executable_tcb_policy(options, ExecutableTcbBuildPolicy::default())
            .with_test_entry(entry_machine_name)
            .compile()
    })
}

/// Compile with deployment-owned executable-TCB admissions and profile policy.
///
/// The policy is evaluated only after exact provider selection and before any
/// emitted artifact is installed in the build directory.
pub fn compile_with_policy(
    options: CompileOptions,
    executable_tcb_policy: ExecutableTcbBuildPolicy,
) -> Result<CompileReport, Vec<Diagnostic>> {
    // Run the whole pipeline on a thread with a large explicit stack. The
    // recursive-descent parser and the recursive tree/layout walks descend once
    // per nesting level with heavy per-level frames (a full operator-precedence
    // chain), so on the host's default stack -- as small as ~1 MiB on Windows --
    // even modestly nested input overflows the stack before the parser's depth
    // guard (`MAX_NESTING_DEPTH`) can reject it. A large stack guarantees the
    // guard is what fires, turning pathological nesting into a clean diagnostic
    // instead of a crash. The size is only reserved address space; pages commit
    // lazily, so ordinary inputs pay nothing. A genuine panic (a compiler bug)
    // is re-raised on the calling thread, preserving today's crash-on-bug
    // behavior.
    run_on_compile_thread(move || {
        Compiler::with_executable_tcb_policy(options, executable_tcb_policy).compile()
    })
}

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

/// Builds the boundary provider registry from `provider` declarations, enforces
/// the package whitelist, and rejects boundary operator bindings that do not
/// resolve to a registered provider (frozen Wave 0 decision #4).
fn validate_boundary_providers(
    syntax: &psi_syntax_trees::SyntaxTrees,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let registry = omega_effects::build_provider_registry(syntax, &mut diagnostics);
    omega_effects::validate_provider_bindings(syntax, &registry, &mut diagnostics);

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// Extract bodyless external leaves into the calling-convention rows consumed
/// by the freestanding ABI builder.
fn extract_external_binding_rows(
    syntax_trees: &psi_syntax_trees::SyntaxTrees,
    selected_target: Option<&str>,
    native_target: omega_target::NativeTarget,
    selected_plan_names: &[String],
    provider_plans: &[omega_effects::provider_plan::ProviderPlan],
    boundary_calling_plan_realizations: &[
        crate::pipeline::calling_policy_plans::BoundaryCallingPlanRealization
    ],
    typed: &psi_typed_trees::TypedTrees,
) -> Result<Vec<omega_calling_conventions::ExternalBindingRow>, Vec<Diagnostic>> {
    use omega_calling_conventions::{CallingPolicy, ExternalBindingKind, ExternalBindingRow};

    let mut rows = Vec::new();
    // A bodyless
    // `satisfies Trait::method via <Binding>;` machine contributes one row
    // for the satisfied requirement; a `<target>`-scoped leaf rides its own
    // marker, an unscoped leaf rides the portable name (resolves to the
    // host target). For table-addressed mechanisms, the leaf's attached data
    // type owns the layout used for table-addressed mechanisms.
    for item in syntax_trees.root_items() {
        let psi_syntax_trees::item::Item::Machine(machine) = item else {
            continue;
        };
        if !machine.bodyless || machine.boundary {
            continue;
        }
        for clause in syntax_trees.items.satisfies_clauses(machine.satisfies) {
            let (Some(binding), Some(requirement)) =
                (clause.via.as_ref(), clause.requirement.as_ref())
            else {
                continue;
            };
            let plan_target = machine.target.as_ref().map_or_else(
                || selected_target.unwrap_or_default().to_owned(),
                |target| target.as_str().to_owned(),
            );
            let provider_type = machine
                .attached_data
                .as_ref()
                .map(|name| name.as_str())
                .unwrap_or_default();
            let plan_name = crate::pipeline::provider_plans::satisfies_plan_name(
                &plan_target,
                clause.trait_name.as_str(),
                provider_type,
            );
            if !selected_plan_names
                .iter()
                .any(|selected| selected == &plan_name)
            {
                continue;
            }
            use psi_syntax_trees::item::ExternalBinding;
            let binding = match binding {
                ExternalBinding::Syscall { number } => {
                    ExternalBindingKind::Syscall { number: *number }
                }
                ExternalBinding::DllImport { module, symbol } => ExternalBindingKind::DllImport {
                    module: module.clone(),
                    symbol: symbol.clone(),
                },
                ExternalBinding::CompilerIntrinsic { name } => {
                    ExternalBindingKind::CompilerIntrinsic { name: name.clone() }
                }
                ExternalBinding::VtableSlot { index } => {
                    ExternalBindingKind::VtableSlot { index: *index }
                }
                ExternalBinding::VtableField { field } => ExternalBindingKind::VtableField {
                    field: field.as_str().to_owned(),
                },
                ExternalBinding::TableFunction { field } => ExternalBindingKind::TableFunction {
                    field: field.as_str().to_owned(),
                },
            };
            let requirement_identity =
                crate::pipeline::provider_plans::satisfied_requirement_identity(
                    typed,
                    machine.name.as_str(),
                    clause.trait_name.as_str(),
                    requirement.as_str(),
                );
            let boundary_entry_plan = selected_source_boundary_entry_plan(
                typed,
                provider_plans,
                selected_plan_names,
                boundary_calling_plan_realizations,
                &plan_name,
                clause.trait_name.as_str(),
                requirement.as_str(),
                &requirement_identity,
            );
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
                    clause.trait_name.as_str(),
                    requirement.as_str(),
                    &requirement_identity,
                    policy,
                    usize::from(matches!(&binding, ExternalBindingKind::TableFunction { .. })),
                )
                .map_err(|reason| {
                    vec![Diagnostic::error(format!(
                        "cannot evaluate compatibility calling plan for `{}::{}`: {reason}",
                        clause.trait_name, requirement
                    ))]
                })?,
                (None, None) => None,
            };
            rows.push(ExternalBindingRow {
                // Target-machine filtering clears the selected machine's
                // marker so it can participate as an ordinary implementation.
                // Preserve deployment identity here from the compile target;
                // an unselected machine still carries its own marker and stays
                // inert. An originally unscoped leaf likewise belongs to the
                // selected build, never implicitly to the compiler host.
                target_name: machine.target.as_ref().map_or_else(
                    || selected_target.unwrap_or("cross_platform_cli").to_owned(),
                    |target| target.as_str().to_owned(),
                ),
                trait_name: clause.trait_name.as_str().to_owned(),
                method: requirement.as_str().to_owned(),
                requirement_identity: requirement_identity.clone(),
                table_type: provider_type.to_owned(),
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
    provider_plans: &[omega_effects::provider_plan::ProviderPlan],
    selected_plan_names: &[String],
    boundary_calling_plan_realizations: &[
        crate::pipeline::calling_policy_plans::BoundaryCallingPlanRealization
    ],
    provider_plan_name: &str,
    trait_name: &str,
    method_name: &str,
    requirement_identity: &str,
) -> Option<omega_calling_conventions::BoundaryEntryPlan> {
    if !selected_plan_names
        .iter()
        .any(|selected| selected == provider_plan_name)
    {
        return None;
    }
    let fingerprint = provider_plans
        .iter()
        .find(|plan| plan.name == provider_plan_name)
        .and_then(|plan| {
            plan.schema.methods.iter().find(|method| {
                method.name == method_name && method.requirement_identity == requirement_identity
            })
        })?
        .calling_plan_fingerprint?;
    let trait_leaf = trait_name.rsplit("::").next().unwrap_or(trait_name);
    boundary_calling_plan_realizations
        .iter()
        .find(|realization| {
            realization.fingerprint == fingerprint
                && typed.traits().iter().any(|definition| {
                    definition.symbol == realization.boundary_trait
                        && definition
                            .name
                            .as_str()
                            .rsplit("::")
                            .next()
                            .is_some_and(|name| name == trait_leaf)
                })
                && typed.traits().iter().any(|definition| {
                    typed
                        .trait_machine_signatures(definition)
                        .iter()
                        .any(|signature| {
                            signature.symbol == realization.requirement_machine
                                && signature.name.as_str() == method_name
                                && typed
                                    .normalized_trait_requirement_overload_identity(
                                        definition, signature,
                                    )
                                    .identity()
                                    == requirement_identity
                        })
                })
        })
        .map(|realization| realization.boundary_entry_plan.clone())
}

pub struct Compiler {
    options: CompileOptions,
    executable_tcb_policy: ExecutableTcbBuildPolicy,
    test_entry_machine_name: Option<String>,
}

impl Compiler {
    pub fn with_executable_tcb_policy(
        options: CompileOptions,
        executable_tcb_policy: ExecutableTcbBuildPolicy,
    ) -> Self {
        Self {
            options,
            executable_tcb_policy,
            test_entry_machine_name: None,
        }
    }

    fn with_test_entry(mut self, entry_machine_name: String) -> Self {
        self.test_entry_machine_name = Some(entry_machine_name);
        self
    }

    pub fn compile(self) -> Result<CompileReport, Vec<Diagnostic>> {
        let workers = WorkerPool::with_available_parallelism();
        let mut timings = CompileTimings::default();

        let (source_file_count, mut syntax) = source_files_to_syntax_trees(
            &self.options.root_path,
            self.options.target_name.as_deref(),
            &mut timings,
        )?;
        let evaluated = psi_build_time_evaluation::evaluate_pre_resolution(syntax.syntax_trees)?;
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
        // The BUILD-MACHINE identity is FILE-based (owner answer #3:
        // build.omg is the home; a `Builder::build` in ordinary source is
        // just a machine): collect the machines declared at build.omg roots
        // BEFORE the syntax storage moves into resolution. Typed machines
        // carry no source file (TASKS_FS open item), so the name list is
        // the thread.
        let build_file_machine_names: Vec<String> = syntax
            .files
            .iter()
            .filter(|file| {
                file.path.file_name().and_then(|name| name.to_str()) == Some("build.omg")
            })
            .flat_map(|file| file.root_items.iter())
            .filter_map(|handle| match syntax.syntax_trees.root_item(*handle) {
                // The syntax machine's `name` is already the FULL spelled
                // path (`Stager::build` -- split_machine_path joins it).
                psi_syntax_trees::item::Item::Machine(machine) => {
                    Some(machine.name.as_str().to_owned())
                }
                _ => None,
            })
            .collect();
        remove_stale_phase_diagrams(&self.options)?;
        write_pipeline_index(&self.options)?;
        write_syntax_snapshot(&self.options, &syntax)?;
        write_boundary_report(&self.options, &syntax.syntax_trees)?;
        validate_boundary_providers(&syntax.syntax_trees)?;
        let syntax_trees = syntax.syntax_trees.clone();

        let resolved = syntax_trees_to_symbol_resolved_trees(syntax, &mut timings)?;
        write_resolved_snapshot(&self.options, &resolved)?;

        let mut typed = symbol_resolved_trees_to_typed_trees(resolved, &mut timings)?;
        psi_build_time_evaluation::evaluate_pre_check(
            &mut typed,
            &plan_laid_records,
            &placed_view_records,
        )?;
        let boundary_calling_plan_realizations =
            crate::pipeline::calling_policy_plans::compute_boundary_calling_plans(&mut typed)?;
        // PDI3 selected operation/algebra authority is public type identity,
        // including for generic trust receipts emitted before checked
        // lowering. Bind it on the typed tree before snapshots and lockfile
        // fingerprints consume the declaration graph.
        psi_typed_trees_to_checked_trees::normalize_open_index_identities(&mut typed)?;
        // BUILD CONFIG (build_and_package_model.md): image facts from
        // build.omg's augmenting `build(b: &mut Build)` machine, evaluated at
        // build time. When present it is AUTHORITATIVE; the legacy in-source
        // `target { subsystem }` word is the fallback until its removal.
        let computed_build_config =
            crate::pipeline::build_config::compute_build_config(&typed, &build_file_machine_names)?;
        let build_evaluation_usage = computed_build_config.evaluation_usage;
        let build_config = computed_build_config.config;
        let selected_program_entry = crate::pipeline::build_config::selected_program_entry_machine(
            &build_config,
            self.options.target_name.as_deref(),
        )?;
        let selected_program_entry_receiver =
            if let Some(selected_program_entry) = selected_program_entry {
                crate::pipeline::build_config::validate_selected_program_entry_shape(
                    &typed,
                    selected_program_entry,
                )?
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
            .map(|(plan, _)| plan.clone());
        let entry_machine_name = selected_program_entry
            .map(|selected| selected.machine_name.to_owned())
            .or(self.test_entry_machine_name.clone());
        let target_provider_defaults =
            crate::pipeline::build_config::compute_target_provider_defaults(
                &typed,
                &target_default_machine_names,
            )?;
        let build_machine_present = typed.machines().iter().any(|machine| {
            crate::pipeline::build_config::is_build_machine(machine, &build_file_machine_names)
        });
        // ASM DISCHARGE v0 (privileged_effects_and_binary_trust): asm
        // intrinsics (`hlt`, port I/O) are permitted only in a FREESTANDING
        // boundary root. The gate lives here because it consumes a
        // BuildConfig fact the typed->checked validations never see.
        psi_typed_trees_to_checked_trees::validate_asm_discharge(
            &typed,
            build_config.freestanding,
        )?;
        write_typed_snapshot(&self.options, &typed)?;
        let provider_plans = crate::pipeline::provider_plans::derive_satisfies_plans(
            &syntax_trees,
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
        let selected_provider_plans = crate::pipeline::provider_plans::select_provider_plan_names(
            &provider_plans,
            selected_native_target,
            &target_provider_defaults,
            &build_config.provider_selections,
        )?;
        crate::pipeline::provider_plans::validate_selected_synchronous_invocation_cycles(
            &typed,
            &provider_plans,
            &selected_provider_plans,
        )?;
        let selected_provider_plan_facts =
            omega_effects::SelectedProviderPlanFacts::from_selection(
                &provider_plans,
                &selected_provider_plans,
            )
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        crate::pipeline::trust_lockfile::enforce_trust_lockfile(
            &self.options,
            &typed,
            &build_config.grants,
            &provider_plans,
            &selected_provider_plan_facts,
        )?;
        crate::pipeline::wire_report::write_wire_protocol_report(
            &self.options,
            &typed,
            &build_config.wire_compatibility_demands,
        )?;

        // Capture the selected provider's validated source calling plans
        // before typed ownership moves into checked lowering. The rows carry
        // them beside their mechanisms into the host-ABI/backend path.
        let external_binding_rows = extract_external_binding_rows(
            &syntax_trees,
            self.options.target_name.as_deref(),
            selected_native_target,
            &selected_provider_plans,
            &provider_plans,
            &boundary_calling_plan_realizations,
            &typed,
        )?;

        let mut checked = typed_trees_to_checked_trees(typed, &mut timings)?;
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
        crate::pipeline::trust_report::write_trust_report(
            &self.options,
            &checked.program.typed,
            &build_config.grants,
            &provider_plans,
            &checked.selected_provider_plans,
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
        write_checked_snapshot(
            &self.options,
            &checked.program,
            entry_machine_name.as_deref(),
            &checked.selected_provider_plans,
            &checked.task_activations,
        )?;
        write_boundary_report_with_capabilities(&self.options, &syntax_trees, &checked.program)?;
        let backend_surface =
            build_backend_surface_report(&checked.program, entry_machine_name.as_deref());

        // A check-only compilation with no selected runtime root ends at
        // checked semantics. Requiring an entry merely to produce the
        // frontend artifacts would turn `--check` into implicit execution
        // policy; callers that need native validation either select an exact
        // `ProgramEntry` or use the explicit legacy test-entry seam.
        if !self.options.write_output && entry_machine_name.is_none() {
            write_pipeline_shell(&self.options)?;
            return Ok(CompileReport {
                root_path: self.options.root_path,
                source_file_count,
                wrote_output: false,
                program_storage_entry: None,
                program_storage_entry_bridge: None,
                build_evaluation_usage,
            });
        }

        let state_graph = checked_trees_to_state_graph(&checked, workers.handle(), &mut timings)?;
        write_state_graph_snapshot(&self.options, &state_graph)?;
        let control_flow = state_graph_to_control_flow(state_graph, &mut timings)?;
        write_control_flow_snapshot(&self.options, &control_flow)?;

        // Build image subsystem and freestanding trust independently. PE
        // consumes the subsystem metadata; other formats ignore it. The
        // freestanding flag selects an empty ambient host ABI baseline. Both
        // facts come from build.omg (build_and_package_model.md); the old
        // in-source `target { subsystem }` word is retired.
        let _ = build_machine_present;
        let (subsystem, freestanding) = (build_config.subsystem, build_config.freestanding);
        let program_storage_entry_provider = program_entry_realization
            .as_ref()
            .map(|(_, selected)| {
                crate::pipeline::provider_plans::optional_selected_external_root_provider_plan(
                    &checked.selected_provider_plans,
                    &selected.schema().trait_name,
                )
                .map_err(|diagnostic| vec![Diagnostic::error(diagnostic.to_string())])
            })
            .transpose()?
            .flatten();
        // Selected external leaves become the target's source-authored
        // platform surface.
        let backend = control_flow_to_backend_plan(
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
            .map(|(_, selected)| {
                let plan = backend.plan.entry_boundary_plan.as_ref().ok_or_else(|| {
                    vec![Diagnostic::error(
                        "selected program-storage entry lost its retained calling plan before backend binding",
                    )]
                })?;
                crate::pipeline::program_storage_entry::bind_generated_program_storage_entry_plan(
                    selected,
                    plan,
                    &backend.plan.runtime_storage,
                    &backend.plan.layouts,
                    backend.plan.entry_key,
                    selected_program_entry_receiver.as_deref(),
                )
                .map_err(|diagnostic| vec![Diagnostic::error(diagnostic.to_string())])
            })
            .transpose()?;
        let program_storage_entry_bridge = program_storage_entry
            .map(|binding| {
                crate::pipeline::program_storage_entry::bind_emitted_program_storage_entry_native_bridge(
                    binding,
                    program_storage_entry_provider,
                    self.options
                        .target_name
                        .clone()
                        .unwrap_or_else(|| "host".to_owned()),
                    &backend.plan.object,
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
            })
            .transpose()?;
        if self.options.write_output {
            if let Some(bridge) = &program_storage_entry_bridge {
                write_program_storage_entry_snapshot(&self.options, bridge)?;
            }
            write_backend_report(&self.options, &backend_surface, &backend.plan)?;
        }

        let (emission_plan, emitted) =
            backend_plan_to_native_image_payload(&backend, subsystem, &mut timings)?;

        if self.options.write_output {
            let output_path = write_output(
                &self.options,
                &executable_tcb_installation_authorization,
                emitted,
                &backend.plan.encoded_machine.semantics.boundaries.footprints,
            )?;
            write_emission_plan(
                &self.options,
                &backend.plan,
                &emission_plan,
                Some(output_path.as_path()),
            )?;
            write_timings(&self.options, timings.as_slice())?;
        } else {
            write_emission_plan(&self.options, &backend.plan, &emission_plan, None)?;
        }

        write_pipeline_shell(&self.options)?;

        Ok(CompileReport {
            root_path: self.options.root_path,
            source_file_count,
            wrote_output: self.options.write_output,
            program_storage_entry: program_storage_entry_bridge
                .as_ref()
                .map(|bridge| bridge.binding().clone()),
            program_storage_entry_bridge,
            build_evaluation_usage,
        })
    }
}
