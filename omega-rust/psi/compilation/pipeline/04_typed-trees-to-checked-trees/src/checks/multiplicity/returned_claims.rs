//! Current custody at a completed structural return.
//!
//! A call's origin alternatives and the claim currently held by its caller
//! answer different questions. This query starts at the flow owner's exact
//! outgoing capture, joins its consuming permission occurrences, and keeps
//! the caller's identities. A lineage consumer can subsequently expand those
//! identities through call receipts; content substitution must not replace a
//! completed invocation with its callee's static establishment.

use super::linear_obligations::LinearPlace;
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
    if operand.state_symbol != outgoing.state_symbol
        || operand.statement_index != outgoing.statement_index
    {
        return Err("returned capture does not belong to this typed outgoing occurrence");
    }
    captured_claims(
        program,
        facts,
        operand,
        Default::default(),
        &|operand, place, _| returned_place_claims(facts, outgoing, operand, place),
    )
}

/// Observe the live frontier before the enclosing operation moves its result.
/// This uses the same capture/path traversal as outgoing correspondence, but
/// reads the current ownership walk rather than future transfer events.
pub(crate) fn completed_owned_claims(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    operand: &FlowOperandFact,
    places: &[LinearPlace],
    evaluation_events: &[checked_trees::FlowPermissionEventFact],
) -> Result<Vec<ReturnedClaim>, &'static str> {
    captured_claims(
        program,
        facts,
        operand,
        Default::default(),
        &|capture, source, parent| {
            // An assembled child has already moved into this value. Its exact
            // transfer retains the original claim even if a later sibling restored
            // the source slot with a different owner. Only the current permission
            // producer's evaluation prefix is supplied here; Call-source moves
            // belong to the callee and cannot provide construction custody.
            if parent.is_valid() {
                let transferred = transferred_place_claims(
                    facts,
                    capture.state_symbol,
                    capture.statement_index,
                    capture,
                    source,
                    evaluation_events.iter(),
                )?;
                if !transferred.is_empty() {
                    return Ok(transferred);
                }
            }
            places
                .iter()
                .filter_map(|place| {
                    if place.root != source.root || !place.ever_established || place.zero_image {
                        return None;
                    }
                    let path = place.path.strip_prefix(source.segments.as_slice())?;
                    if !place.live && !place.case_excluded {
                        return None;
                    }
                    Some(if place.case_excluded {
                        Ok(ReturnedClaim {
                            path: path.to_vec(),
                            source: ReturnedClaimSource::Inactive,
                        })
                    } else {
                        match place.claim_identity {
                            Some(identity) if identity != PermissionClaimIdentity::Unknown => {
                                Ok(ReturnedClaim {
                                    path: path.to_vec(),
                                    source: ReturnedClaimSource::Live(identity),
                                })
                            }
                            _ => Err("completed live owner has no exact claim identity"),
                        }
                    })
                })
                .collect()
        },
    )
}

fn returned_place_claims(
    facts: &CheckFacts,
    outgoing: &FlowStateOutgoingFact,
    operand: &FlowOperandFact,
    place: &validation::CanonicalPlace,
) -> Result<Vec<ReturnedClaim>, &'static str> {
    let evaluation = if outgoing.transition_target.is_valid() {
        FlowPermissionEvaluation::TransitionTarget(outgoing.transition_target)
    } else {
        FlowPermissionEvaluation::Statement
    };
    transferred_place_claims(
        facts,
        outgoing.state_symbol,
        outgoing.statement_index,
        operand,
        place,
        facts
            .flow
            .ownership
            .permissions
            .iter()
            .filter(|(_, event)| {
                event.machine_symbol == outgoing.machine_symbol && event.evaluation == evaluation
            })
            .map(|(_, event)| event),
    )
}

/// Claim movement is read from the produced ledger, never inferred from a
/// dead source slot. The caller scopes this iterator to the exact evaluation;
/// an in-progress producer has not yet finalized every event's lane field.
fn transferred_place_claims<'event>(
    facts: &CheckFacts,
    state: symbols::SymbolHandle,
    statement_index: usize,
    operand: &FlowOperandFact,
    place: &validation::CanonicalPlace,
    events: impl Iterator<Item = &'event checked_trees::FlowPermissionEventFact>,
) -> Result<Vec<ReturnedClaim>, &'static str> {
    let mut claims = Vec::new();
    for event in events.filter(|event| {
        event.state_symbol == state
            && event.expression == operand.expression
            && event.kind == PermissionEventKind::Transfer
            && event.access == PermissionAccess::Owned
            && event.root == place.root
            && event.source
                == language_semantics::PermissionEventSource::Statement { statement_index }
    }) {
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
    Ok(claims)
}

fn captured_claims(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    operand: &FlowOperandFact,
    parent: typed_trees::expression::ExpressionHandle,
    observe: &impl Fn(
        &FlowOperandFact,
        &validation::CanonicalPlace,
        typed_trees::expression::ExpressionHandle,
    ) -> Result<Vec<ReturnedClaim>, &'static str>,
) -> Result<Vec<ReturnedClaim>, &'static str> {
    if !operand.type_reference.is_valid() {
        return Err("completed capture has no result type");
    }
    let frontier = super::linear_claim_frontier(program, operand.type_reference);
    if frontier.is_empty() {
        return Ok(Vec::new());
    }
    let mut claims = match validation::ownership_place_from_expression_in_state(
        program,
        operand.state_symbol,
        operand.statement_index,
        operand.expression,
    ) {
        Some(place) => observe(operand, &place, parent)?,
        None => Vec::new(),
    };
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
            if child.parent != operand.expression
                || child_operand.state_symbol != operand.state_symbol
                || child_operand.statement_index != operand.statement_index
            {
                return Err("child capture belongs to another evaluation occurrence");
            }
            for mut claim in
                captured_claims(program, facts, child_operand, operand.expression, observe)?
            {
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
