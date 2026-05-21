use crate::SyntaxTrees;
use crate::item::{Item, StateNode};
use crate::statement::{
    StatementNode, TableCall, TableTransition, TransitionGuardNode, TransitionTargetNode,
};
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::{PhaseDiagram, PhaseDiagramBuilder};

impl PhaseDiagram for SyntaxTrees {
    fn phase_html(&self) -> String {
        let mut diagram = PhaseDiagramBuilder::new("syntax_trees");
        let root = diagram.node("root", "SyntaxTrees", "root", 0);

        for (item_index, item_handle) in self.root_item_handles().iter().copied().enumerate() {
            let item_id = diagram.node(
                format!("item_{item_index}"),
                item_label(self.root_item(item_handle)),
                item_kind(self.root_item(item_handle)),
                1,
            );
            diagram.containment_edge(&root, &item_id);

            if let Item::Machine(machine) = self.root_item(item_handle) {
                let state_handles = self
                    .items
                    .state_handles(machine.states)
                    .iter()
                    .copied()
                    .collect::<Vec<_>>();
                let state_nodes = state_handles
                    .iter()
                    .copied()
                    .enumerate()
                    .map(|(state_index, state_handle)| {
                        let state = self.items.state(state_handle);
                        (
                            state.name.as_str().to_owned(),
                            format!("state_{item_index}_{state_index}"),
                        )
                    })
                    .collect::<Vec<_>>();

                for (state_index, state_handle) in state_handles.iter().copied().enumerate() {
                    let state = self.items.state(state_handle);
                    append_state(
                        &mut diagram,
                        self,
                        &item_id,
                        &state_nodes,
                        item_index,
                        state_index,
                        state,
                    );
                }
            }
        }

        diagram.finish()
    }
}

fn append_state(
    diagram: &mut PhaseDiagramBuilder,
    syntax: &SyntaxTrees,
    parent_id: &str,
    state_nodes: &[(String, String)],
    item_index: usize,
    state_index: usize,
    state: &StateNode,
) {
    let state_id = diagram.node(
        format!("state_{item_index}_{state_index}"),
        format!(
            "state {}\nparams: {}\nstatements: {}",
            state.name.as_str(),
            state.parameters.len(),
            state.statements.len()
        ),
        "state",
        2,
    );
    diagram.containment_edge(parent_id, &state_id);

    let mut previous_id: Option<String> = None;
    for (statement_index, statement_handle) in syntax
        .items
        .statements(state.statements)
        .iter()
        .copied()
        .enumerate()
    {
        let statement_id = diagram.node(
            format!("stmt_{item_index}_{state_index}_{statement_index}"),
            format!(
                "{}: {}",
                statement_index,
                statement_label(syntax, syntax.statements.statement(statement_handle))
            ),
            "statement",
            3,
        );
        diagram.containment_edge(&state_id, &statement_id);
        if let Some(previous_id) = previous_id.as_ref() {
            diagram.sequence_edge(previous_id, &statement_id);
        }
        if let StatementNode::Transition(transition) = syntax.statements.statement(statement_handle)
        {
            if let Some(target_id) = transition_target_id(syntax, state_nodes, transition) {
                diagram.edge(&statement_id, target_id, "transition_target");
            }
        }
        previous_id = Some(statement_id);
    }
}

fn item_kind(item: &Item) -> &'static str {
    match item {
        Item::Data(_) => "data",
        Item::Machine(_) | Item::Platform(_) => "machine",
        Item::Trait(_) => "trait",
        _ => "state",
    }
}

fn item_label(item: &Item) -> String {
    match item {
        Item::Capability(value) => format!("capability {}", value.name.as_str()),
        Item::Data(value) => format!(
            "data {}\nmembers: {}",
            value.name.as_str(),
            value.members.len()
        ),
        Item::Invariant(value) => format!("invariant {}", value.name.as_str()),
        Item::Library(value) => {
            let name = value
                .name
                .as_ref()
                .map(|name| name.as_str())
                .unwrap_or("<anon>");
            format!("library {name}\nfunctions: {}", value.functions.len())
        }
        Item::Machine(value) => {
            format!(
                "machine {}\nstates: {}",
                value.name.as_str(),
                value.states.len()
            )
        }
        Item::Platform(value) => {
            format!(
                "platform {}\nstates: {}",
                value.name.as_str(),
                value.states.len()
            )
        }
        Item::Trait(value) => {
            let prefix = if value.is_boundary {
                "boundary trait"
            } else {
                "trait"
            };
            format!(
                "{} {}\nmachines: {}",
                prefix,
                value.name.as_str(),
                value.machines.len()
            )
        }
        Item::Target(value) => format!("target {}", value.name.as_str()),
        Item::TrustDefinition(value) => format!("trust {}", value.name.as_str()),
        Item::Use(value) => format!("use\nsegments: {}", value.path.len()),
    }
}

fn statement_label(syntax: &SyntaxTrees, statement: &StatementNode) -> String {
    match statement {
        StatementNode::Assignment(_) => "assignment".to_owned(),
        StatementNode::Call(call) => call_label(syntax, call),
        StatementNode::Expression(_) => "expression".to_owned(),
        StatementNode::LocalData(value) => format!("local {}", value.name.as_str()),
        StatementNode::Transition(transition) => transition_label(syntax, transition),
    }
}

fn call_label(syntax: &SyntaxTrees, call: &TableCall) -> String {
    let receiver = syntax
        .statements
        .identifier_path_members(call.receiver)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join(".");
    let target = if receiver.is_empty() {
        call.target.as_str().to_owned()
    } else {
        format!("{receiver}.{}", call.target.as_str())
    };
    format!("call {target}\nargs: {}", call.arguments.len())
}

fn transition_label(syntax: &SyntaxTrees, transition: &TableTransition) -> String {
    let target = transition_target_label(syntax, transition.target);
    let guard = match transition.guard {
        TransitionGuardNode::Always => "always",
        TransitionGuardNode::When(_) => "when",
    };
    format!("transition {target}\nguard: {guard}")
}

fn transition_target_label(
    syntax: &SyntaxTrees,
    target: crate::statement::TransitionTargetHandle,
) -> String {
    if !target.is_valid() {
        return "terminal".to_owned();
    }

    match syntax.statements.transition_target(target) {
        TransitionTargetNode::Named {
            path, arguments, ..
        } => {
            let path = syntax
                .statements
                .identifier_path_members(*path)
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
    syntax: &SyntaxTrees,
    state_nodes: &'a [(String, String)],
    transition: &TableTransition,
) -> Option<&'a str> {
    if !transition.target.is_valid() {
        return None;
    }

    match syntax.statements.transition_target(transition.target) {
        TransitionTargetNode::Named { path, .. } => {
            state_id_for_name(state_nodes, transition_target_name(syntax, *path)?)
        }
        TransitionTargetNode::Value(_)
        | TransitionTargetNode::SelfTarget
        | TransitionTargetNode::Terminal => None,
    }
}

fn transition_target_name(
    syntax: &SyntaxTrees,
    path: HandleSpan<crate::identifier::Identifier>,
) -> Option<&str> {
    syntax
        .statements
        .identifier_path_members(path)
        .last()
        .map(|member| member.as_str())
}

fn state_id_for_name<'a>(state_nodes: &'a [(String, String)], name: &str) -> Option<&'a str> {
    state_nodes
        .iter()
        .find(|(state_name, _)| state_name == name)
        .map(|(_, state_id)| state_id.as_str())
}
