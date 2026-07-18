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

/// Parsed `provides` arms -> the calling-convention rows the freestanding ABI
/// builder consumes (`<target> provides <Trait> { method -> VtableSlot(1) }`).
fn extract_provides_rows(
    syntax_trees: &omega_syntax_trees::SyntaxTrees,
) -> Vec<omega_calling_conventions::ProvidesRow> {
    use omega_calling_conventions::{ProvidesBindingKind, ProvidesRow};
    use omega_syntax_trees::item::HostProviderMappingKind;

    let mut rows = Vec::new();
    for item in syntax_trees.root_items() {
        let omega_syntax_trees::item::Item::HostProvider(provider) = item else {
            continue;
        };
        let trait_name = syntax_trees
            .items
            .identifier_path_members(provider.boundary_trait)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("::");
        // The trait's LAST path member names the boundary trait item; its
        // machine signatures supply each bound method's declared parameter
        // count (the field-model encoders compare it against a call's
        // operand list to detect a prepended result place).
        let trait_item_name = syntax_trees
            .items
            .identifier_path_members(provider.boundary_trait)
            .last()
            .map(|member| member.as_str().to_owned())
            .unwrap_or_default();
        for mapping in syntax_trees.items.host_provider_mappings(provider.mappings) {
            let binding = match &mapping.binding {
                HostProviderMappingKind::Syscall { number } => {
                    ProvidesBindingKind::Syscall { number: *number }
                }
                HostProviderMappingKind::DllImport { module, symbol } => {
                    ProvidesBindingKind::DllImport {
                        module: module.clone(),
                        symbol: symbol.clone(),
                    }
                }
                HostProviderMappingKind::VtableSlot { index } => {
                    ProvidesBindingKind::VtableSlot { index: *index }
                }
                HostProviderMappingKind::VtableField { field } => {
                    ProvidesBindingKind::VtableField {
                        field: field.as_str().to_owned(),
                    }
                }
                HostProviderMappingKind::TableFunction { field } => {
                    ProvidesBindingKind::TableFunction {
                        field: field.as_str().to_owned(),
                    }
                }
                HostProviderMappingKind::Value { value } => {
                    ProvidesBindingKind::Value { value: *value }
                }
            };
            rows.push(ProvidesRow {
                target_name: provider.target.as_str().to_owned(),
                trait_name: trait_name.clone(),
                method: mapping.machine.as_str().to_owned(),
                vtable_struct: provider.vtable_struct.as_str().to_owned(),
                parameter_count: boundary_trait_method_parameter_count(
                    syntax_trees,
                    &trait_item_name,
                    mapping.machine.as_str(),
                ),
                binding,
            });
        }
    }

    // PRV4 step 1c: EXTERNAL LEAVES feed the same row stream. A bodyless
    // `satisfies Trait::method via <Binding>;` machine contributes one row
    // for the satisfied requirement; a `<target>`-scoped leaf rides its own
    // marker, an unscoped leaf rides the portable name (resolves to the
    // host target). Table-addressed mechanisms (VtableField/TableFunction)
    // wait for the leaf `over`-struct surface and are skipped here -- the
    // via validation rung keeps them from silently dropping.
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
            use omega_syntax_trees::item::HostProviderMappingKind;
            let binding = match binding {
                HostProviderMappingKind::Syscall { number } => {
                    ProvidesBindingKind::Syscall { number: *number }
                }
                HostProviderMappingKind::DllImport { module, symbol } => {
                    ProvidesBindingKind::DllImport {
                        module: module.clone(),
                        symbol: symbol.clone(),
                    }
                }
                HostProviderMappingKind::VtableSlot { index } => {
                    ProvidesBindingKind::VtableSlot { index: *index }
                }
                HostProviderMappingKind::Value { value } => {
                    ProvidesBindingKind::Value { value: *value }
                }
                HostProviderMappingKind::VtableField { .. }
                | HostProviderMappingKind::TableFunction { .. } => continue,
            };
            rows.push(ProvidesRow {
                target_name: machine
                    .target
                    .as_ref()
                    .map(|target| target.as_str().to_owned())
                    .unwrap_or_else(|| "cross_platform_cli".to_owned()),
                trait_name: clause.trait_name.as_str().to_owned(),
                method: requirement.as_str().to_owned(),
                vtable_struct: String::new(),
                parameter_count: boundary_trait_method_parameter_count(
                    syntax_trees,
                    clause.trait_name.as_str(),
                    requirement.as_str(),
                ),
                binding,
            });
        }
    }
    rows
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
        // PLAN-LAID VALUE TYPES (layouts L4), desugar half: synthesize the
        // `Policy<Schema>` instance definitions before resolution so every
        // later stage sees ordinary records.
        crate::pipeline::generic_instances::desugar_generic_data_instances(
            &mut syntax.syntax_trees,
        )?;
        let plan_laid_records =
            crate::pipeline::plan_laid::desugar_plan_laid_value_types(&mut syntax.syntax_trees)?;
        // PORTABLE VALUES (2026-07-07 settle, rung V2): the SELECTED target's
        // provides VALUE rows substitute into `Trait::NAME` paths here, so no
        // later stage -- including the interpreter -- grows a per-target
        // concept; each use IS the target's number (const-v0 discipline).
        crate::pipeline::provides_values::substitute_provides_values(
            &mut syntax.syntax_trees,
            self.options.target_name.as_deref(),
        )?;
        // TARGET-SCOPED MACHINES (fs portable-contract settle 2026-07-18):
        // the SELECTED target's `<target> machine` implementations become
        // ordinary machines; every other target's stay inert. Loud edges:
        // duplicate / missing implementations for the selected target.
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
            .filter_map(
                |handle| match syntax.syntax_trees.root_item(*handle) {
                    // The syntax machine's `name` is already the FULL spelled
                    // path (`Stager::build` -- split_machine_path joins it).
                    omega_syntax_trees::item::Item::Machine(machine) => {
                        Some(machine.name.as_str().to_owned())
                    }
                    _ => None,
                },
            )
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
        // COMPTIME STAGE 1: evaluate effect-free machine calls in fixed-array
        // length position and substitute concrete literals BEFORE checking,
        // proof facts, and layout consume the lengths.
        crate::pipeline::const_lengths::evaluate_const_array_lengths(&mut typed)?;
        // PLAN-LAID VALUE TYPES, plan half: evaluate + validate each policy
        // application and record the placements for the layout builder.
        crate::pipeline::plan_laid::compute_plan_laid_layouts(&mut typed, &plan_laid_records)?;
        // WIRE PLANS (mint arc rung 2a): derive each numbered schema's
        // placement plan; the wire codec selection consumes it (tag + framing
        // from the plan, asserted against its own walk).
        crate::pipeline::wire_plans::compute_wire_plans(&mut typed)?;
    // PRV4 adapter dispatch (both engines, before checking): boundary-trait
    // calls with a unique satisfying adapter rewrite to direct calls.
    crate::pipeline::adapter_dispatch::rewrite_adapter_calls(&mut typed)?;
        // BUILD CONFIG (build_and_package_model.md): image facts from
        // build.omg's augmenting `build(b: &mut Build)` machine, evaluated at
        // build time. When present it is AUTHORITATIVE; the legacy in-source
        // `target { subsystem }` word is the fallback until its removal.
        let build_config = crate::pipeline::build_config::compute_build_config(
            &typed,
            &build_file_machine_names,
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
        let mut provider_plans =
            crate::pipeline::provider_plans::derive_provider_plans(&syntax_trees, &typed);
        provider_plans.extend(crate::pipeline::provider_plans::derive_satisfies_plans(
            &syntax_trees,
            &typed,
        ));
        let selected_native_target = omega_target::NativeTarget::from_omega_target_name(
            self.options.target_name.as_deref(),
        )
        .unwrap_or_else(|_| omega_target::NativeTarget::host());
        let mut selection_diagnostics = crate::pipeline::provider_plans::validate_slot_selection(
            &provider_plans,
            selected_native_target,
        );
        selection_diagnostics.extend(
            crate::pipeline::provider_plans::validate_adapter_refinement(&typed, &provider_plans),
        );
        if !selection_diagnostics.is_empty() {
            return Err(selection_diagnostics);
        }
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
        crate::pipeline::wire_report::write_wire_protocol_report(&self.options, &typed)?;

        let checked = typed_trees_to_checked_trees(typed, &mut timings)?;
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
        // provides-sourced bindings (extern brief §12): parsed rows become
        // the freestanding target's authored platform surface.
        let provides_rows = extract_provides_rows(&syntax_trees);

        let backend = control_flow_to_backend_plan(
            checked,
            self.options.target_name.as_deref(),
            freestanding,
            &provides_rows,
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
            let output_path = write_output(&self.options, emitted)?;
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
