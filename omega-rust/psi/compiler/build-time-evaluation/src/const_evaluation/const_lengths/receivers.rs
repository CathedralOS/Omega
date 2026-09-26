//! Private current-owner joins for folded type positions. A live arena slot
//! is insufficient: replay must reach it through the same declaration or cast
//! and the same ordered type-child edges that owned the original invocation.
use symbols::SymbolHandle;
use typed_trees::{
    TypedTrees,
    data::DataMember,
    expression::ExpressionNode,
    statement::StatementNode,
    types::{TypeReferenceHandle, TypeReferenceNode},
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Owner {
    Declaration {
        parent: SymbolHandle,
        declaration: SymbolHandle,
    },
    Cast {
        expression: typed_trees::expression::ExpressionHandle,
        origin: checked_trees::CheckedValueOrigin,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Edge {
    Reference,
    Constraint,
    Array,
    Slice,
    Argument(usize),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Receiver {
    owner: Owner,
    root: TypeReferenceHandle,
    path: Vec<Edge>,
}

fn children(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Vec<(Edge, TypeReferenceHandle)> {
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Reference { referee, .. } => vec![(Edge::Reference, *referee)],
        TypeReferenceNode::Constrained { base_type, .. } => vec![(Edge::Constraint, *base_type)],
        TypeReferenceNode::FixedArray { element_type, .. } => vec![(Edge::Array, *element_type)],
        TypeReferenceNode::Slice { element_type } => vec![(Edge::Slice, *element_type)],
        TypeReferenceNode::Generic { arguments, .. } => program
            .type_reference_table
            .type_reference_handles(*arguments)
            .iter()
            .enumerate()
            .map(|(ordinal, reference)| (Edge::Argument(ordinal), *reference))
            .collect(),
        _ => Vec::new(),
    }
}

fn paths(
    program: &TypedTrees,
    current: TypeReferenceHandle,
    target: TypeReferenceHandle,
    visited: &mut Vec<TypeReferenceHandle>,
    path: &mut Vec<Edge>,
    output: &mut Vec<Vec<Edge>>,
) {
    if current == target {
        output.push(path.clone());
        return;
    }
    if !current.is_valid() || visited.contains(&current) {
        return;
    }
    visited.push(current);
    for (edge, child) in children(program, current) {
        path.push(edge);
        paths(program, child, target, visited, path, output);
        path.pop();
    }
    visited.pop();
}

pub(super) struct Roots(Vec<(Owner, TypeReferenceHandle)>);

pub(super) fn roots(program: &TypedTrees) -> Roots {
    let mut roots = Vec::new();
    let mut declaration = |parent, symbol, reference| {
        roots.push((
            Owner::Declaration {
                parent,
                declaration: symbol,
            },
            reference,
        ));
    };
    for data in program.data_definitions() {
        for member in program.data_members(data) {
            let (parent, fields) = match member {
                DataMember::Field(field) => (data.symbol, std::slice::from_ref(field)),
                DataMember::Variant(variant) => {
                    (variant.symbol, program.data_payload_fields(variant))
                }
            };
            for field in fields {
                declaration(parent, field.symbol, field.type_reference);
            }
        }
    }
    for machine in program.machines() {
        for data in program.machine_owned_data(machine) {
            declaration(machine.symbol, data.symbol, data.type_reference);
        }
        for state in program.machine_states(machine) {
            declaration(machine.symbol, state.symbol, state.return_type);
            for parameter in program.state_parameters(state) {
                declaration(state.symbol, parameter.symbol, parameter.type_reference);
            }
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let StatementNode::LocalData(local) = statement {
                    declaration(state.symbol, local.symbol, local.type_reference);
                }
            }
        }
    }
    for domain in program.domain_definitions() {
        declaration(domain.symbol, domain.symbol, domain.target_type);
    }
    for operator in program.operators().iter().chain(
        program
            .domain_definitions()
            .iter()
            .flat_map(|domain| program.domain_operators(domain)),
    ) {
        declaration(operator.symbol, operator.symbol, operator.return_type);
        for parameter in program.operator_parameters(operator) {
            declaration(operator.symbol, parameter.symbol, parameter.type_reference);
        }
    }
    for trait_definition in program.traits() {
        for signature in program.trait_machine_signatures(trait_definition) {
            declaration(
                trait_definition.symbol,
                signature.symbol,
                signature.return_type,
            );
            for parameter in program.state_signature_parameters(signature) {
                declaration(signature.symbol, parameter.symbol, parameter.type_reference);
            }
        }
    }
    let values = typed_trees_to_checked_trees::derive_pre_flow_value_origins(program);
    for (_, value) in values.values.iter() {
        if let ExpressionNode::Cast(cast) = program.expression_table.expression(value.expression) {
            let root = (
                Owner::Cast {
                    expression: value.expression,
                    origin: value.origin,
                },
                cast.target_type,
            );
            if !roots.contains(&root) {
                roots.push(root);
            }
        }
    }
    Roots(roots)
}

pub(super) fn capture(
    program: &TypedTrees,
    roots: &Roots,
    target: TypeReferenceHandle,
) -> Vec<Receiver> {
    let mut receivers = Vec::new();
    for (owner, root) in &roots.0 {
        let root = *root;
        let mut found = Vec::new();
        paths(
            program,
            root,
            target,
            &mut Vec::new(),
            &mut Vec::new(),
            &mut found,
        );
        receivers.extend(found.into_iter().map(|path| Receiver {
            owner: owner.clone(),
            root,
            path,
        }));
    }
    receivers
}

pub(super) fn validate(
    program: &TypedTrees,
    roots: &Roots,
    target: TypeReferenceHandle,
    expected: &[Receiver],
) -> Result<(), String> {
    let current = capture(program, roots, target);
    if expected.is_empty()
        || expected.iter().any(|receiver| {
            current
                .iter()
                .filter(|current| *current == receiver)
                .count()
                != 1
        })
    {
        return Err("folded array length lost its exact declaring type position".into());
    }
    Ok(())
}

pub(super) fn require_unique(receivers: &[Receiver]) -> Result<(), String> {
    if receivers.is_empty()
        || receivers
            .iter()
            .enumerate()
            .any(|(index, receiver)| receivers[..index].contains(receiver))
    {
        return Err("folded array length has no unique declaring type position".into());
    }
    Ok(())
}
