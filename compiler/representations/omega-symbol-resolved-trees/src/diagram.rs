use crate::statement::{Statement, Transition, TransitionGuard, TransitionTarget};
use crate::{SymbolResolvedTrees, state::State};
use omega_core::diagnostics::{PhaseDiagram, PhaseDiagramBuilder};
use omega_core::symbols::SymbolHandle;

impl PhaseDiagram for SymbolResolvedTrees {
    fn phase_html(&self) -> String {
        let mut diagram = PhaseDiagramBuilder::new("symbol_resolved_trees");
        let root = diagram.node("root", "SymbolResolvedTrees", "root", 0);

        for (data_index, data) in self.roots.data_definitions.iter().enumerate() {
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

        for (machine_index, machine) in self.roots.machines.iter().enumerate() {
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

            for (state_index, state_handle) in self
                .machine_state_handles(machine.states)
                .iter()
                .copied()
                .enumerate()
            {
                append_state(
                    &mut diagram,
                    self,
                    &machine_id,
                    machine_index,
                    state_index,
                    self.machine_state(state_handle),
                );
            }
        }

        diagram.finish()
    }
}

fn append_state(
    diagram: &mut PhaseDiagramBuilder,
    program: &SymbolResolvedTrees,
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
            state.statements.len()
        ),
        "state",
        2,
    );
    diagram.containment_edge(parent_id, &state_id);

    let mut previous_id: Option<String> = None;
    for (statement_index, statement) in program
        .state_statements(state.statements)
        .iter()
        .enumerate()
    {
        let statement_id = diagram.node(
            format!("stmt_{machine_index}_{state_index}_{statement_index}"),
            format!("{}: {}", statement_index, statement_label(statement)),
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

fn statement_label(statement: &Statement) -> String {
    match statement {
        Statement::Assignment(_) => "assignment".to_owned(),
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

fn symbol_label(symbol: SymbolHandle) -> String {
    if symbol.is_valid() {
        format!("#{}", symbol.arena_index())
    } else {
        "invalid".to_owned()
    }
}
