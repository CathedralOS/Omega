use crate::SyntaxTrees;
use crate::item::{Item, StateNode};
use crate::statement::{
    StatementNode, TableCall, TableTransition, TransitionGuardNode, TransitionTargetNode,
};
use omega_core::diagnostics::PhaseDiagram;

impl PhaseDiagram for SyntaxTrees {
    fn phase_mermaid(&self) -> String {
        let mut diagram = MermaidBuilder::new("syntax_trees");
        let root = diagram.node("root", "SyntaxTrees");

        for (item_index, item_handle) in self.root_item_handles().iter().copied().enumerate() {
            let item_id = diagram.node(
                format!("item_{item_index}"),
                item_label(self.root_item(item_handle)),
            );
            diagram.edge(&root, &item_id);

            if let Item::Machine(machine) = self.root_item(item_handle) {
                for (state_index, state_handle) in self
                    .items
                    .state_handles(machine.states)
                    .iter()
                    .copied()
                    .enumerate()
                {
                    let state = self.items.state(state_handle);
                    append_state(&mut diagram, self, &item_id, item_index, state_index, state);
                }
            }
        }

        diagram.finish()
    }
}

fn append_state(
    diagram: &mut MermaidBuilder,
    syntax: &SyntaxTrees,
    parent_id: &str,
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
    );
    diagram.edge(parent_id, &state_id);

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
        );
        diagram.edge(&state_id, &statement_id);
        if let Some(previous_id) = previous_id.as_ref() {
            diagram.edge(previous_id, &statement_id);
        }
        previous_id = Some(statement_id);
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

struct MermaidBuilder {
    output: String,
}

impl MermaidBuilder {
    fn new(title: &str) -> Self {
        let mut output = String::new();
        output.push_str("---\n");
        output.push_str("title: ");
        output.push_str(title);
        output.push_str("\n---\nflowchart TD\n");
        Self { output }
    }

    fn node(&mut self, id: impl AsRef<str>, label: impl AsRef<str>) -> String {
        let id = sanitize_id(id.as_ref());
        self.output.push_str("  ");
        self.output.push_str(&id);
        self.output.push_str("[\"");
        self.output.push_str(&escape_label(label.as_ref()));
        self.output.push_str("\"]\n");
        id
    }

    fn edge(&mut self, from: &str, to: &str) {
        self.output.push_str("  ");
        self.output.push_str(from);
        self.output.push_str(" --> ");
        self.output.push_str(to);
        self.output.push('\n');
    }

    fn finish(self) -> String {
        self.output
    }
}

fn sanitize_id(id: &str) -> String {
    id.chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

fn escape_label(label: &str) -> String {
    label
        .replace('\\', "\\\\")
        .replace('"', "&quot;")
        .replace('\n', "<br/>")
}
