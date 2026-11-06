//! Verbose call failures retain the actual entry premises and selected calls.
//! This distinguishes a missing guarantee from a substitution or selection
//! mismatch without requiring the full structural proof trace.

use crate::published::labels::{machine_name, semantic_fact_requirement_label};
use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
use facts::{Fact, FactContextHandle, FactPayload};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

pub(super) fn trace_unproved_call(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    requirement: &Fact,
    contexts: &[FactContextHandle],
) {
    if std::env::var_os("OMEGA_PROVIDER_DEBUG").is_none() {
        return;
    }
    eprintln!(
        "CALLREQ unproved caller={} target={:?} statement={} ordinal={} required={}",
        machine_name(program, state.machine_symbol),
        call.target_symbol,
        call.statement_index,
        call.call_ordinal,
        semantic_fact_requirement_label(program, &facts.semantic, requirement),
    );
    trace_contract(program, facts, "required", requirement);
    let contracts = || {
        contexts
            .iter()
            .flat_map(|context| {
                facts
                    .semantic
                    .context_view(facts.semantic.contexts.get(*context))
                    .facts()
            })
            .filter(|fact| {
                matches!(
                    fact.payload,
                    FactPayload::Contract { .. }
                        | FactPayload::ContractBooleanExpression { .. }
                        | FactPayload::ContractDomainMembership { .. }
                        | FactPayload::ContractPropositionApplication { .. }
                        | FactPayload::ContractCarryPermission { .. }
                )
            })
    };
    let count = contracts().count();
    eprintln!(
        "CALLREQ entry contracts={count} showing_first={}",
        count.min(64)
    );
    for fact in contracts().take(64) {
        trace_contract(program, facts, "entry", fact);
    }
}

fn trace_contract(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    role: &str,
    fact: &Fact,
) {
    eprintln!(
        "CALLREQ {role} point={:?} payload={:?} label={}",
        fact.point,
        fact.payload,
        semantic_fact_requirement_label(program, &facts.semantic, fact),
    );
    let FactPayload::ContractBooleanExpression { expression, .. } = fact.payload else {
        return;
    };
    trace_selected_calls(program, role, expression);
}

fn trace_selected_calls(
    program: &dyn validation::CheckingInput,
    role: &str,
    expression: ExpressionHandle,
) {
    let mut count = 0usize;
    validation::expression_traversal::visit_pre_order(program, vec![expression], |handle, node| {
        if let ExpressionNode::Call(call) = node {
            let declared_result = validation::call_target_return_type(program, call.target_symbol)
                .filter(|reference| reference.is_valid())
                .map(|reference| (reference, program.normalized_type_identity(reference)));
            eprintln!(
                "CALLREQ {role} call={handle:?} node={call:?} declared_result={declared_result:?}"
            );
            count += 1;
            if count == 32 {
                eprintln!("CALLREQ {role} call list stopped at 32");
                return true;
            }
        }
        false
    });
}
