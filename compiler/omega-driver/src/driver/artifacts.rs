use std::fs;
use std::path::{Path, PathBuf};

use crate::ast::item::Item;
use crate::diagnostics::Diagnostic;
use crate::driver::compile::{LoadedFile, LoadedProgram, PhaseTiming};
use crate::ir::Program;
use crate::ir::statement::TransitionGuard;
use crate::native::control_flow::{
    ControlFlowPlan, Operation, PlannedTransitionTarget, StateFlow, TransitionFlow,
};
use crate::native::layout::{DataShape, FieldLayout};
use crate::native::object::{SectionPlan, SymbolPlan};
use crate::native::plan::NativePlan;
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
        self.write("05_driver_ir.txt", &format!("{program:#?}\n"))
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
        for machine in &control_flow.machines {
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

        for data_layout in &native_plan.layouts.data_layouts {
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

        for machine_layout in &native_plan.layouts.machine_layouts {
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

        self.write("09_native_plan.txt", &output)
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
    }
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

fn write_symbol_plan(output: &mut String, symbol: &SymbolPlan) {
    let section = symbol.section.as_deref().unwrap_or("none");
    output.push_str(&format!(
        "- symbol {} {:?}: section {}, offset {}, size {}\n",
        symbol.name, symbol.kind, section, symbol.offset, symbol.size
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
        PlannedTransitionTarget::Nested { receiver, state } => format!("{receiver}.{state}"),
        PlannedTransitionTarget::SelfTarget => "self".to_owned(),
        PlannedTransitionTarget::Terminal => "terminal".to_owned(),
    }
}
