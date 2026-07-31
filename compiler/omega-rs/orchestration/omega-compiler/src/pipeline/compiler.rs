use crate::pipeline::artifacts::{
    remove_stale_phase_diagrams, write_backend_report, write_checked_snapshot,
    write_control_flow_snapshot, write_emission_plan, write_pipeline_index, write_pipeline_shell,
    write_resolved_snapshot, write_state_graph_snapshot, write_syntax_snapshot, write_timings,
    write_typed_snapshot,
};
use crate::pipeline::boundary_report::{
    write_boundary_report, write_boundary_report_with_capabilities,
};
use crate::pipeline::compile_options::CompileOptions;
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
use omega_core::diagnostics::Diagnostic;
use omega_core::parallel::WorkerPool;
use std::sync::Arc;

pub fn compile(options: CompileOptions) -> Result<CompileReport, Vec<Diagnostic>> {
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
    const COMPILE_STACK_SIZE: usize = 256 * 1024 * 1024;
    std::thread::Builder::new()
        .name("omega-compile".to_owned())
        .stack_size(COMPILE_STACK_SIZE)
        .spawn(move || Compiler::new(options).compile())
        .expect("failed to spawn compiler thread")
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
}

/// Builds the boundary provider registry from `provider` declarations, enforces
/// the package whitelist, and rejects boundary operator bindings that do not
/// resolve to a registered provider (frozen Wave 0 decision #4).
fn validate_boundary_providers(
    syntax: &omega_syntax_trees::SyntaxTrees,
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
    syntax_trees: &omega_syntax_trees::SyntaxTrees,
    selected_target: Option<&str>,
    selected_plan_names: &[String],
    provider_plans: &[omega_effects::provider_plan::ProviderPlan],
    typed: &omega_typed_trees::TypedTrees,
) -> Vec<omega_calling_conventions::ExternalBindingRow> {
    use omega_calling_conventions::{ExternalBindingKind, ExternalBindingRow};

    let mut rows = Vec::new();
    // A bodyless
    // `satisfies Trait::method via <Binding>;` machine contributes one row
    // for the satisfied requirement; a `<target>`-scoped leaf rides its own
    // marker, an unscoped leaf rides the portable name (resolves to the
    // host target). For table-addressed mechanisms, the leaf's attached data
    // type owns the layout used for table-addressed mechanisms.
    for item in syntax_trees.root_items() {
        let omega_syntax_trees::item::Item::Machine(machine) = item else {
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
            use omega_syntax_trees::item::ExternalBinding;
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
                table_type: provider_type.to_owned(),
                parameter_count: boundary_trait_method_parameter_count(
                    syntax_trees,
                    clause.trait_name.as_str(),
                    requirement.as_str(),
                ),
                boundary_entry_plan: selected_source_boundary_entry_plan(
                    typed,
                    provider_plans,
                    selected_plan_names,
                    &plan_name,
                    clause.trait_name.as_str(),
                    requirement.as_str(),
                ),
                binding,
            });
        }
    }
    rows
}

/// Resolve implementation evidence only through the provider candidate that
/// selection admitted. The public provider/schema identity carries the
/// canonical fingerprint; the typed program retains the corresponding plan
/// internally so lowering never has to rediscover or re-run policy source.
fn selected_source_boundary_entry_plan(
    typed: &omega_typed_trees::TypedTrees,
    provider_plans: &[omega_effects::provider_plan::ProviderPlan],
    selected_plan_names: &[String],
    provider_plan_name: &str,
    trait_name: &str,
    method_name: &str,
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
            plan.schema
                .methods
                .iter()
                .find(|method| method.name == method_name)
        })?
        .calling_plan_fingerprint?;
    let trait_leaf = trait_name.rsplit("::").next().unwrap_or(trait_name);
    typed
        .boundary_calling_plans
        .iter()
        .find(|identity| {
            identity.fingerprint == fingerprint
                && typed.traits().iter().any(|definition| {
                    definition.symbol == identity.boundary_trait
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
                            signature.symbol == identity.requirement_machine
                                && signature.name.as_str() == method_name
                        })
                })
        })
        .map(|identity| identity.boundary_entry_plan.clone())
}

/// The declared parameter count of `method` on the boundary trait named
/// `trait_name`, or 0 when either is not found (the static mechanisms never
/// read it; the field-model merge sees 0 only for a binding whose trait is
/// missing, which the resolver refuses elsewhere).
fn boundary_trait_method_parameter_count(
    syntax_trees: &omega_syntax_trees::SyntaxTrees,
    trait_name: &str,
    method: &str,
) -> usize {
    for item in syntax_trees.root_items() {
        let omega_syntax_trees::item::Item::Trait(trait_definition) = item else {
            continue;
        };
        if trait_definition.name.as_str() != trait_name {
            continue;
        }
        for signature_handle in syntax_trees
            .items
            .state_signatures(trait_definition.machines)
        {
            let signature = syntax_trees.items.state_signature(*signature_handle);
            if signature.name.as_str() == method {
                return syntax_trees
                    .items
                    .state_parameters(signature.parameters)
                    .len();
            }
        }
    }
    0
}

pub struct Compiler {
    options: CompileOptions,
}

impl Compiler {
    pub fn new(options: CompileOptions) -> Self {
        Self { options }
    }

    pub fn compile(self) -> Result<CompileReport, Vec<Diagnostic>> {
        let workers = WorkerPool::with_available_parallelism();
        let mut timings = CompileTimings::default();

        let (source_file_count, mut syntax) = source_files_to_syntax_trees(
            &self.options.root_path,
            self.options.target_name.as_deref(),
            &mut timings,
        )?;
        crate::pipeline::const_generic_calls::evaluate_const_generic_calls(
            &mut syntax.syntax_trees,
        )?;
        crate::pipeline::trait_defaults::synthesize_trait_defaults(&mut syntax.syntax_trees)?;
        let placed_view_records =
            crate::pipeline::placed_views::desugar_placed_views(&mut syntax.syntax_trees)?;
        // PLAN-LAID VALUE TYPES (layouts L4), desugar half: synthesize the
        // `Policy<Schema>` instance definitions before resolution so every
        // later stage sees ordinary records.
        crate::pipeline::generic_instances::desugar_generic_data_instances(
            &mut syntax.syntax_trees,
        )?;
        let plan_laid_records =
            crate::pipeline::plan_laid::desugar_plan_laid_value_types(&mut syntax.syntax_trees)?;
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
                omega_syntax_trees::item::Item::Machine(machine) => {
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
        // COMPTIME STAGE 1: evaluate build-time-admissible machine calls in fixed-array
        // length position and substitute concrete literals BEFORE checking,
        // proof facts, and layout consume the lengths.
        crate::pipeline::const_lengths::evaluate_const_array_lengths(&mut typed)?;
        crate::pipeline::const_domain_facts::evaluate_const_domain_facts(&mut typed)?;
        // PLAN-LAID VALUE TYPES, plan half: evaluate + validate each policy
        // application and record the placements for the layout builder.
        crate::pipeline::plan_laid::compute_plan_laid_layouts(&mut typed, &plan_laid_records)?;
        crate::pipeline::placed_views::validate_placed_view_plans(
            &mut typed,
            &placed_view_records,
        )?;
        // WIRE PLANS (mint arc rung 2a): derive each numbered schema's
        // placement plan; the wire codec selection consumes it (tag + framing
        // from the plan, asserted against its own walk).
        crate::pipeline::wire_plans::compute_wire_plans(&mut typed)?;
        crate::pipeline::calling_policy_plans::compute_boundary_calling_plans(&mut typed)?;
        // BUILD CONFIG (build_and_package_model.md): image facts from
        // build.omg's augmenting `build(b: &mut Build)` machine, evaluated at
        // build time. When present it is AUTHORITATIVE; the legacy in-source
        // `target { subsystem }` word is the fallback until its removal.
        let build_config =
            crate::pipeline::build_config::compute_build_config(&typed, &build_file_machine_names)?;
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
        omega_typed_trees_to_checked_trees::validate_asm_discharge(
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
        crate::pipeline::trust_lockfile::enforce_trust_lockfile(
            &self.options,
            &typed,
            &build_config.grants,
            &provider_plans,
        )?;
        crate::pipeline::trust_report::write_trust_report(
            &self.options,
            &typed,
            &build_config.grants,
            &provider_plans,
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
            &selected_provider_plans,
            &provider_plans,
            &typed,
        );

        let mut checked = typed_trees_to_checked_trees(typed, &mut timings)?;
        crate::pipeline::provider_plans::retain_selected_provider_plan_facts(
            Arc::get_mut(&mut checked.program)
                .expect("checked program must be uniquely owned before backend fan-out"),
            &provider_plans,
            &selected_provider_plans,
            &build_config.grants,
        )?;
        let selected_provider_plan_facts = checked.program.selected_provider_plans().clone();
        // PRV4 adapter dispatch (both engines, after checking): semantic facts
        // stay attached to the admitted boundary requirement, while execution
        // alone is redirected to the uniquely selected checked adapter.
        crate::pipeline::adapter_dispatch::rewrite_adapter_calls(
            &mut Arc::get_mut(&mut checked.program)
                .expect("checked program must be uniquely owned before backend fan-out")
                .typed,
            &selected_provider_plan_facts,
        )?;
        crate::pipeline::task_plans::elaborate_task_activation_plans(
            Arc::get_mut(&mut checked.program)
                .expect("checked program must be uniquely owned before backend fan-out"),
            selected_native_target,
        )?;
        write_checked_snapshot(&self.options, &checked.program)?;
        write_boundary_report_with_capabilities(&self.options, &syntax_trees, &checked.program)?;
        let backend_surface = build_backend_surface_report(&checked.program);

        let state_graph = checked_trees_to_state_graph(&checked, workers.handle(), &mut timings)?;
        write_state_graph_snapshot(&self.options, &state_graph)?;
        let control_flow = state_graph_to_control_flow(state_graph, &mut timings)?;
        write_control_flow_snapshot(&self.options, &control_flow)?;

        // The selected target's declared image subsystem (`subsystem
        // console|gui|efi_application`, ch: target blocks); console when the
        // target declares none. PE consumes it; other formats ignore it.
        // Resolved BEFORE the backend build because `efi_application` also
        // means FREESTANDING: the target trusts no host boundary packages, so
        // the backend builds against an empty host ABI plan (no bindings, no
        // import thunks -- services arrive via the entry's parameters).
        // Image facts come from build.omg (build_and_package_model.md); the
        // in-source `target { subsystem }` word is retired.
        let _ = build_machine_present;
        let (subsystem, freestanding) = (build_config.subsystem, build_config.freestanding);
        // Selected external leaves become the target's source-authored
        // platform surface.
        let backend = control_flow_to_backend_plan(
            checked,
            self.options.target_name.as_deref(),
            freestanding,
            &external_binding_rows,
            control_flow,
            workers.handle(),
            &mut timings,
        )?;
        if self.options.write_output {
            write_backend_report(&self.options, &backend_surface, &backend.plan)?;
        }

        let (emission_plan, emitted) =
            backend_plan_to_native_image_payload(&backend, subsystem, &mut timings)?;

        if self.options.write_output {
            let output_path = write_output(
                &self.options,
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
        })
    }
}
