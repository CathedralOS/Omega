use crate::statement::{
    StatementNode, TableCall, TableTransition, TransitionGuardNode, TransitionTargetNode,
};
use crate::{TypedTrees, state::State};
use crate::{data::DataMember, machine::Machine};
use omega_core::diagnostics::{PhaseDiagram, PhaseDiagramBuilder};
use omega_core::symbols::SymbolHandle;

impl PhaseDiagram for TypedTrees {
    fn phase_html(&self) -> String {
        let mut diagram = PhaseDiagramBuilder::new("typed_trees");
        let root = diagram.node("root", "TypedTrees", "root", 0);
        let mut data_nodes: Vec<(SymbolHandle, String, String)> = Vec::new();

        for (data_index, data) in self.data_definitions().iter().enumerate() {
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
            diagram.containment_edge(&root, &data_id);
            data_nodes.push((data.symbol, data_id, data.name.as_str().to_owned()));
        }

        for (data_index, data) in self.data_definitions().iter().enumerate() {
            let Some((_, data_id, _)) = data_nodes.get(data_index) else {
                continue;
            };
            for member in self.data_members(data) {
                if let DataMember::Field(field) = member {
                    let target_symbol = self.type_reference_symbol(field.type_reference);
                    if let Some(target_id) = data_id_for_symbol(&data_nodes, target_symbol) {
                        diagram.edge(data_id, target_id, "field_type");
                    }
                }
            }
        }

        for (machine_index, machine) in self.machines().iter().enumerate() {
            let machine_id = diagram.node(
                format!("machine_{machine_index}"),
                format!(
                    "machine {}\nsymbol: {}\nstates: {}",
                    machine.name.as_str(),
                    symbol_label(machine.symbol),
                    machine.states.len()
                ),
                "machine",
                1,
            );
            diagram.containment_edge(&root, &machine_id);
            append_machine_relationships(
                &mut diagram,
                self,
                &data_nodes,
                &machine_id,
                machine_index,
                machine,
            );

            let states = self.machine_states(machine);
            let state_nodes = states
                .iter()
                .enumerate()
                .map(|(state_index, state)| {
                    (
                        state.symbol,
                        state.name.as_str().to_owned(),
                        format!("state_{machine_index}_{state_index}"),
                    )
                })
                .collect::<Vec<_>>();

            for (state_index, state) in states.iter().enumerate() {
                append_state(
                    &mut diagram,
                    self,
                    &machine_id,
                    &state_nodes,
                    machine_index,
                    state_index,
                    state,
                );
            }
        }

        diagram.finish()
    }
}

fn append_machine_relationships(
    diagram: &mut PhaseDiagramBuilder,
    program: &TypedTrees,
    data_nodes: &[(SymbolHandle, String, String)],
    machine_id: &str,
    machine_index: usize,
    machine: &Machine,
) {
    if let Some(data_id) = data_id_for_name(data_nodes, machine.name.as_str()) {
        diagram.edge(data_id, machine_id, "implements_data");
    }

    for (object_index, object) in program
        .machine_contained_objects(machine)
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
        if let Some(data_id) = data_id_for_symbol(data_nodes, object.type_symbol) {
            diagram.edge(&object_id, data_id, "contained_object");
        }
    }

    for (owned_index, owned) in program.machine_owned_data(machine).iter().enumerate() {
        let object_id = diagram.node(
            format!("owned_{machine_index}_{owned_index}"),
            format!(
                "owns {}\ntype: {}\nsymbol: {}",
                owned.name.as_str(),
                program.display_type_reference(owned.type_reference),
                symbol_label(owned.symbol)
            ),
            "object",
            2,
        );
        diagram.containment_edge(machine_id, &object_id);
        let target_symbol = program.type_reference_symbol(owned.type_reference);
        if let Some(data_id) = data_id_for_symbol(data_nodes, target_symbol) {
            diagram.edge(&object_id, data_id, "owned_data");
        }
    }
}

fn append_state(
    diagram: &mut PhaseDiagramBuilder,
    program: &TypedTrees,
    parent_id: &str,
    state_nodes: &[(SymbolHandle, String, String)],
    machine_index: usize,
    state_index: usize,
    state: &State,
) {
    let state_id = diagram.node(
        format!("state_{machine_index}_{state_index}"),
        format!(
            "state {}\nsymbol: {}\nparams: {}\nstatements: {}",
            state.name.as_str(),
            symbol_label(state.symbol),
            state.parameters.len(),
            state.statement_nodes.len()
        ),
        "state",
        2,
    );
    diagram.containment_edge(parent_id, &state_id);

    let mut previous_id: Option<String> = None;
    for (statement_index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        let statement_id = diagram.node(
            format!("stmt_{machine_index}_{state_index}_{statement_index}"),
            format!(
                "{}: {}",
                statement_index,
                statement_label(program, statement)
            ),
            "statement",
            3,
        );
        diagram.containment_edge(&state_id, &statement_id);
        if let Some(previous_id) = previous_id.as_ref() {
            diagram.sequence_edge(previous_id, &statement_id);
        }
        if let StatementNode::Transition(transition) = statement {
            if let Some(target_id) = transition_target_id(program, state_nodes, transition) {
                diagram.edge(&statement_id, target_id, "transition_target");
            }
        }
        previous_id = Some(statement_id);
    }
}

fn statement_label(program: &TypedTrees, statement: &StatementNode) -> String {
    match statement {
        StatementNode::Assignment(_) => "assignment".to_owned(),
        StatementNode::Call(call) => call_label(call),
        StatementNode::Expression(_) => "expression".to_owned(),
        StatementNode::LocalData(local) => format!(
            "local {}: {}\nsymbol: {}",
            local.name.as_str(),
            program.display_type_reference_with_constraints(local.type_reference),
            symbol_label(local.symbol)
        ),
        StatementNode::Transition(transition) => transition_label(program, transition),
    }
}

fn call_label(call: &TableCall) -> String {
    format!(
        "call {}\ntarget: {}\nargs: {}",
        call.target.as_str(),
        symbol_label(call.target_symbol),
        call.arguments.len()
    )
}

fn transition_label(program: &TypedTrees, transition: &TableTransition) -> String {
    let target = transition_target_label(program, transition.target);
    let guard = match transition.guard {
        TransitionGuardNode::Always => "always",
        TransitionGuardNode::When(_) => "when",
    };
    format!("transition {target}\nguard: {guard}")
}

fn transition_target_label(
    program: &TypedTrees,
    target: crate::statement::TransitionTargetHandle,
) -> String {
    if !target.is_valid() {
        return "terminal".to_owned();
    }

    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named { path, arguments } => {
            let path = program
                .statement_table
                .name_path_members(path.members)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join(".");
            format!("{path}({})", arguments.len())
        }
        TransitionTargetNode::Value(_) => "value".to_owned(),
        TransitionTargetNode::SelfTarget => "self".to_owned(),
        TransitionTargetNode::Terminal => "terminal".to_owned(),
    }
}

fn transition_target_id<'a>(
    program: &TypedTrees,
    state_nodes: &'a [(SymbolHandle, String, String)],
    transition: &TableTransition,
) -> Option<&'a str> {
    if !transition.target.is_valid() {
        return None;
    }

    match program.statement_table.transition_target(transition.target) {
        TransitionTargetNode::Named { path, .. } => state_id_for_symbol(state_nodes, path.symbol)
            .or_else(|| state_id_for_name(state_nodes, transition_target_name(program, *path)?)),
        TransitionTargetNode::Value(_)
        | TransitionTargetNode::SelfTarget
        | TransitionTargetNode::Terminal => None,
    }
}

fn transition_target_name(
    program: &TypedTrees,
    path: crate::statement::TableNamePath,
) -> Option<&str> {
    program
        .statement_table
        .name_path_members(path.members)
        .last()
        .map(|member| member.as_str())
}

fn symbol_label(symbol: SymbolHandle) -> String {
    if symbol.is_valid() {
        format!("#{}", symbol.arena_index())
    } else {
        "invalid".to_owned()
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

fn state_id_for_name<'a>(
    state_nodes: &'a [(SymbolHandle, String, String)],
    name: &str,
) -> Option<&'a str> {
    state_nodes
        .iter()
        .find(|(_, state_name, _)| state_name == name)
        .map(|(_, _, state_id)| state_id.as_str())
}
