use crate::compiler::{ArtifactEmissionPolicy, CompileOptions};
use crate::pipeline::stages::AssembledSyntax;
use omega_artifacts::{ArtifactWriter, PhaseTiming};
use omega_backend_report::{BackendReportInput, BackendReportPhaseTiming, backend_report_text};
use psi_diagnostics::Diagnostic;
use std::path::Path;

/// Checked-source observation retained until the corresponding backend plan is
/// available. Report suppression and non-native compilation produce canonical
/// absence; a captured surface is consumed exactly once at the unchanged
/// post-backend reporting point.
pub(super) struct BackendReportObservation {
    surface: Option<omega_artifacts::BackendSurfaceReport>,
}

impl BackendReportObservation {
    pub(super) fn capture(
        program: &psi_checked_trees::CheckedTrees,
        selected_entry_machine: Option<&str>,
        policy: ArtifactEmissionPolicy,
        requires_native_backend: bool,
    ) -> Self {
        let surface = (policy.emits_auxiliary_artifacts() && requires_native_backend).then(|| {
            omega_artifacts::build_backend_surface_report(program, selected_entry_machine)
        });
        Self { surface }
    }

    pub(super) fn write(
        self,
        options: &CompileOptions,
        plan: &omega_backend_plan::BackendPlan,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(surface) = self.surface else {
            return Ok(());
        };
        write_backend_report(options, &surface, plan)
    }
}

pub(super) enum FinalPipelineObservation<'a> {
    CheckedOnly,
    InstalledOutput {
        backend: &'a omega_backend_plan::BackendPlan,
        emission: &'a omega_artifacts::EmissionPlan,
        storage_bridge: Option<&'a super::ProgramStorageEntryNativeBridgePlan>,
        output_path: &'a Path,
        timings: &'a [PhaseTiming],
    },
    UnpublishedNative {
        backend: &'a omega_backend_plan::BackendPlan,
        emission: &'a omega_artifacts::EmissionPlan,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FinalPipelineObservationDisposition<'a> {
    CheckedOnly,
    InstalledOutput {
        has_storage_bridge: bool,
        output_path: &'a Path,
    },
    UnpublishedNative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FinalPipelineObservationStep<'a> {
    ProgramStorageEntry,
    Emission { output_path: Option<&'a Path> },
    Timings,
    PipelineShell,
}

fn final_pipeline_observation_steps<'a>(
    policy: ArtifactEmissionPolicy,
    disposition: FinalPipelineObservationDisposition<'a>,
) -> Vec<FinalPipelineObservationStep<'a>> {
    if !policy.emits_auxiliary_artifacts() {
        return Vec::new();
    }

    match disposition {
        FinalPipelineObservationDisposition::CheckedOnly => {
            vec![FinalPipelineObservationStep::PipelineShell]
        }
        FinalPipelineObservationDisposition::InstalledOutput {
            has_storage_bridge,
            output_path,
        } => {
            let mut steps = Vec::with_capacity(4);
            if has_storage_bridge {
                steps.push(FinalPipelineObservationStep::ProgramStorageEntry);
            }
            steps.extend([
                FinalPipelineObservationStep::Emission {
                    output_path: Some(output_path),
                },
                FinalPipelineObservationStep::Timings,
                FinalPipelineObservationStep::PipelineShell,
            ]);
            steps
        }
        FinalPipelineObservationDisposition::UnpublishedNative => vec![
            FinalPipelineObservationStep::Emission { output_path: None },
            FinalPipelineObservationStep::PipelineShell,
        ],
    }
}

pub(super) fn write_final_pipeline_observations(
    options: &CompileOptions,
    policy: ArtifactEmissionPolicy,
    observation: FinalPipelineObservation<'_>,
) -> Result<(), Vec<Diagnostic>> {
    let disposition = match &observation {
        FinalPipelineObservation::CheckedOnly => FinalPipelineObservationDisposition::CheckedOnly,
        FinalPipelineObservation::InstalledOutput {
            storage_bridge,
            output_path,
            ..
        } => FinalPipelineObservationDisposition::InstalledOutput {
            has_storage_bridge: storage_bridge.is_some(),
            output_path,
        },
        FinalPipelineObservation::UnpublishedNative { .. } => {
            FinalPipelineObservationDisposition::UnpublishedNative
        }
    };

    for step in final_pipeline_observation_steps(policy, disposition) {
        match step {
            FinalPipelineObservationStep::ProgramStorageEntry => {
                let bridge = match &observation {
                    FinalPipelineObservation::InstalledOutput {
                        storage_bridge: Some(bridge),
                        ..
                    } => bridge,
                    _ => unreachable!(
                        "final observation roster requested an absent program-storage bridge"
                    ),
                };
                write_program_storage_entry_snapshot(options, bridge)?;
            }
            FinalPipelineObservationStep::Emission { output_path } => {
                let (backend, emission) = match &observation {
                    FinalPipelineObservation::InstalledOutput {
                        backend, emission, ..
                    }
                    | FinalPipelineObservation::UnpublishedNative {
                        backend, emission, ..
                    } => (backend, emission),
                    FinalPipelineObservation::CheckedOnly => {
                        unreachable!("checked-only observation roster requested native emission")
                    }
                };
                write_emission_plan(options, backend, emission, output_path)?;
            }
            FinalPipelineObservationStep::Timings => {
                let timings = match &observation {
                    FinalPipelineObservation::InstalledOutput { timings, .. } => timings,
                    _ => unreachable!("final observation roster requested absent timings"),
                };
                write_timings(options, timings)?;
            }
            FinalPipelineObservationStep::PipelineShell => write_pipeline_shell(options)?,
        }
    }

    Ok(())
}

pub(super) fn write_pipeline_index(options: &CompileOptions) -> Result<(), Vec<Diagnostic>> {
    write_phase_diagram(
        options,
        "00_pipeline.html",
        &omega_visualizations::pipeline_index_html(),
    )
}

pub(super) fn write_pipeline_shell(options: &CompileOptions) -> Result<(), Vec<Diagnostic>> {
    let build_dir = options.build_dir();
    let page_specs = [
        ("00", "Timings", "timings", "00_timings.html"),
        ("02", "Syntax", "syntax", "02_syntax_trees.html"),
        ("03", "Symbols", "symbols", "03_symbol_resolved_trees.html"),
        ("04", "Typed", "typed", "04_typed_trees.html"),
        ("05", "Checked", "checked", "05_checked_trees.html"),
        (
            "cap",
            "Capabilities",
            "capabilities",
            "05_capability_manifest.html",
        ),
        ("06", "State Graph", "state-graph", "06_state_graph.html"),
        ("07", "Control Flow", "control-flow", "07_control_flow.html"),
        (
            "08",
            "Abstract Operations",
            "abstract-operations",
            "08_abstract_operations.html",
        ),
        (
            "09",
            "Target Operations",
            "target-operations",
            "09_target_operations.html",
        ),
        ("10", "Boundary", "boundary", "10_boundary.html"),
        (
            "11",
            "Assigned Target Operations",
            "assigned-target-operations",
            "10_assigned_target_operations.html",
        ),
        (
            "12",
            "Machine Instructions",
            "machine-instructions",
            "11_machine_instructions.html",
        ),
        ("13", "Emission", "emission", "12_emission.html"),
    ];
    let mut page_html = Vec::new();
    let mut present_page_specs = Vec::new();
    for page_spec in page_specs {
        let (_, _, _, file_name) = page_spec;
        let path = build_dir.join(file_name);
        let Ok(contents) = std::fs::read_to_string(&path) else {
            continue;
        };
        present_page_specs.push(page_spec);
        page_html.push(contents);
    }
    if present_page_specs.is_empty() {
        return write_pipeline_index(options);
    }

    let pages = present_page_specs
        .iter()
        .zip(page_html.iter())
        .map(
            |((number, label, id, _), html)| omega_visualizations::PipelineEmbeddedPage {
                number,
                label,
                id,
                html,
            },
        )
        .collect::<Vec<_>>();

    write_phase_diagram(
        options,
        "00_pipeline.html",
        &omega_visualizations::pipeline_shell_html(&pages),
    )
}

pub(super) fn write_syntax_snapshot(
    options: &CompileOptions,
    syntax: &AssembledSyntax,
) -> Result<(), Vec<Diagnostic>> {
    let files = syntax
        .files
        .iter()
        .map(|file| omega_visualizations::SyntaxSourceFile {
            path: display_path(&file.path),
            root_items: file.root_items.clone(),
        })
        .collect::<Vec<_>>();
    write_phase_diagram(
        options,
        "02_syntax_trees.html",
        &omega_visualizations::syntax_trees_with_files_html(&syntax.syntax_trees, &files),
    )?;
    write_phase_json(
        options,
        "02_syntax_trees.json",
        &syntax
            .syntax_trees
            .snapshot_json_pretty()
            .map_err(json_diagnostic)?,
    )
}

fn display_path(path: &Path) -> String {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    std::env::current_dir()
        .ok()
        .and_then(|current_dir| {
            canonical
                .strip_prefix(current_dir)
                .ok()
                .map(Path::to_path_buf)
        })
        .unwrap_or(canonical)
        .display()
        .to_string()
}

pub(super) fn write_resolved_snapshot(
    options: &CompileOptions,
    resolved: &psi_symbol_resolved_trees::SymbolResolvedTrees,
) -> Result<(), Vec<Diagnostic>> {
    write_phase_diagram(
        options,
        "03_symbol_resolved_trees.html",
        &omega_visualizations::symbol_resolved_trees_html(resolved),
    )?;
    write_phase_json(
        options,
        "03_symbol_resolved_trees.json",
        &resolved.snapshot_json_pretty().map_err(json_diagnostic)?,
    )
}

pub(super) fn write_typed_snapshot(
    options: &CompileOptions,
    typed: &psi_typed_trees::TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    write_phase_diagram(
        options,
        "04_typed_trees.html",
        &omega_visualizations::typed_trees_html(typed),
    )?;
    write_phase_json(
        options,
        "04_typed_trees.json",
        &typed.snapshot_json_pretty().map_err(json_diagnostic)?,
    )
}

pub(crate) fn write_checked_snapshot(
    options: &CompileOptions,
    checked: &psi_checked_trees::CheckedTrees,
    selected_entry_machine: Option<&str>,
    selected_provider_plans: &omega_effects::SelectedProviderPlanFacts,
    task_activations: &omega_task_plans::TaskActivationPlanSet,
    component_progress: Option<&omega_effects::ComponentProgressManifest>,
) -> Result<(), Vec<Diagnostic>> {
    write_phase_diagram(
        options,
        "05_checked_trees.html",
        &omega_visualizations::checked_trees_html(checked),
    )?;
    write_phase_diagram(
        options,
        "05_capability_manifest.html",
        &omega_visualizations::capability_manifest_html_with_composition(
            checked,
            selected_entry_machine,
            Some(selected_provider_plans),
            component_progress,
        ),
    )?;
    write_phase_json(
        options,
        "05_capability_manifest.json",
        &omega_visualizations::capability_manifest_json_with_composition(
            checked,
            selected_entry_machine,
            Some(selected_provider_plans),
            component_progress,
        ),
    )?;
    write_phase_json(
        options,
        "05_machine_contracts.json",
        &omega_visualizations::machine_contract_manifest_json(checked),
    )?;
    write_phase_json(
        options,
        "05_qualification_evidence.json",
        &omega_visualizations::qualification_evidence_manifest_json(
            checked,
            selected_provider_plans,
        ),
    )?;
    write_phase_json(
        options,
        "05_index_compatibility.json",
        &omega_visualizations::index_compatibility_manifest_json(checked),
    )?;
    write_phase_json(
        options,
        "05_claim_outcomes.json",
        &omega_visualizations::claim_outcome_manifest_json(checked),
    )?;
    write_phase_json(
        options,
        "05_carry_manifest.json",
        &omega_visualizations::carry_manifest_json(checked),
    )?;
    write_phase_json(
        options,
        "05_task_activations.json",
        &omega_visualizations::task_activation_manifest_json(checked, task_activations),
    )?;
    write_phase_json(
        options,
        "05_executable_tcb_manifest.json",
        &omega_visualizations::executable_tcb_manifest_json(selected_provider_plans),
    )
}

pub(super) fn write_state_graph_snapshot(
    options: &CompileOptions,
    state_graph: &omega_state_graph::StateGraph,
) -> Result<(), Vec<Diagnostic>> {
    write_phase_diagram(
        options,
        "06_state_graph.html",
        &omega_visualizations::state_graph_html(state_graph),
    )
}

pub(super) fn write_control_flow_snapshot(
    options: &CompileOptions,
    control_flow: &omega_control_flow::ControlFlowPlan,
) -> Result<(), Vec<Diagnostic>> {
    write_phase_diagram(
        options,
        "07_control_flow.html",
        &omega_visualizations::control_flow_html(control_flow),
    )
}

fn write_backend_report(
    options: &CompileOptions,
    backend_surface: &omega_artifacts::BackendSurfaceReport,
    plan: &omega_backend_plan::BackendPlan,
) -> Result<(), Vec<Diagnostic>> {
    let phase_timings = plan
        .phase_timings
        .iter()
        .map(|(_, timing)| BackendReportPhaseTiming {
            phase: timing.phase.to_owned(),
            microseconds: timing.microseconds,
            allocations: timing.allocations,
        })
        .collect::<Vec<_>>();
    let report = backend_report_text(
        backend_surface,
        &BackendReportInput {
            target: plan.target,
            entry_key: plan.entry_key,
            phase_timings: &phase_timings,
            host_abi: &plan.host_abi,
            host_calls: &plan.host_calls,
            state_calls: &plan.state_calls,
            alias_flow: &plan.alias_flow,
            state_storage: &plan.state_storage,
            state_values: &plan.state_values,
            data: &plan.data,
            abstract_operations: &plan.abstract_operations,
            target_operations: &plan.target_operations,
            assigned_target_operations: &plan.assigned_target_operations,
            control_flow: &plan.control_flow,
            runtime_flow: &plan.runtime_flow,
            state_dispatch: &plan.state_dispatch,
            state_guards: &plan.state_guards,
            runtime_bodies: &plan.runtime_bodies,
            runtime_branching_calls: &plan.runtime_branching_calls,
            runtime_dispatch_loop: &plan.runtime_dispatch_loop,
            runtime_storage: &plan.runtime_storage,
            runtime_text: &plan.runtime_text,
            layouts: &plan.layouts,
            machine_instructions: &plan.machine_instructions,
            encoded_machine: &plan.encoded_machine,
            object: &plan.object,
            relocations: &plan.relocations,
        },
    );

    write_phase_diagram(
        options,
        "08_abstract_operations.html",
        &omega_visualizations::abstract_operations_html(
            &plan.abstract_operations,
            &plan.control_flow,
        ),
    )?;
    write_phase_text(
        options,
        "08_boundary_footprints.json",
        &omega_visualizations::boundary_footprint_fragments_json(&plan.encoded_machine),
    )?;
    write_phase_diagram(
        options,
        "09_target_operations.html",
        &omega_visualizations::target_operations_html(&plan.target_operations, &plan.control_flow),
    )?;
    write_phase_diagram(
        options,
        "10_assigned_target_operations.html",
        &omega_visualizations::assigned_target_operations_html(
            &plan.assigned_target_operations,
            &plan.control_flow,
        ),
    )?;
    write_phase_diagram(
        options,
        "11_machine_instructions.html",
        &omega_visualizations::machine_instructions_html(
            &plan.machine_instructions,
            &plan.assigned_target_operations,
            &plan.control_flow,
        ),
    )?;
    // Plain-text twin of the HTML report. The HTML wraps the same text in a
    // graph/`<pre>` shell, which makes it awkward to grep; the `.txt` keeps the
    // full backend report (state guards, dispatch loop, codegen) directly
    // readable for debugging.
    write_phase_text(options, "backend_report.txt", &report)?;
    // Machine-readable frame-slot side-table: maps every logical slot
    // (machine/state/param/local) to its absolute runtime byte offset inside the
    // `omega_runtime_frame_storage` region, so a debugger/script can translate a
    // named slot to its frame offset without disassembly. Same content as the
    // `OMEGA_DUMP_SLOTS` stderr dump.
    write_phase_text(
        options,
        "slots.txt",
        &omega_backend_pipeline::render_frame_slot_table(&plan.runtime_storage, &plan.runtime_flow),
    )?;
    write_phase_diagram(
        options,
        "backend_report.html",
        &omega_visualizations::text_report_html("backend_report", &report),
    )
}

pub(super) fn write_program_storage_entry_snapshot(
    options: &CompileOptions,
    bridge: &super::ProgramStorageEntryNativeBridgePlan,
) -> Result<(), Vec<Diagnostic>> {
    write_phase_json(
        options,
        "10_program_storage_entry.json",
        &program_storage_entry_manifest_json(bridge),
    )
}

fn push_normalized_identity(output: &mut String, identity: u64) {
    output.push_str(&format!("0x{identity:016x}"));
}

fn program_storage_entry_manifest_json(
    bridge: &super::ProgramStorageEntryNativeBridgePlan,
) -> String {
    let binding = bridge.binding();
    let mut output = String::from("{\n  \"root_slot\": \"");
    output.push_str(&format!(
        "0x{:016x}",
        binding.root_slot().normalized_identity()
    ));
    output.push_str("\",\n  \"semantic_requirement\": ");
    push_json_string(&mut output, binding.requirement_identity());
    output.push_str(",\n  \"semantic_continuation_calling_plan_fingerprint\": \"");
    output.push_str(&format!(
        "0x{:016x}",
        binding.boundary_contract_fingerprint()
    ));
    output.push_str("\",\n  \"physical_contract\": ");
    if let Some(physical) = binding.physical_contract() {
        output.push_str("{\"status\": \"planned_not_invoked\", \"requirement\": ");
        push_json_string(&mut output, physical.requirement_identity());
        output.push_str(", \"target_package\": ");
        push_json_string(&mut output, physical.target_package_identity());
        output.push_str(", \"target_package_fingerprint\": \"");
        push_normalized_identity(&mut output, physical.target_package_fingerprint());
        output.push('"');
        output.push_str(", \"calling_plan_fingerprint\": \"");
        push_normalized_identity(&mut output, physical.calling_plan_fingerprint());
        output.push_str("\", \"parameter_type_identities\": [");
        for (index, identity) in physical.parameter_type_identities().iter().enumerate() {
            if index > 0 {
                output.push_str(", ");
            }
            push_json_string(&mut output, identity);
        }
        output.push_str("], \"result_type_identity\": ");
        push_json_string(&mut output, physical.result_type_identity());
        output.push_str(", \"physical_shell_emitted\": false, \"bootstrap_invoked\": false}");
    } else {
        output.push_str("null");
    }
    output.push_str(",\n  \"semantic_parameters\": [\n    ");
    push_program_storage_parameter_json(&mut output, "image", binding.image());
    output.push_str(",\n    ");
    push_program_storage_parameter_json(&mut output, "initial_storage", binding.initial_storage());
    output.push_str("\n  ],\n  \"receiver_storage\": ");
    if let Some(receiver) = binding.receiver() {
        output.push_str("{\"status\": \"reservation_required\", \"type_identity\": ");
        push_json_string(&mut output, receiver.type_identity());
        output.push_str(", \"byte_size\": ");
        output.push_str(&receiver.byte_size().to_string());
        output.push_str(", \"byte_alignment\": ");
        output.push_str(&receiver.byte_alignment().to_string());
        output.push('}');
    } else {
        output.push_str("null");
    }
    output.push_str(",\n  \"native_bridge\": {\n    \"status\": \"pending_runtime_installation\",\n    \"target_profile\": ");
    push_json_string(&mut output, bridge.target_profile());
    output.push_str(",\n    \"entry_symbol\": ");
    push_json_string(&mut output, bridge.entry_symbol());
    output.push_str(",\n    \"entry_text_offset\": ");
    output.push_str(&bridge.entry_text_offset().to_string());
    output.push_str(",\n    \"entry_text_size\": ");
    output.push_str(&bridge.entry_text_size().to_string());
    output.push_str(",\n    \"source_continuation\": {\"machine\": ");
    push_json_string(&mut output, bridge.continuation_machine());
    output.push_str(", \"state\": ");
    push_json_string(&mut output, bridge.continuation_state());
    output.push_str("},\n    \"selected_root_provider_plan\": ");
    if let Some(provider) = bridge.selected_provider() {
        output.push('"');
        push_normalized_identity(&mut output, provider.identity.normalized_identity());
        output.push('"');
    } else {
        output.push_str("null");
    }
    output.push_str(",\n    \"emitted_wrapper_evidence\": ");
    if let Some(evidence) = bridge.emitted_wrapper_evidence() {
        output.push_str("{\"wrapper_symbol\": ");
        push_json_string(&mut output, evidence.wrapper_symbol());
        output.push_str(", \"wrapper_section_offset\": ");
        output.push_str(&evidence.wrapper_section_offset().to_string());
        output.push_str(", \"wrapper_address\": \"");
        push_normalized_identity(&mut output, evidence.wrapper_address());
        output.push_str("\", \"wrapper_byte_count\": ");
        output.push_str(&evidence.wrapper_byte_count().to_string());
        output.push_str(", \"wrapper_byte_fingerprint\": \"");
        push_normalized_identity(&mut output, evidence.wrapper_byte_fingerprint());
        output.push_str("\", \"continuation_symbol\": ");
        push_json_string(&mut output, evidence.continuation_symbol());
        output.push_str(", \"continuation_section_offset\": ");
        output.push_str(&evidence.continuation_section_offset().to_string());
        output.push_str(", \"continuation_address\": \"");
        push_normalized_identity(&mut output, evidence.continuation_address());
        output.push_str("\", \"continuation_byte_count\": ");
        output.push_str(&evidence.continuation_byte_count().to_string());
        output.push_str(", \"continuation_byte_fingerprint\": \"");
        push_normalized_identity(&mut output, evidence.continuation_byte_fingerprint());
        output.push_str("\", \"call_section_offset\": ");
        output.push_str(&evidence.call_section_offset().to_string());
        output.push_str(", \"final_call_bytes\": [");
        for (index, byte) in evidence.final_call_bytes().iter().enumerate() {
            if index > 0 {
                output.push_str(", ");
            }
            output.push_str(&byte.to_string());
        }
        output.push_str("], \"semantic_wrapper_arrival\": ");
        push_program_storage_arrival_json(&mut output, evidence.arrival());
        output.push_str(", \"compiler_text_derivation_fingerprint\": \"");
        push_normalized_identity(
            &mut output,
            evidence.compiler_text_validation().derivation_fingerprint,
        );
        output.push_str("\", \"compiler_function_validation_fingerprint\": \"");
        push_normalized_identity(
            &mut output,
            evidence
                .compiler_function_validation()
                .evidence_fingerprint(),
        );
        output.push_str("\", \"executable_inventory_fingerprint\": \"");
        push_normalized_identity(&mut output, evidence.executable_inventory_fingerprint());
        output.push_str("\"}");
    } else {
        output.push_str("null");
    }
    output.push_str("\n  }");
    output.push_str(
        ",\n  \"runtime_installation\": {\n    \"status\": \"required\",\n    \"geometry_source\": \"selected_entry_provider\",\n    \"predicate\": \"no_wrap\",\n    \"admission_order\": \"validate_geometry_and_receiver_reservation_before_consuming_either_grant\"\n  }\n}\n",
    );
    output
}

fn push_program_storage_arrival_json(
    output: &mut String,
    evidence: &super::ProgramStorageEntryEmittedArrivalEvidence,
) {
    use omega_calling_conventions::{IndirectPointerLocation, ValueLocation};

    output.push_str("{\"calling_plan_fingerprint\": \"");
    push_normalized_identity(output, evidence.boundary_contract_fingerprint());
    output.push_str("\", \"roots\": [");
    for (root_index, root) in evidence.roots().iter().enumerate() {
        if root_index > 0 {
            output.push_str(", ");
        }
        let role = match root.role() {
            super::ProgramStorageEntryRootRole::Image => "image",
            super::ProgramStorageEntryRootRole::InitialStorage => "initial_storage",
        };
        output.push_str("{\"role\": ");
        push_json_string(output, role);
        output.push_str(", \"parameter_index\": ");
        output.push_str(&root.arrival_parameter_index().to_string());
        let [
            ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset,
                byte_size,
                alignment,
            },
        ] = root.physical_arrival_placement().locations.as_slice()
        else {
            unreachable!("sealed emitted arrival evidence has one indirect placement")
        };
        let IndirectPointerLocation::Register(register) = pointer else {
            unreachable!("sealed emitted arrival evidence uses an indirect register")
        };
        output.push_str(", \"pointer_register\": ");
        push_json_string(output, &format!("{register:?}"));
        output.push_str(", \"caller_copy_stack_byte_offset\": ");
        output.push_str(
            &copy_stack_byte_offset
                .expect("sealed emitted arrival evidence has a caller copy")
                .to_string(),
        );
        output.push_str(", \"byte_size\": ");
        output.push_str(&byte_size.to_string());
        output.push_str(", \"alignment\": ");
        output.push_str(&alignment.to_string());
        output.push_str(", \"copies\": [");
        for (copy_index, copy) in root.copies().iter().enumerate() {
            if copy_index > 0 {
                output.push_str(", ");
            }
            output.push_str("{\"source_byte_offset\": ");
            output.push_str(&copy.source_byte_offset().to_string());
            output.push_str(", \"stack_byte_offset\": ");
            output.push_str(&copy.caller_copy_stack_byte_offset().to_string());
            output.push_str(", \"selected_instruction_index\": ");
            output.push_str(&copy.selected_instruction_index().to_string());
            output.push_str(", \"section_byte_range\": [");
            output.push_str(&copy.section_byte_range().start.to_string());
            output.push_str(", ");
            output.push_str(&copy.section_byte_range().end.to_string());
            output.push_str("], \"final_bytes\": [");
            for (byte_index, byte) in copy.final_bytes().iter().enumerate() {
                if byte_index > 0 {
                    output.push_str(", ");
                }
                output.push_str(&byte.to_string());
            }
            output.push_str("]}");
        }
        output.push_str("]}");
    }
    output.push_str("]}");
}

fn push_program_storage_parameter_json(
    output: &mut String,
    role: &str,
    parameter: &super::ProgramStorageEntryParameter,
) {
    output.push_str("{\"role\": ");
    push_json_string(output, role);
    output.push_str(", \"parameter_index\": ");
    output.push_str(&parameter.parameter_index().to_string());
    output.push_str(", \"type_identity\": ");
    push_json_string(output, parameter.parameter_type_identity());
    output.push_str(", \"carrier_identity\": ");
    push_json_string(output, parameter.carrier_identity());
    output.push_str(", \"domain\": ");
    push_json_string(output, parameter.domain());
    output.push_str(", \"effective_carry\": ");
    push_carry_policy_json(output, parameter.effective_carry());
    output.push_str(", \"calling_placement\": ");
    output.push_str(&omega_artifacts::value_placement_json(
        parameter.placement(),
    ));
    output.push_str(", \"capture\": {\"destination_byte_offset\": ");
    output.push_str(&parameter.destination_byte_offset().to_string());
    output.push_str(", \"write_range\": {\"start\": ");
    output.push_str(&parameter.write_range().start.to_string());
    output.push_str(", \"end\": ");
    output.push_str(&parameter.write_range().end.to_string());
    output.push_str("}}}");
}

fn push_carry_policy_json(output: &mut String, policy: psi_language_semantics::CarryPolicy) {
    use psi_language_semantics::{CarryAddress, CarryCpu, CarryHostThread, CarrySuspension};

    output.push_str("{\"suspension\": ");
    push_json_string(
        output,
        match policy.suspension {
            CarrySuspension::Forbidden => "forbidden",
            CarrySuspension::Allowed => "allowed",
        },
    );
    output.push_str(", \"cpu\": ");
    push_json_string(
        output,
        match policy.cpu {
            CarryCpu::Origin => "same",
            CarryCpu::Any => "any",
        },
    );
    output.push_str(", \"thread\": ");
    push_json_string(
        output,
        match policy.host_thread {
            CarryHostThread::Origin => "same",
            CarryHostThread::Any => "any",
        },
    );
    output.push_str(", \"address\": ");
    push_json_string(
        output,
        match policy.address {
            CarryAddress::Stable => "stable",
            CarryAddress::Movable => "movable",
        },
    );
    output.push('}');
}

fn push_json_string(output: &mut String, value: &str) {
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                output.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => output.push(character),
        }
    }
    output.push('"');
}

pub(super) fn write_emission_plan(
    options: &CompileOptions,
    plan: &omega_backend_plan::BackendPlan,
    emission_plan: &omega_artifacts::EmissionPlan,
    output_path: Option<&Path>,
) -> Result<(), Vec<Diagnostic>> {
    let writer =
        ArtifactWriter::new(&options.build_dir()).map_err(|diagnostic| vec![diagnostic])?;
    writer
        .write_emission_plan(emission_plan)
        .map_err(|diagnostic| vec![diagnostic])?;
    let disassembly = output_path
        .and_then(|output_path| load_native_disassembly(plan.target, output_path).ok())
        .flatten();
    write_phase_diagram(
        options,
        "12_emission.html",
        &omega_visualizations::emission_html(
            &plan.encoded_machine,
            &plan.machine_instructions,
            &plan.assigned_target_operations,
            &plan.control_flow,
            &plan.object,
            &plan.relocations,
            disassembly.as_deref(),
        ),
    )
}

fn load_native_disassembly(
    target: omega_target::NativeTarget,
    output_path: &Path,
) -> Result<Option<String>, Diagnostic> {
    match target.object_format {
        omega_target::ObjectFormat::MachO => {
            run_disassembler("otool", &["-tvV"], output_path).map(Some)
        }
        omega_target::ObjectFormat::Elf | omega_target::ObjectFormat::Coff => {
            for tool in ["llvm-objdump", "objdump"] {
                if let Ok(output) = run_disassembler(tool, &["-d"], output_path) {
                    return Ok(Some(output));
                }
            }
            Ok(None)
        }
    }
}

fn run_disassembler(tool: &str, args: &[&str], output_path: &Path) -> Result<String, Diagnostic> {
    let output = std::process::Command::new(tool)
        .args(args)
        .arg(output_path)
        .output()
        .map_err(|error| {
            Diagnostic::error(format!(
                "failed to run disassembler `{tool}` on {}: {error}",
                output_path.display()
            ))
        })?;
    if !output.status.success() {
        return Err(Diagnostic::error(format!(
            "disassembler `{tool}` failed on {} with status {}",
            output_path.display(),
            output.status
        )));
    }
    String::from_utf8(output.stdout).map_err(|error| {
        Diagnostic::error(format!(
            "disassembler `{tool}` produced non-utf8 output for {}: {error}",
            output_path.display()
        ))
    })
}

pub(super) fn write_timings(
    options: &CompileOptions,
    timings: &[PhaseTiming],
) -> Result<(), Vec<Diagnostic>> {
    let writer =
        ArtifactWriter::new(&options.build_dir()).map_err(|diagnostic| vec![diagnostic])?;
    writer
        .write_timings(timings)
        .map_err(|diagnostic| vec![diagnostic])
}

pub(super) fn remove_stale_phase_diagrams(options: &CompileOptions) -> Result<(), Vec<Diagnostic>> {
    let writer =
        ArtifactWriter::new(&options.build_dir()).map_err(|diagnostic| vec![diagnostic])?;
    writer
        .remove_files([
            "02_syntax_trees.mmd",
            "03_symbol_resolved_trees.mmd",
            "04_typed_trees.mmd",
            "04_wire_protocols.txt",
            "00_timings.txt",
            "00_timings.html",
            "01_sources.txt",
            "01_sources.html",
            "02_ast.txt",
            "02_ast.html",
            "03_resolve.txt",
            "04_types.txt",
            "05_typed_program.txt",
            "06_validation.txt",
            "07_graph.txt",
            "08_proof.txt",
            "09_backend_plan.txt",
            "09_backend_report.txt",
            "09_backend_report.html",
            "09_native_plan.txt",
            "backend_report.txt",
            "slots.txt",
            "08_abstract_operations.html",
            "08_boundary_footprints.json",
            "09_target_operations.html",
            "10_assigned_target_operations.html",
            "10_program_storage_entry.json",
            omega_program_storage::PROGRAM_STORAGE_INSTALLATION_ARTIFACT,
            "11_machine_instructions.html",
            "backend_report.html",
            "10_boundary.txt",
            "10_boundary.html",
            "11_emission.txt",
            "11_emission.html",
            "12_emission.txt",
            "12_emission.html",
            "12_emitted_output.txt",
            "12_emitted_output.html",
            "13_executable_regions.json",
            "13_finalization.txt",
            "13_finalization.html",
            "13_emitted_output.txt",
            "13_emitted_output.html",
            "14_finalization.txt",
            "14_finalization.html",
        ])
        .map_err(|diagnostic| vec![diagnostic])
}

fn write_phase_json(
    options: &CompileOptions,
    file_name: &str,
    contents: &str,
) -> Result<(), Vec<Diagnostic>> {
    write_phase_text(options, file_name, contents)
}

fn write_phase_diagram(
    options: &CompileOptions,
    file_name: &str,
    contents: &str,
) -> Result<(), Vec<Diagnostic>> {
    write_phase_text(options, file_name, contents)
}

fn write_phase_text(
    options: &CompileOptions,
    file_name: &str,
    contents: &str,
) -> Result<(), Vec<Diagnostic>> {
    let writer =
        ArtifactWriter::new(&options.build_dir()).map_err(|diagnostic| vec![diagnostic])?;
    writer
        .write_text(file_name, contents)
        .map_err(|diagnostic| vec![diagnostic])
}

fn json_diagnostic(error: impl std::fmt::Display) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "failed to serialize phase snapshot: {error}"
    ))]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_report_observation_captures_only_full_native_compilations_exactly() {
        let program = psi_checked_trees::CheckedTrees::default();
        let expected = omega_artifacts::build_backend_surface_report(&program, None);

        let captured =
            BackendReportObservation::capture(&program, None, ArtifactEmissionPolicy::Full, true);
        assert_eq!(captured.surface, Some(expected));

        for (policy, requires_native_backend) in [
            (ArtifactEmissionPolicy::OutputOnly, true),
            (ArtifactEmissionPolicy::Full, false),
            (ArtifactEmissionPolicy::OutputOnly, false),
        ] {
            assert!(
                BackendReportObservation::capture(&program, None, policy, requires_native_backend,)
                    .surface
                    .is_none()
            );
        }
    }

    #[test]
    fn output_only_suppresses_every_final_observation_roster() {
        let output_path = Path::new("exact-output");
        for disposition in [
            FinalPipelineObservationDisposition::CheckedOnly,
            FinalPipelineObservationDisposition::InstalledOutput {
                has_storage_bridge: true,
                output_path,
            },
            FinalPipelineObservationDisposition::UnpublishedNative,
        ] {
            assert!(
                final_pipeline_observation_steps(ArtifactEmissionPolicy::OutputOnly, disposition,)
                    .is_empty()
            );
        }
    }

    #[test]
    fn full_checked_and_unpublished_rosters_preserve_their_distinct_boundaries() {
        assert_eq!(
            final_pipeline_observation_steps(
                ArtifactEmissionPolicy::Full,
                FinalPipelineObservationDisposition::CheckedOnly,
            ),
            [FinalPipelineObservationStep::PipelineShell]
        );
        assert_eq!(
            final_pipeline_observation_steps(
                ArtifactEmissionPolicy::Full,
                FinalPipelineObservationDisposition::UnpublishedNative,
            ),
            [
                FinalPipelineObservationStep::Emission { output_path: None },
                FinalPipelineObservationStep::PipelineShell,
            ]
        );
    }

    #[test]
    fn installed_output_roster_retains_the_exact_path_and_optional_bridge() {
        let output_path = Path::new("exact-output");
        assert_eq!(
            final_pipeline_observation_steps(
                ArtifactEmissionPolicy::Full,
                FinalPipelineObservationDisposition::InstalledOutput {
                    has_storage_bridge: true,
                    output_path,
                },
            ),
            [
                FinalPipelineObservationStep::ProgramStorageEntry,
                FinalPipelineObservationStep::Emission {
                    output_path: Some(output_path),
                },
                FinalPipelineObservationStep::Timings,
                FinalPipelineObservationStep::PipelineShell,
            ]
        );
        assert_eq!(
            final_pipeline_observation_steps(
                ArtifactEmissionPolicy::Full,
                FinalPipelineObservationDisposition::InstalledOutput {
                    has_storage_bridge: false,
                    output_path,
                },
            ),
            [
                FinalPipelineObservationStep::Emission {
                    output_path: Some(output_path),
                },
                FinalPipelineObservationStep::Timings,
                FinalPipelineObservationStep::PipelineShell,
            ]
        );
    }
}
