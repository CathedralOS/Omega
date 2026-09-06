use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::signature::SignatureContractKind;
use typed_trees::state::State;

use super::facts::RangeFacts;
use super::guards::seed_guard_facts;

/// Machine preconditions belong to the callable entry. Named states receive
/// their own declared preconditions and facts transported by incoming edges,
/// never a replay of a different state's same-spelled parameter expressions.
pub(super) fn seed_state_requires(
    program: &typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    machine: &Machine,
    state: &State,
) {
    let is_entry = state.symbol.is_valid()
        && program
            .machine_states(machine)
            .first()
            .is_some_and(|entry| entry.symbol == state.symbol);
    for contract in program
        .machine_contracts(machine)
        .iter()
        .filter(|_| is_entry)
        .chain(program.state_contracts(state))
    {
        if contract.kind != SignatureContractKind::Requires {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            match fact {
                typed_trees::domain::ProofFact::Expression(expression) => {
                    seed_guard_facts(program, machine, state, facts, *expression);
                    seed_index_proofs_from_expression(program, facts, *expression);
                }
                typed_trees::domain::ProofFact::Membership(membership) => {
                    seed_index_proofs_from_expression(program, facts, membership.value);
                }
                typed_trees::domain::ProofFact::Proposition(application) => {
                    for argument in program
                        .expression_table
                        .expression_handles(application.arguments)
                    {
                        seed_index_proofs_from_expression(program, facts, *argument);
                    }
                }
            }
        }
    }
}

/// Seeds slice proof vocabulary implied by an assumed-valid subslice access.
///
/// When a `requires` clause asserts `coll[a..=b]` is valid, we may assume two
/// things as first-class slice facts:
///   * the collection is `non_empty` — an inclusive non-empty range has at least
///     `b + 1 >= 1` elements, so `prove_minimum_length(coll, 1)`;
///   * the inclusive end `b` is itself a valid index of `coll`, which connects
///     range validity to index validity (`prove_index`).
///
/// Exclusive ranges carry no such guarantee (`a..a` is empty), so nothing is
/// seeded for them here.
fn seed_subslice_facts(
    program: &typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    collection: ExpressionHandle,
    index: ExpressionHandle,
) {
    let ExpressionNode::Range(range) = program.expression_table.expression(index) else {
        return;
    };
    if !range.end_inclusive || !range.end.is_valid() {
        return;
    }

    let collection_label = program.expression_table.display_name(collection);
    // non_empty: an inclusive non-empty subslice implies length >= 1.
    facts.prove_minimum_length(collection_label.clone(), 1);
    // The inclusive end is a valid index of the collection (`b < len`).
    let end_label = program.expression_table.display_name(range.end);
    facts.prove_index(collection_label, end_label);
}

fn seed_index_proofs_from_expression(
    program: &typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    expression: ExpressionHandle,
) {
    if !expression.is_valid() {
        return;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            seed_index_proofs_from_expression(program, facts, atomic.value)
        }
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                seed_index_proofs_from_expression(program, facts, *value);
            }
        }
        ExpressionNode::Binary(binary) => {
            seed_index_proofs_from_expression(program, facts, binary.left);
            seed_index_proofs_from_expression(program, facts, binary.right);
        }
        ExpressionNode::Call(call) => {
            seed_index_proofs_from_expression(program, facts, call.receiver);
            for argument in program.expression_table.expression_handles(call.arguments) {
                seed_index_proofs_from_expression(program, facts, *argument);
            }
        }
        ExpressionNode::Cast(cast) => seed_index_proofs_from_expression(program, facts, cast.value),
        ExpressionNode::Indexed(indexed) => {
            seed_subslice_facts(program, facts, indexed.collection, indexed.index);
            facts.prove_index(
                program.expression_table.display_name(indexed.collection),
                program.expression_table.display_name(indexed.index),
            );
            seed_index_proofs_from_expression(program, facts, indexed.collection);
            seed_index_proofs_from_expression(program, facts, indexed.index);
        }
        ExpressionNode::Member(member) => {
            seed_index_proofs_from_expression(program, facts, member.receiver);
        }
        ExpressionNode::Borrow(inner) => {
            seed_index_proofs_from_expression(program, facts, inner.target)
        }
        ExpressionNode::Unary(unary) => {
            seed_index_proofs_from_expression(program, facts, unary.operand)
        }
        ExpressionNode::Range(range) => {
            seed_index_proofs_from_expression(program, facts, range.start);
            seed_index_proofs_from_expression(program, facts, range.end);
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                seed_index_proofs_from_expression(program, facts, field.value);
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}
