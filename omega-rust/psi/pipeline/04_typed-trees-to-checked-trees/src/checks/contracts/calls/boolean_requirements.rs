//! Prove a Boolean arrival requirement at one exact call occurrence.
//!
//! Each conjunct may use a different existing proof route. No child is installed
//! as a premise: all queries keep the same substituted operands and live context.
use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

use super::super::places::expression_is_boolean_place_like;
use super::super::prover::{
    call_entry_contexts_prove_boolean_contract_expression, closed_boolean_value,
};
use super::{incoming_guard_proves_requires, transition_guard_proves_boolean_requirement};

#[cfg(test)]
mod tests;

pub(super) fn proves(
    program: &TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    entry_contexts: &[facts::FactContextHandle],
    expression: ExpressionHandle,
    incoming_guards: &[crate::checks::ranges::incoming_guards::IncomingGuard],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let proven = closed_boolean_value(program, &facts.operators, expression) == Some(true)
        || super::super::call_bounds::proves(
            program, facts, state_flow, call_flow, expression, None,
        )
        || call_entry_contexts_prove_boolean_contract_expression(
            program,
            facts,
            state_flow,
            call_flow,
            entry_contexts,
            expression,
            call_frames,
        )
        || super::super::call_bounds::proves_in_context(
            program,
            facts,
            state_flow,
            call_flow,
            entry_contexts,
            expression,
            call_frames,
        )
        || super::super::entailment::structural_call_requirement(
            program,
            facts,
            state_flow,
            call_flow,
            expression,
            call_frames,
        )
        || if !expression_is_boolean_place_like(program, expression) {
            // R1: a DOMINATING incoming-arm guard establishes a
            // boolean requires fact -- the ranges machinery's
            // IncomingGuard walk-back is the soundness gate
            // (single-predecessor edges, rewrite-fenced across
            // intermediate states), and the caller state's OWN
            // statements must preserve the named fields up to
            // the call (conservative whole-state scan).
            incoming_guard_proves_requires(
                program,
                facts,
                state_flow,
                call_flow,
                expression,
                incoming_guards,
                call_frames,
            )
        } else {
            // The actual-aware proof above already checked
            // this formal. A raw callee name is not a caller
            // fact, even when both parameters spell `flag`.
            false
        }
        || transition_guard_proves_boolean_requirement(
            program,
            facts,
            state_flow,
            call_flow,
            expression,
            call_frames,
        );
    if proven {
        return true;
    }
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return false;
    };
    // Preserve selected operator meanings. Splitting a builtin conjunction
    // combines verdicts, never assumptions, and cannot turn a disjunction into
    // independently available premises.
    binary.operator == BinaryOperator::And
        && super::super::prover::has_builtin_operators(program, &facts.operators, expression)
        && [binary.left, binary.right].into_iter().all(|conjunct| {
            proves(
                program,
                facts,
                state_flow,
                call_flow,
                entry_contexts,
                conjunct,
                incoming_guards,
                call_frames,
            )
        })
}
