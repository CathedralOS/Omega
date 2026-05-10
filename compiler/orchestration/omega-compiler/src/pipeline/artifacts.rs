use std::path::{Path, PathBuf};

use omega_artifacts::{
    ArtifactWriter as ArtifactSink, AstArtifact, EmissionPlan, ExecutableFinalization, PhaseTiming,
    SourceLoadArtifact, TrustReport,
};
use omega_control_flow::{
    ControlFlowPlan, Operation, PlannedTransitionTarget, StateFlow, TransitionFlow,
};
use omega_core::diagnostics::Diagnostic;
use omega_effects::{EffectPlan, StateEffects};
use omega_graph::{SourceGraphReport, SourceGraphState};
use omega_image::EmittedImageOutput;
use omega_names::ResolveReport;
use omega_proof::ProofSurfaceReport;
use omega_proof::obligations::{ProofObligation, ProofPlan};
use omega_typed_program::Program;
use omega_typed_program::data::DataMember;
use omega_typed_program::statement::TransitionGuard;
use omega_types::TypeSurfaceReport;

pub(crate) struct ArtifactWriter {
    sink: ArtifactSink,
}

impl ArtifactWriter {
    pub(crate) fn new(build_dir: &Path) -> Result<Self, Diagnostic> {
        Ok(Self {
            sink: ArtifactSink::new(build_dir)?,
        })
    }

    pub(crate) fn write_sources(
        &self,
        source_artifact: &SourceLoadArtifact,
    ) -> Result<(), Diagnostic> {
        self.sink.write_sources(source_artifact)
    }

    pub(crate) fn write_ast(&self, ast_artifact: &AstArtifact) -> Result<(), Diagnostic> {
        self.sink.write_ast(ast_artifact)
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
        let resolved_references = resolve_report
            .references
            .iter()
            .filter(|(_, reference)| reference.symbol.is_valid())
            .count();
        output.push_str(&format!("resolved references: {resolved_references}\n\n"));
        output.push_str("## Symbol Arenas\n");
        output.push_str(&format!(
            "symbols: {}\n",
            resolve_report.symbols.symbols().len()
        ));
        output.push_str(&format!(
            "names: {}\n",
            resolve_report.symbols.names().len()
        ));
        let name_storage = resolve_report.symbols.name_storage_counts();
        output.push_str(&format!("source names: {}\n", name_storage.source_names));
        output.push_str(&format!("static names: {}\n", name_storage.static_names));
        output.push_str(&format!("owned names: {}\n", name_storage.owned_names));
        output.push_str(&format!(
            "debug names: {}\n",
            resolve_report.symbols.debug_names().len()
        ));
        output.push_str(&format!(
            "stored path members: {}\n\n",
            resolve_report.symbols.path_member_arena().len()
        ));
        let reference_name_storage = resolve_report.name_storage_counts();
        output.push_str("## Name Members\n");
        output.push_str(&format!(
            "source members: {}\n",
            reference_name_storage.source_members
        ));
        output.push_str(&format!(
            "generated members: {}\n",
            reference_name_storage.generated_members
        ));
        output.push_str(&format!(
            "missing members: {}\n\n",
            reference_name_storage.missing
        ));

        output.push_str("## Imports\n");
        for (_, import) in resolve_report.imports.iter() {
            output.push_str(&format!("- {}\n", resolve_report.import_path(import)));
        }

        output.push_str("\n## Definitions\n");
        for (_, definition) in resolve_report.definitions.iter() {
            let name = resolve_report.symbols.name(definition.symbol);
            let symbol_suffix = if definition.symbol.is_valid() {
                format!(" symbol #{}", definition.symbol.arena_index())
            } else {
                " unresolved".to_owned()
            };
            output.push_str(&format!(
                "- {:?} `{}`{}\n",
                definition.kind, name, symbol_suffix
            ));
        }

        output.push_str("\n## References\n");
        for (_, reference) in resolve_report.references.iter() {
            let reference_name = resolve_report.reference_name(reference);
            let resolved_path = resolve_report.symbols.display_path(reference.symbol, "::");
            let resolved_suffix = if resolved_path.is_empty() {
                " unresolved".to_owned()
            } else {
                format!(
                    " -> {resolved_path} symbol #{}",
                    reference.symbol.arena_index()
                )
            };
            output.push_str(&format!(
                "- {:?} `{}` from {}{}\n",
                reference.kind, reference_name, reference.owner, resolved_suffix
            ));
        }

        self.write_text("03_resolve.txt", &output)
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
        output.push_str(&format!(
            "tables: expressions {} type_references {} type_constraints {}\n\n",
            program.expression_table.expression_count(),
            program.type_reference_table.type_reference_count(),
            program.type_reference_table.constraint_count()
        ));

        let identity_storage = omega_typed_program::identity::count_identity_storage(program);
        output.push_str("## Identity Storage\n");
        output.push_str(&format!(
            "owned identity strings: {}\n",
            identity_storage.owned_identity_strings()
        ));
        output.push_str(&format!(
            "declaration names: {}\n",
            identity_storage.declaration_names
        ));
        output.push_str(&format!(
            "source declaration names: {}\n",
            identity_storage.source_declaration_names
        ));
        output.push_str(&format!(
            "generated declaration names: {}\n",
            identity_storage.generated_declaration_names
        ));
        output.push_str(&format!("type names: {}\n", identity_storage.type_names));
        output.push_str(&format!(
            "source type names: {}\n",
            identity_storage.source_type_names
        ));
        output.push_str(&format!(
            "generated type names: {}\n",
            identity_storage.generated_type_names
        ));
        output.push_str(&format!(
            "expression path members: {}\n",
            identity_storage.expression_path_members
        ));
        output.push_str(&format!(
            "source expression path members: {}\n",
            identity_storage.source_expression_path_members
        ));
        output.push_str(&format!(
            "generated expression path members: {}\n",
            identity_storage.generated_expression_path_members
        ));
        output.push_str(&format!(
            "transition path members: {}\n",
            identity_storage.transition_path_members
        ));
        output.push_str(&format!(
            "source transition path members: {}\n",
            identity_storage.source_transition_path_members
        ));
        output.push_str(&format!(
            "generated transition path members: {}\n",
            identity_storage.generated_transition_path_members
        ));
        output.push_str(&format!("call names: {}\n", identity_storage.call_names));
        output.push_str(&format!(
            "source call names: {}\n",
            identity_storage.source_call_names
        ));
        output.push_str(&format!(
            "generated call names: {}\n",
            identity_storage.generated_call_names
        ));
        output.push_str(&format!(
            "struct literal names: {}\n",
            identity_storage.struct_literal_names
        ));
        output.push_str(&format!(
            "source struct literal names: {}\n",
            identity_storage.source_struct_literal_names
        ));
        output.push_str(&format!(
            "generated struct literal names: {}\n",
            identity_storage.generated_struct_literal_names
        ));
        output.push_str(&format!(
            "string literals: {}\n",
            identity_storage.string_literals
        ));
        output.push_str(&format!(
            "float literals: {}\n",
            identity_storage.float_literals
        ));
        output.push_str(&format!(
            "parsed float literals: {}\n\n",
            identity_storage.parsed_float_literals
        ));

        output.push_str("## Symbol Arenas\n");
        output.push_str(&format!("symbols: {}\n", program.symbols.symbols().len()));
        output.push_str(&format!("names: {}\n", program.symbols.names().len()));
        let name_storage = program.symbols.name_storage_counts();
        output.push_str(&format!("source names: {}\n", name_storage.source_names));
        output.push_str(&format!("static names: {}\n", name_storage.static_names));
        output.push_str(&format!("owned names: {}\n", name_storage.owned_names));
        output.push_str(&format!(
            "debug names: {}\n",
            program.symbols.debug_names().len()
        ));
        output.push_str(&format!(
            "stored path members: {}\n\n",
            program.symbols.path_member_arena().len()
        ));

        write_typed_program_data_definitions(&mut output, program);
        write_typed_program_invariants(&mut output, program);
        write_typed_program_platforms(&mut output, program);
        write_typed_program_machines(&mut output, program);

        self.write_text("05_typed_program.txt", &output)
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

        self.write_text("04_types.txt", &output)
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

        self.write_text("06_validation.txt", &output)
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

        self.write_text("07_graph.txt", &output)
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

        self.write_text("08_proof.txt", &output)
    }

    pub(crate) fn write_trust_report(&self, trust_report: &TrustReport) -> Result<(), Diagnostic> {
        self.sink.write_trust_report(trust_report)
    }

    pub(crate) fn write_emission_plan(
        &self,
        emission_plan: &EmissionPlan,
    ) -> Result<(), Diagnostic> {
        self.sink.write_emission_plan(emission_plan)
    }

    pub(crate) fn write_emitted_native_output(
        &self,
        emitted_output: &EmittedImageOutput,
    ) -> Result<PathBuf, Diagnostic> {
        self.sink.write_emitted_native_output(emitted_output)
    }

    pub(crate) fn remove_stale_native_output(&self) -> Result<(), Diagnostic> {
        self.sink.remove_stale_native_output()
    }

    pub(crate) fn write_executable_finalization_report(
        &self,
        finalization: &ExecutableFinalization,
    ) -> Result<(), Diagnostic> {
        self.sink.write_executable_finalization_report(finalization)
    }

    pub(crate) fn write_timings(&self, timings: &[PhaseTiming]) -> Result<(), Diagnostic> {
        self.sink.write_timings(timings)
    }

    pub(crate) fn root(&self) -> &Path {
        self.sink.root()
    }

    pub(crate) fn write_text(&self, file_name: &str, contents: &str) -> Result<(), Diagnostic> {
        self.sink.write_text(file_name, contents)
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
        omega_typed_program::statement::TransitionTarget::Named { path, .. } => {
            omega_typed_program::expression::display_name_path(path, ".")
        }
        omega_typed_program::statement::TransitionTarget::SelfTarget => "self".to_owned(),
        omega_typed_program::statement::TransitionTarget::Terminal => "terminal".to_owned(),
        omega_typed_program::statement::TransitionTarget::Value(expression) => {
            format!("value {}", expression.display_name())
        }
    }
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
        PlannedTransitionTarget::State { name, .. } => name.to_string(),
        PlannedTransitionTarget::Nested {
            receiver, state, ..
        } => format!("{receiver}.{state}"),
        PlannedTransitionTarget::SelfTarget => "self".to_owned(),
        PlannedTransitionTarget::Terminal => "terminal".to_owned(),
    }
}
