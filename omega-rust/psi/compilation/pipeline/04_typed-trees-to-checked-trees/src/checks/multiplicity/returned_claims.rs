//! Current custody at a completed structural return.
//!
//! A call's origin alternatives and the claim currently held by its caller
//! answer different questions. This query starts at the flow owner's exact
//! outgoing capture, joins its consuming permission occurrences, and keeps
//! the caller's identities. A lineage consumer can subsequently expand those
//! identities through call receipts; content substitution must not replace a
//! completed invocation with its callee's static establishment.

use checked_trees::{CheckFacts, FlowOperandFact, FlowPermissionEvaluation, FlowStateOutgoingFact};
use facts::PlaceSegment;
use language_semantics::{PermissionAccess, PermissionClaimIdentity, PermissionEventKind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ReturnedClaimSource {
    Live(PermissionClaimIdentity),
    Inactive,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ReturnedClaim {
    pub path: Vec<PlaceSegment>,
    pub source: ReturnedClaimSource,
}

/// Every declared live result path must have one exact consuming occurrence.
/// Missing captures and ambiguous events cannot silently remove an exit or a
/// child. Named transfers have no local result capture; their caller consumes
/// the selected target's complete summary instead.
pub(crate) fn current_returned_claims(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    outgoing: &FlowStateOutgoingFact,
) -> Result<Vec<ReturnedClaim>, &'static str> {
    if !outgoing.returned_value.is_valid() {
        return Err("outgoing edge has no completed value capture");
    }
    let operand = facts.flow.control.operands.get(outgoing.returned_value);
    captured_claims(program, facts, outgoing, operand)
}

fn captured_claims(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    outgoing: &FlowStateOutgoingFact,
    operand: &FlowOperandFact,
) -> Result<Vec<ReturnedClaim>, &'static str> {
    if !operand.type_reference.is_valid()
        || operand.state_symbol != outgoing.state_symbol
        || operand.statement_index != outgoing.statement_index
    {
        return Err("returned capture does not belong to this typed outgoing occurrence");
    }
    let frontier = super::linear_claim_frontier(program, operand.type_reference);
    if frontier.is_empty() {
        return Ok(Vec::new());
    }
    let evaluation = if outgoing.transition_target.is_valid() {
        FlowPermissionEvaluation::TransitionTarget(outgoing.transition_target)
    } else {
        FlowPermissionEvaluation::Statement
    };
    let mut claims = Vec::new();
    if let Some(place) = validation::canonical_place_from_expression_in_state(
        program,
        outgoing.state_symbol,
        outgoing.statement_index,
        operand.expression,
    ) {
        for (_, event) in facts
            .flow
            .ownership
            .permissions
            .iter()
            .filter(|(_, event)| {
                event.machine_symbol == outgoing.machine_symbol
                    && event.state_symbol == outgoing.state_symbol
                    && event.evaluation == evaluation
                    && event.expression == operand.expression
                    && event.kind == PermissionEventKind::Transfer
                    && event.access == PermissionAccess::Owned
                    && event.root == place.root
                    && event.source
                        == language_semantics::PermissionEventSource::Statement {
                            statement_index: outgoing.statement_index,
                        }
            })
        {
            let path = facts.flow.ownership.segments.span_or_empty(event.segments);
            let Some(suffix) = path.strip_prefix(place.segments.as_slice()) else {
                continue;
            };
            let source = if event.obligation_live {
                if event.claim_identity == PermissionClaimIdentity::Unknown {
                    return Err("returned live permission has no established claim identity");
                }
                ReturnedClaimSource::Live(event.claim_identity)
            } else {
                ReturnedClaimSource::Inactive
            };
            claims.push(ReturnedClaim {
                path: suffix.to_vec(),
                source,
            });
        }
    }
    if claims.is_empty() {
        // Child paths are resolved destination identities retained by operand
        // evaluation. Constructors, nested aggregates and detached values use
        // that same capture tree; no source-form return recognizer is needed.
        for child in facts
            .flow
            .control
            .operand_children
            .span_or_empty(operand.children)
        {
            let child_operand = facts.flow.control.operands.get(child.value);
            for mut claim in captured_claims(program, facts, outgoing, child_operand)? {
                let mut path = facts
                    .flow
                    .control
                    .operand_child_paths
                    .span_or_empty(child.path)
                    .to_vec();
                path.append(&mut claim.path);
                claim.path = path;
                claims.push(claim);
            }
        }
    }
    for expected in &frontier {
        let mut matches = claims.iter().filter(|claim| claim.path == expected.path);
        let Some(found) = matches.next() else {
            return Err("returned capture omits a declared claim path");
        };
        if matches.next().is_some() {
            return Err("returned claim path has more than one consuming occurrence");
        }
        if found.source == ReturnedClaimSource::Inactive && !expected.conditional {
            return Err("unconditional returned claim was recorded as inactive");
        }
    }
    if claims.len() != frontier.len() {
        return Err("returned permission frontier contains an undeclared claim path");
    }
    Ok(claims)
}
