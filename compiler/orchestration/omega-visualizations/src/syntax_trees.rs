use crate::phase_diagram::PhaseDiagramBuilder;
use omega_core::arena::HandleSpan;
use omega_syntax_trees::identifier::Identifier;
use omega_syntax_trees::item::{Item, StateNode, StateSignatureNode};
use omega_syntax_trees::statement::{
    StatementNode, TableCall, TableTransition, TransitionGuardNode, TransitionTargetHandle,
    TransitionTargetNode,
};
use omega_syntax_trees::SyntaxTrees;

pub fn syntax_trees_html(syntax: &SyntaxTrees) -> String {
    let mut diagram = PhaseDiagramBuilder::new("syntax_trees");
    let root = diagram.node("root", "SyntaxTrees", "root", 0);

    for (item_index, item_handle) in syntax.root_item_handles().iter().copied().enumerate() {
        let item_id = diagram.node(
            format!("item_{item_index}"),
            item_label(syntax.root_item(item_handle)),
            item_kind(syntax.root_item(item_handle)),
            1,
        );
        diagram.containment_edge(&root, &item_id);

        match syntax.root_item(item_handle) {
            Item::Machine(machine) => {
                let state_handles = syntax
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
                        let state = syntax.items.state(state_handle);
                        (
                            state.name.as_str().to_owned(),
                            format!("state_{item_index}_{state_index}"),
                        )
                    })
                    .collect::<Vec<_>>();

                for (state_index, state_handle) in state_handles.iter().copied().enumerate() {
                    let state = syntax.items.state(state_handle);
                    append_state(
                        &mut diagram,
                        syntax,
                        &item_id,
                        &state_nodes,
                        item_index,
                        state_index,
                        state,
                    );
                }
            }
            Item::Trait(trait_definition) => {
                for (signature_index, signature_handle) in syntax
                    .items
                    .state_signatures(trait_definition.machines)
                    .iter()
                    .copied()
                    .enumerate()
                {
                    append_state_signature(
                        &mut diagram,
                        &item_id,
                        item_index,
                        signature_index,
                        syntax.items.state_signature(signature_handle),
                    );
                }
            }
            _ => {}
        }
    }

    diagram.finish()
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
        state_label(syntax, state),
        "state",
        2,
    );
    diagram.containment_edge(parent_id, &state_id);

    for statement_handle in syntax.items.statements(state.statements).iter().copied() {
        if let StatementNode::Transition(transition) = syntax.statements.statement(statement_handle)
        {
            if let Some(target_id) = transition_target_id(syntax, state_nodes, transition) {
                diagram.edge(&state_id, target_id, "transition_target");
            }
        }
    }
}

fn state_label(syntax: &SyntaxTrees, state: &StateNode) -> String {
    let mut label = format!(
        "state {}\nparams: {}\nstatements: {}",
        state.name.as_str(),
        state.parameters.len(),
        state.statements.len()
    );

    for (statement_index, statement_handle) in syntax
        .items
        .statements(state.statements)
        .iter()
        .copied()
        .enumerate()
    {
        label.push('\n');
        label.push_str("  ");
        label.push_str(&statement_index.to_string());
        label.push_str(": ");
        label.push_str(&inline_label(statement_label(
            syntax,
            syntax.statements.statement(statement_handle),
        )));
    }

    label
}

fn inline_label(label: String) -> String {
    label.replace('\n', " | ")
}

fn append_state_signature(
    diagram: &mut PhaseDiagramBuilder,
    parent_id: &str,
    item_index: usize,
    signature_index: usize,
    signature: &StateSignatureNode,
) {
    let signature_id = diagram.node(
        format!("signature_{item_index}_{signature_index}"),
        format!(
            "machine {}\nparams: {}",
            signature.name.as_str(),
            signature.parameters.len()
        ),
        "state",
        2,
    );
    diagram.containment_edge(parent_id, &signature_id);
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
                "machine {}\nsatisfies: {}\nstates: {}",
                value.name.as_str(),
                value.satisfies.len(),
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
                "{} {}\nrequires: {}\nmachines: {}",
                prefix,
                value.name.as_str(),
                value.requires.len(),
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
        StatementNode::Assignment(assignment) => format!(
            "{} = {}",
            syntax.expressions.display_name(assignment.target),
            syntax.expressions.display_name(assignment.value)
        ),
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

fn transition_target_label(syntax: &SyntaxTrees, target: TransitionTargetHandle) -> String {
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

fn transition_target_name(syntax: &SyntaxTrees, path: HandleSpan<Identifier>) -> Option<&str> {
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
