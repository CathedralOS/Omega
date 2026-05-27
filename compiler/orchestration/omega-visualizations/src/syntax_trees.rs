use crate::phase_diagram::PhaseDiagramBuilder;
use omega_core::arena::HandleSpan;
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::identifier::Identifier;
use omega_syntax_trees::item::{Item, ItemHandle, StateNode, StateSignatureNode};
use omega_syntax_trees::statement::{
    StatementNode, TableCall, TableTransition, TransitionGuardNode, TransitionTargetHandle,
    TransitionTargetNode,
};
use omega_syntax_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub struct SyntaxSourceFile {
    pub path: String,
    pub root_items: Vec<ItemHandle>,
}

pub fn syntax_trees_html(syntax: &SyntaxTrees) -> String {
    syntax_trees_with_files_html(syntax, &[])
}

pub fn syntax_trees_with_files_html(syntax: &SyntaxTrees, files: &[SyntaxSourceFile]) -> String {
    let mut diagram = PhaseDiagramBuilder::new("syntax_trees");
    if files.is_empty() {
        for (item_index, item_handle) in syntax.root_item_handles().iter().copied().enumerate() {
            append_root_item(
                &mut diagram,
                syntax,
                None,
                item_index,
                item_index,
                item_handle,
            );
        }
    } else {
        let file_nodes = files
            .iter()
            .enumerate()
            .map(|(file_index, file)| {
                diagram.node(
                    format!("file_{file_index}"),
                    format!("file {}\nitems: {}", file.path, file.root_items.len()),
                    "file",
                    1,
                )
            })
            .collect::<Vec<_>>();
        for (file_index, file) in files.iter().enumerate() {
            let file_id = &file_nodes[file_index];

            for (item_index, item_handle) in file.root_items.iter().copied().enumerate() {
                append_root_item(
                    &mut diagram,
                    syntax,
                    Some(file_id),
                    file_index,
                    item_index,
                    item_handle,
                );
            }
        }
    }

    diagram.finish()
}

fn append_root_item(
    diagram: &mut PhaseDiagramBuilder,
    syntax: &SyntaxTrees,
    parent_id: Option<&str>,
    owner_index: usize,
    item_index: usize,
    item_handle: ItemHandle,
) {
    let item = syntax.root_item(item_handle);
    let item_id = diagram.node(
        format!("item_{owner_index}_{item_index}"),
        item_label(syntax, item),
        item_kind(item),
        2,
    );
    if let Some(parent_id) = parent_id {
        diagram.containment_edge(parent_id, &item_id);
    }

    match item {
        Item::Machine(machine) => {
            let state_handles = syntax
                .items
                .state_handles(machine.states)
                .iter()
                .copied()
                .collect::<Vec<_>>();
            let entry_state = state_handles
                .first()
                .copied()
                .map(|state_handle| syntax.items.state(state_handle));
            let targetable_states = state_handles
                .iter()
                .copied()
                .enumerate()
                .skip(1)
                .collect::<Vec<_>>();
            let state_nodes = targetable_states
                .iter()
                .copied()
                .map(|(state_index, state_handle)| {
                    let state = syntax.items.state(state_handle);
                    (
                        state.name.as_str().to_owned(),
                        format!("state_{owner_index}_{item_index}_{state_index}"),
                    )
                })
                .collect::<Vec<_>>();
            append_entry_transitions(diagram, syntax, &item_id, &state_nodes, entry_state);

            for (state_index, state_handle) in targetable_states {
                let state = syntax.items.state(state_handle);
                append_state(
                    diagram,
                    syntax,
                    &item_id,
                    &state_nodes,
                    owner_index,
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
                    diagram,
                    syntax,
                    &item_id,
                    owner_index,
                    item_index,
                    signature_index,
                    syntax.items.state_signature(signature_handle),
                );
            }
        }
        _ => {}
    }
}

fn append_entry_transitions(
    diagram: &mut PhaseDiagramBuilder,
    syntax: &SyntaxTrees,
    machine_id: &str,
    state_nodes: &[(String, String)],
    entry_state: Option<&StateNode>,
) {
    let Some(entry_state) = entry_state else {
        return;
    };

    for statement_handle in syntax
        .items
        .statements(entry_state.statements)
        .iter()
        .copied()
    {
        if let StatementNode::Transition(transition) = syntax.statements.statement(statement_handle)
        {
            if let Some(target_id) = transition_target_id(syntax, state_nodes, transition) {
                diagram.edge(machine_id, target_id, "transition_target");
            }
        }
    }
}

fn append_state(
    diagram: &mut PhaseDiagramBuilder,
    syntax: &SyntaxTrees,
    parent_id: &str,
    state_nodes: &[(String, String)],
    owner_index: usize,
    item_index: usize,
    state_index: usize,
    state: &StateNode,
) {
    let state_id = diagram.node(
        format!("state_{owner_index}_{item_index}_{state_index}"),
        state_label(syntax, state),
        "state",
        3,
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
        "state {}\nparams: {}",
        state.name.as_str(),
        state.parameters.len()
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

fn contract_summary(
    syntax: &SyntaxTrees,
    contracts: HandleSpan<omega_syntax_trees::item::CapabilityContract>,
) -> String {
    if contracts.len() == 0 {
        return "0".to_owned();
    }

    format!(
        "{} / facts: {}",
        contracts.len(),
        contract_fact_count(syntax, contracts)
    )
}

fn contract_fact_count(
    syntax: &SyntaxTrees,
    contracts: HandleSpan<omega_syntax_trees::item::CapabilityContract>,
) -> usize {
    syntax
        .items
        .capability_contracts(contracts)
        .iter()
        .map(|contract| syntax.items.proof_facts(contract.facts).len())
        .sum()
}

fn append_contract_facts(
    label: &mut String,
    syntax: &SyntaxTrees,
    contracts: HandleSpan<omega_syntax_trees::item::CapabilityContract>,
) {
    for (contract_index, contract) in syntax
        .items
        .capability_contracts(contracts)
        .iter()
        .enumerate()
    {
        let facts = proof_fact_labels(syntax, contract.facts);
        if facts.is_empty() {
            continue;
        }

        label.push_str("\n  contract ");
        label.push_str(&contract_index.to_string());
        label.push_str(": ");
        label.push_str(&facts.join(" | "));
    }
}

fn proof_fact_labels(
    syntax: &SyntaxTrees,
    facts: HandleSpan<omega_syntax_trees::item::ProofFact>,
) -> Vec<String> {
    syntax
        .items
        .proof_facts(facts)
        .iter()
        .take(5)
        .map(|fact| proof_fact_label(syntax, fact))
        .collect()
}

fn proof_fact_label(syntax: &SyntaxTrees, fact: &omega_syntax_trees::item::ProofFact) -> String {
    match fact {
        omega_syntax_trees::item::ProofFact::Expression(expression) => {
            syntax.expressions.display_name(*expression)
        }
        omega_syntax_trees::item::ProofFact::Membership(membership) => {
            let domain = syntax
                .items
                .identifier_path_members(membership.domain)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            format!(
                "{} in {}",
                syntax.expressions.display_name(membership.value),
                domain
            )
        }
    }
}

fn append_state_signature(
    diagram: &mut PhaseDiagramBuilder,
    syntax: &SyntaxTrees,
    parent_id: &str,
    owner_index: usize,
    item_index: usize,
    signature_index: usize,
    signature: &StateSignatureNode,
) {
    let mut label = format!(
        "machine {}\nparams: {}",
        signature.name.as_str(),
        signature.parameters.len()
    );
    let effects = syntax.items.identifier_path_members(signature.effects);
    if !effects.is_empty() {
        label.push_str("\neffects: ");
        label.push_str(
            &effects
                .iter()
                .map(|effect| effect.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    if signature.contracts.len() > 0 {
        label.push_str("\ncontracts: ");
        label.push_str(&contract_summary(syntax, signature.contracts));
        append_contract_facts(&mut label, syntax, signature.contracts);
    }
    let signature_id = diagram.node(
        format!("signature_{owner_index}_{item_index}_{signature_index}"),
        label,
        "state",
        3,
    );
    diagram.containment_edge(parent_id, &signature_id);
}

fn item_kind(item: &Item) -> &'static str {
    match item {
        Item::Data(_) => "data",
        Item::Domain(_) => "domain",
        Item::Export(_) => "export",
        Item::Machine(_) | Item::Platform(_) => "machine",
        Item::Operator(_) => "operator",
        Item::Trait(_) => "trait",
        _ => "state",
    }
}

fn item_label(syntax: &SyntaxTrees, item: &Item) -> String {
    match item {
        Item::Capability(value) => format!("capability {}", value.name.as_str()),
        Item::Data(value) => format!(
            "data {}\nmembers: {}",
            value.name.as_str(),
            value.members.len()
        ),
        Item::Domain(value) => {
            let target = type_reference_label(syntax, value.target_type);
            let mut label = format!(
                "domain {}\ntarget: {}\nfacts: {}\nbody tokens: {}",
                value.name.as_str(),
                target,
                syntax.items.proof_facts(value.facts).len(),
                value.body_token_count
            );
            for fact in proof_fact_labels(syntax, value.facts) {
                label.push_str("\n  ");
                label.push_str(&fact);
            }
            label
        }
        Item::Invariant(value) => format!("invariant {}", value.name.as_str()),
        Item::Library(value) => {
            let name = value
                .name
                .as_ref()
                .map(|name| name.as_str())
                .unwrap_or("<anon>");
            format!("library {name}\nfunctions: {}", value.functions.len())
        }
        Item::Operator(value) => {
            let name = syntax
                .items
                .identifier_path_members(value.name)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            format!(
                "operator {name}\nparameters: {}\ncontracts: {}\ntokens: {}",
                value.parameters.len(),
                contract_summary(syntax, value.contracts),
                value.token_count,
            )
        }
        Item::Machine(value) => {
            let state_handles = syntax.items.state_handles(value.states);
            let entry_state = state_handles
                .first()
                .copied()
                .map(|entry_handle| syntax.items.state(entry_handle));
            let attached_data = value
                .attached_data
                .as_ref()
                .map(|name| name.as_str())
                .unwrap_or("<none>");
            let mut label = format!(
                "machine {}\nattached data: {}\nsatisfies: {}\ncontracts: {}",
                value.name.as_str(),
                attached_data,
                value.satisfies.len(),
                contract_summary(syntax, value.contracts)
            );
            let effects = syntax.items.identifier_path_members(value.effects);
            if !effects.is_empty() {
                label.push_str("\neffects: ");
                label.push_str(
                    &effects
                        .iter()
                        .map(|effect| effect.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                );
            }
            append_contract_facts(&mut label, syntax, value.contracts);
            if let Some(entry_state) = entry_state {
                append_entry_statements(&mut label, syntax, entry_state);
            }
            label
        }
        Item::Platform(value) => {
            format!(
                "platform {}\nsignatures: {}",
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
        Item::Export(value) => {
            let path = syntax
                .items
                .identifier_path_members(value.path)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            if let Some(alias) = &value.alias {
                format!(
                    "export {path} as {}\nsegments: {}",
                    alias.as_str(),
                    value.path.len()
                )
            } else {
                format!("export {path}\nsegments: {}", value.path.len())
            }
        }
        Item::Use(value) => {
            let path = syntax
                .items
                .identifier_path_members(value.path)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            format!("use {path}\nsegments: {}", value.path.len())
        }
    }
}

fn type_reference_label(syntax: &SyntaxTrees, handle: TypeReferenceHandle) -> String {
    if !handle.is_valid() {
        return "<missing>".to_owned();
    }

    match syntax.type_references.type_reference(handle) {
        TypeReferenceNode::Reference {
            referee,
            is_mutable,
        } => {
            if *is_mutable {
                format!("&mut {}", type_reference_label(syntax, *referee))
            } else {
                format!("&{}", type_reference_label(syntax, *referee))
            }
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_label(syntax, *base_type)
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => format!(
            "[{}; {}]",
            type_reference_label(syntax, *element_type),
            length
        ),
        TypeReferenceNode::Slice { element_type } => {
            format!("[{}]", type_reference_label(syntax, *element_type))
        }
        TypeReferenceNode::Generic {
            base_name,
            arguments,
        } => {
            let arguments = syntax
                .type_references
                .type_reference_handles(*arguments)
                .iter()
                .map(|argument| type_reference_label(syntax, *argument))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{base_name}<{arguments}>")
        }
        TypeReferenceNode::Named(name) => name.to_string(),
        TypeReferenceNode::SelfType => "Self".to_owned(),
        TypeReferenceNode::Unit => "()".to_owned(),
    }
}

fn append_entry_statements(label: &mut String, syntax: &SyntaxTrees, entry_state: &StateNode) {
    for (statement_index, statement_handle) in syntax
        .items
        .statements(entry_state.statements)
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
