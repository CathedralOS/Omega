//! A callee's `requires` type equations extend ordinary argument inference.
//!
//! `requires Length == [u8; Capacity]` relates two declared binders. Once
//! argument/result compatibility has proposed `Length`, matching the
//! equation's other side against that proposal recovers `Capacity` through
//! the same constructor traversal that binds ordinary formals
//! ([`infer_static_bindings`]). This runs per call site, so a call inside a
//! generic body that forwards its own `[u8; N]` proposes `Capacity = N` and
//! stays an open selection until the enclosing tuple specializes; the
//! specialized copy then proposes the closed extent.
//!
//! Only binders without any proposal are filled. An explicit argument or an
//! argument-derived binding is never replaced: the complete tuple must still
//! satisfy every declared predicate, which specialization checks before body
//! substitution (`requires type predicate does not hold`). Conjunctions
//! contribute their structural leaves; alternatives and negations constrain an
//! already selected tuple and never choose an argument. Runtime contents,
//! literal values and proved bounds are never consulted.

use super::proposals::{BindingContext, StaticProposals};
use super::static_bindings::infer_static_bindings;
use crate::monomorphization::{ExpressionHandle, TypeReferenceHandle, TypeReferenceNode};
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionNode};
use typed_trees::signature::SignatureContractKind;

pub(super) fn infer_from_type_equations(
    context: BindingContext<'_>,
    machine: &typed_trees::machine::Machine,
    proposals: &mut StaticProposals,
) {
    let program = context.program;
    let mut equations = Vec::new();
    for contract in program.machine_contracts(machine) {
        if contract.kind != SignatureContractKind::Requires {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            if let ProofFact::Expression(expression) = fact {
                collect_conjoined_equations(program, *expression, &mut equations);
            }
        }
    }
    if equations.is_empty() {
        return;
    }
    // Each round can only fill a binder that had no proposal, so the number
    // of type and const binders bounds the rounds.
    let rounds = context.type_parameters.len() + context.const_parameters.len() + 1;
    for _ in 0..rounds {
        let mut progress = false;
        for &(left, right) in &equations {
            for (binder, structure) in [(left, right), (right, left)] {
                let Some(index) = type_binder_position(context, binder) else {
                    continue;
                };
                let Some(bound) = proposals
                    .types
                    .iter()
                    .find(|(candidate, parameter, _)| {
                        *candidate == context.candidate_index && *parameter == index
                    })
                    .map(|(_, _, reference)| *reference)
                else {
                    continue;
                };
                let mut derived = StaticProposals::default();
                infer_static_bindings(context, structure, bound, &mut derived);
                for proposal in derived.types {
                    if !proposals
                        .types
                        .iter()
                        .any(|existing| existing.0 == proposal.0 && existing.1 == proposal.1)
                    {
                        proposals.types.push(proposal);
                        progress = true;
                    }
                }
                for proposal in derived.constants {
                    if !proposals
                        .constants
                        .iter()
                        .any(|existing| existing.0 == proposal.0 && existing.1 == proposal.1)
                    {
                        proposals.constants.push(proposal);
                        progress = true;
                    }
                }
            }
        }
        if !progress {
            break;
        }
    }
}

fn collect_conjoined_equations(
    program: &dyn validation::CheckingInput,
    expression: ExpressionHandle,
    equations: &mut Vec<(TypeReferenceHandle, TypeReferenceHandle)>,
) {
    if let Some((left, right, false)) = program.static_type_predicate_operands(expression) {
        equations.push((left, right));
        return;
    }
    if let ExpressionNode::Binary(binary) = program.expression_table.expression(expression)
        && binary.operator == BinaryOperator::And
    {
        collect_conjoined_equations(program, binary.left, equations);
        collect_conjoined_equations(program, binary.right, equations);
    }
}

fn type_binder_position(
    context: BindingContext<'_>,
    reference: TypeReferenceHandle,
) -> Option<usize> {
    let TypeReferenceNode::Named { symbol, .. } = context
        .program
        .type_reference_table
        .type_reference(reference)
    else {
        return None;
    };
    if !symbol.is_valid() {
        return None;
    }
    context
        .type_parameters
        .iter()
        .position(|(parameter, _)| parameter == symbol)
}
