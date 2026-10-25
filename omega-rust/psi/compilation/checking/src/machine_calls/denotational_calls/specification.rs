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
    for (owner, _, fact) in typed_trees::references::program_scoped_proof_facts(program) {
        match fact {
            ProofFact::Expression(expression) => pending.push((owner, *expression)),
            ProofFact::Membership(membership) => pending.push((owner, membership.value)),
            ProofFact::Proposition(application) => pending.extend(
                program
                    .expression_table
                    .expression_handles(application.arguments)
                    .iter()
                    .map(|expression| (owner, *expression)),
            ),
        }
    }
    pending.extend(crate::mathematical_applications::scoped_expression_nodes(
        program,
    ));
    for proposition in program.propositions() {
        if let PropositionBody::Transparent {
            proposition: formula,
        } = &proposition.body
        {
            match formula {
                PropositionFormula::BooleanExpression(expression) => {
                    pending.push((proposition.symbol, *expression))
                }
                PropositionFormula::Application(application) => pending.extend(
                    program
                        .expression_table
                        .expression_handles(application.arguments)
                        .iter()
                        .map(|expression| (proposition.symbol, *expression)),
                ),
            }
        }
    }
    let mut requests = Vec::new();
    let mut visited = std::collections::HashSet::new();
    while let Some((caller, expression)) = pending.pop() {
        if !program.expression_table.expression_is_valid(expression)
            || !visited.insert((caller, expression))
        {
            continue;
        }
        let node = program.expression_table.expression(expression);
        crate::expression_traversal::children(program, node, |child| pending.push((caller, child)));
        let ExpressionNode::Call(call) = node else {
            continue;
        };
        if crate::mathematical_applications::selected_definition(program, call).is_some() {
            // Checked mathematical signature formation owns the dependent
            // application. It has no operational closure to classify.
            // The producer has already checked this source application at
            // its instantiated generic telescope. Repeating an uninstantiated
            // carrier check here would lose that exact declaration custody.
            continue;
        }
        let (machine, state) = match selected_call_contract(program, caller, call) {
            Some(SelectedCallContract::Concrete(machine, state)) => (machine, state),
            Some(SelectedCallContract::Operator(operator)) => {
                if super::selected_operator_has_closed_value_denotation(program, operator) {
                    continue;
                }
                let Some(body) = super::selected_operator_authored_body(program, operator) else {
                    diagnostics.push(Diagnostic::error(format!(
                            "specification call `{}` has no observation-free, reach-free, crash-free value denotation",
                            program.symbols.name(operator.symbol),
                        )));
                    continue;
                };
                body
            }
            Some(SelectedCallContract::Signature(signature))
            | Some(SelectedCallContract::DynamicSlot { signature, .. }) => {
                // An abstract invocation consumes its selected public promise,
                // never a realized private implementation's stronger summary.
                if !matches!(
                    &signature.termination_guarantee,
                    language_semantics::TerminationGuarantee::Terminates { premises }
                        if premises.is_empty()
                ) {
                    diagnostics.push(Diagnostic::error(format!(
                            "specification call `{}` is not denotational: the selected machine is not unconditionally terminating",
                            signature.name,
                        )));
                }
                if !signature_has_closed_value_effects(program, signature) {
                    diagnostics.push(Diagnostic::error(format!(
                            "specification call `{}` has no observation-free, reach-free, crash-free value denotation",
                            signature.name,
                        )));
                }
                continue;
            }
            None => continue,
        };
        if let Err(reason) = super::checked_value_call_candidate(
            program,
            machine,
            state,
            operational,
            service_reaches,
        ) {
            diagnostics.push(Diagnostic::error(format!(
                    "specification call `{}` has no observation-free, reach-free, crash-free value denotation: {reason}",
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
    requests.dedup();
    requests
}

/// Abstract functions have no checked body closure to inspect. Their exact
/// interface must forbid runtime observations and control effects and expose
/// only immutable value inputs; ordinary call formation still proves Requires.
pub(super) fn signature_has_closed_value_effects(
    program: &dyn crate::CheckingInput,
    signature: &typed_trees::signature::StateSignature,
) -> bool {
    !signature.suspends
        && !signature.blocks
        && !signature.service_reach_is_installation_bound
        && program
            .service_reach_rows
            .services(signature.service_reach_row)
            .is_empty()
        && program.state_signature_invokes(signature).is_empty()
        && program
            .state_signature_parameters(signature)
            .iter()
            .all(|parameter| {
                !parameter.is_mutable
                    && crate::reference_shell(program, parameter.type_reference).is_none()
            })
        && program
            .state_signature_contracts(signature)
            .iter()
            .all(|contract| !contract.is_crashes())
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
        let summary = termination.checked_termination(request.target_machine);
        if !matches!(
            &summary,
            Some(language_semantics::TerminationGuarantee::Terminates { premises })
                if premises.is_empty()
        ) {
            if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
                eprintln!(
                    "DENOTATION termination request={request:?} machine={} summary={summary:?} expression={:?}",
                    program.symbols.name(request.target_machine),
                    program.expression_table.expression(request.call_expression),
                );
            }
            diagnostics.push(Diagnostic::error(format!(
                "specification call `{}` is not denotational: the selected machine is not unconditionally terminating",
                program.symbols.name(request.target_machine),
            )));
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
