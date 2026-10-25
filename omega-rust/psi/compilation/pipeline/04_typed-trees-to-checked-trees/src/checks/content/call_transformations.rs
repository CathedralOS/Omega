//! Bind a completed conservation call to its caller-local claim occurrences.
//!
//! An authored callee equation is a conditional normal-return contract. It
//! becomes a replacement edge only after the ownership walk has transferred
//! every exact input and established every exact result. The callee still owes
//! its body proof. These edges do not grant membership, consume storage, or
//! identify independent roots; they let a later transaction follow the complete
//! live descendants of an account it already owned.

use super::body_conservation::owned_return_sides;
use super::call_results::{
    Invocation, InvocationEvents, input_claim, projection_places, result_claim,
};
use super::content_paths::{ClaimBindings, content_segments_to_fact_path};
use checked_trees::facts::{ContentClaimTransformation, ContentClaimTransformationSource};
use checked_trees::{CheckFacts, FlowPermissionEvaluation, FlowPermissionEventFact};
use language_semantics::content::ContentPlaceRoot;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;

pub(crate) fn observe_call_completion(
    program: &dyn validation::CheckingInput,
    facts: &mut CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: usize,
    expression: ExpressionHandle,
    evaluation: FlowPermissionEvaluation,
    events: &[FlowPermissionEventFact],
) {
    if facts.qualifications.content.conservation_plans.is_empty() {
        return;
    }
    let Some(state_flow) = facts.flow.control.state(machine, state) else {
        return;
    };
    let invocations = facts
        .flow
        .control
        .calls
        .span_or_empty(state_flow.calls)
        .iter()
        .filter(|call| call.statement_index == statement && call.authored_expression == expression)
        .map(|call| call.call_ordinal)
        .collect::<Vec<_>>();
    for ordinal in invocations {
        let Some(validation::CallSite::Expression {
            expression: actual,
            call,
        }) = validation::find_call_site(program, machine, state, statement, ordinal)
        else {
            continue;
        };
        if actual != expression {
            continue;
        }
        let invocation = Invocation {
            machine,
            state,
            statement,
            ordinal,
            expression,
            call,
            events: InvocationEvents::Completed(events),
        };
        if invocation.evaluation(program) != Some(evaluation) {
            continue;
        }
        let selected = program.requirement_call_signature(machine, call);
        let Some(parameters) = selected
            .map(|signature| program.state_signature_parameters(signature))
            .or_else(|| validation::call_target_parameters(program, call.target_symbol))
        else {
            continue;
        };
        let plans = facts
            .qualifications
            .content
            .conservation_plans
            .iter()
            .enumerate()
            .filter(|(_, plan)| {
                plan.owner == call.target_symbol || plan.callable == call.target_symbol
            })
            .map(|(position, plan)| (position, plan.clone()))
            .collect::<Vec<_>>();
        for (plan_index, plan) in plans {
            if facts
                .qualifications
                .content
                .claim_transformations
                .iter()
                .any(|(_, previous)| {
                    previous.machine_symbol == machine
                        && previous.state_symbol == state
                        && previous.statement_index == statement
                        && previous.evaluation == evaluation
                        && previous.source
                            == ContentClaimTransformationSource::Call {
                                call_ordinal: ordinal,
                                plan_index,
                            }
                })
            {
                continue;
            }
            let Some((input, output)) =
                owned_return_sides(plan.equation.left(), plan.equation.right())
            else {
                continue;
            };
            for projection in facts.qualifications.content.plans.clone() {
                let mut input_subjects = Vec::new();
                let mut output_subjects = Vec::new();
                if !projection_places(input, &projection, &mut input_subjects)
                    || !projection_places(output, &projection, &mut output_subjects)
                {
                    continue;
                }
                let bound = (|| {
                    let mut inputs = ClaimBindings::default();
                    let mut outputs = ClaimBindings::default();
                    for subject in input_subjects {
                        let ContentPlaceRoot::Parameter {
                            position, symbol, ..
                        } = subject.root
                        else {
                            return None;
                        };
                        let position = usize::try_from(position).ok()?;
                        if parameters.get(position)?.symbol != symbol {
                            return None;
                        }
                        let path = content_segments_to_fact_path(&subject.segments)?;
                        let identity = input_claim(
                            program,
                            facts,
                            &invocation,
                            parameters,
                            position,
                            &path,
                            &projection,
                        )?;
                        let state_flow = facts.flow.control.state(machine, state)?;
                        let call_flow = facts
                            .flow
                            .control
                            .calls
                            .span_or_empty(state_flow.calls)
                            .iter()
                            .find(|candidate| {
                                candidate.statement_index == statement
                                    && candidate.call_ordinal == ordinal
                            })?;
                        let (_, receiver_offset) = crate::flow::captured_call_operands(
                            program,
                            &facts.flow,
                            state_flow,
                            call_flow,
                        )?;
                        let observation = call_flow
                            .operands
                            .handles()
                            .nth(position + receiver_offset)?;
                        inputs.push(
                            projection.domain,
                            projection.semantic_domain,
                            projection.machine,
                            subject,
                            path,
                            identity,
                            observation,
                        )?;
                    }
                    for subject in output_subjects {
                        let path = content_segments_to_fact_path(&subject.segments)?;
                        let identity = result_claim(program, facts, &invocation, &path)?;
                        outputs.push(
                            projection.domain,
                            projection.semantic_domain,
                            projection.machine,
                            subject,
                            path,
                            identity,
                            arena::Handle::invalid(),
                        )?;
                    }
                    (!inputs.is_empty() && !outputs.is_empty()).then_some((inputs, outputs))
                })();
                let Some((inputs, outputs)) = bound else {
                    // A relation without an exact custody join supplies no
                    // ancestry. Ordinary call qualification checking reports
                    // missing/ambiguous result custody on the public path.
                    continue;
                };
                let inputs = inputs.publish(facts);
                let outputs = outputs.publish(facts);
                facts.qualifications.content.claim_transformations.insert(
                    ContentClaimTransformation {
                        machine_symbol: machine,
                        state_symbol: state,
                        statement_index: statement,
                        expression,
                        evaluation,
                        source: ContentClaimTransformationSource::Call {
                            call_ordinal: ordinal,
                            plan_index,
                        },
                        inputs,
                        outputs,
                    },
                );
            }
        }
    }
}
