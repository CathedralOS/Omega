//! Current custody at an exact completed structural consumption.
//!
//! A call's origin alternatives and the claim currently held by its caller
//! answer different questions. This query starts at the flow owner's exact
//! outgoing capture, joins its consuming permission occurrences, and keeps
//! the caller's identities. A lineage consumer can subsequently expand those
//! identities through call receipts; content substitution must not replace a
//! completed invocation with its callee's static establishment.
//! Affine storage transfers accompany these claims but are not claim rows:
//! their debt-free carrier is neither an inactive linear payload nor a new
//! aggregate root claim. Storage movement and cleanup retain those events in
//! their own custody replay.

use super::linear_obligations::LinearPlace;
use crate::flow::{CapturedValueChoice, SelectionCondition, SelectionConditions};
use checked_trees::{
    CheckFacts, FlowOperandCompletion, FlowOperandFact, FlowPermissionEvaluation,
    FlowStateOutgoingFact,
};
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
    pub condition: SelectionCondition,
}

/// Operand completion precedes its enclosing store. Only the receiving cut
/// may read the current destination after its owned frontier is established.
#[derive(Clone, Copy, Debug)]
pub(crate) enum CompletedClaimLocation {
    Expression,
    ReceivingStorage,
}

/// One authentic consumer occurrence. The completed operand has its own
/// evaluation prefix; the enclosing exit or invocation only names its owner.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ClaimConsumption {
    pub machine: symbols::SymbolHandle,
    pub state: symbols::SymbolHandle,
    pub statement_index: usize,
    pub evaluation: FlowPermissionEvaluation,
    pub source: language_semantics::PermissionEventSource,
}

impl ClaimConsumption {
    /// A construction receives its children during expression evaluation. The
    /// invocation consumes only the completed anonymous owner afterwards.
    fn for_construction_child(self) -> Self {
        Self {
            source: language_semantics::PermissionEventSource::Statement {
                statement_index: self.statement_index,
            },
            ..self
        }
    }

    fn consumes(
        self,
        operand: &FlowOperandFact,
        event: &checked_trees::FlowPermissionEventFact,
    ) -> bool {
        event.machine_symbol == self.machine
            && event.state_symbol == self.state
            && event.evaluation == self.evaluation
            && event.expression == operand.expression
            && event.source == self.source
            && event.kind == PermissionEventKind::Transfer
            && event.multiplicity == language_semantics::Multiplicity::Linear
            && event.access == PermissionAccess::Owned
    }

    fn for_operand(self, operand: &FlowOperandFact) -> Self {
        if operand.statement_index == self.statement_index {
            self
        } else {
            Self {
                statement_index: operand.statement_index,
                evaluation: FlowPermissionEvaluation::Statement,
                source: language_semantics::PermissionEventSource::Statement {
                    statement_index: operand.statement_index,
                },
                ..self
            }
        }
    }
}

/// Every declared live result path must have one exact consuming occurrence.
/// Missing captures and ambiguous events cannot silently remove an exit or a
/// child. Named transfers have no local result capture; their caller consumes
/// the selected target's complete summary instead.
pub(crate) fn current_returned_claims(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    outgoing: &FlowStateOutgoingFact,
    conditions: &mut SelectionConditions,
) -> Result<Vec<ReturnedClaim>, &'static str> {
    if !outgoing.returned_value.is_valid() {
        return Err("outgoing edge has no completed value capture");
    }
    let operand = facts.flow.control.operands.get(outgoing.returned_value);
    let consumer = ClaimConsumption {
        machine: outgoing.machine_symbol,
        state: outgoing.state_symbol,
        statement_index: outgoing.statement_index,
        evaluation: if outgoing.transition_target.is_valid() {
            FlowPermissionEvaluation::TransitionTarget(outgoing.transition_target)
        } else {
            FlowPermissionEvaluation::Statement
        },
        source: language_semantics::PermissionEventSource::Statement {
            statement_index: outgoing.statement_index,
        },
    };
    captured_claims(
        program,
        facts,
        consumer,
        operand,
        ClaimObservation::Consumed,
        None,
        conditions,
    )
}

pub(crate) fn consumed_operand_claims(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    consumer: ClaimConsumption,
    operand: arena::Handle<FlowOperandFact>,
    conditions: &mut SelectionConditions,
) -> Result<Vec<ReturnedClaim>, &'static str> {
    if !facts.flow.control.operands.is_valid(operand) {
        return Err("consumer has no completed operand");
    }
    let operand = facts.flow.control.operands.get(operand);
    if operand.statement_index != consumer.statement_index {
        return Err("consumer and operand have different evaluation coordinates");
    }
    captured_claims(
        program,
        facts,
        consumer,
        operand,
        ClaimObservation::Consumed,
        None,
        conditions,
    )
}

/// Ownership before publication and custody after consumption are distinct
/// observations of the same retained completion tree.
#[derive(Clone, Copy)]
enum ClaimObservation<'a> {
    Consumed,
    Completed {
        places: &'a [LinearPlace],
        events: &'a [checked_trees::FlowPermissionEventFact],
        parent: typed_trees::expression::ExpressionHandle,
    },
}

impl ClaimObservation<'_> {
    fn for_child(self, parent: typed_trees::expression::ExpressionHandle) -> Self {
        match self {
            Self::Consumed => self,
            Self::Completed { places, events, .. } => Self::Completed {
                places,
                events,
                parent,
            },
        }
    }
}

pub(crate) fn completed_owned_claims(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    value: arena::Handle<FlowOperandFact>,
    location: CompletedClaimLocation,
    places: &[LinearPlace],
    evaluation_events: &[checked_trees::FlowPermissionEventFact],
    conditions: &mut SelectionConditions,
) -> Result<Vec<ReturnedClaim>, &'static str> {
    if !facts.flow.control.operands.is_valid(value) {
        return Err("completed owner has no exact value capture");
    }
    let operand = facts.flow.control.operands.get(value);
    let destination = match location {
        CompletedClaimLocation::Expression => None,
        CompletedClaimLocation::ReceivingStorage => receiving_destination(program, facts, value)?,
    };
    let owner = facts
        .flow
        .control
        .states
        .iter()
        .find_map(|(_, state)| {
            (state.state_symbol == operand.state_symbol).then_some(state.machine_symbol)
        })
        .ok_or("completed owner has no state flow")?;
    let consumer = ClaimConsumption {
        machine: owner,
        state: operand.state_symbol,
        statement_index: operand.statement_index,
        evaluation: FlowPermissionEvaluation::Statement,
        source: language_semantics::PermissionEventSource::Statement {
            statement_index: operand.statement_index,
        },
    };
    captured_claims(
        program,
        facts,
        consumer,
        operand,
        ClaimObservation::Completed {
            places,
            events: evaluation_events,
            parent: Default::default(),
        },
        destination.as_ref(),
        conditions,
    )
}

/// A retained store handle and destination identify the receiving occurrence.
/// A malformed actual store withholds custody; a non-store completion keeps its
/// expression coordinate. The live frontier still supplies every identity.
fn receiving_destination(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    value: arena::Handle<FlowOperandFact>,
) -> Result<Option<validation::CanonicalPlace>, &'static str> {
    let capture = facts.flow.control.operands.get(value);
    let owner = program
        .machine_holding_state(capture.state_symbol)
        .ok_or("completed store has no declared machine owner")?;
    let state = facts
        .flow
        .control
        .state(owner.symbol, capture.state_symbol)
        .ok_or("completed store has no state flow")?;
    let Some(statement) = facts.flow.state_statement(state, capture.statement_index) else {
        // Entry/implicit return completions need not be stored statements.
        return Ok(None);
    };
    if statement.stored_value != value {
        return Ok(None);
    }
    if !facts.semantic.places.is_valid(statement.stored_destination) {
        return Err("completed store has no exact receiving destination");
    }
    crate::flow::canonical_place_from_semantic_place(
        program,
        &facts.semantic,
        facts.semantic.places.get(statement.stored_destination),
    )
    .map(Some)
    .ok_or("completed store destination has no canonical identity")
}

fn captured_claims(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    consumer: ClaimConsumption,
    operand: &FlowOperandFact,
    observation: ClaimObservation<'_>,
    receiving: Option<&validation::CanonicalPlace>,
    conditions: &mut SelectionConditions,
) -> Result<Vec<ReturnedClaim>, &'static str> {
    if !operand.type_reference.is_valid() || operand.state_symbol != consumer.state {
        return Err("returned capture does not belong to this typed outgoing occurrence");
    }
    let frontier = super::linear_claim_frontier(program, operand.type_reference);
    if frontier.is_empty() {
        return Ok(Vec::new());
    }
    if let FlowOperandCompletion::Selected(alternatives) = operand.completion {
        let rows = facts
            .flow
            .control
            .operand_alternatives
            .span(alternatives)
            .filter(|rows| !rows.is_empty())
            .ok_or("selected return has no completed alternatives")?;
        let mut claims = Vec::new();
        for (index, alternative) in rows.iter().enumerate() {
            if !alternative.source_arm.is_valid()
                || !facts.flow.control.operands.is_valid(alternative.value)
            {
                return Err("selected return omits a typed arm completion");
            }
            let choice = conditions
                .choice(
                    &facts.flow.control,
                    CapturedValueChoice {
                        selection: alternatives,
                        alternative: alternatives.handle_at(
                            u32::try_from(index)
                                .map_err(|_| "selection arm exceeds arena coordinates")?,
                        ),
                    },
                )
                .ok_or("selected return has an invalid choice coordinate")?;
            for mut claim in captured_claims(
                program,
                facts,
                consumer,
                facts.flow.control.operands.get(alternative.value),
                observation,
                receiving,
                conditions,
            )? {
                claim.condition = conditions.all(choice, claim.condition);
                claims.push(claim);
            }
        }
        check_frontier(program, facts, operand, &claims, conditions)?;
        return Ok(claims);
    }
    let consumer = consumer.for_operand(operand);
    let evaluation = consumer.evaluation;
    let mut claims = Vec::new();
    let transferred_place = if matches!(observation, ClaimObservation::Consumed) {
        crate::flow::owned_operand_transfer_place(
            facts,
            consumer.machine,
            consumer.state,
            operand.expression,
            evaluation,
            consumer.source,
        )?
    } else {
        None
    };
    let has_carrier_transfer = transferred_place.is_some();
    let mut source_place = receiving.cloned().or(transferred_place).or_else(|| {
        validation::ownership_place_from_expression_in_state(
            program,
            operand.state_symbol,
            operand.statement_index,
            operand.expression,
        )
    });
    if matches!(observation, ClaimObservation::Consumed) {
        // A borrowed extraction can complete a new owned value before the
        // enclosing invocation consumes it. Its candidate storage coordinate
        // still names the alias slot; only the committed consuming event names
        // the completed owner. Never borrow that slot's claims for the value.
        let anonymous = facts::PlaceRoot::Expression(operand.expression);
        let consumed_owner = operand.expression.is_valid()
            && facts
                .flow
                .ownership
                .permissions
                .iter()
                .any(|(_, event)| consumer.consumes(operand, event) && event.root == anonymous);
        if consumed_owner {
            if let Some(place) = &source_place
                && place.root != anonymous
                && facts.flow.ownership.permissions.iter().any(|(_, event)| {
                    consumer.consumes(operand, event)
                        && event.root == place.root
                        && facts
                            .flow
                            .ownership
                            .segments
                            .span_or_empty(event.segments)
                            .starts_with(&place.segments)
                })
            {
                return Err("completed operand has competing consuming owners");
            }
            source_place = Some(validation::CanonicalPlace {
                root: anonymous,
                segments: Vec::new(),
            });
        }
    }
    let mut unresolved_transfers: Vec<Vec<PlaceSegment>> = Vec::new();
    if let Some(place) = &source_place {
        match observation {
            ClaimObservation::Consumed => {
                for (_, event) in facts
                    .flow
                    .ownership
                    .permissions
                    .iter()
                    .filter(|(_, event)| {
                        consumer.consumes(operand, event) && event.root == place.root
                    })
                {
                    let path = facts.flow.ownership.segments.span_or_empty(event.segments);
                    let Some(suffix) = path.strip_prefix(place.segments.as_slice()) else {
                        continue;
                    };
                    let source = if event.obligation_live {
                        if event.claim_identity == PermissionClaimIdentity::Unknown {
                            // Unknown is never source authority. The exact consuming
                            // occurrence may instead carry a retained selected store;
                            // resolve that checked transport below.
                            if unresolved_transfers
                                .iter()
                                .any(|path| path.as_slice() == suffix)
                            {
                                return Err(
                                    "completed owner has duplicate unresolved claim transfers",
                                );
                            }
                            unresolved_transfers.push(suffix.to_vec());
                            continue;
                        }
                        ReturnedClaimSource::Live(event.claim_identity)
                    } else {
                        ReturnedClaimSource::Inactive
                    };
                    claims.push(ReturnedClaim {
                        path: suffix.to_vec(),
                        source,
                        condition: conditions.always(),
                    });
                }
            }
            ClaimObservation::Completed {
                places,
                events,
                parent,
            } => {
                // An assembled child moved before later siblings ran. Keep that
                // exact prefix transfer instead of reading a replacement owner.
                if parent.is_valid() {
                    for event in events.iter().filter(|event| {
                        event.state_symbol == operand.state_symbol
                            && event.expression == operand.expression
                            && event.kind == PermissionEventKind::Transfer
                            && event.multiplicity == language_semantics::Multiplicity::Linear
                            && event.access == PermissionAccess::Owned
                            && event.root == place.root
                            && event.source
                                == language_semantics::PermissionEventSource::Statement {
                                    statement_index: operand.statement_index,
                                }
                    }) {
                        let path = facts.flow.ownership.segments.span_or_empty(event.segments);
                        let Some(suffix) = path.strip_prefix(place.segments.as_slice()) else {
                            continue;
                        };
                        let source = if event.obligation_live {
                            if event.claim_identity == PermissionClaimIdentity::Unknown {
                                return Err("completed child transfer has no exact identity");
                            }
                            ReturnedClaimSource::Live(event.claim_identity)
                        } else {
                            ReturnedClaimSource::Inactive
                        };
                        claims.push(ReturnedClaim {
                            path: suffix.to_vec(),
                            source,
                            condition: conditions.always(),
                        });
                    }
                }
                if claims.is_empty() {
                    for current in places.iter().filter(|current| {
                        current.root == place.root
                            && current.multiplicity == language_semantics::Multiplicity::Linear
                            && current.ever_established
                            && !current.zero_image
                            && (current.live || current.case_excluded)
                    }) {
                        let Some(path) = current.path.strip_prefix(place.segments.as_slice())
                        else {
                            continue;
                        };
                        let source = if current.case_excluded {
                            ReturnedClaimSource::Inactive
                        } else {
                            let identity = current
                                .claim_identity
                                .filter(|identity| *identity != PermissionClaimIdentity::Unknown)
                                .ok_or("completed live owner has no exact claim identity")?;
                            ReturnedClaimSource::Live(identity)
                        };
                        claims.push(ReturnedClaim {
                            path: path.to_vec(),
                            source,
                            condition: conditions.always(),
                        });
                    }
                }
            }
        }
    }
    if receiving.is_none() {
        // A whole anonymous transfer publishes its live identities, while an
        // absent payload has no resource transfer. Retain those live rows and
        // complete only uncovered paths from the actual construction children.
        // Children move at the statement occurrence before the enclosing call
        // consumes the completed owner. Neither source epoch replaces the other.
        let transferred_paths = claims
            .iter()
            .map(|claim| claim.path.clone())
            .collect::<Vec<_>>();
        for child in facts
            .flow
            .control
            .operand_children
            .span_or_empty(operand.constructed_children())
        {
            let destination = facts
                .flow
                .control
                .operand_child_paths
                .span(child.path)
                .ok_or("constructed child has no typed destination path")?;
            if !frontier.iter().any(|expected| {
                expected.path.starts_with(destination)
                    && !transferred_paths.contains(&expected.path)
            }) {
                continue;
            }
            if !facts.flow.control.operands.is_valid(child.value) {
                return Err("construction omits its completed child");
            }
            let child_operand = facts.flow.control.operands.get(child.value);
            if child.parent != operand.expression
                || child_operand.state_symbol != operand.state_symbol
                || child_operand.statement_index != operand.statement_index
            {
                return Err("child capture belongs to another evaluation occurrence");
            }
            for mut claim in captured_claims(
                program,
                facts,
                consumer.for_construction_child(),
                child_operand,
                observation.for_child(operand.expression),
                None,
                conditions,
            )? {
                let mut path = destination.to_vec();
                path.append(&mut claim.path);
                claim.path = path;
                if transferred_paths.contains(&claim.path)
                    || ((!transferred_paths.is_empty() || !unresolved_transfers.is_empty())
                        && !unresolved_transfers.contains(&claim.path)
                        && claim.source != ReturnedClaimSource::Inactive)
                {
                    // A child cannot repair a missing live transfer from an
                    // already published whole owner. An exact unresolved live
                    // transfer can retain its own completed child identity;
                    // otherwise only authenticated absence can fill a path.
                    continue;
                }
                claims.push(claim);
            }
        }
    }
    if claims.is_empty() && matches!(observation, ClaimObservation::Consumed) {
        claims = completed_call_claims(program, facts, consumer, operand, conditions)?;
    }
    for expected in &frontier {
        if claims.iter().any(|claim| claim.path == expected.path) {
            continue;
        }
        if receiving.is_none()
            && expected.conditional
            && crate::flow::literal_value_path_is_inactive(
                program,
                operand.expression,
                operand.type_reference,
                &expected.path,
            )
        {
            claims.push(ReturnedClaim {
                path: expected.path.clone(),
                source: ReturnedClaimSource::Inactive,
                condition: conditions.always(),
            });
            continue;
        }
        // A whole carrier can transfer a proven absent payload without a
        // resource Transfer event for that payload. The exact consuming
        // capture may retain its prior debt-free establishment, provided the
        // source stayed unchanged through this statement. A missing claim or
        // Unknown identity alone never supplies this positive absence proof.
        if receiving.is_none()
            && expected.conditional
            && let Some(mut place) = source_place.clone()
            && let Some(state) = program.state_by_symbol(operand.state_symbol)
        {
            place.segments.extend_from_slice(&expected.path);
            if super::claim_outcomes::preserved_inactive_place(
                program,
                &facts.borrow,
                state,
                operand.statement_index,
                &place,
                &facts.flow.ownership,
                facts
                    .flow
                    .ownership
                    .permissions
                    .iter()
                    .map(|(_, event)| event),
            )
            .is_some()
            {
                claims.push(ReturnedClaim {
                    path: expected.path.clone(),
                    source: ReturnedClaimSource::Inactive,
                    condition: conditions.always(),
                });
                continue;
            }
        }
        if matches!(observation, ClaimObservation::Consumed)
            && let Some(stored) = stored_claims(
                program,
                facts,
                consumer,
                operand,
                &expected.path,
                source_place.as_ref(),
                has_carrier_transfer,
                evaluation,
                conditions,
            )?
        {
            claims.extend(stored);
        }
    }
    check_frontier(program, facts, operand, &claims, conditions)?;
    Ok(claims)
}

/// An owned transfer retains its completed RHS even when that value was not
/// copyable. This is claim transport, not the scalar content observation query:
/// a write below a claim leaves its identity intact while independently
/// retiring the producer's content theorem. A replacement at/above the claim
/// stops this trace. No declaration initializer is replayed.
fn stored_claims(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    consumer: ClaimConsumption,
    operand: &FlowOperandFact,
    path: &[PlaceSegment],
    source_place: Option<&validation::CanonicalPlace>,
    has_carrier_transfer: bool,
    evaluation: FlowPermissionEvaluation,
    conditions: &mut SelectionConditions,
) -> Result<Option<Vec<ReturnedClaim>>, &'static str> {
    let Some(mut source) = source_place.cloned() else {
        return Ok(None);
    };
    source.segments.extend_from_slice(path);
    if !matches!(source.root, facts::PlaceRoot::Symbol(_)) {
        return Ok(None);
    }
    // A live claim needs its committed resource transfer. Whole-carrier move
    // discovery also retains absent payloads, for which production deliberately
    // emits no leaf Transfer. That coordinate may request positive absence from
    // the actual stored completion, but can never authorize a live claim.
    let consumed = facts.flow.ownership.permissions.iter().any(|(_, event)| {
        event.machine_symbol == consumer.machine
            && event.state_symbol == operand.state_symbol
            && event.evaluation == evaluation
            && event.expression == operand.expression
            && event.source == consumer.source
            && event.kind == PermissionEventKind::Transfer
            && event.access == PermissionAccess::Owned
            && event.obligation_live
            && event.root == source.root
            && facts.flow.ownership.segments.span_or_empty(event.segments) == source.segments
    });
    if !consumed && !has_carrier_transfer {
        return Ok(None);
    }
    if !crate::flow::binding_origin_is_stable(
        program,
        &facts.flow.control,
        operand.entry_origin_writes,
        operand.entry_origin_unknown_write,
        &source,
    ) {
        return Ok(None);
    }
    let state = program
        .state_by_symbol(operand.state_symbol)
        .ok_or("stored claim has no state owner")?;
    let flow = facts
        .flow
        .control
        .state(consumer.machine, operand.state_symbol)
        .ok_or("stored claim has no state flow")?;
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    let end = order
        .position(operand.statement_index)
        .or_else(|| {
            (operand.statement_index
                == program
                    .statement_table
                    .statements(state.statement_nodes)
                    .len())
            .then_some(order.indices().len())
        })
        .ok_or("stored claim has no admitted execution coordinate")?;
    for &index in order.indices()[..end].iter().rev() {
        let statement = facts
            .flow
            .state_statement(flow, index)
            .ok_or("stored claim trace omits an executed statement")?;
        if facts.semantic.places.is_valid(statement.stored_destination)
            && facts.flow.control.operands.is_valid(statement.stored_value)
        {
            let destination = crate::flow::canonical_place_from_semantic_place(
                program,
                &facts.semantic,
                facts.semantic.places.get(statement.stored_destination),
            )
            .ok_or("stored claim destination has no canonical identity")?;
            if destination.root == source.root && source.segments.starts_with(&destination.segments)
            {
                let saved = facts.flow.control.operands.get(statement.stored_value);
                if saved.state_symbol != operand.state_symbol
                    || saved.statement_index != index
                    || !saved.expression.is_valid()
                {
                    return Ok(None);
                }
                let relative = &source.segments[destination.segments.len()..];
                let claims = captured_claims(
                    program,
                    facts,
                    consumer.for_operand(saved),
                    saved,
                    ClaimObservation::Consumed,
                    None,
                    conditions,
                )?;
                let mut selected = Vec::new();
                for mut claim in claims.into_iter().filter(|claim| {
                    claim.path == relative
                        && (consumed || claim.source == ReturnedClaimSource::Inactive)
                }) {
                    claim.path = path.to_vec();
                    selected.push(claim);
                }
                return Ok((!selected.is_empty()).then_some(selected));
            }
        }
        if !crate::flow::binding_origin_is_stable(
            program,
            &facts.flow.control,
            statement.completed_writes,
            statement.completed_unknown_write,
            &source,
        ) {
            return Ok(None);
        }
    }
    Ok(None)
}

fn check_frontier(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    operand: &FlowOperandFact,
    claims: &[ReturnedClaim],
    conditions: &mut SelectionConditions,
) -> Result<(), &'static str> {
    let frontier = super::linear_claim_frontier(program, operand.type_reference);
    for expected in &frontier {
        let matching = claims
            .iter()
            .filter(|claim| claim.path == expected.path)
            .collect::<Vec<_>>();
        let mut coverage = conditions.never();
        for (index, claim) in matching.iter().enumerate() {
            if claim.source == ReturnedClaimSource::Inactive && !expected.conditional {
                return Err("unconditional returned claim was recorded as inactive");
            }
            for previous in &matching[..index] {
                let overlap = conditions.all(previous.condition, claim.condition);
                if conditions.possible(&facts.flow.control, overlap) {
                    return Err("returned claim path has overlapping consuming occurrences");
                }
            }
            coverage = conditions.any(coverage, claim.condition);
        }
        if !conditions.covers(&facts.flow.control, coverage) {
            trace_missing_frontier(program, facts, operand, &expected.path, claims);
            return Err("returned capture omits a declared claim path on an alternative");
        }
    }
    for (index, claim) in claims.iter().enumerate() {
        if matches!(claim.source, ReturnedClaimSource::Inactive) {
            continue;
        }
        for other in &claims[index + 1..] {
            if claim.path != other.path
                && claim.source == other.source
                && !super::claim_outcomes::claim_paths_are_case_alternatives(
                    &claim.path,
                    &other.path,
                )
            {
                let overlap = conditions.all(claim.condition, other.condition);
                if conditions.possible(&facts.flow.control, overlap) {
                    return Err("one live claim supplies simultaneous returned paths");
                }
            }
        }
    }
    if claims
        .iter()
        .any(|claim| !frontier.iter().any(|expected| expected.path == claim.path))
    {
        return Err("returned permission frontier contains an undeclared claim path");
    }
    Ok(())
}

/// A fresh result is already owned at its exact completion. Selection does not
/// synthesize a transfer event for that temporary. Join its actual invocation
/// and receiving establishment, retaining the caller's current identity.
fn completed_call_claims(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    consumer: ClaimConsumption,
    operand: &FlowOperandFact,
    conditions: &SelectionConditions,
) -> Result<Vec<ReturnedClaim>, &'static str> {
    let Some(state_flow) = facts
        .flow
        .control
        .states
        .iter()
        .find_map(|(_, state)| (state.state_symbol == operand.state_symbol).then_some(state))
    else {
        return Err("completed result has no state flow");
    };
    let mut calls = facts
        .flow
        .control
        .calls
        .span_or_empty(state_flow.calls)
        .iter()
        .filter(|call| {
            call.statement_index == operand.statement_index
                && call.authored_expression == operand.expression
        });
    let Some(call) = calls.next() else {
        return Ok(Vec::new());
    };
    if calls.next().is_some() {
        return Err("completed result has ambiguous invocation identity");
    }
    let machine = program
        .machine_by_symbol(consumer.machine)
        .ok_or("result owner machine is absent")?;
    let state = program
        .state_by_symbol(operand.state_symbol)
        .ok_or("result owner state is absent")?;
    let evaluation = crate::flow::call_permission_evaluation(
        program,
        machine,
        state,
        call.statement_index,
        call.call_ordinal,
    );
    if evaluation == FlowPermissionEvaluation::Unknown {
        return Err("completed invocation has no permission evaluation");
    }
    let mut claims = Vec::new();
    for (_, event) in facts.flow.ownership.permissions.iter() {
        if event.machine_symbol != consumer.machine
            || event.state_symbol != operand.state_symbol
            || event.evaluation != evaluation
            || event.kind != PermissionEventKind::Establish
            || event.access != PermissionAccess::Owned
            || !event.obligation_live
            || event.claim_identity == PermissionClaimIdentity::Unknown
        {
            continue;
        }
        let Some((statement, expression, path)) =
            validation::receiving_value(program, &facts.flow.ownership.segments, event)
        else {
            continue;
        };
        if statement != call.statement_index || expression != call.authored_expression {
            continue;
        }
        if let language_semantics::PermissionEventSource::Call {
            call_ordinal,
            target_symbol,
            ..
        } = event.source
            && (call_ordinal != call.call_ordinal || target_symbol != call.target_symbol)
        {
            continue;
        }
        let claim = ReturnedClaim {
            path: path.to_vec(),
            source: ReturnedClaimSource::Live(event.claim_identity),
            condition: conditions.always(),
        };
        if !claims.contains(&claim) {
            claims.push(claim);
        }
    }
    Ok(claims)
}

// Temporary bounded diagnostic. Root removes this with its call before landing.
fn trace_missing_frontier(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    operand: &FlowOperandFact,
    path: &[PlaceSegment],
    claims: &[ReturnedClaim],
) {
    let Ok(filter) = std::env::var("OMEGA_TRACE_CLAIM_FRONTIER") else {
        return;
    };
    let Some(machine) = program.machine_holding_state(operand.state_symbol) else {
        return;
    };
    if filter != "*" && filter != program.symbols.name(machine.symbol) {
        return;
    }
    static EMITTED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    if EMITTED.fetch_add(1, std::sync::atomic::Ordering::Relaxed) >= 48 {
        return;
    }
    let ownership = &facts.flow.ownership;
    let state = facts
        .flow
        .control
        .state(machine.symbol, operand.state_symbol);
    let events = ownership
        .permissions
        .iter()
        .filter_map(|(_, event)| {
            (event.state_symbol == operand.state_symbol
                && super::linear_validation::permission_event_statement_index(event.source)
                    == Some(operand.statement_index))
            .then(|| (event, ownership.segments.span_or_empty(event.segments)))
        })
        .collect::<Vec<_>>();
    let transfers = state
        .map(|state| {
            ownership
                .storage_transfers
                .span_or_empty(state.storage_transfers)
        })
        .unwrap_or(&[])
        .iter()
        .filter_map(|event| {
            let statement = match event.source {
                checked_trees::FlowStorageTransferSource::Statement { statement_index }
                | checked_trees::FlowStorageTransferSource::Call {
                    statement_index, ..
                } => statement_index,
            };
            (statement == operand.statement_index)
                .then(|| (event, ownership.segments.span_or_empty(event.segments)))
        })
        .collect::<Vec<_>>();
    let source = validation::ownership_place_from_expression_in_state(
        program,
        operand.state_symbol,
        operand.statement_index,
        operand.expression,
    );
    let prefix_events = ownership
        .permissions
        .iter()
        .filter_map(|(_, event)| {
            (event.machine_symbol == machine.symbol
                && event.state_symbol == operand.state_symbol
                && (event.root == facts::PlaceRoot::Expression(operand.expression)
                    || source
                        .as_ref()
                        .is_some_and(|source| source.root == event.root)
                    || transfers
                        .iter()
                        .any(|(transfer, _)| transfer.root == event.root))
                && super::linear_validation::permission_event_statement_index(event.source)
                    .is_some_and(|statement| statement <= operand.statement_index))
            .then(|| (event, ownership.segments.span_or_empty(event.segments)))
        })
        .collect::<Vec<_>>();
    eprintln!(
        "CLAIM_SOURCE_PREFIX expression={:?} source={source:?} events={prefix_events:?}",
        operand.expression
    );
    let children = facts
        .flow
        .control
        .operand_children
        .span_or_empty(operand.constructed_children())
        .iter()
        .map(|child| {
            (
                child,
                facts
                    .flow
                    .control
                    .operand_child_paths
                    .span_or_empty(child.path),
                facts.flow.control.operands.get(child.value),
            )
        })
        .collect::<Vec<_>>();
    eprintln!(
        "CLAIM_FRONTIER machine={} state={} missing={path:?} operand={operand:?} syntax={:?} claims={claims:?} events={events:?} transfers={transfers:?} children={children:?}",
        program.symbols.name(machine.symbol),
        program.symbols.name(operand.state_symbol),
        operand
            .expression
            .is_valid()
            .then(|| program.expression_table.expression(operand.expression))
    );
}
