use std::fs;
use std::path::{Path, PathBuf};

use crate::ast::item::Item;
use crate::diagnostics::Diagnostic;
use crate::ir::Program;
use crate::ir::data::DataMember;
use crate::ir::statement::TransitionGuard;
use crate::native::abi::{
    HostBinding, HostBindingMechanism, PlatformCallData, PlatformCallLowering,
};
use crate::native::control_flow::{
    ControlFlowPlan, Operation, PlannedTransitionTarget, StateFlow, TransitionFlow,
};
use crate::native::data::NativeDataObject;
use crate::native::emission::EmissionPlan;
use crate::native::emitter::EmittedNativeObject;
use crate::native::host_calls::{
    HostCall, HostCallArgument, HostCallArgumentKind, LoweredHostOperation,
};
use crate::native::instructions::{
    FunctionInstructionPlan, InstructionOperandKind, SelectedInstruction, SelectedInstructionKind,
};
use crate::native::layout::{DataShape, FieldLayout};
use crate::native::linker::{LinkOutput, LinkStatus};
use crate::native::machine_code::{MachineFunctionCode, MachineInstruction};
use crate::native::object::{SectionPlan, SymbolPlan};
use crate::native::plan::NativePlan;
use crate::native::relocations::RelocationRecord;
use crate::native::runtime_flow::RuntimeTransitionTarget;
use crate::native::state_schedule::build_entry_state_schedule;
use crate::pipeline::compile::{LoadedFile, LoadedProgram, PhaseTiming};
use crate::pipeline::trust::TrustReport;
use crate::proof::obligations::{ProofObligation, ProofPlan};
use crate::semantic::effects::{EffectPlan, StateEffects};
use omega_graph::{SourceGraphReport, SourceGraphState};
use omega_native::NativeSurfaceReport;
use omega_proof::ProofSurfaceReport;
use omega_resolve::ResolveReport;
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

    pub(crate) fn write_ir(&self, program: &Program) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega Driver IR\n\n");
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

        write_ir_data_definitions(&mut output, program);
        write_ir_invariants(&mut output, program);
        write_ir_platforms(&mut output, program);
        write_ir_machines(&mut output, program);

        self.write("05_driver_ir.txt", &output)
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
        if native_plan.state_guards.guards.is_empty() {
            output.push_str("none\n");
        } else {
            for (_, guard) in native_plan.state_guards.guards.iter() {
                output.push_str(&format!(
                    "- #{} {}.{} edge {} -> #{} {} {:?}",
                    guard.source_dispatch_index,
                    guard.source_machine,
                    guard.source_state,
                    guard.statement_order,
                    guard.target_dispatch_index,
                    runtime_transition_target_name(&guard.target),
                    guard.kind
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

    pub(crate) fn write_emitted_native_object(
        &self,
        emitted_object: &EmittedNativeObject,
    ) -> Result<PathBuf, Diagnostic> {
        let object_path = self.root.join(&emitted_object.file_name);
        fs::write(&object_path, &emitted_object.bytes).map_err(|error| {
            Diagnostic::error(format!(
                "failed to write native object {}: {error}",
                object_path.display()
            ))
        })?;

        let mut output = String::new();
        output.push_str("# Omega Emitted Native Object\n\n");
        output.push_str(&format!("path: {}\n", object_path.display()));
        output.push_str(&format!("format: {}\n", emitted_object.format));
        output.push_str(&format!("bytes: {}\n", emitted_object.bytes.len()));
        output.push_str(&format!("text bytes: {}\n", emitted_object.text_bytes));
        output.push_str(&format!("data bytes: {}\n", emitted_object.data_bytes));
        output.push_str(&format!("bss bytes: {}\n", emitted_object.bss_bytes));
        output.push_str(&format!("symbols: {}\n", emitted_object.symbols));
        output.push_str(&format!("relocations: {}\n", emitted_object.relocations));

        self.write("12_emitted_object.txt", &output)?;

        Ok(object_path)
    }

    pub(crate) fn write_link_report(&self, link_output: &LinkOutput) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega Link\n\n");
        output.push_str(&format!("status: {:?}\n", link_output.status));
        output.push_str(&format!(
            "output: {}\n",
            link_output.executable_path.display()
        ));
        if link_output.command.is_empty() {
            output.push_str("command: none\n");
        } else {
            output.push_str(&format!("command: {}\n", link_output.command.join(" ")));
        }
        if !link_output.stdout.is_empty() {
            output.push_str("\n## stdout\n");
            output.push_str(&link_output.stdout);
            output.push('\n');
        }
        if !link_output.stderr.is_empty() {
            output.push_str("\n## stderr\n");
            output.push_str(&link_output.stderr);
            output.push('\n');
        }
        if link_output.status == LinkStatus::Skipped {
            output.push_str("\nlinking was skipped; compile output is the native object file.\n");
        }

        self.write("13_link.txt", &output)
    }

    pub(crate) fn write_timings(&self, timings: &[PhaseTiming]) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega Phase Timings\n\n");

        for timing in timings {
            output.push_str(&format!("{}: {} us\n", timing.phase, timing.microseconds));
        }

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

fn write_ir_data_definitions(output: &mut String, program: &Program) {
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

fn write_ir_invariants(output: &mut String, program: &Program) {
    output.push_str("## Invariants\n");

    if program.invariant_definitions.is_empty() {
        output.push_str("none\n\n");
        return;
    }

    for invariant in &program.invariant_definitions {
        output.push_str(&format!(
            "- invariant `{}` = {}\n",
            invariant.name,
            ir_constraint_span_name(program, invariant.constraints)
        ));
    }

    output.push('\n');
}

fn write_ir_platforms(output: &mut String, program: &Program) {
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
                ir_parameters_name(program, &state.parameters),
                ir_return_type_name(program, state.return_type.as_ref())
            ));
        }
    }

    output.push('\n');
}

fn write_ir_machines(output: &mut String, program: &Program) {
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
                ir_parameters_name(program, &state.parameters),
                ir_return_type_name(program, state.return_type.as_ref()),
                state.statements.len()
            ));
        }
    }

    output.push('\n');
}

fn ir_constraint_span_name(
    program: &Program,
    span: omega_core::arena::HandleSpan<crate::ir::types::TypeConstraint>,
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

fn ir_parameters_name(
    program: &Program,
    parameters: &[crate::ir::signature::StateParameter],
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

fn ir_return_type_name(
    program: &Program,
    return_type: Option<&crate::ir::types::TypeReference>,
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
    constraints: omega_core::arena::HandleSpan<crate::ir::types::TypeConstraint>,
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
            .map(crate::ir::types::TypeConstraint::display_name)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn proof_transition_target_name(target: &crate::ir::statement::TransitionTarget) -> String {
    match target {
        crate::ir::statement::TransitionTarget::Named { path, .. } => path.join("."),
        crate::ir::statement::TransitionTarget::SelfTarget => "self".to_owned(),
        crate::ir::statement::TransitionTarget::Terminal => "terminal".to_owned(),
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
        HostBindingMechanism::Syscall { name, number } => {
            output.push_str(&format!(
                "- {}.{} syscall {}({}) trust `{}`\n",
                binding.capability, binding.operation, name, number, binding.trust_policy
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
    operands: omega_core::arena::HandleSpan<crate::native::instructions::InstructionOperand>,
) -> String {
    let Some(operands) = native_plan.instructions.operands.span(operands) else {
        return "invalid operands".to_owned();
    };

    operands
        .iter()
        .map(|operand| match &operand.kind {
            InstructionOperandKind::DataAddress { symbol } => format!("addr {symbol}"),
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
