use std::fs;
use std::path::{Path, PathBuf};

use crate::ast::item::Item;
use crate::pipeline::compile::{LoadedFile, LoadedProgram, PhaseTiming};
use crate::pipeline::trust::TrustReport;
use omega_core::diagnostics::Diagnostic;
use omega_effects::{EffectPlan, StateEffects};
use omega_graph::{SourceGraphReport, SourceGraphState};
use omega_names::ResolveReport;
use omega_native::NativeSurfaceReport;
use omega_native::abi::{
    HostBinding, HostBindingMechanism, PlatformCallData, PlatformCallLowering,
};
use omega_native::control_flow::{
    ControlFlowPlan, Operation, PlannedTransitionTarget, StateFlow, TransitionFlow,
};
use omega_native::data::NativeDataObject;
use omega_native::emission::EmissionPlan;
use omega_native::emitter::EmittedNativeOutput;
use omega_native::executable_finalization::{ExecutableFinalization, ExecutableFinalizationStatus};
use omega_native::host_calls::{
    HostCall, HostCallArgument, HostCallArgumentKind, LoweredHostOperation,
};
use omega_native::instructions::{
    FunctionInstructionPlan, InstructionOperandKind, SelectedInstruction, SelectedInstructionKind,
};
use omega_native::layout::{DataShape, FieldLayout};
use omega_native::machine_code::{MachineFunctionCode, MachineInstruction};
use omega_native::object::{SectionPlan, SymbolPlan};
use omega_native::plan::NativePlan;
use omega_native::relocations::RelocationRecord;
use omega_native::runtime_dispatch::branching::{
    RuntimeLeafBranchOperation, RuntimeLeafBranchOperationKind, RuntimeStraightLineBranchOperation,
    RuntimeStraightLineBranchOperationKind,
};
use omega_native::runtime_flow::RuntimeTransitionTarget;
use omega_native::state_schedule::build_entry_state_schedule;
use omega_proof::ProofSurfaceReport;
use omega_proof::obligations::{ProofObligation, ProofPlan};
use omega_typed_program::Program;
use omega_typed_program::data::DataMember;
use omega_typed_program::statement::TransitionGuard;
use omega_types::TypeSurfaceReport;

pub(crate) struct ArtifactWriter {
    root: PathBuf,
}

impl ArtifactWriter {
    pub(crate) fn new(build_dir: &Path) -> Result<Self, Diagnostic> {
        let root = build_dir.to_path_buf();
        fs::create_dir_all(&root).map_err(|error| {
            Diagnostic::error(format!(
                "failed to create artifact directory {}: {error}",
                root.display()
            ))
        })?;

        Ok(Self { root })
    }

    pub(crate) fn write_sources(&self, loaded_program: &LoadedProgram) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega Source Load\n\n");
        output.push_str(&format!("files: {}\n", loaded_program.files.len()));
        output.push_str(&format!("items: {}\n\n", loaded_program.items.len()));

        for file in &loaded_program.files {
            write_loaded_file(&mut output, file);
        }

        self.write("01_sources.txt", &output)
    }

    pub(crate) fn write_ast(&self, loaded_program: &LoadedProgram) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega AST\n\n");
        output.push_str(&format!("files: {}\n", loaded_program.files.len()));
        output.push_str(&format!("items: {}\n\n", loaded_program.items.len()));

        for file in &loaded_program.files {
            write_ast_file(&mut output, loaded_program, file);
        }

        self.write("02_ast.txt", &output)
    }

    pub(crate) fn write_resolve_report(
        &self,
        resolve_report: &ResolveReport,
    ) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega Resolve\n\n");
        output.push_str(&format!(
            "definitions: {}\n",
            resolve_report.definitions.len()
        ));
        output.push_str(&format!("imports: {}\n", resolve_report.imports.len()));
        output.push_str(&format!(
            "references: {}\n\n",
            resolve_report.references.len()
        ));

        output.push_str("## Imports\n");
        for (_, import) in resolve_report.imports.iter() {
            output.push_str(&format!("- {}\n", import.path));
        }

        output.push_str("\n## Definitions\n");
        for (_, definition) in resolve_report.definitions.iter() {
            output.push_str(&format!("- {:?} `{}`\n", definition.kind, definition.name));
        }

        output.push_str("\n## References\n");
        for (_, reference) in resolve_report.references.iter() {
            output.push_str(&format!(
                "- {:?} `{}` from {}\n",
                reference.kind, reference.name, reference.owner
            ));
        }

        self.write("03_resolve.txt", &output)
    }

    pub(crate) fn write_typed_program(&self, program: &Program) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega Typed Program\n\n");
        output.push_str(&format!(
            "data definitions: {}\n",
            program.data_definitions.len()
        ));
        output.push_str(&format!(
            "invariant definitions: {}\n",
            program.invariant_definitions.len()
        ));
        output.push_str(&format!("platforms: {}\n", program.platforms.len()));
        output.push_str(&format!("machines: {}\n", program.machines.len()));
        output.push_str(&format!(
            "type constraints: {}\n\n",
            program.type_constraints.len()
        ));

        write_typed_program_data_definitions(&mut output, program);
        write_typed_program_invariants(&mut output, program);
        write_typed_program_platforms(&mut output, program);
        write_typed_program_machines(&mut output, program);

        self.write("05_typed_program.txt", &output)
    }

    pub(crate) fn write_type_surface_and_effects(
        &self,
        type_surface: &TypeSurfaceReport,
        effect_plan: &EffectPlan,
    ) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega Types And Effects\n\n");
        output.push_str(&format!(
            "type declarations: {}\n",
            type_surface.declarations.len()
        ));
        output.push_str(&format!(
            "type references: {}\n",
            type_surface.references.len()
        ));
        output.push_str(&format!(
            "effect machines: {}\n",
            effect_plan.machines.len()
        ));
        output.push_str(&format!("effect states: {}\n\n", effect_plan.states.len()));

        output.push_str("## Type Declarations\n");
        for (_, declaration) in type_surface.declarations.iter() {
            output.push_str(&format!(
                "- {:?} `{}`\n",
                declaration.kind, declaration.name
            ));
        }

        output.push_str("\n## Type References\n");
        for (_, reference) in type_surface.references.iter() {
            output.push_str(&format!(
                "- {:?} `{}` from {}\n",
                reference.kind, reference.name, reference.owner
            ));
        }

        output.push_str("\n## Effects\n");

        for machine in &effect_plan.machines {
            output.push_str(&format!("## machine {}\n", machine.name));

            let Some(states) = effect_plan.states.span(machine.states) else {
                output.push_str("invalid state span\n\n");
                continue;
            };

            for state in states {
                write_state_effect(&mut output, state);
            }

            output.push('\n');
        }

        self.write("04_types.txt", &output)
    }

    pub(crate) fn write_validation(&self, program: &Program) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega Validation\n\n");
        output.push_str("status: ok\n");
        output.push_str(&format!(
            "data definitions: {}\n",
            program.data_definitions.len()
        ));
        output.push_str(&format!("platforms: {}\n", program.platforms.len()));
        output.push_str(&format!("machines: {}\n", program.machines.len()));

        self.write("06_validation.txt", &output)
    }

    pub(crate) fn write_graphs(
        &self,
        source_graph: &SourceGraphReport,
        control_flow: &ControlFlowPlan,
    ) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega Graphs\n\n");
        output.push_str(&format!(
            "source machines: {}\n",
            source_graph.machines.len()
        ));
        output.push_str(&format!("source states: {}\n", source_graph.states.len()));
        output.push_str(&format!(
            "source transitions: {}\n",
            source_graph.transitions.len()
        ));
        output.push_str(&format!(
            "control-flow machines: {}\n",
            control_flow.machines.len()
        ));
        output.push_str(&format!(
            "control-flow states: {}\n",
            control_flow.states.len()
        ));
        output.push_str(&format!("operations: {}\n", control_flow.operations.len()));
        output.push_str(&format!(
            "control-flow transitions: {}\n\n",
            control_flow.transitions.len()
        ));

        output.push_str("## Source Graph\n");
        for (_, machine) in source_graph.machines.iter() {
            output.push_str(&format!("### machine {}\n", machine.name));

            let Some(states) = source_graph.states.span(machine.states) else {
                output.push_str("invalid state span\n\n");
                continue;
            };

            for state in states {
                write_source_graph_state(&mut output, source_graph, state);
            }
        }

        output.push_str("\n## Lowered Control Flow\n");
        for (_, machine) in control_flow.machines.iter() {
            output.push_str(&format!("### machine {}\n", machine.name));

            let Some(states) = control_flow.states.span(machine.states) else {
                output.push_str("invalid state span\n\n");
                continue;
            };

            for state in states {
                write_state_flow(&mut output, control_flow, state);
            }

            output.push('\n');
        }

        self.write("07_graph.txt", &output)
    }

    pub(crate) fn write_proof_report(
        &self,
        proof_surface: &ProofSurfaceReport,
        proof_plan: &ProofPlan,
    ) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega Proof Plan\n\n");
        output.push_str(&format!(
            "source invariants: {}\n",
            proof_surface.invariants.len()
        ));
        output.push_str(&format!(
            "source bounded sites: {}\n",
            proof_surface.bounded_sites.len()
        ));
        output.push_str(&format!(
            "lowered obligations: {}\n",
            proof_plan.obligations.len()
        ));
        output.push_str(&format!(
            "constraints: {}\n\n",
            proof_plan.type_constraints.len()
        ));

        output.push_str("## Source Invariants\n");
        for (_, invariant) in proof_surface.invariants.iter() {
            output.push_str(&format!(
                "- invariant {} = {}\n",
                invariant.name, invariant.constraints
            ));
        }

        output.push_str("\n## Source Bounded Sites\n");
        for (_, bounded_site) in proof_surface.bounded_sites.iter() {
            output.push_str(&format!(
                "- {} : {} {}\n",
                bounded_site.owner, bounded_site.base_type, bounded_site.constraints
            ));
        }

        output.push_str("\n## Lowered Obligations\n");
        for obligation in &proof_plan.obligations {
            write_proof_obligation(&mut output, proof_plan, obligation);
        }

        self.write("08_proof.txt", &output)
    }

    pub(crate) fn write_trust_report(&self, trust_report: &TrustReport) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega Trust\n\n");
        output.push_str(&format!("targets: {}\n", trust_report.targets.len()));
        output.push_str(&format!(
            "trust roots: {}\n",
            trust_report.trust_roots.len()
        ));
        output.push_str(&format!(
            "trusted contracts: {}\n",
            trust_report.trusted_contracts.len()
        ));
        output.push_str(&format!(
            "unresolved trusts: {}\n",
            trust_report.unresolved_trusts.len()
        ));
        output.push_str(&format!(
            "unchecked policies: {}\n\n",
            trust_report.unchecked_policies.len()
        ));

        output.push_str("## Targets\n");
        if trust_report.targets.is_empty() {
            output.push_str("none\n");
        } else {
            for (_, target) in trust_report.targets.iter() {
                output.push_str(&format!(
                    "- target `{}` host `{}` settings {} checked trusts {} unchecked trusts {}\n",
                    target.name,
                    target.host_provider,
                    target.host_settings,
                    target.checked_trusts,
                    target.unchecked_trusts
                ));
            }
        }

        output.push_str("\n## Trust Roots\n");
        if trust_report.trust_roots.is_empty() {
            output.push_str("none\n");
        } else {
            for (_, root) in trust_report.trust_roots.iter() {
                output.push_str(&format!(
                    "- trust `{}` body tokens {}\n",
                    root.name, root.token_count
                ));
            }
        }

        output.push_str("\n## Trusted Contracts\n");
        if trust_report.trusted_contracts.is_empty() {
            output.push_str("none\n");
        } else {
            for (_, contract) in trust_report.trusted_contracts.iter() {
                output.push_str(&format!(
                    "- {}.{} trusts `{}` ({}) requires {} ensures {}\n",
                    contract.capability,
                    contract.state,
                    contract.trust_level,
                    if contract.resolved {
                        "resolved"
                    } else {
                        "unresolved"
                    },
                    contract.requires_count,
                    contract.ensures_count
                ));
            }
        }

        output.push_str("\n## Unresolved Trusts\n");
        if trust_report.unresolved_trusts.is_empty() {
            output.push_str("none\n");
        } else {
            for (_, unresolved) in trust_report.unresolved_trusts.iter() {
                output.push_str(&format!(
                    "- {}.{} trust `{}`\n",
                    unresolved.capability, unresolved.state, unresolved.trust_level
                ));
            }
        }

        output.push_str("\n## Unchecked Policies\n");
        if trust_report.unchecked_policies.is_empty() {
            output.push_str("none\n");
        } else {
            for (_, policy) in trust_report.unchecked_policies.iter() {
                output.push_str(&format!(
                    "- target `{}` trusts unchecked `{}`\n",
                    policy.target, policy.name
                ));
            }
        }

        self.write("10_trust.txt", &output)
    }

    pub(crate) fn write_native_report(
        &self,
        native_surface: &NativeSurfaceReport,
        native_plan: &NativePlan,
    ) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega Native Plan\n\n");
        output.push_str(&format!("target: {:?}\n", native_plan.target));
        output.push_str(&format!(
            "entry: {}.{} as `{}`\n\n",
            native_plan.entry_machine, native_plan.entry_state, native_plan.object.entry_symbol
        ));

        output.push_str("## Host ABI\n");
        output.push_str(&format!(
            "bindings: {}\n",
            native_plan.host_abi.bindings.len()
        ));
        for (_, binding) in native_plan.host_abi.bindings.iter() {
            write_host_binding(&mut output, binding);
        }
        output.push_str(&format!(
            "platform lowerings: {}\n",
            native_plan.host_abi.platform_call_lowerings.len()
        ));
        for (_, lowering) in native_plan.host_abi.platform_call_lowerings.iter() {
            write_platform_call_lowering(&mut output, native_plan, lowering);
        }
        output.push('\n');

        output.push_str("## Host Call Lowering\n");
        output.push_str(&format!("calls: {}\n", native_plan.host_calls.calls.len()));
        output.push_str(&format!(
            "unsupported calls: {}\n",
            native_plan.host_calls.unsupported_calls.len()
        ));
        output.push_str(&format!(
            "operations: {}\n",
            native_plan.host_calls.operations.len()
        ));
        if native_plan.host_calls.calls.is_empty() {
            output.push_str("none\n");
        } else {
            for (_, call) in native_plan.host_calls.calls.iter() {
                write_host_call(&mut output, native_plan, call);
            }
        }
        if !native_plan.host_calls.unsupported_calls.is_empty() {
            output.push_str("unsupported:\n");
            for (_, unsupported_call) in native_plan.host_calls.unsupported_calls.iter() {
                output.push_str(&format!(
                    "- {}.{} statement {} `{}`: {}\n",
                    unsupported_call.machine,
                    unsupported_call.state,
                    unsupported_call.statement_index,
                    unsupported_call.platform_call,
                    unsupported_call.reason
                ));
            }
        }
        output.push('\n');

        output.push_str("## State Call Lowering\n");
        output.push_str(&format!("calls: {}\n", native_plan.state_calls.calls.len()));
        if native_plan.state_calls.calls.is_empty() {
            output.push_str("none\n");
        } else {
            for (_, state_call) in native_plan.state_calls.calls.iter() {
                output.push_str(&format!(
                    "- {}.{} statement {} `{}` -> {}.{} args {} {:?}/{:?} reachable {} required {}\n",
                    state_call.source_machine,
                    state_call.source_state,
                    state_call.statement_index,
                    state_call.receiver,
                    if state_call.target_machine.is_empty() {
                        "unresolved"
                    } else {
                        state_call.target_machine.as_str()
                    },
                    state_call.target_state,
                    state_call.argument_count,
                    state_call.resolution,
                    state_call.lowering,
                    state_call.reachable,
                    state_call.required
                ));

                match native_plan.state_calls.arguments.span(state_call.arguments) {
                    Some(arguments) if arguments.is_empty() => {
                        output.push_str("  arguments: none\n");
                    }
                    Some(arguments) => {
                        output.push_str("  arguments:\n");
                        for argument in arguments {
                            output.push_str(&format!(
                                "    - #{} `{}` {:?}: `{}` required {}\n",
                                argument.index,
                                argument.parameter_name,
                                argument.kind,
                                argument.expression.display_name(),
                                argument.required
                            ));
                        }
                    }
                    None => output.push_str("  arguments: invalid span\n"),
                }
            }
        }
        output.push('\n');

        output.push_str("## Alias Flow\n");
        output.push_str(&format!(
            "aliases: {}\n",
            native_plan.alias_flow.aliases.len()
        ));
        if native_plan.alias_flow.aliases.is_empty() {
            output.push_str("none\n");
        } else {
            for (_, alias) in native_plan.alias_flow.aliases.iter() {
                output.push_str(&format!(
                    "- {}.{} statement {} -> {}.{} `{}` aliases `{}` required {}\n",
                    alias.caller_machine,
                    alias.caller_state,
                    alias.statement_index,
                    alias.callee_machine,
                    alias.callee_state,
                    alias.parameter_name,
                    alias.argument.display_name(),
                    alias.required
                ));
            }
        }
        output.push('\n');

        output.push_str("## State Storage\n");
        output.push_str(&format!(
            "locals: {}\n",
            native_plan.state_storage.locals.len()
        ));
        for (_, local) in native_plan.state_storage.locals.iter() {
            output.push_str(&format!(
                "- {}.{} statement {} local `{}`: {} required {}\n",
                local.machine,
                local.state,
                local.statement_index,
                local.name,
                local.type_name,
                local.required
            ));
        }
        output.push_str(&format!(
            "mutations: {}\n",
            native_plan.state_storage.mutations.len()
        ));
        for (_, mutation) in native_plan.state_storage.mutations.iter() {
            output.push_str(&format!(
                "- {}.{} statement {} {:?}/{:?}: `{}` = `{}` required {}\n",
                mutation.machine,
                mutation.state,
                mutation.statement_index,
                mutation.mutation_kind,
                mutation.lowering,
                mutation.target.display_name(),
                mutation.value.display_name(),
                mutation.required
            ));
        }
        output.push('\n');

        output.push_str("## Runtime Storage\n");
        output.push_str(&format!(
            "frame slots: {}\n",
            native_plan.runtime_storage.frame_slots.len()
        ));
        for (_, slot) in native_plan.runtime_storage.frame_slots.iter() {
            output.push_str(&format!(
                "- #{} {}.{} statement {} local `{}`: {} offset {} bytes {} align {}\n",
                slot.dispatch_index,
                slot.source_machine,
                slot.source_state,
                slot.statement_index,
                slot.name,
                slot.type_name,
                slot.byte_offset,
                slot.byte_size,
                slot.alignment
            ));
        }
        output.push_str(&format!(
            "writes: {}\n",
            native_plan.runtime_storage.writes.len()
        ));
        for (_, write) in native_plan.runtime_storage.writes.iter() {
            output.push_str(&format!(
                "- #{} {}.{} statement {} {:?}/{:?}: `{}` = `{}`\n",
                write.dispatch_index,
                write.source_machine,
                write.source_state,
                write.statement_index,
                write.mutation_kind,
                write.lowering,
                write.target.display_name(),
                write.value.display_name()
            ));
        }
        output.push('\n');

        output.push_str("## State Values\n");
        output.push_str(&format!(
            "values: {}\n",
            native_plan.state_values.values.len()
        ));
        for (_, value) in native_plan.state_values.values.iter() {
            output.push_str(&format!(
                "- {}.{} statement {} {:?}/{:?}: `{}` required {}\n",
                value.machine,
                value.state,
                value.statement_index,
                value.role,
                value.kind,
                value.expression.display_name(),
                value.required
            ));
        }
        output.push('\n');

        output.push_str("## Runtime Text\n");
        output.push_str(&format!("uses: {}\n", native_plan.runtime_text.uses.len()));
        output.push_str(&format!(
            "buffers: {}\n",
            native_plan.runtime_text.buffers.len()
        ));
        output.push_str(&format!(
            "slots: {}\n",
            native_plan.runtime_text.slots.len()
        ));
        output.push_str(&format!(
            "writes: {}\n",
            native_plan.runtime_text.writes.len()
        ));
        output.push_str(&format!(
            "builders: {}\n",
            native_plan.runtime_text.builders.len()
        ));
        output.push_str(&format!(
            "builder segments: {}\n",
            native_plan.runtime_text.builder_segments.len()
        ));
        if native_plan.runtime_text.uses.is_empty() {
            output.push_str("uses: none\n");
        } else {
            for (_, text_use) in native_plan.runtime_text.uses.iter() {
                output.push_str(&format!(
                    "- {}.{} statement {} `{}` {:?} newline {}\n",
                    text_use.machine,
                    text_use.state,
                    text_use.statement_index,
                    text_use.expression.display_name(),
                    text_use.source,
                    text_use.append_newline
                ));
            }
        }
        if native_plan.runtime_text.buffers.is_empty() {
            output.push_str("buffers: none\n");
        } else {
            for (_, text_buffer) in native_plan.runtime_text.buffers.iter() {
                output.push_str(&format!(
                    "- buffer {}.{} statement {} `{}` bytes {}\n",
                    text_buffer.machine,
                    text_buffer.state,
                    text_buffer.statement_index,
                    text_buffer.target.display_name(),
                    text_buffer.byte_capacity
                ));
            }
        }
        if native_plan.runtime_text.slots.is_empty() {
            output.push_str("slots: none\n");
        } else {
            for (_, text_slot) in native_plan.runtime_text.slots.iter() {
                output.push_str(&format!(
                    "- slot `{}` bytes {} input_buffer {}\n",
                    text_slot.place.display_name(),
                    text_slot.byte_capacity,
                    text_slot.has_input_buffer
                ));
            }
        }
        if native_plan.runtime_text.writes.is_empty() {
            output.push_str("writes: none\n");
        } else {
            for (_, text_write) in native_plan.runtime_text.writes.iter() {
                output.push_str(&format!(
                    "- write {}.{} statement {} `{}` = `{}` {:?}\n",
                    text_write.machine,
                    text_write.state,
                    text_write.statement_index,
                    text_write.target.display_name(),
                    text_write.value.display_name(),
                    text_write.kind
                ));
            }
        }
        if native_plan.runtime_text.builders.is_empty() {
            output.push_str("builders: none\n");
        } else {
            for (_, text_builder) in native_plan.runtime_text.builders.iter() {
                output.push_str(&format!(
                    "- builder {}.{} statement {} `{}` segments {}\n",
                    text_builder.machine,
                    text_builder.state,
                    text_builder.statement_index,
                    text_builder.target.display_name(),
                    text_builder.segments.count()
                ));
                if let Some(segments) = native_plan
                    .runtime_text
                    .builder_segments
                    .span(text_builder.segments)
                {
                    for segment in segments {
                        output.push_str(&format!(
                            "  - segment `{}` {:?}\n",
                            segment.expression.display_name(),
                            segment.kind
                        ));
                    }
                }
            }
        }
        output.push('\n');

        output.push_str("## Native Data\n");
        output.push_str(&format!("objects: {}\n", native_plan.data.objects.len()));
        output.push_str(&format!("bytes: {}\n", native_plan.data.bytes.len()));
        if native_plan.data.objects.is_empty() {
            output.push_str("none\n");
        } else {
            for (_, data_object) in native_plan.data.objects.iter() {
                write_native_data_object(&mut output, native_plan, data_object);
            }
        }
        output.push('\n');

        output.push_str("## Instruction Selection\n");
        output.push_str(&format!(
            "functions: {}\n",
            native_plan.instructions.functions.len()
        ));
        output.push_str(&format!(
            "instructions: {}\n",
            native_plan.instructions.instructions.len()
        ));
        output.push_str(&format!(
            "operands: {}\n",
            native_plan.instructions.operands.len()
        ));
        for (_, function) in native_plan.instructions.functions.iter() {
            write_function_instruction_plan(&mut output, native_plan, function);
        }
        output.push('\n');

        output.push_str("## Machine Code Shape\n");
        output.push_str(&format!(
            "functions: {}\n",
            native_plan.machine_code.functions.len()
        ));
        output.push_str(&format!(
            "instructions: {}\n",
            native_plan.machine_code.instructions.len()
        ));
        output.push_str(&format!(
            "encoded bytes: {}\n",
            native_plan.machine_code.bytes.len()
        ));
        output.push_str(&format!("bytes: {}\n", native_plan.machine_code.byte_count));
        for (_, function) in native_plan.machine_code.functions.iter() {
            write_machine_function_code(&mut output, native_plan, function);
        }
        output.push('\n');

        output.push_str("## Source Native Surface\n");
        output.push_str(&format!(
            "entry candidates: {}\n",
            native_surface.entry_points.len()
        ));
        for (_, entry_point) in native_surface.entry_points.iter() {
            output.push_str(&format!(
                "- entry {}.{}\n",
                entry_point.machine, entry_point.state
            ));
        }

        output.push_str(&format!("platforms: {}\n", native_surface.platforms.len()));
        for (_, platform) in native_surface.platforms.iter() {
            output.push_str(&format!(
                "- platform {}: {} state(s)\n",
                platform.name, platform.states
            ));
        }

        output.push_str(&format!("machines: {}\n", native_surface.machines.len()));
        for (_, machine) in native_surface.machines.iter() {
            output.push_str(&format!(
                "- machine {}: contains {}, owned data {}, states {}\n",
                machine.name, machine.contained_objects, machine.owned_data, machine.states
            ));
        }
        output.push('\n');

        output.push_str("## State Schedule\n");
        match build_entry_state_schedule(native_plan) {
            Ok(schedule) if schedule.is_empty() => output.push_str("states: 0\nnone\n"),
            Ok(schedule) => {
                output.push_str(&format!("states: {}\n", schedule.len()));
                for scheduled_state in schedule {
                    output.push_str(&format!(
                        "- {}.{}\n",
                        scheduled_state.machine, scheduled_state.state
                    ));
                }
            }
            Err(reason) => {
                output.push_str("status: blocked\n");
                output.push_str(&format!("reason: {reason}\n"));
            }
        }

        output.push_str("\n## Runtime State Flow\n");
        output.push_str(&format!(
            "states: {}\n",
            native_plan.runtime_flow.states.len()
        ));
        output.push_str(&format!(
            "edges: {}\n",
            native_plan.runtime_flow.edges.len()
        ));
        output.push_str(&format!(
            "cycles: {}\n",
            native_plan.runtime_flow.cycles.len()
        ));
        if native_plan.runtime_flow.states.is_empty() {
            output.push_str("none\n");
        } else {
            output.push_str("states:\n");
            for (_, state) in native_plan.runtime_flow.states.iter() {
                output.push_str(&format!("- {}.{}\n", state.machine, state.state));
            }
        }
        if !native_plan.runtime_flow.edges.is_empty() {
            output.push_str("edges:\n");
            for (_, edge) in native_plan.runtime_flow.edges.iter() {
                output.push_str(&format!(
                    "- {}.{} -> {} {}",
                    edge.from_machine,
                    edge.from_state,
                    runtime_transition_target_name(&edge.target),
                    transition_guard_name(&edge.guard)
                ));

                if edge.continuation != RuntimeTransitionTarget::None {
                    output.push_str(&format!(
                        " -> {}",
                        runtime_transition_target_name(&edge.continuation)
                    ));
                }

                if edge.forms_cycle {
                    output.push_str(" [cycle]");
                }

                output.push('\n');
            }
        }
        if !native_plan.runtime_flow.cycles.is_empty() {
            output.push_str("cycle paths:\n");
            for (_, cycle) in native_plan.runtime_flow.cycles.iter() {
                match native_plan.runtime_flow.cycle_states.span(cycle.states) {
                    Some(states) => {
                        let path = states
                            .iter()
                            .map(|state| format!("{}.{}", state.machine, state.state))
                            .collect::<Vec<_>>()
                            .join(" -> ");
                        output.push_str(&format!("- {path}\n"));
                    }
                    None => output.push_str("- invalid cycle span\n"),
                }
            }
        }

        output.push_str("\n## Runtime Dispatch\n");
        output.push_str(&format!(
            "states: {}\n",
            native_plan.state_dispatch.states.len()
        ));
        output.push_str(&format!(
            "edges: {}\n",
            native_plan.state_dispatch.edges.len()
        ));
        if native_plan.state_dispatch.states.is_empty() {
            output.push_str("none\n");
        } else {
            for (_, state) in native_plan.state_dispatch.states.iter() {
                output.push_str(&format!(
                    "- #{} {}.{} label `{}`\n",
                    state.dispatch_index, state.machine, state.state, state.label
                ));

                match native_plan.state_dispatch.edges.span(state.edges) {
                    Some(edges) if edges.is_empty() => output.push_str("  edges: none\n"),
                    Some(edges) => {
                        output.push_str("  edges:\n");
                        for edge in edges {
                            output.push_str(&format!(
                                "    - -> #{} {} {}",
                                edge.target_dispatch_index,
                                runtime_transition_target_name(&edge.target),
                                transition_guard_name(&edge.guard)
                            ));

                            if edge.continuation != RuntimeTransitionTarget::None {
                                output.push_str(&format!(
                                    " -> #{} {}",
                                    edge.continuation_dispatch_index,
                                    runtime_transition_target_name(&edge.continuation)
                                ));
                            }

                            if edge.forms_cycle {
                                output.push_str(" [cycle]");
                            }

                            output.push('\n');
                        }
                    }
                    None => output.push_str("  edges: invalid span\n"),
                }
            }
        }

        output.push_str("\n## Runtime Guards\n");
        output.push_str(&format!(
            "guards: {}\n",
            native_plan.state_guards.guards.len()
        ));
        output.push_str(&format!(
            "operands: {}\n",
            native_plan.state_guards.operands.len()
        ));
        if native_plan.state_guards.guards.is_empty() {
            output.push_str("none\n");
        } else {
            for (_, guard) in native_plan.state_guards.guards.iter() {
                output.push_str(&format!(
                    "- #{} {}.{} edge {} -> #{} {} {:?}/{:?}/{:?}",
                    guard.source_dispatch_index,
                    guard.source_machine,
                    guard.source_state,
                    guard.statement_order,
                    guard.target_dispatch_index,
                    runtime_transition_target_name(&guard.target),
                    guard.kind,
                    guard.operator,
                    guard.lowering
                ));

                if guard.has_expression {
                    output.push_str(&format!(" `{}`", guard.expression.display_name()));
                }

                if guard.continuation != RuntimeTransitionTarget::None {
                    output.push_str(&format!(
                        " -> #{} {}",
                        guard.continuation_dispatch_index,
                        runtime_transition_target_name(&guard.continuation)
                    ));
                }

                if guard.forms_cycle {
                    output.push_str(" [cycle]");
                }

                output.push('\n');
                if let Some(operands) = native_plan.state_guards.operands.span(guard.operands)
                    && !operands.is_empty()
                {
                    for operand in operands {
                        output.push_str(&format!(
                            "  - operand `{}` {:?} {:?} offset {} bytes {}\n",
                            operand.expression.display_name(),
                            operand.kind,
                            operand.storage,
                            operand.byte_offset,
                            operand.byte_size
                        ));
                        if operand.has_resolved_value {
                            output.push_str(&format!(
                                "    resolved value: {}\n",
                                operand.resolved_value
                            ));
                        }
                    }
                }
            }
        }

        output.push_str("\n## Runtime Dispatch Loop\n");
        output.push_str(&format!(
            "needed: {}\n",
            native_plan.runtime_dispatch_loop.needed
        ));
        output.push_str(&format!(
            "entry dispatch index: #{}\n",
            native_plan.runtime_dispatch_loop.entry_dispatch_index
        ));
        output.push_str(&format!(
            "terminal dispatch index: #{}\n",
            native_plan.runtime_dispatch_loop.terminal_dispatch_index
        ));
        output.push_str(&format!(
            "current state slot: `{}`\n",
            native_plan.runtime_dispatch_loop.current_state_slot
        ));
        output.push_str(&format!(
            "next state slot: `{}`\n",
            native_plan.runtime_dispatch_loop.next_state_slot
        ));
        output.push_str(&format!(
            "cases: {}\n",
            native_plan.runtime_dispatch_loop.cases.len()
        ));
        output.push_str(&format!(
            "edges: {}\n",
            native_plan.runtime_dispatch_loop.edges.len()
        ));
        if native_plan.runtime_dispatch_loop.cases.is_empty() {
            output.push_str("none\n");
        } else {
            for (_, dispatch_case) in native_plan.runtime_dispatch_loop.cases.iter() {
                output.push_str(&format!(
                    "- #{} {}.{} label `{}` operations {}\n",
                    dispatch_case.dispatch_index,
                    dispatch_case.machine,
                    dispatch_case.state,
                    dispatch_case.label,
                    dispatch_case.operation_count
                ));

                match native_plan
                    .runtime_dispatch_loop
                    .edges
                    .span(dispatch_case.edges)
                {
                    Some(edges) if edges.is_empty() => output.push_str("  edges: none\n"),
                    Some(edges) => {
                        output.push_str("  edges:\n");
                        for edge in edges {
                            output.push_str(&format!(
                                "    - #{} -> #{} {} {:?}/{:?} {}",
                                edge.order,
                                edge.target_dispatch_index,
                                runtime_transition_target_name(&edge.target),
                                edge.guard_lowering,
                                edge.action,
                                transition_guard_name(&edge.guard)
                            ));
                            if edge.guard_has_storage {
                                output.push_str(&format!(
                                    " storage offset {} bytes {} expected {}",
                                    edge.guard_byte_offset,
                                    edge.guard_byte_size,
                                    edge.guard_expected_value
                                ));
                            }

                            if edge.continuation != RuntimeTransitionTarget::None {
                                output.push_str(&format!(
                                    " -> #{} {}",
                                    edge.continuation_dispatch_index,
                                    runtime_transition_target_name(&edge.continuation)
                                ));
                            }

                            if edge.forms_cycle {
                                output.push_str(" [cycle]");
                            }

                            output.push('\n');
                        }
                    }
                    None => output.push_str("  edges: invalid span\n"),
                }
            }
        }

        output.push_str("\n## Runtime Bodies\n");
        output.push_str(&format!(
            "bodies: {}\n",
            native_plan.runtime_bodies.bodies.len()
        ));
        output.push_str(&format!(
            "operations: {}\n",
            native_plan.runtime_bodies.operations.len()
        ));
        if native_plan.runtime_bodies.bodies.is_empty() {
            output.push_str("none\n");
        } else {
            for (_, body) in native_plan.runtime_bodies.bodies.iter() {
                output.push_str(&format!(
                    "- #{} {}.{}\n",
                    body.dispatch_index, body.machine, body.state
                ));

                match native_plan.runtime_bodies.operations.span(body.operations) {
                    Some(operations) if operations.is_empty() => {
                        output.push_str("  operations: none\n");
                    }
                    Some(operations) => {
                        output.push_str("  operations:\n");
                        for operation in operations {
                            output.push_str(&format!(
                                "    - {}.{} statement {} {:?}\n",
                                operation.source_machine,
                                operation.source_state,
                                operation.statement_index,
                                operation.kind
                            ));
                        }
                    }
                    None => output.push_str("  operations: invalid span\n"),
                }
            }
        }

        output.push_str("\n## Runtime Branching Calls\n");
        output.push_str(&format!(
            "calls: {}\n",
            native_plan.runtime_branching_calls.calls.len()
        ));
        output.push_str(&format!(
            "edges: {}\n",
            native_plan.runtime_branching_calls.edges.len()
        ));
        if native_plan.runtime_branching_calls.calls.is_empty() {
            output.push_str("none\n");
        } else {
            for (_, call) in native_plan.runtime_branching_calls.calls.iter() {
                output.push_str(&format!(
                    "- #{} {}.{} statement {} -> {}.{} args {}\n",
                    call.dispatch_index,
                    call.source_machine,
                    call.source_state,
                    call.statement_index,
                    call.target_machine,
                    call.target_state,
                    call.argument_count
                ));

                match native_plan.runtime_branching_calls.edges.span(call.edges) {
                    Some(edges) if edges.is_empty() => output.push_str("  edges: none\n"),
                    Some(edges) => {
                        output.push_str(&format!("  expansion: {:?}\n", call.expansion));
                        output.push_str("  edges:\n");
                        for edge in edges {
                            output.push_str(&format!(
                                "    - #{} -> {} {:?} {:?} {}",
                                edge.order,
                                runtime_transition_target_name(&edge.target),
                                edge.lowering,
                                edge.guard_kind,
                                transition_guard_name(&edge.guard)
                            ));

                            let target_arguments = native_plan
                                .runtime_branching_calls
                                .target_arguments
                                .span_or_empty(edge.target_arguments);
                            if !target_arguments.is_empty() {
                                output.push_str(&format!(
                                    " args ({})",
                                    target_arguments
                                        .iter()
                                        .map(|argument| argument.display_name())
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                ));
                            }

                            if edge.continuation != RuntimeTransitionTarget::None {
                                output.push_str(&format!(
                                    " -> {}",
                                    runtime_transition_target_name(&edge.continuation)
                                ));
                            }

                            output.push('\n');
                        }
                    }
                    None => output.push_str("  edges: invalid span\n"),
                }
            }
        }

        output.push_str("\n## Runtime Leaf Branch Expansions\n");
        output.push_str(&format!(
            "expansions: {}\n",
            native_plan.runtime_branching_calls.leaf_expansions.len()
        ));
        output.push_str(&format!(
            "operations: {}\n",
            native_plan.runtime_branching_calls.leaf_operations.len()
        ));
        output.push_str(&format!(
            "bindings: {}\n",
            native_plan.runtime_branching_calls.leaf_bindings.len()
        ));
        if native_plan
            .runtime_branching_calls
            .leaf_expansions
            .is_empty()
        {
            output.push_str("none\n");
        } else {
            for (_, expansion) in native_plan.runtime_branching_calls.leaf_expansions.iter() {
                output.push_str(&format!(
                    "- #{} {}.{} statement {} {}.{} edge {} -> {}.{} {:?} {}\n",
                    expansion.dispatch_index,
                    expansion.source_machine,
                    expansion.source_state,
                    expansion.statement_index,
                    expansion.branch_machine,
                    expansion.branch_state,
                    expansion.edge_order,
                    expansion.leaf_machine,
                    expansion.leaf_state,
                    expansion.guard_kind,
                    transition_guard_name(&expansion.guard)
                ));
                if expansion.resolved_guard != expansion.guard {
                    output.push_str(&format!(
                        "  resolved guard: {}\n",
                        transition_guard_name(&expansion.resolved_guard)
                    ));
                }

                match native_plan
                    .runtime_branching_calls
                    .leaf_bindings
                    .span(expansion.bindings)
                {
                    Some(bindings) if bindings.is_empty() => {
                        output.push_str("  bindings: none\n");
                    }
                    Some(bindings) => {
                        output.push_str("  bindings:\n");
                        for binding in bindings {
                            output.push_str(&format!(
                                "    - {:?} `{}` = `{}`\n",
                                binding.kind,
                                binding.parameter_name,
                                binding.expression.display_name()
                            ));
                        }
                    }
                    None => output.push_str("  bindings: invalid span\n"),
                }

                match native_plan
                    .runtime_branching_calls
                    .leaf_operations
                    .span(expansion.operations)
                {
                    Some(operations) if operations.is_empty() => {
                        output.push_str("  operations: none\n");
                    }
                    Some(operations) => {
                        output.push_str("  operations:\n");
                        for operation in operations {
                            write_runtime_leaf_branch_operation(&mut output, operation);
                        }
                    }
                    None => output.push_str("  operations: invalid span\n"),
                }
            }
        }

        output.push_str("\n## Runtime Straight-Line Branch Expansions\n");
        output.push_str(&format!(
            "expansions: {}\n",
            native_plan
                .runtime_branching_calls
                .straight_line_expansions
                .len()
        ));
        output.push_str(&format!(
            "operations: {}\n",
            native_plan
                .runtime_branching_calls
                .straight_line_operations
                .len()
        ));
        output.push_str(&format!(
            "bindings: {}\n",
            native_plan
                .runtime_branching_calls
                .straight_line_bindings
                .len()
        ));
        if native_plan
            .runtime_branching_calls
            .straight_line_expansions
            .is_empty()
        {
            output.push_str("none\n");
        } else {
            for (_, expansion) in native_plan
                .runtime_branching_calls
                .straight_line_expansions
                .iter()
            {
                output.push_str(&format!(
                    "- #{} {}.{} statement {} {}.{} edge {} -> {}.{} {:?} {}\n",
                    expansion.dispatch_index,
                    expansion.source_machine,
                    expansion.source_state,
                    expansion.statement_index,
                    expansion.branch_machine,
                    expansion.branch_state,
                    expansion.edge_order,
                    expansion.target_machine,
                    expansion.target_state,
                    expansion.guard_kind,
                    transition_guard_name(&expansion.guard)
                ));
                if expansion.resolved_guard != expansion.guard {
                    output.push_str(&format!(
                        "  resolved guard: {}\n",
                        transition_guard_name(&expansion.resolved_guard)
                    ));
                }

                match native_plan
                    .runtime_branching_calls
                    .straight_line_bindings
                    .span(expansion.bindings)
                {
                    Some(bindings) if bindings.is_empty() => {
                        output.push_str("  bindings: none\n");
                    }
                    Some(bindings) => {
                        output.push_str("  bindings:\n");
                        for binding in bindings {
                            output.push_str(&format!(
                                "    - {:?} `{}` = `{}`\n",
                                binding.kind,
                                binding.parameter_name,
                                binding.expression.display_name()
                            ));
                        }
                    }
                    None => output.push_str("  bindings: invalid span\n"),
                }

                match native_plan
                    .runtime_branching_calls
                    .straight_line_operations
                    .span(expansion.operations)
                {
                    Some(operations) if operations.is_empty() => {
                        output.push_str("  operations: none\n");
                    }
                    Some(operations) => {
                        output.push_str("  operations:\n");
                        for operation in operations {
                            write_runtime_straight_line_branch_operation(&mut output, operation);
                        }
                    }
                    None => output.push_str("  operations: invalid span\n"),
                }
            }
        }

        output.push_str("\n## Layouts\n");
        output.push_str(&format!(
            "data layouts: {}\n",
            native_plan.layouts.data_layouts.len()
        ));
        output.push_str(&format!(
            "machine layouts: {}\n",
            native_plan.layouts.machine_layouts.len()
        ));
        output.push_str(&format!("fields: {}\n\n", native_plan.layouts.fields.len()));

        for (_, data_layout) in native_plan.layouts.data_layouts.iter() {
            output.push_str(&format!(
                "- data {}: size {}, align {}\n",
                data_layout.name, data_layout.layout.size, data_layout.layout.alignment
            ));

            match &data_layout.shape {
                DataShape::Enum { variants } => {
                    output.push_str(&format!("  variants: {}\n", variants.join(", ")));
                }
                DataShape::Record { fields } => {
                    write_field_layouts(&mut output, &native_plan.layouts.fields, *fields);
                }
            }
        }

        for (_, machine_layout) in native_plan.layouts.machine_layouts.iter() {
            output.push_str(&format!(
                "- machine {}: size {}, align {}\n",
                machine_layout.name, machine_layout.layout.size, machine_layout.layout.alignment
            ));
            write_field_layouts(
                &mut output,
                &native_plan.layouts.fields,
                machine_layout.fields,
            );
        }

        output.push_str("\n## Object\n");
        output.push_str(&format!(
            "sections: {}\n",
            native_plan.object.sections.len()
        ));
        for (_, section) in native_plan.object.sections.iter() {
            write_section_plan(&mut output, section);
        }

        output.push_str(&format!("symbols: {}\n", native_plan.object.symbols.len()));
        for (_, symbol) in native_plan.object.symbols.iter() {
            write_symbol_plan(&mut output, symbol);
        }
        output.push('\n');

        output.push_str("## Relocations\n");
        output.push_str(&format!(
            "records: {}\n",
            native_plan.relocations.records.len()
        ));
        if native_plan.relocations.records.is_empty() {
            output.push_str("none\n");
        } else {
            for (_, relocation) in native_plan.relocations.records.iter() {
                write_relocation_record(&mut output, relocation);
            }
        }

        self.write("09_native_plan.txt", &output)
    }

    pub(crate) fn write_emission_plan(
        &self,
        emission_plan: &EmissionPlan,
    ) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega Emission Plan\n\n");
        output.push_str(&format!(
            "object format: {:?}\n",
            emission_plan.object_format
        ));
        output.push_str(&format!("entry symbol: {}\n", emission_plan.entry_symbol));
        output.push_str(&format!("sections: {}\n", emission_plan.sections));
        output.push_str(&format!("symbols: {}\n", emission_plan.symbols));
        output.push_str(&format!("host bindings: {}\n", emission_plan.host_bindings));
        output.push_str(&format!("host calls: {}\n", emission_plan.host_calls));
        output.push_str(&format!("data bytes: {}\n", emission_plan.data_bytes));
        output.push_str(&format!(
            "selected instructions: {}\n",
            emission_plan.selected_instructions
        ));
        output.push_str(&format!(
            "instruction operands: {}\n",
            emission_plan.instruction_operands
        ));
        output.push_str(&format!(
            "machine code bytes: {}\n",
            emission_plan.machine_code_bytes
        ));
        output.push_str(&format!(
            "encoded machine bytes: {}\n",
            emission_plan.encoded_machine_bytes
        ));
        output.push_str(&format!("relocations: {}\n", emission_plan.relocations));
        output.push_str(&format!("blockers: {}\n\n", emission_plan.blockers.len()));

        if emission_plan.blockers.is_empty() {
            output.push_str("status: ready to emit\n");
        } else {
            output.push_str("status: blocked before byte emission\n\n");
            output.push_str("## Blockers\n");

            for (_, blocker) in emission_plan.blockers.iter() {
                output.push_str(&format!("- {}: {}\n", blocker.stage, blocker.reason));
            }
        }

        self.write("11_emission.txt", &output)
    }

    pub(crate) fn write_emitted_native_output(
        &self,
        emitted_output: &EmittedNativeOutput,
    ) -> Result<PathBuf, Diagnostic> {
        let output_path = self.root.join(&emitted_output.file_name);
        fs::write(&output_path, &emitted_output.bytes).map_err(|error| {
            Diagnostic::error(format!(
                "failed to write native output {}: {error}",
                output_path.display()
            ))
        })?;

        let mut output = String::new();
        output.push_str("# Omega Emitted Native Output\n\n");
        output.push_str(&format!("path: {}\n", output_path.display()));
        output.push_str(&format!("format: {}\n", emitted_output.format));
        output.push_str(&format!("kind: {:?}\n", emitted_output.kind));
        output.push_str(&format!("bytes: {}\n", emitted_output.bytes.len()));
        output.push_str(&format!("text bytes: {}\n", emitted_output.text_bytes));
        output.push_str(&format!("data bytes: {}\n", emitted_output.data_bytes));
        output.push_str(&format!("bss bytes: {}\n", emitted_output.bss_bytes));
        output.push_str(&format!("symbols: {}\n", emitted_output.symbols));
        output.push_str(&format!("relocations: {}\n", emitted_output.relocations));
        output.push_str(&format!(
            "final image symbols: {}\n",
            emitted_output.final_image_symbols
        ));
        output.push_str(&format!(
            "final image imports: {}\n",
            emitted_output.final_image_imports
        ));
        output.push_str(&format!(
            "final image relocations: {}\n",
            emitted_output.final_image_relocations
        ));

        self.write("12_emitted_output.txt", &output)?;

        Ok(output_path)
    }

    pub(crate) fn write_executable_finalization_report(
        &self,
        finalization: &ExecutableFinalization,
    ) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega Executable Finalization\n\n");
        output.push_str(&format!("status: {:?}\n", finalization.status));
        output.push_str(&format!(
            "output: {}\n",
            finalization.executable_path.display()
        ));
        if finalization.command.is_empty() {
            output.push_str("command: none\n");
        } else {
            output.push_str(&format!("command: {}\n", finalization.command.join(" ")));
        }
        if !finalization.stdout.is_empty() {
            output.push_str("\n## stdout\n");
            output.push_str(&finalization.stdout);
            output.push('\n');
        }
        if !finalization.stderr.is_empty() {
            output.push_str("\n## stderr\n");
            output.push_str(&finalization.stderr);
            output.push('\n');
        }
        if finalization.status == ExecutableFinalizationStatus::AlreadyExecutable {
            output.push_str(
                "\nno finalization command was needed; the backend emitted an executable image directly.\n",
            );
        }

        self.write("13_finalization.txt", &output)
    }

    pub(crate) fn write_timings(&self, timings: &[PhaseTiming]) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega Phase Timings\n\n");

        let total_microseconds = timings
            .iter()
            .map(|timing| timing.microseconds)
            .sum::<u128>();
        let total_allocation_calls = timings
            .iter()
            .map(|timing| timing.allocations.allocation_calls)
            .sum::<u64>();
        let total_deallocation_calls = timings
            .iter()
            .map(|timing| timing.allocations.deallocation_calls)
            .sum::<u64>();
        let total_allocated_bytes = timings
            .iter()
            .map(|timing| timing.allocations.allocated_bytes)
            .sum::<u64>();
        let total_deallocated_bytes = timings
            .iter()
            .map(|timing| timing.allocations.deallocated_bytes)
            .sum::<u64>();
        let total_net_live_bytes =
            i128::from(total_allocated_bytes) - i128::from(total_deallocated_bytes);
        let slowest = timings.iter().max_by_key(|timing| timing.microseconds);
        let allocation_heaviest = timings
            .iter()
            .max_by_key(|timing| timing.allocations.allocated_bytes);
        let average_microseconds = if timings.is_empty() {
            0
        } else {
            total_microseconds / timings.len() as u128
        };

        output.push_str("## Summary\n\n");
        output.push_str(&format!("phase count: {}\n", timings.len()));
        output.push_str(&format!(
            "total measured: {}\n",
            format_duration(total_microseconds)
        ));
        output.push_str(&format!(
            "average phase: {}\n",
            format_duration(average_microseconds)
        ));
        output.push_str(&format!(
            "allocation calls: {} alloc / {} free\n",
            format_integer(u128::from(total_allocation_calls)),
            format_integer(u128::from(total_deallocation_calls))
        ));
        output.push_str(&format!(
            "allocated bytes: {} allocated / {} freed / {} net\n",
            format_bytes(total_allocated_bytes),
            format_bytes(total_deallocated_bytes),
            format_signed_bytes(total_net_live_bytes)
        ));
        if let Some(slowest) = slowest {
            output.push_str(&format!(
                "slowest phase: {} ({}, {})\n",
                slowest.phase,
                format_duration(slowest.microseconds),
                format_percentage(slowest.microseconds, total_microseconds)
            ));
        }
        if let Some(allocation_heaviest) = allocation_heaviest {
            output.push_str(&format!(
                "allocation-heaviest phase: {} ({} in {} allocation calls)\n",
                allocation_heaviest.phase,
                format_bytes(allocation_heaviest.allocations.allocated_bytes),
                format_integer(u128::from(allocation_heaviest.allocations.allocation_calls))
            ));
        }

        output.push_str("\n## Phases\n\n");
        let phase_width = timings
            .iter()
            .map(|timing| timing.phase.len())
            .max()
            .unwrap_or("phase".len())
            .max("phase".len());
        let duration_width = timings
            .iter()
            .map(|timing| format_duration(timing.microseconds).len())
            .chain(std::iter::once("time".len()))
            .max()
            .unwrap_or("time".len());
        let raw_width = timings
            .iter()
            .map(|timing| format!("{} us", format_integer(timing.microseconds)).len())
            .chain(std::iter::once("raw".len()))
            .max()
            .unwrap_or("raw".len());
        let alloc_calls_width = timings
            .iter()
            .map(|timing| format_integer(u128::from(timing.allocations.allocation_calls)).len())
            .chain(std::iter::once("allocs".len()))
            .max()
            .unwrap_or("allocs".len());
        let allocated_width = timings
            .iter()
            .map(|timing| format_bytes(timing.allocations.allocated_bytes).len())
            .chain(std::iter::once("allocated".len()))
            .max()
            .unwrap_or("allocated".len());
        let net_width = timings
            .iter()
            .map(|timing| format_signed_bytes(timing.allocations.net_live_bytes()).len())
            .chain(std::iter::once("net".len()))
            .max()
            .unwrap_or("net".len());

        output.push_str(&format!(
            "{:<phase_width$}  {:>duration_width$}  {:>7}  {:>raw_width$}  {:>alloc_calls_width$}  {:>allocated_width$}  {:>net_width$}\n",
            "phase", "time", "share", "raw", "allocs", "allocated", "net"
        ));
        output.push_str(&format!(
            "{:-<phase_width$}  {:-<duration_width$}  {:-<7}  {:-<raw_width$}  {:-<alloc_calls_width$}  {:-<allocated_width$}  {:-<net_width$}\n",
            "", "", "", "", "", "", ""
        ));
        for timing in timings {
            output.push_str(&format!(
                "{:<phase_width$}  {:>duration_width$}  {:>7}  {:>raw_width$}  {:>alloc_calls_width$}  {:>allocated_width$}  {:>net_width$}\n",
                timing.phase,
                format_duration(timing.microseconds),
                format_percentage(timing.microseconds, total_microseconds),
                format!("{} us", format_integer(timing.microseconds)),
                format_integer(u128::from(timing.allocations.allocation_calls)),
                format_bytes(timing.allocations.allocated_bytes),
                format_signed_bytes(timing.allocations.net_live_bytes()),
            ));
        }
        output.push_str(&format!(
            "{:-<phase_width$}  {:-<duration_width$}  {:-<7}  {:-<raw_width$}  {:-<alloc_calls_width$}  {:-<allocated_width$}  {:-<net_width$}\n",
            "", "", "", "", "", "", ""
        ));
        output.push_str(&format!(
            "{:<phase_width$}  {:>duration_width$}  {:>7}  {:>raw_width$}  {:>alloc_calls_width$}  {:>allocated_width$}  {:>net_width$}\n",
            "total",
            format_duration(total_microseconds),
            "100.00%",
            format!("{} us", format_integer(total_microseconds)),
            format_integer(u128::from(total_allocation_calls)),
            format_bytes(total_allocated_bytes),
            format_signed_bytes(total_net_live_bytes),
        ));

        self.write("00_timings.txt", &output)
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    fn write(&self, file_name: &str, contents: &str) -> Result<(), Diagnostic> {
        let path = self.root.join(file_name);
        fs::write(&path, contents).map_err(|error| {
            Diagnostic::error(format!(
                "failed to write artifact {}: {error}",
                path.display()
            ))
        })
    }
}

fn format_duration(microseconds: u128) -> String {
    if microseconds >= 1_000_000 {
        format!("{:.3} s", microseconds as f64 / 1_000_000.0)
    } else {
        format!("{:.3} ms", microseconds as f64 / 1_000.0)
    }
}

fn format_percentage(part: u128, total: u128) -> String {
    if total == 0 {
        return "0.00%".to_owned();
    }

    format!("{:.2}%", part as f64 * 100.0 / total as f64)
}

fn format_integer(value: u128) -> String {
    let digits = value.to_string();
    let mut output = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, character) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            output.push(',');
        }
        output.push(character);
    }
    output
}

fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;

    let bytes = bytes as f64;
    if bytes >= GIB {
        format!("{:.2} GiB", bytes / GIB)
    } else if bytes >= MIB {
        format!("{:.2} MiB", bytes / MIB)
    } else if bytes >= KIB {
        format!("{:.2} KiB", bytes / KIB)
    } else {
        format!("{} B", bytes as u64)
    }
}

fn format_signed_bytes(bytes: i128) -> String {
    if bytes < 0 {
        format!("-{}", format_bytes(bytes.unsigned_abs() as u64))
    } else {
        format_bytes(bytes as u64)
    }
}

fn write_loaded_file(output: &mut String, file: &LoadedFile) {
    output.push_str(&format!("## {}\n", file.path.display()));
    output.push_str(&format!("first item: {}\n", file.first_item));
    output.push_str(&format!("item count: {}\n\n", file.item_count));
}

fn write_ast_file(output: &mut String, loaded_program: &LoadedProgram, file: &LoadedFile) {
    output.push_str(&format!("## {}\n", file.path.display()));

    let Some(items) = loaded_program
        .items
        .get(file.first_item..file.first_item + file.item_count)
    else {
        output.push_str("invalid item range\n\n");
        return;
    };

    if items.is_empty() {
        output.push_str("items: none\n\n");
        return;
    }

    for (index, item) in items.iter().enumerate() {
        output.push_str(&format!(
            "- item {}: {}\n",
            file.first_item + index,
            ast_item_summary(item)
        ));
    }

    output.push('\n');
}

fn ast_item_summary(item: &Item) -> String {
    match item {
        Item::Capability(capability) => {
            let mut field_count = 0usize;
            let mut state_count = 0usize;
            let mut contract_count = 0usize;

            for member in &capability.members {
                match member {
                    crate::ast::item::CapabilityMember::Field(_) => field_count += 1,
                    crate::ast::item::CapabilityMember::State(state) => {
                        state_count += 1;
                        contract_count += state.contracts.len();
                    }
                }
            }

            format!(
                "capability `{}` fields {} states {} contracts {}",
                capability.name, field_count, state_count, contract_count
            )
        }
        Item::Data(data_definition) => {
            let mut field_count = 0usize;
            let mut variant_count = 0usize;

            for member in &data_definition.members {
                match member {
                    crate::ast::item::DataMember::Field(_) => field_count += 1,
                    crate::ast::item::DataMember::Variant(_) => variant_count += 1,
                }
            }

            format!(
                "data `{}` fields {} variants {}",
                data_definition.name, field_count, variant_count
            )
        }
        Item::Invariant(invariant) => {
            format!(
                "invariant `{}` constraints {}",
                invariant.name,
                invariant.constraints.len()
            )
        }
        Item::TrustDefinition(trust_definition) => format!(
            "trust `{}` body tokens {}",
            trust_definition.name, trust_definition.token_count
        ),
        Item::Use(use_item) => format!("use {}", use_item.path.join("::")),
        Item::Machine(machine) => format!(
            "machine `{}` contains {} owned data {} states {}",
            machine.name,
            machine.contains.len(),
            machine.owned_data.len(),
            machine.states.len()
        ),
        Item::Platform(platform) => {
            format!(
                "platform `{}` states {}",
                platform.name,
                platform.states.len()
            )
        }
        Item::Target(target) => format!(
            "target `{}` host {} trust policies {}",
            target.name,
            target
                .host
                .as_ref()
                .map(|host| host.provider.join("::"))
                .unwrap_or_else(|| "none".to_owned()),
            target.trust_policies.len()
        ),
    }
}

fn write_typed_program_data_definitions(output: &mut String, program: &Program) {
    output.push_str("## Data Definitions\n");

    if program.data_definitions.is_empty() {
        output.push_str("none\n\n");
        return;
    }

    for data_definition in &program.data_definitions {
        output.push_str(&format!(
            "- data `{}` {:?}: members {}\n",
            data_definition.name,
            data_definition.shape_kind(),
            data_definition.members.len()
        ));

        for member in &data_definition.members {
            match member {
                DataMember::Field(field) => output.push_str(&format!(
                    "  - field {}: {}\n",
                    field.name,
                    field
                        .type_reference
                        .display_name_with_constraints(&program.type_constraints)
                )),
                DataMember::Variant(variant) => {
                    output.push_str(&format!("  - variant {}\n", variant.name));
                }
            }
        }
    }

    output.push('\n');
}

fn write_typed_program_invariants(output: &mut String, program: &Program) {
    output.push_str("## Invariants\n");

    if program.invariant_definitions.is_empty() {
        output.push_str("none\n\n");
        return;
    }

    for invariant in &program.invariant_definitions {
        output.push_str(&format!(
            "- invariant `{}` = {}\n",
            invariant.name,
            typed_program_constraint_span_name(program, invariant.constraints)
        ));
    }

    output.push('\n');
}

fn write_typed_program_platforms(output: &mut String, program: &Program) {
    output.push_str("## Platforms\n");

    if program.platforms.is_empty() {
        output.push_str("none\n\n");
        return;
    }

    for platform in &program.platforms {
        output.push_str(&format!(
            "- platform `{}` states {}\n",
            platform.name,
            platform.states.len()
        ));

        for state in &platform.states {
            output.push_str(&format!(
                "  - state {}({}){}\n",
                state.name,
                typed_program_parameters_name(program, &state.parameters),
                typed_program_return_type_name(program, state.return_type.as_ref())
            ));
        }
    }

    output.push('\n');
}

fn write_typed_program_machines(output: &mut String, program: &Program) {
    output.push_str("## Machines\n");

    if program.machines.is_empty() {
        output.push_str("none\n\n");
        return;
    }

    for machine in &program.machines {
        output.push_str(&format!(
            "- machine `{}` contains {} owned data {} states {}\n",
            machine.name,
            machine.contains.len(),
            machine.owned_data.len(),
            machine.states.len()
        ));

        for contained_object in &machine.contains {
            output.push_str(&format!(
                "  - contains {}: {}\n",
                contained_object.name, contained_object.type_name
            ));
        }

        for owned_data in &machine.owned_data {
            output.push_str(&format!(
                "  - owns {}: {}{}\n",
                owned_data.name,
                owned_data
                    .type_reference
                    .display_name_with_constraints(&program.type_constraints),
                if owned_data.initial_value.is_some() {
                    " = <initializer>"
                } else {
                    ""
                }
            ));
        }

        for state in &machine.states {
            output.push_str(&format!(
                "  - state {}({}){}: statements {}\n",
                state.name,
                typed_program_parameters_name(program, &state.parameters),
                typed_program_return_type_name(program, state.return_type.as_ref()),
                state.statements.len()
            ));
        }
    }

    output.push('\n');
}

fn typed_program_constraint_span_name(
    program: &Program,
    span: omega_core::arena::HandleSpan<omega_typed_program::types::TypeConstraint>,
) -> String {
    let Some(constraints) = program.type_constraints.span(span) else {
        return "[invalid constraint span]".to_owned();
    };

    if constraints.is_empty() {
        return "[]".to_owned();
    }

    let mut output = String::new();
    output.push('[');

    for (index, constraint) in constraints.iter().enumerate() {
        if index > 0 {
            output.push_str(", ");
        }

        output.push_str(&constraint.display_name());
    }

    output.push(']');
    output
}

fn typed_program_parameters_name(
    program: &Program,
    parameters: &[omega_typed_program::signature::StateParameter],
) -> String {
    let mut output = String::new();

    for (index, parameter) in parameters.iter().enumerate() {
        if index > 0 {
            output.push_str(", ");
        }

        if parameter.is_const {
            output.push_str("const ");
        }

        if parameter.is_mutable {
            output.push_str("mut ");
        }

        if parameter.is_self {
            output.push_str("self");
        } else {
            output.push_str(&parameter.name);
        }

        output.push_str(": ");
        output.push_str(
            &parameter
                .type_reference
                .display_name_with_constraints(&program.type_constraints),
        );
    }

    output
}

fn typed_program_return_type_name(
    program: &Program,
    return_type: Option<&omega_typed_program::types::TypeReference>,
) -> String {
    return_type
        .map(|return_type| {
            format!(
                " -> {}",
                return_type.display_name_with_constraints(&program.type_constraints)
            )
        })
        .unwrap_or_default()
}

fn write_proof_obligation(
    output: &mut String,
    proof_plan: &ProofPlan,
    obligation: &ProofObligation,
) {
    match obligation {
        ProofObligation::BoundedAssignment(obligation) => {
            output.push_str(&format!(
                "- bounded assignment {}.{}: {} = {} : {} {}\n",
                obligation.machine,
                obligation.state,
                obligation.target.display_name(),
                obligation.value.display_name(),
                obligation.base_type.display_name(),
                proof_constraints_name(proof_plan, obligation.constraints)
            ));
            output.push_str(&format!(
                "  value constraints: {}\n",
                proof_constraints_name(proof_plan, obligation.value_constraints)
            ));
        }
        ProofObligation::BoundedCallArgument(obligation) => {
            let target = obligation
                .receiver
                .as_ref()
                .map(|receiver| format!("{receiver}.{}", obligation.target))
                .unwrap_or_else(|| obligation.target.clone());
            output.push_str(&format!(
                "- bounded call argument {}.{} -> {}({}): {} : {} {}\n",
                obligation.machine,
                obligation.state,
                target,
                obligation.parameter,
                obligation.argument.display_name(),
                obligation.base_type.display_name(),
                proof_constraints_name(proof_plan, obligation.constraints)
            ));
            output.push_str(&format!(
                "  argument constraints: {}\n",
                proof_constraints_name(proof_plan, obligation.argument_constraints)
            ));
        }
        ProofObligation::BoundedInitializer(obligation) => {
            output.push_str(&format!(
                "- bounded initializer {} = {} : {} {}\n",
                obligation.owner,
                obligation.value.display_name(),
                obligation.base_type.display_name(),
                proof_constraints_name(proof_plan, obligation.constraints)
            ));
        }
        ProofObligation::BoundedStateReturn(obligation) => {
            output.push_str(&format!(
                "- bounded state return {}.{}: {} : {} {}\n",
                obligation.machine,
                obligation.state,
                obligation.value.display_name(),
                obligation.base_type.display_name(),
                proof_constraints_name(proof_plan, obligation.constraints)
            ));
            output.push_str(&format!(
                "  value constraints: {}\n",
                proof_constraints_name(proof_plan, obligation.value_constraints)
            ));
        }
        ProofObligation::BoundedTransitionArgument(obligation) => {
            output.push_str(&format!(
                "- bounded transition argument {}.{} -> {}({}): {} : {} {} when {:?}\n",
                obligation.machine,
                obligation.state,
                proof_transition_target_name(&obligation.target),
                obligation.parameter,
                obligation.argument.display_name(),
                obligation.base_type.display_name(),
                proof_constraints_name(proof_plan, obligation.constraints),
                obligation.guard
            ));
            output.push_str(&format!(
                "  argument constraints: {}\n",
                proof_constraints_name(proof_plan, obligation.argument_constraints)
            ));
        }
        ProofObligation::BoundedValue(obligation) => {
            output.push_str(&format!(
                "- bounded value {} : {} {}\n",
                obligation.owner,
                obligation.base_type.display_name(),
                proof_constraints_name(proof_plan, obligation.constraints)
            ));
        }
        ProofObligation::GuardedTransition(obligation) => {
            output.push_str(&format!(
                "- guarded transition {}.{} -> {} when {:?}\n",
                obligation.machine,
                obligation.state,
                proof_transition_target_name(&obligation.target),
                obligation.guard
            ));
        }
    }
}

fn proof_constraints_name(
    proof_plan: &ProofPlan,
    constraints: omega_core::arena::HandleSpan<omega_typed_program::types::TypeConstraint>,
) -> String {
    let Some(constraints) = proof_plan.type_constraints.span(constraints) else {
        return "[invalid constraint span]".to_owned();
    };

    if constraints.is_empty() {
        return "[]".to_owned();
    }

    format!(
        "[{}]",
        constraints
            .iter()
            .map(omega_typed_program::types::TypeConstraint::display_name)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn proof_transition_target_name(
    target: &omega_typed_program::statement::TransitionTarget,
) -> String {
    match target {
        omega_typed_program::statement::TransitionTarget::Named { path, .. } => path.join("."),
        omega_typed_program::statement::TransitionTarget::SelfTarget => "self".to_owned(),
        omega_typed_program::statement::TransitionTarget::Terminal => "terminal".to_owned(),
    }
}

fn write_field_layouts(
    output: &mut String,
    fields: &omega_core::arena::Arena<FieldLayout>,
    field_span: omega_core::arena::HandleSpan<FieldLayout>,
) {
    let Some(fields) = fields.span(field_span) else {
        output.push_str("  fields: invalid span\n");
        return;
    };

    if fields.is_empty() {
        output.push_str("  fields: none\n");
        return;
    }

    output.push_str("  fields:\n");
    for field in fields {
        output.push_str(&format!(
            "    - {} @{}: {} size {}, align {}\n",
            field.name, field.offset, field.type_name, field.layout.size, field.layout.alignment
        ));
    }
}

fn write_section_plan(output: &mut String, section: &SectionPlan) {
    output.push_str(&format!(
        "- section {} {:?}: size {}, align {}\n",
        section.name, section.kind, section.size, section.alignment
    ));
}

fn write_host_binding(output: &mut String, binding: &HostBinding) {
    match &binding.mechanism {
        HostBindingMechanism::Import { library, symbol } => {
            output.push_str(&format!(
                "- {}.{} import {}!{} trust `{}`\n",
                binding.capability, binding.operation, library, symbol, binding.trust_policy
            ));
        }
        HostBindingMechanism::Syscall {
            name,
            number,
            number_register,
            supervisor_call,
        } => {
            output.push_str(&format!(
                "- {}.{} syscall {}({}) register x{} svc #{} trust `{}`\n",
                binding.capability,
                binding.operation,
                name,
                number,
                number_register,
                supervisor_call,
                binding.trust_policy
            ));
        }
    }
}

fn write_platform_call_lowering(
    output: &mut String,
    native_plan: &NativePlan,
    lowering: &PlatformCallLowering,
) {
    let operations = native_plan
        .host_abi
        .host_operations
        .span(lowering.operations)
        .map(|operations| {
            operations
                .iter()
                .map(|operation| format!("{}.{}", operation.capability, operation.operation))
                .collect::<Vec<_>>()
                .join(" -> ")
        })
        .unwrap_or_else(|| "invalid operation span".to_owned());

    output.push_str(&format!(
        "- {}.{} => {}",
        lowering.platform, lowering.state, operations
    ));
    match lowering.data {
        PlatformCallData::None => {}
        PlatformCallData::FirstTextArgument { append_newline } => output.push_str(&format!(
            " data first_text_argument append_newline={append_newline}"
        )),
        PlatformCallData::MutableOutputBuffer { byte_capacity } => output.push_str(&format!(
            " data mutable_output_buffer byte_capacity={byte_capacity}"
        )),
    }
    output.push('\n');
}

fn write_host_call(output: &mut String, native_plan: &NativePlan, call: &HostCall) {
    output.push_str(&format!(
        "- {}.{} statement {} `{}`\n",
        call.machine, call.state, call.statement_index, call.platform_call
    ));

    match native_plan.host_calls.arguments.span(call.arguments) {
        Some(arguments) if arguments.is_empty() => output.push_str("  arguments: none\n"),
        Some(arguments) => {
            output.push_str("  arguments:\n");
            for argument in arguments {
                write_host_call_argument(output, argument);
            }
        }
        None => output.push_str("  arguments: invalid span\n"),
    }

    match native_plan.host_calls.operations.span(call.operations) {
        Some(operations) if operations.is_empty() => output.push_str("  operations: none\n"),
        Some(operations) => {
            output.push_str("  operations:\n");
            for operation in operations {
                write_lowered_host_operation(output, operation);
            }
        }
        None => output.push_str("  operations: invalid span\n"),
    }
}

fn write_host_call_argument(output: &mut String, argument: &HostCallArgument) {
    let argument_name = match &argument.kind {
        HostCallArgumentKind::Text(text) => format!("text {text:?}"),
        HostCallArgumentKind::Integer(value) => format!("integer {value}"),
        HostCallArgumentKind::Expression(expression) => {
            format!("expression {}", expression.display_name())
        }
    };

    output.push_str(&format!("  - {argument_name}\n"));
}

fn write_lowered_host_operation(output: &mut String, operation: &LoweredHostOperation) {
    output.push_str(&format!(
        "  - {}.{}\n",
        operation.capability, operation.operation
    ));
}

fn write_native_data_object(
    output: &mut String,
    native_plan: &NativePlan,
    data_object: &NativeDataObject,
) {
    let byte_count = native_plan
        .data
        .bytes
        .span(data_object.bytes)
        .map_or(0, |bytes| bytes.len());

    output.push_str(&format!(
        "- {} @{} bytes {} align {} from {}.{} statement {}\n",
        data_object.symbol,
        data_object.offset,
        byte_count,
        data_object.alignment,
        data_object.source_machine,
        data_object.source_state,
        data_object.source_statement
    ));
}

fn write_function_instruction_plan(
    output: &mut String,
    native_plan: &NativePlan,
    function: &FunctionInstructionPlan,
) {
    output.push_str(&format!(
        "- function {} from {}.{}\n",
        function.symbol, function.machine, function.state
    ));

    match native_plan
        .instructions
        .instructions
        .span(function.instructions)
    {
        Some(instructions) if instructions.is_empty() => output.push_str("  instructions: none\n"),
        Some(instructions) => {
            output.push_str("  instructions:\n");
            for instruction in instructions {
                write_selected_instruction(output, native_plan, instruction);
            }
        }
        None => output.push_str("  instructions: invalid span\n"),
    }
}

fn write_selected_instruction(
    output: &mut String,
    native_plan: &NativePlan,
    instruction: &SelectedInstruction,
) {
    output.push_str(&format!(
        "    - statement {}: {}\n",
        instruction.source_statement,
        selected_instruction_name(native_plan, &instruction.kind)
    ));
}

fn selected_instruction_name(native_plan: &NativePlan, kind: &SelectedInstructionKind) -> String {
    match kind {
        SelectedInstructionKind::EnterFunction => "enter function".to_owned(),
        SelectedInstructionKind::EnterDispatchLoop {
            entry_dispatch_index,
            terminal_dispatch_index,
            current_state_slot,
            next_state_slot,
        } => format!(
            "enter dispatch loop entry #{entry_dispatch_index} terminal #{terminal_dispatch_index} current `{current_state_slot}` next `{next_state_slot}`"
        ),
        SelectedInstructionKind::EnterDispatchCase {
            dispatch_index,
            label,
        } => format!("enter dispatch case #{dispatch_index} `{label}`"),
        SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering,
            operator,
            byte_offset,
            byte_size,
            expected_value,
            has_storage,
        } => {
            if *has_storage {
                format!(
                    "evaluate dispatch guard {guard_lowering:?}/{operator:?} offset {byte_offset} bytes {byte_size} expected {expected_value}"
                )
            } else {
                format!("evaluate dispatch guard {guard_lowering:?}/{operator:?}")
            }
        }
        SelectedInstructionKind::CompareRuntimeTextLiteral {
            buffer_symbol,
            literal,
        } => {
            format!("compare runtime text `{buffer_symbol}` with {literal:?}")
        }
        SelectedInstructionKind::CompareRuntimeTextStorage {
            buffer_symbol,
            source_symbol,
            source_offset,
            operator,
        } => {
            format!(
                "compare runtime text storage {source_symbol}@{source_offset} {operator:?} `{buffer_symbol}`"
            )
        }
        SelectedInstructionKind::CompareRuntimeStorage {
            left_symbol,
            left_offset,
            right_symbol,
            right_offset,
            byte_size,
            operator,
        } => {
            format!(
                "compare runtime storage {left_symbol}@{left_offset} {operator:?} {right_symbol}@{right_offset} bytes {byte_size}"
            )
        }
        SelectedInstructionKind::CompareRuntimeStorageValue {
            symbol,
            byte_offset,
            byte_size,
            expected_value,
            operator,
        } => {
            format!(
                "compare runtime storage {symbol}@{byte_offset} {operator:?} {expected_value} bytes {byte_size}"
            )
        }
        SelectedInstructionKind::WriteRuntimeTextLiteral {
            buffer_symbol,
            literal,
        } => {
            format!("write runtime text `{buffer_symbol}` = {literal:?}")
        }
        SelectedInstructionKind::WriteRuntimeTextLiteralSegment {
            buffer_symbol,
            byte_offset,
            literal,
        } => {
            format!("write runtime text segment `{buffer_symbol}`@{byte_offset} = {literal:?}")
        }
        SelectedInstructionKind::AppendRuntimeTextStoredSuffix {
            buffer_symbol,
            buffer_offset,
            source_symbol,
            source_offset,
            target_symbol,
            target_offset,
            length_delta,
        } => {
            format!(
                "append runtime text suffix {source_symbol}@{source_offset} -> `{buffer_symbol}`@{buffer_offset}, descriptor {target_symbol}@{target_offset}, len +{length_delta}"
            )
        }
        SelectedInstructionKind::MaterializeRuntimeTextBuffer {
            buffer_symbol,
            target_symbol,
            target_offset,
        } => {
            format!(
                "materialize runtime text buffer `{buffer_symbol}` for {target_symbol}@{target_offset}"
            )
        }
        SelectedInstructionKind::AppendRuntimeTextStoredPlace {
            buffer_symbol,
            source_symbol,
            source_offset,
            target_symbol,
            target_offset,
        } => {
            format!(
                "append runtime text stored place {source_symbol}@{source_offset} -> `{buffer_symbol}`, descriptor {target_symbol}@{target_offset}"
            )
        }
        SelectedInstructionKind::AppendRuntimeTextLiteral {
            buffer_symbol,
            target_symbol,
            target_offset,
            literal,
        } => {
            format!(
                "append runtime text literal `{buffer_symbol}`, descriptor {target_symbol}@{target_offset} += {literal:?}"
            )
        }
        SelectedInstructionKind::WriteRuntimeMachineInteger {
            byte_offset,
            byte_size,
            value,
        } => {
            format!(
                "write runtime machine integer offset {byte_offset} bytes {byte_size} value {value}"
            )
        }
        SelectedInstructionKind::WriteRuntimeMachineString {
            byte_offset,
            data_symbol,
            byte_length,
        } => {
            format!(
                "write runtime machine string offset {byte_offset} data `{data_symbol}` len {byte_length}"
            )
        }
        SelectedInstructionKind::ReadRuntimeTextLine {
            buffer_symbol,
            target_symbol,
            target_offset,
            byte_capacity,
            syscall_number,
            syscall_number_register,
            supervisor_call,
        } => {
            format!(
                "read runtime text line syscall {syscall_number} via x{syscall_number_register}/svc #{supervisor_call} -> `{buffer_symbol}` cap {byte_capacity}, descriptor {target_symbol}@{target_offset}"
            )
        }
        SelectedInstructionKind::CopyRuntimeStorage {
            source_symbol,
            source_offset,
            target_symbol,
            target_offset,
            byte_count,
        } => {
            format!(
                "copy runtime storage {source_symbol}@{source_offset} -> {target_symbol}@{target_offset} bytes {byte_count}"
            )
        }
        SelectedInstructionKind::SetDispatchState { dispatch_index } => {
            format!("set dispatch state #{dispatch_index}")
        }
        SelectedInstructionKind::TerminateDispatch => "terminate dispatch".to_owned(),
        SelectedInstructionKind::LeaveDispatchCase => "leave dispatch case".to_owned(),
        SelectedInstructionKind::LeaveDispatchLoop => "leave dispatch loop".to_owned(),
        SelectedInstructionKind::BeginPlatformCall { platform_call } => {
            format!("begin platform call `{platform_call}`")
        }
        SelectedInstructionKind::HostOperation {
            capability,
            operation,
            operands,
        } => {
            format!(
                "call host operation {capability}.{operation}({})",
                selected_instruction_operands_name(native_plan, *operands)
            )
        }
        SelectedInstructionKind::LeaveFunction => "leave function".to_owned(),
    }
}

fn selected_instruction_operands_name(
    native_plan: &NativePlan,
    operands: omega_core::arena::HandleSpan<omega_native::instructions::InstructionOperand>,
) -> String {
    let Some(operands) = native_plan.instructions.operands.span(operands) else {
        return "invalid operands".to_owned();
    };

    operands
        .iter()
        .map(|operand| match &operand.kind {
            InstructionOperandKind::DataAddress { symbol } => format!("addr {symbol}"),
            InstructionOperandKind::RuntimeMachineStringPointer { byte_offset } => {
                format!("machine string ptr @{byte_offset}")
            }
            InstructionOperandKind::RuntimeMachineStringLength { byte_offset } => {
                format!("machine string len @{byte_offset}")
            }
            InstructionOperandKind::ImmediateInteger(value) => value.to_string(),
            InstructionOperandKind::ByteLength(value) => format!("len {value}"),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn write_machine_function_code(
    output: &mut String,
    native_plan: &NativePlan,
    function: &MachineFunctionCode,
) {
    output.push_str(&format!(
        "- function {} @{} bytes {}\n",
        function.symbol, function.offset, function.byte_count
    ));

    match native_plan
        .machine_code
        .instructions
        .span(function.instructions)
    {
        Some(instructions) if instructions.is_empty() => output.push_str("  instructions: none\n"),
        Some(instructions) => {
            output.push_str("  instructions:\n");
            for instruction in instructions {
                write_machine_instruction(output, native_plan, instruction);
            }
        }
        None => output.push_str("  instructions: invalid span\n"),
    }
}

fn write_machine_instruction(
    output: &mut String,
    native_plan: &NativePlan,
    instruction: &MachineInstruction,
) {
    output.push_str(&format!(
        "    - selected #{} @{} bytes {} {:?} encoded {}\n",
        instruction.selected_instruction_index,
        instruction.offset,
        instruction.byte_width,
        instruction.kind,
        machine_instruction_bytes_name(native_plan, instruction)
    ));
}

fn machine_instruction_bytes_name(
    native_plan: &NativePlan,
    instruction: &MachineInstruction,
) -> String {
    let Some(bytes) = native_plan.machine_code.bytes.span(instruction.bytes) else {
        return "invalid".to_owned();
    };

    if bytes.is_empty() {
        return "none".to_owned();
    }

    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn write_symbol_plan(output: &mut String, symbol: &SymbolPlan) {
    let section = symbol.section.as_deref().unwrap_or("none");
    output.push_str(&format!(
        "- symbol {} {:?}: section {}, offset {}, size {}\n",
        symbol.name, symbol.kind, section, symbol.offset, symbol.size
    ));
}

fn write_relocation_record(output: &mut String, relocation: &RelocationRecord) {
    output.push_str(&format!(
        "- {:?} {} text @{} width {} instruction #{} -> {}\n",
        relocation.kind,
        relocation.function_symbol,
        relocation.text_offset,
        relocation.byte_width,
        relocation.selected_instruction_index,
        relocation.symbol
    ));
}

fn write_state_effect(output: &mut String, state: &StateEffects) {
    output.push_str(&format!("- state {}: {:?}\n", state.name, state.effect));
}

fn write_source_graph_state(
    output: &mut String,
    source_graph: &SourceGraphReport,
    state: &SourceGraphState,
) {
    output.push_str(&format!("- state {}\n", state.name));

    match source_graph.transitions.span(state.transitions) {
        Some(transitions) if transitions.is_empty() => output.push_str("  transitions: none\n"),
        Some(transitions) => {
            output.push_str("  transitions:\n");
            for transition in transitions {
                output.push_str(&format!(
                    "    - -> {} {} continuation {}\n",
                    transition.target, transition.guard, transition.continuation
                ));
            }
        }
        None => output.push_str("  transitions: invalid span\n"),
    }
}

fn write_state_flow(output: &mut String, control_flow: &ControlFlowPlan, state: &StateFlow) {
    output.push_str(&format!("- state {} #{}\n", state.name, state.index));

    match control_flow.operations.span(state.operations) {
        Some(operations) if operations.is_empty() => output.push_str("  operations: none\n"),
        Some(operations) => {
            output.push_str("  operations:\n");
            for operation in operations {
                write_operation(output, operation);
            }
        }
        None => output.push_str("  operations: invalid span\n"),
    }

    match control_flow.transitions.span(state.transitions) {
        Some(transitions) if transitions.is_empty() => output.push_str("  transitions: none\n"),
        Some(transitions) => {
            output.push_str("  transitions:\n");
            for transition in transitions {
                write_transition(output, transition);
            }
        }
        None => output.push_str("  transitions: invalid span\n"),
    }
}

fn write_runtime_leaf_branch_operation(
    output: &mut String,
    operation: &RuntimeLeafBranchOperation,
) {
    match &operation.kind {
        RuntimeLeafBranchOperationKind::HostCall { platform_call } => {
            output.push_str(&format!(
                "    - {}.{} statement {} host call `{}`\n",
                operation.source_machine,
                operation.source_state,
                operation.statement_index,
                platform_call
            ));
        }
        RuntimeLeafBranchOperationKind::Mutation {
            mutation_kind,
            lowering,
            target,
            value,
        } => {
            output.push_str(&format!(
                "    - {}.{} statement {} {:?}/{:?}: `{}` = `{}`\n",
                operation.source_machine,
                operation.source_state,
                operation.statement_index,
                mutation_kind,
                lowering,
                target.display_name(),
                value.display_name()
            ));
        }
        RuntimeLeafBranchOperationKind::Other => {
            output.push_str(&format!(
                "    - {}.{} statement {} other\n",
                operation.source_machine, operation.source_state, operation.statement_index
            ));
        }
    }
}

fn write_runtime_straight_line_branch_operation(
    output: &mut String,
    operation: &RuntimeStraightLineBranchOperation,
) {
    match &operation.kind {
        RuntimeStraightLineBranchOperationKind::HostCall { platform_call } => {
            output.push_str(&format!(
                "    - {}.{} statement {} host call `{}`\n",
                operation.source_machine,
                operation.source_state,
                operation.statement_index,
                platform_call
            ));
        }
        RuntimeStraightLineBranchOperationKind::Mutation {
            mutation_kind,
            lowering,
            target,
            value,
        } => {
            output.push_str(&format!(
                "    - {}.{} statement {} {:?}/{:?}: `{}` = `{}`\n",
                operation.source_machine,
                operation.source_state,
                operation.statement_index,
                mutation_kind,
                lowering,
                target.display_name(),
                value.display_name()
            ));
        }
        RuntimeStraightLineBranchOperationKind::StateCall {
            target_machine,
            target_state,
            argument_count,
            lowering,
        } => {
            output.push_str(&format!(
                "    - {}.{} statement {} state call {}.{} args {} {:?}\n",
                operation.source_machine,
                operation.source_state,
                operation.statement_index,
                target_machine,
                target_state,
                argument_count,
                lowering
            ));
        }
        RuntimeStraightLineBranchOperationKind::LocalData => {
            output.push_str(&format!(
                "    - {}.{} statement {} local data\n",
                operation.source_machine, operation.source_state, operation.statement_index
            ));
        }
        RuntimeStraightLineBranchOperationKind::Other => {
            output.push_str(&format!(
                "    - {}.{} statement {} other\n",
                operation.source_machine, operation.source_state, operation.statement_index
            ));
        }
    }
}

fn write_operation(output: &mut String, operation: &Operation) {
    output.push_str(&format!(
        "    - statement {}: {:?}\n",
        operation.statement_index, operation.kind
    ));
}

fn write_transition(output: &mut String, transition: &TransitionFlow) {
    output.push_str(&format!(
        "    - -> {} {}",
        transition_target_name(&transition.target),
        transition_guard_name(&transition.guard)
    ));

    if let Some(continuation) = &transition.continuation {
        output.push_str(&format!(" -> {}", transition_target_name(continuation)));
    }

    output.push('\n');
}

fn transition_guard_name(guard: &TransitionGuard) -> String {
    match guard {
        TransitionGuard::Always => "always".to_owned(),
        TransitionGuard::When(expression) => format!("when {}", expression.display_name()),
    }
}

fn transition_target_name(target: &PlannedTransitionTarget) -> String {
    match target {
        PlannedTransitionTarget::State { name, .. } => name.clone(),
        PlannedTransitionTarget::Nested {
            receiver, state, ..
        } => format!("{receiver}.{state}"),
        PlannedTransitionTarget::SelfTarget => "self".to_owned(),
        PlannedTransitionTarget::Terminal => "terminal".to_owned(),
    }
}

fn runtime_transition_target_name(target: &RuntimeTransitionTarget) -> String {
    match target {
        RuntimeTransitionTarget::State { machine, state } => format!("{machine}.{state}"),
        RuntimeTransitionTarget::Terminal => "terminal".to_owned(),
        RuntimeTransitionTarget::None => "none".to_owned(),
        RuntimeTransitionTarget::Unknown { name } => format!("unknown {name}"),
    }
}
