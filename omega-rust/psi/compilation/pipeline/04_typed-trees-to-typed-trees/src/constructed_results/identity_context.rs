//! Source relations and mathematical inputs supply constructor type arguments.
//! Retain the exact peer or declared receiving type under the same telescope.
//! These are inference proposals, never domain, ownership or identity evidence.

use super::ValueTypeOwner;
use std::collections::HashMap;
use typed_trees::{
    TypedTrees,
    domain::ProofFact,
    expression::{BinaryOperator, ExpressionHandle},
    types::{TypeReferenceHandle, TypeReferenceNode},
};

#[derive(Clone, Copy)]
pub(super) enum ConstructorContext {
    Peer(ExpressionHandle),
    Declared(TypeReferenceHandle),
}

pub(super) type ConstructorPeers =
    HashMap<(ValueTypeOwner, ExpressionHandle), Vec<ConstructorContext>>;

pub(super) fn append(
    program: &dyn validation::CheckingInput,
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
        if let Some(call) = program.expression_table.call_node(expression)
            && let Some(types) =
                validation::mathematical_applications::argument_type_references(program, call)
        {
            for (subject, reference) in program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .zip(types)
            {
                if let Some(reference) = reference
                    && program
                        .expression_table
                        .struct_literal_node(*subject)
                        .is_some()
                {
                    peers
                        .entry((owner, *subject))
                        .or_default()
                        .push(ConstructorContext::Declared(reference));
                }
            }
        }
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
                peers
                    .entry((owner, constructor))
                    .or_default()
                    .push(ConstructorContext::Peer(peer));
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
            let reference = match peer {
                ConstructorContext::Peer(peer) => scope.result_type(program, *peer)?,
                ConstructorContext::Declared(reference) => *reference,
            };
            application_arguments(program, reference, declaration)
        })
        .collect()
}

/// Only the selected nominal declaration's argument tuple participates in
/// inference. A receiving qualification is not evidence about the new value.
pub(super) fn application_arguments(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    declaration: symbols::SymbolHandle,
) -> Option<Vec<TypeReferenceHandle>> {
    if !program
        .type_reference_table
        .contains_type_reference(reference)
    {
        return None;
    }
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
}
