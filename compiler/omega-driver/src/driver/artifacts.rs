use std::fs;
use std::path::{Path, PathBuf};

use crate::diagnostics::Diagnostic;
use crate::driver::compile::{LoadedFile, LoadedProgram, PhaseTiming};
use crate::ir::Program;
use crate::native::control_flow::{
    ControlFlowPlan, Operation, PlannedTransitionTarget, StateFlow, TransitionFlow,
};
use crate::native::plan::NativePlan;
use crate::proof::obligations::ProofPlan;
use crate::semantic::effects::EffectPlan;

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
        self.write("02_ast.txt", &format!("{:#?}\n", loaded_program.items))
    }

    pub(crate) fn write_ir(&self, program: &Program) -> Result<(), Diagnostic> {
        self.write("05_driver_ir.txt", &format!("{program:#?}\n"))
    }

    pub(crate) fn write_effects(&self, effect_plan: &EffectPlan) -> Result<(), Diagnostic> {
        self.write("04_types.txt", &format!("{effect_plan:#?}\n"))
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

    pub(crate) fn write_control_flow(
        &self,
        control_flow: &ControlFlowPlan,
    ) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega Control Flow\n\n");
        output.push_str(&format!("machines: {}\n", control_flow.machines.len()));
        output.push_str(&format!("states: {}\n", control_flow.states.len()));
        output.push_str(&format!("operations: {}\n", control_flow.operations.len()));
        output.push_str(&format!(
            "transitions: {}\n\n",
            control_flow.transitions.len()
        ));

        for machine in &control_flow.machines {
            output.push_str(&format!("## machine {}\n", machine.name));

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

    pub(crate) fn write_proof_plan(&self, proof_plan: &ProofPlan) -> Result<(), Diagnostic> {
        self.write("08_proof.txt", &format!("{proof_plan:#?}\n"))
    }

    pub(crate) fn write_native_plan(&self, native_plan: &NativePlan) -> Result<(), Diagnostic> {
        self.write("09_native_plan.txt", &format!("{native_plan:#?}\n"))
    }

    pub(crate) fn write_timings(&self, timings: &[PhaseTiming]) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega Phase Timings\n\n");

        for timing in timings {
            output.push_str(&format!("{}: {} us\n", timing.phase, timing.microseconds));
        }

        self.write("00_timings.txt", &output)
    }

    pub(crate) fn write_placeholder(&self, file_name: &str, title: &str) -> Result<(), Diagnostic> {
        self.write(
            file_name,
            &format!("# {title}\n\nstatus: not implemented yet\n"),
        )
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
        "    - -> {} when {:?}",
        transition_target_name(&transition.target),
        transition.guard
    ));

    if let Some(continuation) = &transition.continuation {
        output.push_str(&format!(" -> {}", transition_target_name(continuation)));
    }

    output.push('\n');
}

fn transition_target_name(target: &PlannedTransitionTarget) -> String {
    match target {
        PlannedTransitionTarget::State { name, .. } => name.clone(),
        PlannedTransitionTarget::Nested { receiver, state } => format!("{receiver}.{state}"),
        PlannedTransitionTarget::SelfTarget => "self".to_owned(),
        PlannedTransitionTarget::Terminal => "terminal".to_owned(),
    }
}
