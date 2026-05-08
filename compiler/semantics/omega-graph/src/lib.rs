//! State graph and transition analysis.
//!
//! This crate currently builds a source-level graph report from the AST. The
//! orchestration crate still owns the lowered control-flow plan until that
//! boundary is ready for its own representation crate.

use omega_abstract_syntax_tree::item::{Item, Machine, State};
use omega_abstract_syntax_tree::statement::{
    Statement, Transition, TransitionGuard, TransitionTarget,
};
use omega_core::arena::{Arena, HandleSpan};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceGraphReport {
    pub machines: Arena<SourceGraphMachine>,
    pub states: Arena<SourceGraphState>,
    pub transitions: Arena<SourceGraphTransition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceGraphMachine {
    pub name: String,
    pub states: HandleSpan<SourceGraphState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceGraphState {
    pub name: String,
    pub transitions: HandleSpan<SourceGraphTransition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceGraphTransition {
    pub target: String,
    pub continuation: String,
    pub guard: String,
}

pub fn build_source_graph_report(items: &[Item]) -> SourceGraphReport {
    let mut report = SourceGraphReport::default();

    for item in items {
        let Item::Machine(machine) = item else {
            continue;
        };

        let machine_graph = collect_machine(&mut report, machine);
        report.machines.insert(machine_graph);
    }

    report
}

fn collect_machine(report: &mut SourceGraphReport, machine: &Machine) -> SourceGraphMachine {
    let states = machine
        .states
        .iter()
        .map(|state| collect_state(report, state))
        .collect::<Vec<_>>();
    let states = report.states.insert_many(states);

    SourceGraphMachine {
        name: machine.name.to_string(),
        states,
    }
}

fn collect_state(report: &mut SourceGraphReport, state: &State) -> SourceGraphState {
    let transitions = state.statements.iter().filter_map(|statement| {
        let Statement::Transition(transition) = statement else {
            return None;
        };

        Some(collect_transition(transition))
    });
    let transitions = report.transitions.insert_many(transitions);

    SourceGraphState {
        name: state.name.to_string(),
        transitions,
    }
}

fn collect_transition(transition: &Transition) -> SourceGraphTransition {
    SourceGraphTransition {
        target: transition_target_name(&transition.target),
        continuation: transition
            .continuation
            .as_ref()
            .map(transition_target_name)
            .unwrap_or_else(|| "none".to_owned()),
        guard: transition_guard_name(&transition.guard),
    }
}

fn transition_target_name(target: &TransitionTarget) -> String {
    match target {
        TransitionTarget::Named { path, .. } => path.join("::"),
        TransitionTarget::SelfTarget => "self".to_owned(),
        TransitionTarget::Terminal => "terminal".to_owned(),
    }
}

fn transition_guard_name(guard: &TransitionGuard) -> String {
    match guard {
        TransitionGuard::Always => "always".to_owned(),
        TransitionGuard::When(expression) => format!("when {}", expression.display_name()),
    }
}

#[cfg(test)]
mod tests {
    use omega_abstract_syntax_tree::identifier::{Identifier, IdentifierPath};
    use omega_abstract_syntax_tree::item::{Item, Machine, State};
    use omega_abstract_syntax_tree::statement::{
        Statement, Transition, TransitionGuard, TransitionTarget,
    };

    use super::build_source_graph_report;

    fn identifier_path(members: &[&str]) -> IdentifierPath {
        members
            .iter()
            .copied()
            .map(Identifier::generated)
            .collect::<Vec<_>>()
            .into()
    }

    #[test]
    fn collects_machine_state_transitions() {
        let report = build_source_graph_report(&[Item::Machine(Machine {
            name: Identifier::generated("main"),
            contains: Vec::new(),
            owned_data: Vec::new(),
            states: vec![State {
                name: Identifier::generated("entry"),
                parameters: Vec::new(),
                return_type: None,
                statements: vec![Statement::Transition(Transition {
                    target: TransitionTarget::Named {
                        path: identifier_path(&["running"]),
                        arguments: Vec::new(),
                    },
                    continuation: None,
                    guard: TransitionGuard::Always,
                })],
            }],
        })]);

        assert_eq!(report.machines.len(), 1);
        assert_eq!(report.states.len(), 1);
        assert_eq!(report.transitions.len(), 1);

        let (_, transition) = report
            .transitions
            .iter()
            .next()
            .expect("transition should be recorded");

        assert_eq!(transition.target, "running");
        assert_eq!(transition.guard, "always");
    }
}
