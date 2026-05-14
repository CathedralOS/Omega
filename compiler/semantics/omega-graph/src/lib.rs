//! State graph and transition analysis.
//!
//! This crate currently builds a source-level graph report from syntax storage.
//! The orchestration crate still owns the lowered control-flow plan until that
//! boundary is ready for its own representation crate.

use omega_core::arena::{Arena, HandleSpan};
use omega_syntax_trees::item::Item;
use omega_syntax_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use omega_syntax_trees::SyntaxTrees;

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

pub fn build_source_graph_report(syntax_trees: &SyntaxTrees) -> SourceGraphReport {
    let mut report = SourceGraphReport::default();

    for item in syntax_trees.root_items() {
        let Item::Machine(machine) = item else {
            continue;
        };

        let machine_graph = collect_machine(&mut report, syntax_trees, machine);
        report.machines.insert(machine_graph);
    }

    report
}

fn collect_machine(
    report: &mut SourceGraphReport,
    syntax_trees: &SyntaxTrees,
    machine: &omega_syntax_trees::item::Machine,
) -> SourceGraphMachine {
    let states = syntax_trees
        .items
        .state_handles(machine.states)
        .iter()
        .map(|state| collect_state(report, syntax_trees, *state))
        .collect::<Vec<_>>();
    let states = report.states.insert_many(states);

    SourceGraphMachine {
        name: machine.name.to_string(),
        states,
    }
}

fn collect_state(
    report: &mut SourceGraphReport,
    syntax_trees: &SyntaxTrees,
    state: omega_syntax_trees::item::StateHandle,
) -> SourceGraphState {
    let state = syntax_trees.items.state(state);
    let transitions = syntax_trees
        .items
        .statements(state.statements)
        .iter()
        .filter_map(|statement| {
            let StatementNode::Transition(transition) = syntax_trees.statements.statement(*statement) else {
                return None;
            };

            Some(collect_transition(syntax_trees, transition))
        });
    let transitions = report.transitions.insert_many(transitions);

    SourceGraphState {
        name: state.name.to_string(),
        transitions,
    }
}

fn collect_transition(
    syntax_trees: &SyntaxTrees,
    transition: &omega_syntax_trees::statement::TableTransition,
) -> SourceGraphTransition {
    SourceGraphTransition {
        target: transition_target_name(syntax_trees, transition.target),
        continuation: if transition.continuation.is_valid() {
            transition_target_name(syntax_trees, transition.continuation)
        } else {
            "none".to_owned()
        },
        guard: transition_guard_name(syntax_trees, transition.guard),
    }
}

fn transition_target_name(
    syntax_trees: &SyntaxTrees,
    target: omega_syntax_trees::statement::TransitionTargetHandle,
) -> String {
    match syntax_trees.statements.transition_target(target) {
        TransitionTargetNode::Named { path, .. } => syntax_trees
            .statements
            .identifier_path_members(*path)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("::"),
        TransitionTargetNode::SelfTarget => "self".to_owned(),
        TransitionTargetNode::Terminal => "terminal".to_owned(),
        TransitionTargetNode::Value(expression) => {
            format!("value {}", syntax_trees.expressions.display_name(*expression))
        }
    }
}

fn transition_guard_name(
    syntax_trees: &SyntaxTrees,
    guard: TransitionGuardNode,
) -> String {
    match guard {
        TransitionGuardNode::Always => "always".to_owned(),
        TransitionGuardNode::When(expression) => {
            format!("when {}", syntax_trees.expressions.display_name(expression))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::build_source_graph_report;
    use omega_core::arena::HandleSpan;
    use omega_syntax_trees::identifier::Identifier;
    use omega_syntax_trees::item::{Item, Machine, State};
    use omega_syntax_trees::statement::{StatementNode, TableTransition, TransitionGuardNode, TransitionTargetNode};
    use omega_syntax_trees::types::TypeReferenceHandle;
    use omega_syntax_trees::SyntaxTrees;

    #[test]
    fn collects_machine_state_transitions() {
        let mut syntax_trees = SyntaxTrees::new(Default::default());

        let target_path = syntax_trees
            .statements
            .append_identifier_path_member(Identifier::generated("running"));
        let target = syntax_trees
            .statements
            .insert_transition_target(TransitionTargetNode::Named {
                path: HandleSpan::from_parts(target_path, 1),
                arguments: HandleSpan::empty(),
            });
        let transition = syntax_trees
            .statements
            .insert(StatementNode::Transition(TableTransition {
                target,
                continuation: omega_syntax_trees::statement::TransitionTargetHandle::invalid(),
                guard: TransitionGuardNode::Always,
            }));
        let transition = syntax_trees.items.append_statement_handle(transition);
        let state = syntax_trees.items.insert_state(&State {
            name: Identifier::generated("entry"),
            parameters: HandleSpan::empty(),
            return_type: TypeReferenceHandle::invalid(),
            statements: HandleSpan::from_parts(transition, 1),
        });
        let state = syntax_trees.items.append_state_handle(state);

        syntax_trees.push_root_item(Item::Machine(Machine {
            name: Identifier::generated("main"),
            states: HandleSpan::from_parts(state, 1),
        }));

        let report = build_source_graph_report(&syntax_trees);

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
