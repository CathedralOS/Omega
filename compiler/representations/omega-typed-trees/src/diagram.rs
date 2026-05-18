use crate::statement::{
    StatementNode, TableCall, TableTransition, TransitionGuardNode, TransitionTargetNode,
};
use crate::{TypedTrees, state::State};
use omega_core::diagnostics::{PhaseDiagram, PhaseDiagramBuilder};
use omega_core::symbols::SymbolHandle;

impl PhaseDiagram for TypedTrees {
    fn phase_html(&self) -> String {
        let mut diagram = PhaseDiagramBuilder::new("typed_trees");
        let root = diagram.node("root", "TypedTrees", "root", 0);

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

            for (state_index, state) in self.machine_states(machine).iter().enumerate() {
                append_state(
                    &mut diagram,
                    self,
                    &machine_id,
                    machine_index,
                    state_index,
                    state,
                );
            }
        }

        diagram.finish()
    }
}

fn append_state(
    diagram: &mut PhaseDiagramBuilder,
    program: &TypedTrees,
    parent_id: &str,
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

fn symbol_label(symbol: SymbolHandle) -> String {
    if symbol.is_valid() {
        format!("#{}", symbol.arena_index())
    } else {
        "invalid".to_owned()
    }
}
