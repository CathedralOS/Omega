use crate::phase_diagram::PhaseDiagramBuilder;
use omega_core::symbols::SymbolHandle;
use omega_symbol_resolved_trees::SymbolResolvedTrees;
use omega_symbol_resolved_trees::data::DataMember;
use omega_symbol_resolved_trees::machine::Machine;
use omega_symbol_resolved_trees::state::State;
use omega_symbol_resolved_trees::statement::{
    Statement, Transition, TransitionGuard, TransitionTarget,
};
use omega_symbol_resolved_trees::trait_definition::TraitDefinition;
use omega_symbol_resolved_trees::types::TypeReference;

pub fn symbol_resolved_trees_html(program: &SymbolResolvedTrees) -> String {
    let mut diagram = PhaseDiagramBuilder::new("symbol_resolved_trees");
    let mut data_nodes: Vec<(SymbolHandle, String, String)> = Vec::new();
    let mut trait_nodes: Vec<(SymbolHandle, String, String)> = Vec::new();

    for (data_index, data) in program.roots.data_definitions.iter().enumerate() {
        let data_id = diagram.node(
            format!("data_{data_index}"),
            format!(
                "data {}\nsymbol: {}\nmembers: {}",
                data.name.as_str(),
                symbol_label(data.symbol),
                data.members.len()
            ),
            "data",
            1,
        );
        data_nodes.push((data.symbol, data_id, data.name.as_str().to_owned()));
    }

    for (trait_index, trait_definition) in program.roots.traits.iter().enumerate() {
        let trait_id = diagram.node(
            format!("trait_{trait_index}"),
            format!(
                "{} {}\nsymbol: {}\nrequires: {}\nmachines: {}",
                if trait_definition.is_boundary {
                    "boundary trait"
                } else {
                    "trait"
                },
                trait_definition.name.as_str(),
                symbol_label(trait_definition.symbol),
                trait_definition.requires.len(),
                trait_definition.machines.len()
            ),
            "trait",
            1,
        );
        trait_nodes.push((
            trait_definition.symbol,
            trait_id.clone(),
            trait_definition.name.as_str().to_owned(),
        ));
        append_trait_machine_signatures(
            &mut diagram,
            program,
            &trait_id,
            trait_index,
            trait_definition,
        );
    }

    for (trait_index, trait_definition) in program.roots.traits.iter().enumerate() {
        let Some((_, trait_id, _)) = trait_nodes.get(trait_index) else {
            continue;
        };

        append_trait_relationships(
            &mut diagram,
            program,
            &trait_nodes,
            trait_id,
            trait_definition,
        );
    }

    for (data_index, data) in program.roots.data_definitions.iter().enumerate() {
        let Some((_, data_id, _)) = data_nodes.get(data_index) else {
            continue;
        };
        for member in program.data_members(data.members) {
            if let DataMember::Field(field) = member {
                let target_symbol = type_reference_symbol(program, &field.type_reference);
                if let Some(target_id) =
                    type_id_for_symbol(&data_nodes, &trait_nodes, target_symbol)
                {
                    diagram.edge(data_id, target_id, "field_type");
                }
            }
        }
    }

    for (machine_index, machine) in program.roots.machines.iter().enumerate() {
        let machine_id = diagram.node(
            format!("machine_{machine_index}"),
            format!(
                "machine {}\nsymbol: {}\nsatisfies: {}\nstates: {}",
                machine.name.as_str(),
                symbol_label(machine.symbol),
                machine.satisfies.len(),
                machine.states.len()
            ),
            "machine",
            1,
        );
        append_machine_relationships(
            &mut diagram,
            program,
            &data_nodes,
            &trait_nodes,
            &machine_id,
            machine_index,
            machine,
        );

        let state_handles = program
            .machine_state_handles(machine.states)
            .iter()
            .copied()
            .collect::<Vec<_>>();
        let state_nodes = state_handles
            .iter()
            .copied()
            .enumerate()
            .map(|(state_index, state_handle)| {
                let state = program.machine_state(state_handle);
                (
                    state.symbol,
                    state.name.as_str().to_owned(),
                    format!("state_{machine_index}_{state_index}"),
                )
            })
            .collect::<Vec<_>>();

        for (state_index, state_handle) in state_handles.iter().copied().enumerate() {
            append_state(
                &mut diagram,
                program,
                &machine_id,
                &state_nodes,
                machine_index,
                state_index,
                program.machine_state(state_handle),
            );
        }
    }

    diagram.finish()
}

fn append_trait_relationships(
    diagram: &mut PhaseDiagramBuilder,
    program: &SymbolResolvedTrees,
    trait_nodes: &[(SymbolHandle, String, String)],
    trait_id: &str,
    trait_definition: &TraitDefinition,
) {
    for requirement in program.trait_requirements(trait_definition.requires) {
        if let Some(required_trait_id) = trait_id_for_symbol(trait_nodes, requirement.symbol) {
            diagram.edge(trait_id, required_trait_id, "requires_trait");
        }
    }
}

fn append_machine_relationships(
    diagram: &mut PhaseDiagramBuilder,
    program: &SymbolResolvedTrees,
    data_nodes: &[(SymbolHandle, String, String)],
    trait_nodes: &[(SymbolHandle, String, String)],
    machine_id: &str,
    machine_index: usize,
    machine: &Machine,
) {
    if let Some(data_id) = data_id_for_name(data_nodes, machine.name.as_str()) {
        diagram.edge(data_id, machine_id, "implements_data");
    }

    for conformance in program.machine_trait_conformances(machine.satisfies) {
        if let Some(trait_id) = trait_id_for_symbol(trait_nodes, conformance.symbol) {
            diagram.edge(machine_id, trait_id, "satisfies_trait");
        }
    }

    for (object_index, object) in program
        .machine_contained_objects(machine.contains)
        .iter()
        .enumerate()
    {
        let object_id = diagram.node(
            format!("object_{machine_index}_{object_index}"),
            format!(
                "contains {}\ntype: {}\nsymbol: {}\ntype symbol: {}",
                object.name.as_str(),
                object.type_name.as_str(),
                symbol_label(object.symbol),
                symbol_label(object.type_symbol)
            ),
            "object",
            2,
        );
        diagram.containment_edge(machine_id, &object_id);
        if let Some(type_id) = type_id_for_symbol(data_nodes, trait_nodes, object.type_symbol) {
            diagram.edge(&object_id, type_id, "contained_object");
        }
    }

    for (owned_index, owned) in program
        .machine_owned_data(machine.owned_data)
        .iter()
        .enumerate()
    {
        let object_id = diagram.node(
            format!("owned_{machine_index}_{owned_index}"),
            format!(
                "owns {}\ntype: {}\nsymbol: {}",
                owned.name.as_str(),
                owned
                    .type_reference
                    .display_name_with_constraints(&program.tables.types.constraints),
                symbol_label(owned.symbol)
            ),
            "object",
            2,
        );
        diagram.containment_edge(machine_id, &object_id);
        let target_symbol = type_reference_symbol(program, &owned.type_reference);
        if let Some(type_id) = type_id_for_symbol(data_nodes, trait_nodes, target_symbol) {
            diagram.edge(&object_id, type_id, "owned_data");
        }
    }
}

fn append_trait_machine_signatures(
    diagram: &mut PhaseDiagramBuilder,
    program: &SymbolResolvedTrees,
    trait_id: &str,
    trait_index: usize,
    trait_definition: &TraitDefinition,
) {
    for (machine_index, machine) in program
        .trait_machine_signatures(trait_definition.machines)
        .iter()
        .enumerate()
    {
        let machine_id = diagram.node(
            format!("trait_{trait_index}_machine_{machine_index}"),
            format!(
                "machine {}\nsymbol: {}\nparams: {}",
                machine.name.as_str(),
                symbol_label(machine.symbol),
                machine.parameters.len()
            ),
            "state",
            2,
        );
        diagram.containment_edge(trait_id, &machine_id);
    }
}

fn append_state(
    diagram: &mut PhaseDiagramBuilder,
    program: &SymbolResolvedTrees,
    parent_id: &str,
    state_nodes: &[(SymbolHandle, String, String)],
    machine_index: usize,
    state_index: usize,
    state: &State,
) {
    let state_id = diagram.node(
        format!("state_{machine_index}_{state_index}"),
        state_label(program, state),
        "state",
        2,
    );
    diagram.containment_edge(parent_id, &state_id);

    for statement in program.state_statements(state.statements) {
        if let Statement::Transition(transition) = statement {
            if let Some(target_id) = transition_target_id(state_nodes, transition) {
                diagram.edge(&state_id, target_id, "transition_target");
            }
        }
    }
}

fn state_label(program: &SymbolResolvedTrees, state: &State) -> String {
    let mut label = format!(
        "state {}\nsymbol: {}\nparams: {}\nstatements: {}",
        state.name.as_str(),
        symbol_label(state.symbol),
        state.parameters.len(),
        state.statements.len()
    );

    for (statement_index, statement) in program
        .state_statements(state.statements)
        .iter()
        .enumerate()
    {
        label.push('\n');
        label.push_str("  ");
        label.push_str(&statement_index.to_string());
        label.push_str(": ");
        label.push_str(&inline_label(statement_label(program, statement)));
    }

    label
}

fn inline_label(label: String) -> String {
    label.replace('\n', " | ")
}

fn statement_label(program: &SymbolResolvedTrees, statement: &Statement) -> String {
    match statement {
        Statement::Assignment(assignment) => format!(
            "{} = {}",
            program
                .tables
                .bodies
                .expressions
                .display_name(assignment.target),
            program
                .tables
                .bodies
                .expressions
                .display_name(assignment.value)
        ),
        Statement::Call(call) => format!(
            "call {}\ntarget: {}",
            call.target.as_str(),
            symbol_label(call.target_symbol)
        ),
        Statement::Expression(_) => "expression".to_owned(),
        Statement::LocalData(local) => format!(
            "local {}\nsymbol: {}",
            local.name.as_str(),
            symbol_label(local.symbol)
        ),
        Statement::Transition(transition) => transition_label(transition),
    }
}

fn transition_label(transition: &Transition) -> String {
    let guard = match transition.guard {
        TransitionGuard::Always => "always",
        TransitionGuard::When(_) => "when",
    };
    format!(
        "transition {}\nguard: {guard}",
        transition_target_label(&transition.target)
    )
}

fn transition_target_label(target: &TransitionTarget) -> String {
    match target {
        TransitionTarget::Named(named) => {
            format!("state {}", symbol_label(named.symbol))
        }
        TransitionTarget::Value(_) => "value".to_owned(),
        TransitionTarget::SelfTarget => "self".to_owned(),
        TransitionTarget::Terminal => "terminal".to_owned(),
    }
}

fn transition_target_id<'a>(
    state_nodes: &'a [(SymbolHandle, String, String)],
    transition: &Transition,
) -> Option<&'a str> {
    match &transition.target {
        TransitionTarget::Named(named) => state_id_for_symbol(state_nodes, named.symbol),
        TransitionTarget::Value(_) | TransitionTarget::SelfTarget | TransitionTarget::Terminal => {
            None
        }
    }
}

fn symbol_label(symbol: SymbolHandle) -> String {
    if symbol.is_valid() {
        format!("#{}", symbol.arena_index())
    } else {
        "invalid".to_owned()
    }
}

fn type_reference_symbol(
    program: &SymbolResolvedTrees,
    type_reference: &TypeReference,
) -> SymbolHandle {
    match type_reference {
        TypeReference::Reference(reference) => type_reference_symbol(
            program,
            program
                .tables
                .declarations
                .child_type_references
                .get(reference.referee),
        ),
        TypeReference::Constrained(constrained) => type_reference_symbol(
            program,
            program
                .tables
                .declarations
                .child_type_references
                .get(constrained.base_type),
        ),
        TypeReference::FixedArray(fixed_array) => type_reference_symbol(
            program,
            program
                .tables
                .declarations
                .child_type_references
                .get(fixed_array.element_type),
        ),
        TypeReference::Slice(slice) => type_reference_symbol(
            program,
            program
                .tables
                .declarations
                .child_type_references
                .get(slice.element_type),
        ),
        TypeReference::Generic(generic) => generic.base_symbol,
        TypeReference::Named { symbol, .. } | TypeReference::SelfType { symbol } => *symbol,
        TypeReference::Unit => SymbolHandle::invalid(),
    }
}

fn data_id_for_symbol(
    data_nodes: &[(SymbolHandle, String, String)],
    symbol: SymbolHandle,
) -> Option<&str> {
    if !symbol.is_valid() {
        return None;
    }

    data_nodes
        .iter()
        .find(|(data_symbol, _, _)| *data_symbol == symbol)
        .map(|(_, data_id, _)| data_id.as_str())
}

fn type_id_for_symbol<'a>(
    data_nodes: &'a [(SymbolHandle, String, String)],
    trait_nodes: &'a [(SymbolHandle, String, String)],
    symbol: SymbolHandle,
) -> Option<&'a str> {
    data_id_for_symbol(data_nodes, symbol).or_else(|| trait_id_for_symbol(trait_nodes, symbol))
}

fn trait_id_for_symbol(
    trait_nodes: &[(SymbolHandle, String, String)],
    symbol: SymbolHandle,
) -> Option<&str> {
    if !symbol.is_valid() {
        return None;
    }

    trait_nodes
        .iter()
        .find(|(trait_symbol, _, _)| *trait_symbol == symbol)
        .map(|(_, trait_id, _)| trait_id.as_str())
}

fn data_id_for_name<'a>(
    data_nodes: &'a [(SymbolHandle, String, String)],
    name: &str,
) -> Option<&'a str> {
    data_nodes
        .iter()
        .find(|(_, _, data_name)| data_name == name)
        .map(|(_, data_id, _)| data_id.as_str())
}

fn state_id_for_symbol(
    state_nodes: &[(SymbolHandle, String, String)],
    symbol: SymbolHandle,
) -> Option<&str> {
    if !symbol.is_valid() {
        return None;
    }

    state_nodes
        .iter()
        .find(|(state_symbol, _, _)| *state_symbol == symbol)
        .map(|(_, _, state_id)| state_id.as_str())
}
