//! A Prop identity supplies a common complete carrier to its constructors.
//! Retain the exact peer occurrence under the same source telescope; these are
//! inference proposals, never field, domain, ownership or equality evidence.

use super::ValueTypeOwner;
use std::collections::HashMap;
use typed_trees::{
    TypedTrees,
    domain::ProofFact,
    expression::{BinaryOperator, ExpressionHandle},
    types::{TypeReferenceHandle, TypeReferenceNode},
};

pub(super) type ConstructorPeers =
    HashMap<(ValueTypeOwner, ExpressionHandle), Vec<ExpressionHandle>>;

pub(super) fn append(
    program: &TypedTrees,
    owner: ValueTypeOwner,
    fact: &ProofFact,
    peers: &mut ConstructorPeers,
) {
    let ProofFact::Expression(root) = fact else {
        return;
    };
    let mut pending = vec![*root];
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        pending.extend(
            validation::contextual_proposition_children(program, expression)
                .into_iter()
                .flatten(),
        );
        let Some(binary) = program.expression_table.binary_node(expression) else {
            continue;
        };
        if !matches!(
            binary.operator,
            BinaryOperator::Equal | BinaryOperator::NotEqual
        ) {
            continue;
        }
        for (constructor, peer) in [(binary.left, binary.right), (binary.right, binary.left)] {
            if program
                .expression_table
                .struct_literal_node(constructor)
                .is_some()
            {
                peers.entry((owner, constructor)).or_default().push(peer);
            }
        }
    }
}

pub(super) fn argument_tuples(
    program: &TypedTrees,
    scope: ValueTypeOwner,
    constructor: ExpressionHandle,
    declaration: symbols::SymbolHandle,
    peers: &ConstructorPeers,
) -> Vec<Vec<TypeReferenceHandle>> {
    peers
        .get(&(scope, constructor))
        .into_iter()
        .flatten()
        .filter_map(|peer| {
            let reference = scope.result_type(program, *peer)?;
            let mut carrier = program
                .type_reference_table
                .unconstrained_type_reference(reference);
            if let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(carrier)
            {
                carrier = program
                    .unique_data_definition_by_symbol(*symbol)?
                    .generic_instance?;
            }
            let TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } = program.type_reference_table.type_reference(carrier)
            else {
                return None;
            };
            (*base_symbol == declaration).then(|| {
                program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .to_vec()
            })
        })
        .collect()
}
