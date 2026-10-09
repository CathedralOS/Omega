//! Total invocation formation for ordinary calls in specification values.
//!
//! The source proof judge may discharge a clause from a preceding premise,
//! including a verbatim premise. That never establishes the calls' totality.
//! Retain exact concrete invocation obligations independently of proof search,
//! then complete them with termination facts from this checking invocation.
//! Abstract mathematical and sealed catalog applications retain their existing
//! declaration-owned formation authority; no runtime occurrence is manufactured.

use crate::proof_contracts::contract_entailment::{SelectedCallContract, selected_call_contract};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::proposition::{PropositionBody, PropositionFormula};

/// A specification invocation awaiting the checker's termination judgment.
/// These records are construction input, not exported proof certificates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecificationCallRequest {
    pub call_expression: ExpressionHandle,
    pub target_machine: SymbolHandle,
    pub target_state: SymbolHandle,
}

pub(crate) fn validate_specification_calls(
    program: &dyn crate::CheckingInput,
    operational: &flow_effects::OperationalPlan,
    service_reaches: &flow_effects::ServiceReachInferencePlan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<SpecificationCallRequest> {
    let mut pending = Vec::new();
    for (_, fact) in typed_trees::references::program_proof_facts(program) {
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
    for proposition in program.propositions() {
        if let PropositionBody::Transparent { proposition } = &proposition.body {
            match proposition {
                PropositionFormula::BooleanExpression(expression) => pending.push(*expression),
                PropositionFormula::Application(application) => pending.extend_from_slice(
                    program
                        .expression_table
                        .expression_handles(application.arguments),
                ),
            }
        }
    }
    let mut visited = Vec::new();
    let mut requests = Vec::new();
    while let Some(expression) = pending.pop() {
        if !program.expression_table.expression_is_valid(expression)
            || visited.contains(&expression)
        {
            continue;
        }
        visited.push(expression);
        let node = program.expression_table.expression(expression);
        crate::expression_traversal::children(program, node, |child| pending.push(child));
        let ExpressionNode::Call(call) = node else {
            continue;
        };
        let (machine, state) = match selected_call_contract(program, call) {
            Some(SelectedCallContract::Concrete(machine, state)) => (machine, state),
            Some(SelectedCallContract::Operator(operator)) => {
                if !super::selected_operator_has_closed_value_denotation(program, operator) {
                    diagnostics.push(Diagnostic::error(format!(
                        "specification call `{}` has no observation-free, reach-free, crash-free value denotation",
                        program.symbols.name(operator.symbol),
                    )));
                }
                continue;
            }
            // A selected requirement is an abstract mathematical interface,
            // not the exporting operator of an ordinary boundary invocation.
            Some(SelectedCallContract::Signature(_)) | None => continue,
        };
        // A catalog-owned token machine can also expose a concrete signature
        // view. The exact sealed declaration, rather than its boundary body,
        // independently owns that application's complete value meaning.
        if typed_trees::operator::declaration_by_symbol(program, machine.symbol).is_some_and(
            |operator| super::selected_operator_has_closed_value_denotation(program, operator),
        ) {
            continue;
        }
        if !super::has_checked_value_call_candidate(
            program,
            machine,
            state,
            operational,
            service_reaches,
        ) {
            diagnostics.push(Diagnostic::error(format!(
                "specification call `{}` has no observation-free, reach-free, crash-free value denotation",
                machine.name,
            )));
            continue;
        }
        requests.push(SpecificationCallRequest {
            call_expression: expression,
            target_machine: machine.symbol,
            target_state: state.symbol,
        });
    }
    requests.sort_by_key(|request| {
        (
            request.call_expression.arena_index(),
            request.call_expression.generation(),
        )
    });
    requests
}

/// Complete already formed invocation obligations using the existing checked
/// termination oracle. The source program and retained targets are unchanged;
/// this does not replay a proof or infer a second termination summary.
pub fn admit_specification_calls(
    program: &dyn crate::CheckingInput,
    requests: &[SpecificationCallRequest],
    termination: &impl crate::CheckedTerminationOracle,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    for request in requests {
        if !termination.unconditionally_terminates(request.target_machine) {
            diagnostics.push(Diagnostic::error(format!(
                "specification call `{}` is not denotational: the selected machine is not unconditionally terminating",
                program.symbols.name(request.target_state),
            )));
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
