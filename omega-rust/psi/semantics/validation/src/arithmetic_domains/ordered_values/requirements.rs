//! Final totality obligation for scalar calls used as requirement terms.

use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::SignatureContractKind;

/// Check the denotational obligation that normal-return value equality alone
/// cannot establish. Validation may use an authored requirement when checking
/// its body, but the finished checked program must prove that each eligible
/// scalar call in that requirement is total and has no published crash route.
///
/// This consumes the actual finalized termination facts, not the preliminary
/// typed summary. Runtime guards are deliberately outside this scan: their
/// comparisons describe normal completion and may separately publish crashes.
/// Broader call forms remain outside the ordered-value eligibility rung and
/// retain their existing formation and fact-call validation owners.
pub fn validate_ordered_requirement_call_totality(
    program: &TypedTrees,
    operational: &flow_effects::OperationalPlan,
    service_reaches: &flow_effects::ServiceReachInferencePlan,
    termination: &checked_trees::TerminationFacts,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut pending = Vec::new();
    for machine in program.machines() {
        for contract in program
            .machine_contracts(machine)
            .iter()
            .chain(
                program
                    .machine_states(machine)
                    .iter()
                    .flat_map(|state| program.state_contracts(state)),
            )
            .filter(|contract| contract.kind == SignatureContractKind::Requires)
        {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                match fact {
                    ProofFact::Expression(expression) => pending.push(*expression),
                    ProofFact::Membership(membership) => pending.push(membership.value),
                    ProofFact::Proposition(application) => pending.extend_from_slice(
                        program
                            .expression_table
                            .expression_handles(application.arguments),
                    ),
                }
            }
        }
    }
    let mut visited = Vec::<ExpressionHandle>::new();
    while let Some(expression) = pending.pop() {
        if !expression.is_valid() || visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        match program.expression_table.expression(expression) {
            ExpressionNode::Call(call) => {
                if let Ok((machine, _)) = crate::denotational_calls::normal_return_call_candidate(
                    program,
                    call,
                    operational,
                    service_reaches,
                ) {
                    let mut summaries = termination
                        .machines
                        .iter()
                        .filter(|fact| fact.machine == machine.symbol);
                    let unconditional = summaries.next().is_some_and(|fact| {
                        matches!(&fact.plan.checked_summary,
                            language_semantics::TerminationGuarantee::Terminates { premises }
                                if premises.is_empty())
                    }) && summaries.next().is_none();
                    if !unconditional {
                        diagnostics.push(Diagnostic::error(format!(
                            "ordered requirement call `{}` is not denotational: the exact selected machine is not unconditionally terminating",
                            call.target,
                        )));
                    } else if !crate::denotational_calls::has_no_crash_routes(
                        program,
                        machine.symbol,
                        operational,
                    ) {
                        diagnostics.push(Diagnostic::error(format!(
                            "ordered requirement call `{}` is not denotational: the selected call closure has a crash route",
                            call.target,
                        )));
                    }
                }
                pending.push(call.receiver);
                pending
                    .extend_from_slice(program.expression_table.expression_handles(call.arguments));
            }
            ExpressionNode::Binary(binary) => pending.extend([binary.left, binary.right]),
            ExpressionNode::Atomic(atomic) => pending.extend([atomic.value, atomic.result]),
            ExpressionNode::Borrow(borrow) => pending.push(borrow.target),
            ExpressionNode::Cast(cast) => pending.push(cast.value),
            ExpressionNode::Indexed(indexed) => pending.extend([indexed.collection, indexed.index]),
            ExpressionNode::Member(member) => pending.push(member.receiver),
            ExpressionNode::Range(range) => pending.extend([range.start, range.end]),
            ExpressionNode::Unary(unary) => pending.push(unary.operand),
            ExpressionNode::StructLiteral(literal) => pending.extend(
                program
                    .expression_table
                    .struct_fields(literal.fields)
                    .iter()
                    .map(|field| field.value),
            ),
            ExpressionNode::ArrayLiteral(values) => {
                pending.extend_from_slice(program.expression_table.expression_handles(*values))
            }
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Name(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
    }
}
