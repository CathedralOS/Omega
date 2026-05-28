use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::signature::SignatureContractKind;

use super::facts::RangeFacts;
use super::guards::seed_guard_facts;

pub(super) fn seed_machine_requires(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    machine: &Machine,
) {
    for contract in program.machine_contracts(machine) {
        if contract.kind != SignatureContractKind::Requires {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            match fact {
                omega_typed_trees::domain::ProofFact::Expression(expression) => {
                    seed_guard_facts(program, facts, *expression);
                    seed_index_proofs_from_expression(program, facts, *expression);
                }
                omega_typed_trees::domain::ProofFact::Membership(membership) => {
                    seed_index_proofs_from_expression(program, facts, membership.value);
                }
            }
        }
    }
}

fn seed_index_proofs_from_expression(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    expression: ExpressionHandle,
) {
    if !expression.is_valid() {
        return;
    }

    match program.expression_table.expression(expression) {
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
        ExpressionNode::Mutable(inner) => seed_index_proofs_from_expression(program, facts, *inner),
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
        | ExpressionNode::String(_) => {}
    }
}
