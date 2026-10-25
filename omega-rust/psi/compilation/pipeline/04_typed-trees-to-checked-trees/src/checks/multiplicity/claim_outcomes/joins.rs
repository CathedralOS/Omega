//! Bind returned custody to an exact call occurrence, separately from lineage.
//! A callee-local establishment needs this correspondence even with one exit:
//! its static identity cannot identify two simultaneously live invocations.
//! Input forwarding keeps the caller's held identity; differing exit sources
//! retain every alternative rather than manufacturing an unconditional lineage.

use super::{
    bind_claim_outcome_source_at_arguments, claim_paths_are_case_alternatives,
    derive_checked_claim_outcome_maps,
};
use crate::checks::multiplicity::linear_obligations::{
    CheckedClaimOutcomeEntry, CheckedClaimOutcomeMap, CheckedClaimOutcomeSource,
};
use crate::checks::multiplicity::linear_validation::linear_claim_frontier;
use checked_trees::{
    CheckFacts, FlowClaimJoinAlternative, FlowClaimJoinAlternativeSource, FlowClaimJoinExit,
    FlowClaimJoinExitKind, FlowClaimJoinReceipt, FlowClaimOutcomeSource, FlowOwnershipFacts,
    FlowPermissionEventFact,
};
use diagnostics::Diagnostic;
use facts::{PlaceRoot, PlaceSegment};
use language_semantics::{
    PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
    PermissionProvenance,
};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::statement::{StatementNode, TransitionExit, TransitionTargetNode};

mod selected_storage;
use selected_storage::expand_storage_frontier;

#[derive(Clone, Debug, PartialEq, Eq)]
struct ExitFrontier {
    exits: Vec<FlowClaimJoinExit>,
    entries: Vec<CheckedClaimOutcomeEntry>,
    inactive_paths: Vec<Vec<PlaceSegment>>,
    path: Vec<PlaceSegment>,
    captures: Vec<Capture>,
    excluded_input: Option<ExcludedInput>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum JoinedOccurrence {
    Call {
        state: SymbolHandle,
        statement: usize,
        ordinal: usize,
    },
    Storage {
        state: SymbolHandle,
        statement: usize,
        evaluation: checked_trees::FlowPermissionEvaluation,
        expression: ExpressionHandle,
        decision: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Alternative {
    exits: Vec<FlowClaimJoinExit>,
    captures: Vec<Capture>,
    source: Option<(CheckedClaimOutcomeSource, CheckedClaimOutcomeSource)>,
    excluded_input: Option<ExcludedInput>,
}

/// Retain the existing completed return graph as the guard authority. A
/// path query never creates a fresh choice per output field: all projections
/// refer to the same capture and its actual returned identity. Nested call
/// exits namespace these captures at their exact invocation.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Capture {
    operand: arena::Handle<checked_trees::FlowOperandFact>,
    path: Vec<PlaceSegment>,
    identity: PermissionClaimIdentity,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ExcludedInput {
    source: CheckedClaimOutcomeSource,
    operand: arena::Handle<checked_trees::FlowOperandFact>,
    path: Vec<PlaceSegment>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Receipt {
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: usize,
    ordinal: usize,
    target: SymbolHandle,
    expression: ExpressionHandle,
    root: PlaceRoot,
    path: Vec<PlaceSegment>,
    identity: PermissionClaimIdentity,
    alternatives: Vec<Alternative>,
}

/// Diagnostic observations stay outside every permission and receipt. Recording
/// them does not turn a failed candidate into a new checking failure or a join.
#[derive(Clone)]
struct ReturnFrontierFailure {
    exit: FlowClaimJoinExit,
    reason: &'static str,
    source: Option<CheckedClaimOutcomeSource>,
}

#[derive(Default)]
struct ReturnFrontierDiagnostics {
    enabled: bool,
    failures: Vec<ReturnFrontierFailure>,
}

impl ReturnFrontierDiagnostics {
    fn record(&mut self, exit: FlowClaimJoinExit, reason: &'static str) {
        if self.enabled {
            self.failures.push(ReturnFrontierFailure {
                exit,
                reason,
                source: None,
            });
        }
    }

    fn record_binding(
        &mut self,
        exit: FlowClaimJoinExit,
        reason: &'static str,
        source: &CheckedClaimOutcomeSource,
    ) {
        if self.enabled {
            self.failures.push(ReturnFrontierFailure {
                exit,
                reason,
                source: Some(source.clone()),
            });
        }
    }
}

struct PreparedCalleeFrontier {
    state: SymbolHandle,
    path: Vec<PlaceSegment>,
    frontiers: Option<Vec<ExitFrontier>>,
    diagnostics: ReturnFrontierDiagnostics,
}

pub(crate) struct CallClaimJoinFailure {
    receiver: ReceivingClaim,
    statement: usize,
    ordinal: usize,
    target: SymbolHandle,
    reason: &'static str,
    returns: Vec<ReturnFrontierFailure>,
}

impl CallClaimJoinFailure {
    /// An unresolved receiving event can report only its own attempted join.
    pub(crate) fn detail_for(
        &self,
        program: &dyn validation::CheckingInput,
        event: &FlowPermissionEventFact,
        expression: ExpressionHandle,
        segments: &arena::PagedArenaSet<PlaceSegment>,
    ) -> Option<String> {
        let receiver = &self.receiver;
        if receiver.machine != event.machine_symbol
            || receiver.state != event.state_symbol
            || receiver.source != event.source
            || receiver.expression != expression
            || receiver.root != event.root
            || receiver.identity != event.claim_identity
            || receiver.path.as_slice() != segments.span_or_empty(event.segments)
        {
            return None;
        }
        let mut detail = format!(
            "join candidate in machine `{}` state `{}` statement {} call {} to `{}`: {}",
            program.symbols.name(receiver.machine),
            program.symbols.name(receiver.state),
            self.statement,
            self.ordinal,
            program.symbols.name(self.target),
            self.reason,
        );
        for failure in &self.returns {
            detail.push_str(&format!(
                "; return state `{}` statement {} {:?}: {}",
                program.symbols.name(failure.exit.state_symbol),
                failure.exit.statement_index,
                failure.exit.kind,
                failure.reason,
            ));
            if let Some(source) = &failure.source {
                match source {
                    CheckedClaimOutcomeSource::Input {
                        parameter_symbol,
                        path,
                    } => {
                        let source = validation::CanonicalPlace {
                            root: PlaceRoot::Symbol(*parameter_symbol),
                            segments: path.clone(),
                        };
                        detail.push_str(&format!(" (callee input `{}`)", source.label(program)));
                    }
                    CheckedClaimOutcomeSource::Established {
                        claim_identity,
                        provenance,
                    } => {
                        detail.push_str(&format!(
                            " (callee establishment {claim_identity:?}/{provenance:?})",
                        ));
                    }
                }
            }
        }
        Some(detail)
    }
}

/// Preparation copies semantic paths, never handles into an arena that the
/// next source walk rebuilds. The receiving identity alone is insufficient:
/// its ordinal can change when an earlier establishment becomes inactive.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ReceivingClaim {
    machine: SymbolHandle,
    state: SymbolHandle,
    source: PermissionEventSource,
    expression: ExpressionHandle,
    root: PlaceRoot,
    path: Vec<PlaceSegment>,
    identity: PermissionClaimIdentity,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PreparedCallClaimJoin {
    receiver: ReceivingClaim,
    provenance: PermissionProvenance,
}

/// A previous iteration supplies a hypothesis for this exact completion, not
/// authority. Publication rederives every receipt from the current source walk;
/// changed or missing consulted bindings force another walk before its custody
/// observations and cleanup decisions may escape preparation.
#[derive(Default)]
pub(super) struct CallJoinPreparation {
    bindings: Vec<PreparedCallClaimJoin>,
    dependencies: Vec<ReceivingClaim>,
}

impl CallJoinPreparation {
    pub(super) fn begin(&mut self) {
        self.dependencies.clear();
    }

    pub(super) fn update(&mut self, bindings: Vec<PreparedCallClaimJoin>) -> bool {
        let changed = self.dependencies.iter().any(|receiver| {
            self.bindings
                .iter()
                .find(|binding| binding.receiver == *receiver)
                != bindings
                    .iter()
                    .find(|binding| binding.receiver == *receiver)
        });
        // Temporary failure imaging: report changed retained data without
        // re-running any semantic query or changing the preparation decision.
        if changed && std::env::var_os("OMEGA_LOCAL_PRODUCTION_TRACE").is_some() {
            for receiver in &self.dependencies {
                let before = self
                    .bindings
                    .iter()
                    .find(|binding| binding.receiver == *receiver);
                let after = bindings
                    .iter()
                    .find(|binding| binding.receiver == *receiver);
                if before != after {
                    eprintln!(
                        "CLAIM_PREPARATION_JOIN receiver={receiver:?} before={before:?} after={after:?}"
                    );
                }
            }
        }
        self.bindings = bindings;
        changed
    }

    pub(super) fn resolve(
        &mut self,
        segments: &arena::PagedArenaSet<PlaceSegment>,
        expression: ExpressionHandle,
        event: &mut FlowPermissionEventFact,
    ) {
        if event.kind != PermissionEventKind::Establish
            || event.access != PermissionAccess::Owned
            || !event.obligation_live
            || event.provenance
                != super::established_provenance(
                    event.machine_symbol,
                    event.state_symbol,
                    event.source,
                )
            || !matches!(event.claim_identity,
                PermissionClaimIdentity::Established { machine_symbol, state_symbol, source, .. }
                if machine_symbol == event.machine_symbol && state_symbol == event.state_symbol
                    && source == event.source)
        {
            return;
        }
        let receiver = ReceivingClaim {
            machine: event.machine_symbol,
            state: event.state_symbol,
            source: event.source,
            expression,
            root: event.root,
            path: segments.span_or_empty(event.segments).to_vec(),
            identity: event.claim_identity,
        };
        if let Some(binding) = self
            .bindings
            .iter()
            .find(|binding| binding.receiver == receiver)
        {
            event.provenance = binding.provenance;
        }
        // Failed lookups matter too: a new receipt must trigger preparation.
        if !self.dependencies.contains(&receiver) {
            self.dependencies.push(receiver);
        }
    }
}

fn known_source(source: &CheckedClaimOutcomeSource) -> bool {
    match source {
        CheckedClaimOutcomeSource::Input {
            parameter_symbol,
            path,
        } => {
            parameter_symbol.is_valid()
                && path.iter().all(|segment| {
                    matches!(segment,
                PlaceSegment::Field { symbol } if symbol.is_valid())
                        || matches!(segment, PlaceSegment::Case { variant } if variant.is_valid())
                        || matches!(segment, PlaceSegment::FixedIndex { .. })
                })
        }
        CheckedClaimOutcomeSource::Established {
            claim_identity,
            provenance,
        } => {
            *claim_identity != PermissionClaimIdentity::Unknown
                && *provenance != PermissionProvenance::Unknown
        }
    }
}

/// No exit may disappear through filter_map. Missing facts, unknown sources,
/// and recursive expansion all make the complete correspondence unavailable.
#[allow(clippy::too_many_arguments)]
fn state_frontiers(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    events: &[FlowPermissionEventFact],
    maps: &[CheckedClaimOutcomeMap],
    active: &mut Vec<SymbolHandle>,
    path: &[PlaceSegment],
    diagnostics: &mut ReturnFrontierDiagnostics,
) -> Option<Vec<ExitFrontier>> {
    if active.contains(&state.symbol) {
        return None;
    }
    active.push(state.symbol);
    let result = (|| {
        let ownership = &facts.flow.ownership;
        let flow = facts
            .flow
            .control
            .states
            .iter()
            .find_map(|(_, row)| (row.state_symbol == state.symbol).then_some(row))?;
        let mut frontiers = Vec::new();
        let mut conditions = crate::flow::SelectionConditions::new();
        for outgoing in facts.flow.control.outgoing.span_or_empty(flow.outgoing) {
            let (kind, target) = if outgoing.transition_target.is_valid() {
                let StatementNode::Transition(transition) = program
                    .statement_table
                    .statement_at(state, outgoing.statement_index)?
                else {
                    return None;
                };
                if transition.exit != TransitionExit::Ordinary {
                    continue;
                }
                let kind = if transition.target == outgoing.transition_target {
                    FlowClaimJoinExitKind::TransitionTarget
                } else if transition.continuation == outgoing.transition_target {
                    FlowClaimJoinExitKind::TransitionContinuation
                } else {
                    return None;
                };
                (
                    kind,
                    Some(
                        program
                            .statement_table
                            .transition_target(outgoing.transition_target),
                    ),
                )
            } else {
                (FlowClaimJoinExitKind::FinalExpression, None)
            };
            let edge = FlowClaimJoinExit {
                state_symbol: state.symbol,
                statement_index: outgoing.statement_index,
                kind,
            };
            if let Some(TransitionTargetNode::Named {
                path: target,
                arguments,
                ..
            }) = target
            {
                let target = validation::call_target_state(program, target.symbol)?;
                let children = state_frontiers(
                    program,
                    facts,
                    target,
                    events,
                    maps,
                    active,
                    path,
                    diagnostics,
                )?;
                for mut child in children {
                    for entry in &mut child.entries {
                        let Some(source) = bind_claim_outcome_source_at_arguments(
                            program,
                            state,
                            outgoing.statement_index,
                            program.statement_table.expression_handles(*arguments),
                            target,
                            &entry.source,
                            &ownership.segments,
                            events,
                        ) else {
                            diagnostics.record(
                                edge,
                                "named transition cannot bind its returned input source",
                            );
                            return None;
                        };
                        entry.source = source;
                    }
                    child.exits.insert(0, edge);
                    frontiers.push(child);
                }
                continue;
            }
            // This query checks the whole actual return frontier, including
            // every retained alternative and cross-path resource uniqueness,
            // before projecting the one result path this receipt requests.
            let claims = match crate::checks::multiplicity::current_returned_claims(
                program,
                facts,
                outgoing,
                &mut conditions,
            ) {
                Ok(claims) => claims,
                Err(reason) => {
                    diagnostics.record(edge, reason);
                    return None;
                }
            };
            let mut found = false;
            for claim in claims.iter().filter(|claim| claim.path == path) {
                found = true;
                let (entries, inactive_paths, identity) = match claim.source {
                    crate::checks::multiplicity::ReturnedClaimSource::Inactive => (
                        Vec::new(),
                        vec![path.to_vec()],
                        PermissionClaimIdentity::Unknown,
                    ),
                    crate::checks::multiplicity::ReturnedClaimSource::Live(identity) => {
                        let Some(source) =
                            returned_identity_source(state, identity, ownership, events)
                        else {
                            diagnostics.record(
                                edge,
                                "returned live claim has no unique entry or establishment source",
                            );
                            return None;
                        };
                        (
                            vec![CheckedClaimOutcomeEntry {
                                output_path: path.to_vec(),
                                source,
                            }],
                            Vec::new(),
                            identity,
                        )
                    }
                };
                let frontier = ExitFrontier {
                    exits: vec![edge],
                    entries,
                    inactive_paths,
                    path: path.to_vec(),
                    captures: vec![Capture {
                        operand: outgoing.returned_value,
                        path: path.to_vec(),
                        identity,
                    }],
                    excluded_input: None,
                };
                if !complete_frontier(
                    program,
                    state.return_type,
                    path,
                    &frontier.entries,
                    &frontier.inactive_paths,
                ) {
                    diagnostics.record(edge, "returned claim path has no complete frontier");
                    return None;
                }
                let failure_count = diagnostics.failures.len();
                let Some(expanded) = expand_joined_frontier(
                    program,
                    facts,
                    state,
                    events,
                    maps,
                    active,
                    &[],
                    frontier,
                    diagnostics,
                ) else {
                    if diagnostics.failures.len() == failure_count {
                        diagnostics.record(edge, "returned claim source expansion is unavailable");
                    }
                    return None;
                };
                frontiers.extend(expanded);
            }
            if !found {
                diagnostics.record(
                    edge,
                    "completed return omits the requested result claim path",
                );
                return None;
            }
        }
        (!frontiers.is_empty()).then_some(frontiers)
    })();
    active.pop();
    result
}

fn returned_identity_source(
    state: &typed_trees::state::State,
    identity: PermissionClaimIdentity,
    ownership: &FlowOwnershipFacts,
    events: &[FlowPermissionEventFact],
) -> Option<CheckedClaimOutcomeSource> {
    let definitions = events
        .iter()
        .filter(|event| {
            event.state_symbol == state.symbol
                // An exclusive call's checked restoration replaces the
                // returned storage's claims at that exact call occurrence.
                // These identities are actual producers too; the suspended
                // pre-call value is neither their source nor their identity.
                && matches!(
                    event.kind,
                    PermissionEventKind::Establish | PermissionEventKind::Restore
                )
                && event.access == PermissionAccess::Owned
                && event.obligation_live
                && event.claim_identity == identity
        })
        .collect::<Vec<_>>();
    let entries = definitions
        .iter()
        .filter(|event| {
            event.kind == PermissionEventKind::Establish
                && event.source == PermissionEventSource::StateEntry
        })
        .collect::<Vec<_>>();
    if let [entry] = entries.as_slice() {
        return Some(CheckedClaimOutcomeSource::Input {
            parameter_symbol: entry.root.as_symbol()?,
            path: ownership.segments.span(entry.segments)?.to_vec(),
        });
    }
    if !entries.is_empty() {
        return None;
    }
    let first = definitions.first()?;
    if definitions
        .iter()
        .any(|event| event.provenance != first.provenance)
    {
        return None;
    }
    let source = CheckedClaimOutcomeSource::Established {
        claim_identity: identity,
        provenance: first.provenance,
    };
    known_source(&source).then_some(source)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BoundSource {
    source: Option<CheckedClaimOutcomeSource>,
    captures: Vec<Capture>,
    excluded_input: Option<ExcludedInput>,
}

/// Bind one callee input path through the actual completed argument, retaining
/// its original selection graph. Every argument path is checked completely
/// before projection; missing alternatives cannot become a convenient source.
#[allow(clippy::too_many_arguments)]
fn bound_sources_at_call(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement: usize,
    ordinal: usize,
    expression: ExpressionHandle,
    call: &typed_trees::expression::TableCallExpression,
    target: &typed_trees::state::State,
    source: &CheckedClaimOutcomeSource,
    events: &[FlowPermissionEventFact],
) -> Result<Vec<BoundSource>, &'static str> {
    let CheckedClaimOutcomeSource::Input {
        parameter_symbol,
        path,
    } = source
    else {
        return Ok(vec![BoundSource {
            source: Some(source.clone()),
            captures: Vec::new(),
            excluded_input: None,
        }]);
    };
    let flow = facts
        .flow
        .control
        .state(machine, state.symbol)
        .ok_or("caller argument binding has no owning state flow")?;
    let mut matching = facts
        .flow
        .control
        .calls
        .span_or_empty(flow.calls)
        .iter()
        .filter(|row| {
            row.statement_index == statement
                && row.call_ordinal == ordinal
                && row.target_symbol == call.target_symbol
                && row.authored_expression == expression
        });
    let captured = matching
        .next()
        .ok_or("caller argument binding has no exact captured invocation")?;
    if matching.next().is_some() {
        return Err("caller argument binding has multiple captured invocations");
    }
    let parameters = program.state_parameters(target);
    let arguments = program.expression_table.expression_handles(call.arguments);
    let operands = facts
        .flow
        .control
        .operands
        .span(captured.argument_operands())
        .ok_or("caller argument binding has an invalid completed operand span")?;
    if parameters.len() != arguments.len()
        || !operands
            .iter()
            .map(|operand| operand.expression)
            .eq(arguments.iter().copied())
    {
        return Err("caller argument captures disagree with the exact callee parameter roster");
    }
    let position = typed_trees::signature::state_parameter_position(parameters, *parameter_symbol)
        .ok_or("returned input source is not a parameter of the captured call target")?;
    let operand = captured.argument_operands().handle_at(
        u32::try_from(position)
            .map_err(|_| "caller argument position exceeds arena coordinates")?,
    );
    let machine_owner = program
        .machine_by_symbol(machine)
        .ok_or("caller argument binding has no owning machine")?;
    let evaluation =
        crate::flow::call_permission_evaluation(program, machine_owner, state, statement, ordinal);
    if evaluation == checked_trees::FlowPermissionEvaluation::Unknown {
        return Err("caller argument binding has no exact permission evaluation");
    }
    let consumer = crate::checks::multiplicity::ClaimConsumption {
        machine,
        state: state.symbol,
        statement_index: statement,
        evaluation,
        source: PermissionEventSource::Call {
            statement_index: statement,
            call_ordinal: ordinal,
            target_symbol: call.target_symbol,
        },
    };
    let mut conditions = crate::flow::SelectionConditions::new();
    let claims = crate::checks::multiplicity::consumed_operand_claims(
        program,
        facts,
        consumer,
        operand,
        &mut conditions,
    )?;
    let mut sources = Vec::new();
    for claim in claims.iter().filter(|claim| claim.path == *path) {
        let (source, identity) = match claim.source {
            crate::checks::multiplicity::ReturnedClaimSource::Live(identity) => (
                Some(
                    returned_identity_source(state, identity, &facts.flow.ownership, events)
                        .ok_or(
                            "consumed caller argument has no unique entry or establishment source",
                        )?,
                ),
                identity,
            ),
            crate::checks::multiplicity::ReturnedClaimSource::Inactive => {
                (None, PermissionClaimIdentity::Unknown)
            }
        };
        let excluded_input = source.is_none().then(|| ExcludedInput {
            source: CheckedClaimOutcomeSource::Input {
                parameter_symbol: *parameter_symbol,
                path: path.clone(),
            },
            operand,
            path: path.clone(),
        });
        let bound = BoundSource {
            source,
            excluded_input,
            captures: vec![Capture {
                operand,
                path: path.clone(),
                identity,
            }],
        };
        if !sources.contains(&bound) {
            sources.push(bound);
        }
    }
    if sources.is_empty() {
        Err("completed caller argument omits the exact returned input-source path")
    } else {
        Ok(sources)
    }
}

/// A wrapper cannot reuse its callee's local joined occurrence as a global
/// resource id. Reconstruct the producing call and substitute its complete
/// alternatives together, preserving correlations between returned fields.
#[allow(clippy::too_many_arguments)]
fn expand_joined_frontier(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    events: &[FlowPermissionEventFact],
    maps: &[CheckedClaimOutcomeMap],
    active: &mut Vec<SymbolHandle>,
    visited: &[JoinedOccurrence],
    frontier: ExitFrontier,
    diagnostics: &mut ReturnFrontierDiagnostics,
) -> Option<Vec<ExitFrontier>> {
    let ownership = &facts.flow.ownership;
    if let Some(binding) = frontier.entries.iter().find_map(|entry| {
        let CheckedClaimOutcomeSource::Established {
            claim_identity,
            provenance,
        } = entry.source
        else {
            return None;
        };
        ownership
            .selection_place_bindings
            .iter()
            .find_map(|(_, binding)| {
                (binding.selected != binding.remaining
                    && binding.destination.claim_identity == claim_identity
                    && binding.destination.provenance == provenance)
                    .then_some(binding)
            })
    }) {
        return expand_storage_frontier(
            program,
            facts,
            state,
            events,
            maps,
            active,
            visited,
            frontier.clone(),
            binding,
            diagnostics,
        );
    }
    if frontier.entries.iter().any(|entry| {
        matches!(
            entry.source,
            CheckedClaimOutcomeSource::Established {
                provenance: PermissionProvenance::Joined {
                    source: PermissionEventSource::Statement { .. },
                    ..
                },
                ..
            }
        )
    }) {
        return None;
    }
    let joined = frontier
        .entries
        .iter()
        .find_map(|entry| match entry.source {
            CheckedClaimOutcomeSource::Established {
                provenance:
                    PermissionProvenance::Joined {
                        machine_symbol,
                        state_symbol,
                        source:
                            PermissionEventSource::Call {
                                statement_index,
                                call_ordinal,
                                target_symbol,
                            },
                        ..
                    },
                ..
            } => Some((
                machine_symbol,
                state_symbol,
                statement_index,
                call_ordinal,
                target_symbol,
            )),
            _ => None,
        });
    let Some((machine, owner, statement, ordinal, target_symbol)) = joined else {
        return Some(vec![frontier]);
    };
    if owner != state.symbol {
        return None;
    }
    let invocation = JoinedOccurrence::Call {
        state: owner,
        statement,
        ordinal,
    };
    if visited.contains(&invocation) {
        return None;
    }
    let mut visited = visited.to_vec();
    visited.push(invocation);
    let validation::CallSite::Expression {
        call,
        expression: call_expression,
        ..
    } = validation::find_call_site(program, machine, owner, statement, ordinal)?
    else {
        return None;
    };
    if call.target_symbol != target_symbol {
        return None;
    }
    let target = validation::call_target_state(program, target_symbol)?;
    let joined_identity = frontier
        .entries
        .iter()
        .find_map(|entry| match entry.source {
            CheckedClaimOutcomeSource::Established {
                claim_identity,
                provenance: PermissionProvenance::Joined { .. },
            } => Some(claim_identity),
            _ => None,
        })?;
    let origins = events
        .iter()
        .filter(|event| {
            event.kind == PermissionEventKind::Establish
                && event.state_symbol == owner
                && event.claim_identity == joined_identity
        })
        .filter_map(|event| {
            let (at, expression, path) =
                super::receiving_value(program, &ownership.segments, event)?;
            (at == statement && expression == call_expression).then_some(path)
        })
        .fold(Vec::new(), |mut origins, path| {
            // A completed child can establish the same claim at successive
            // receiving homes. Those rows still name one call occurrence and
            // result path; distinct result paths remain ambiguous.
            if !origins.contains(&path) {
                origins.push(path);
            }
            origins
        });
    let [requested] = origins.as_slice() else {
        return None;
    };
    let children = state_frontiers(
        program,
        facts,
        target,
        events,
        maps,
        active,
        requested,
        diagnostics,
    )?;
    let mut expanded = Vec::new();
    for child in children {
        let mut next = frontier.clone();
        next.exits.push(FlowClaimJoinExit {
            state_symbol: owner,
            statement_index: statement,
            kind: FlowClaimJoinExitKind::CallResult {
                call_ordinal: ordinal,
            },
        });
        next.exits.extend(child.exits);
        next.captures.extend(child.captures);
        if next.excluded_input.is_none() {
            next.excluded_input = child.excluded_input;
        }
        // This operation queries one exact result claim path. Its receiving
        // identity stays distinct from the source lineage being substituted.
        let position = next.entries.iter().position(|entry| matches!(entry.source,
            CheckedClaimOutcomeSource::Established { claim_identity, .. } if claim_identity == joined_identity))?;
        let output_path = next.entries[position].output_path.clone();
        if child
            .inactive_paths
            .iter()
            .any(|inactive| inactive == requested)
        {
            next.entries.remove(position);
            next.inactive_paths.push(output_path);
            expanded.push(next);
            continue;
        }
        let mut matching = child
            .entries
            .iter()
            .filter(|entry| entry.output_path == *requested);
        let source = &matching.next()?.source;
        if matching.next().is_some() {
            return None;
        }
        let bound_sources = match bound_sources_at_call(
            program,
            facts,
            machine,
            state,
            statement,
            ordinal,
            call_expression,
            call,
            target,
            source,
            events,
        ) {
            Ok(sources) => sources,
            Err(reason) => {
                diagnostics.record_binding(
                    FlowClaimJoinExit {
                        state_symbol: owner,
                        statement_index: statement,
                        kind: FlowClaimJoinExitKind::CallResult {
                            call_ordinal: ordinal,
                        },
                    },
                    reason,
                    source,
                );
                return None;
            }
        };
        for bound in bound_sources {
            let mut next = next.clone();
            next.captures.extend(bound.captures);
            if next.excluded_input.is_none() {
                next.excluded_input = bound.excluded_input;
            }
            if let Some(mut bound) = bound.source {
                // A callee-local origin is an explanation, not a replacement
                // for this exact caller result occurrence's resource name.
                if matches!(source, CheckedClaimOutcomeSource::Established { .. })
                    && let CheckedClaimOutcomeSource::Established { claim_identity, .. } =
                        &mut bound
                {
                    *claim_identity = joined_identity;
                }
                if matches!(bound, CheckedClaimOutcomeSource::Established { provenance:
                    PermissionProvenance::Joined { machine_symbol: rebound_machine, state_symbol: rebound_state,
                        source: PermissionEventSource::Call { statement_index: rebound_statement, call_ordinal: rebound_ordinal, .. }, .. }, .. }
                    if (rebound_machine, rebound_state, rebound_statement, rebound_ordinal) == (machine, owner, statement, ordinal))
                {
                    return None;
                }
                next.entries[position].source = bound;
            } else {
                next.entries.remove(position);
                next.inactive_paths.push(output_path.clone());
            }
            if !complete_frontier(
                program,
                state.return_type,
                &next.path,
                &next.entries,
                &next.inactive_paths,
            ) {
                return None;
            }
            expanded.extend(expand_joined_frontier(
                program,
                facts,
                state,
                events,
                maps,
                active,
                &visited,
                next,
                diagnostics,
            )?);
        }
    }
    Some(expanded)
}

pub(super) fn complete_whole_frontier(
    program: &dyn ::validation::CheckingInput,
    result_type: typed_trees::types::TypeReferenceHandle,
    entries: &[CheckedClaimOutcomeEntry],
    inactive_paths: &[Vec<PlaceSegment>],
) -> bool {
    validate_frontier(program, result_type, None, entries, inactive_paths)
}

pub(super) fn complete_frontier(
    program: &dyn ::validation::CheckingInput,
    result_type: typed_trees::types::TypeReferenceHandle,
    requested: &[PlaceSegment],
    entries: &[CheckedClaimOutcomeEntry],
    inactive_paths: &[Vec<PlaceSegment>],
) -> bool {
    validate_frontier(
        program,
        result_type,
        Some(requested),
        entries,
        inactive_paths,
    )
}

fn validate_frontier(
    program: &dyn ::validation::CheckingInput,
    result_type: typed_trees::types::TypeReferenceHandle,
    requested: Option<&[PlaceSegment]>,
    entries: &[CheckedClaimOutcomeEntry],
    inactive_paths: &[Vec<PlaceSegment>],
) -> bool {
    let expected = linear_claim_frontier(program, result_type)
        .into_iter()
        .filter(|claim| requested.is_none_or(|requested| claim.path == requested))
        .collect::<Vec<_>>();
    if expected.is_empty()
        || entries.iter().any(|entry| {
            !known_source(&entry.source)
                || !expected.iter().any(|claim| claim.path == entry.output_path)
                || inactive_paths.contains(&entry.output_path)
        })
        || inactive_paths
            .iter()
            .any(|path| !expected.iter().any(|claim| claim.path == *path))
    {
        return false;
    }
    for claim in expected {
        let matching = entries
            .iter()
            .filter(|entry| entry.output_path == claim.path)
            .count();
        if matching != 1 && !(matching == 0 && inactive_paths.contains(&claim.path)) {
            return false;
        }
    }
    !entries.iter().enumerate().any(|(index, entry)| {
        entries[index + 1..].iter().any(|other| {
            entry.source == other.source
                && !claim_paths_are_case_alternatives(&entry.output_path, &other.output_path)
        })
    })
}

/// An absent conditional payload remains absent only while the exact storage
/// path is preserved through the entire consuming statement. Unknown identity
/// alone, and an inactive unguarded allocation, establish no such fact.
#[allow(clippy::too_many_arguments)]
pub(in crate::checks::multiplicity) fn preserved_inactive_place<'a>(
    program: &dyn ::validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    state: &typed_trees::state::State,
    statement_index: usize,
    place: &validation::CanonicalPlace,
    ownership: &FlowOwnershipFacts,
    events: impl IntoIterator<Item = &'a FlowPermissionEventFact>,
) -> Option<usize> {
    InactivePlaceWrites::prepare(program, borrow, state)?.preserved(
        program,
        state,
        statement_index,
        place,
        ownership,
        events,
    )
}

/// Statement effects belong to the immutable source/borrow inputs, not the
/// individual conditional claim. Reconstruct them once for a frontier batch.
/// Unknown effects stay unknown at their original statement; a query still
/// checks exactly the interval after its own establishing occurrence.
pub(super) struct InactivePlaceWrites {
    state: SymbolHandle,
    statements: Vec<Option<Vec<validation::CanonicalPlace>>>,
}

impl InactivePlaceWrites {
    pub(super) fn prepare(
        program: &dyn ::validation::CheckingInput,
        borrow: &checked_trees::BorrowFacts,
        state: &typed_trees::state::State,
    ) -> Option<Self> {
        let (machine, _) = validation::find_state(program, state.symbol)?;
        let frames = validation::CallFrameResolver::new(program)?;
        let borrowed_state = borrow
            .states
            .iter()
            .map(|(_, state)| state)
            .find(|candidate| {
                candidate.machine_symbol == machine.symbol && candidate.state_symbol == state.symbol
            })?;
        let statements = program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .enumerate()
            .map(|(index, statement)| {
                let mut writes = crate::flow::statement_storage_writes(
                    program,
                    machine.symbol,
                    state.symbol,
                    index,
                    statement,
                    Some(&frames),
                )?;
                writes.extend(crate::flow::frame_storage_writes(
                    program,
                    machine.symbol,
                    state.symbol,
                    index,
                    &frames.statement_value_write_frame(machine, statement),
                    Some(&frames),
                )?);
                if let Some(call) = statement.as_call() {
                    writes.extend(crate::flow::frame_storage_writes(
                        program,
                        machine.symbol,
                        state.symbol,
                        index,
                        &frames.may_write_frame(machine, call),
                        Some(&frames),
                    )?);
                }
                // A selected operator's body frame can be incomplete. Keep
                // every independently checked signature ceiling as before.
                for call in borrow
                    .calls
                    .span_or_empty(borrowed_state.calls)
                    .iter()
                    .filter(|call| call.statement_index == index)
                {
                    writes.extend(crate::flow::signature_ceiling_places(
                        program,
                        machine.symbol,
                        state.symbol,
                        None,
                        call,
                        Some(&frames),
                    )?);
                }
                Some(writes)
            })
            .collect();
        Some(Self {
            state: state.symbol,
            statements,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn preserved<'a>(
        &self,
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        statement_index: usize,
        place: &validation::CanonicalPlace,
        ownership: &FlowOwnershipFacts,
        events: impl IntoIterator<Item = &'a FlowPermissionEventFact>,
    ) -> Option<usize> {
        if self.state != state.symbol {
            return None;
        }
        if !place
            .segments
            .iter()
            .any(|segment| matches!(segment, PlaceSegment::Case { .. }))
        {
            return None;
        }
        let establishment = events.into_iter().filter(|event| {
        event.state_symbol == state.symbol && event.kind == PermissionEventKind::Establish
            && event.access == PermissionAccess::Owned && !event.obligation_live
            && event.claim_identity == PermissionClaimIdentity::Unknown
            && event.root == place.root
            && ownership.segments.span_or_empty(event.segments) == place.segments
            && matches!(event.source, PermissionEventSource::Statement { statement_index: index }
                if index < statement_index)
    }).max_by_key(|event| match event.source {
        PermissionEventSource::Statement { statement_index } => statement_index,
        _ => 0,
    })?;
        let PermissionEventSource::Statement {
            statement_index: established_at,
        } = establishment.source
        else {
            return None;
        };
        for index in established_at + 1..=statement_index {
            let writes = self.statements.get(index)?.as_ref()?;
            if writes.iter().any(|write| {
                (write.root) == (place.root)
                    && crate::flow::canonical_place_segments_may_overlap(
                        program,
                        &write.segments,
                        &place.segments,
                    )
            }) {
                return None;
            }
        }
        Some(established_at)
    }
}

fn derive_receipts(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    events: &[FlowPermissionEventFact],
    mut failures: Option<&mut Vec<CallClaimJoinFailure>>,
) -> Result<Vec<Receipt>, &'static str> {
    let borrow = &facts.borrow;
    let ownership = &facts.flow.ownership;
    let maps = derive_checked_claim_outcome_maps(program, borrow, ownership, events)?;
    let mut receipts = Vec::new();
    let trace_derivation = std::env::var_os("OMEGA_LOCAL_PRODUCTION_TRACE").is_some();
    // This cache belongs to one immutable facts/events/maps revision, including
    // its exact completed operand handles. The private frontier projects one
    // received path, so a target alone cannot key it. Actual argument binding
    // and the receiving occurrence remain outside this prepared callee graph.
    let mut prepared_frontiers: Vec<PreparedCalleeFrontier> = Vec::new();
    for event in events {
        if event.kind != PermissionEventKind::Establish
            || event.access != PermissionAccess::Owned
            || !event.obligation_live
        {
            continue;
        }
        let Some((statement_index, expression, received_path)) =
            super::receiving_value(program, &ownership.segments, event)
        else {
            continue;
        };
        let Some(state) = program.state_by_symbol(event.state_symbol) else {
            continue;
        };
        let Some(call) = program.expression_table.call_node(expression) else {
            continue;
        };
        let Some(target) = validation::call_target_state(program, call.target_symbol) else {
            continue;
        };
        let mut ordinal = 0;
        let exact_call = loop {
            let Some(site) = validation::find_call_site(
                program,
                event.machine_symbol,
                event.state_symbol,
                statement_index,
                ordinal,
            ) else {
                break false;
            };
            if matches!(site, validation::CallSite::Expression { expression: found, .. } if found == expression)
            {
                break true;
            }
            ordinal += 1;
        };
        if !exact_call {
            continue;
        }
        if trace_derivation {
            eprintln!(
                "CLAIM_JOIN_DERIVATION_INPUT event={event:?} expression={expression:?} received_path={received_path:?} call_ordinal={ordinal} target={:?}",
                call.target_symbol
            );
        }
        let failure = |reason, returns| {
            let failure = CallClaimJoinFailure {
                receiver: ReceivingClaim {
                    machine: event.machine_symbol,
                    state: event.state_symbol,
                    source: event.source,
                    expression,
                    root: event.root,
                    path: ownership.segments.span_or_empty(event.segments).to_vec(),
                    identity: event.claim_identity,
                },
                statement: statement_index,
                ordinal,
                target: call.target_symbol,
                reason,
                returns,
            };
            if trace_derivation {
                eprintln!(
                    "CLAIM_JOIN_DERIVATION_FAILURE receiver={:?} target={:?} reason={}",
                    failure.receiver, failure.target, failure.reason
                );
                for returned in &failure.returns {
                    eprintln!(
                        "CLAIM_JOIN_DERIVATION_RETURN exit={:?} reason={} source={:?}",
                        returned.exit, returned.reason, returned.source
                    );
                }
            }
            failure
        };
        let frontier_index = match prepared_frontiers.iter().position(|prepared| {
            prepared.state == target.symbol && prepared.path.as_slice() == received_path
        }) {
            Some(index) => index,
            None => {
                let index = prepared_frontiers.len();
                let mut diagnostics = ReturnFrontierDiagnostics {
                    enabled: failures.is_some(),
                    ..Default::default()
                };
                let frontiers = state_frontiers(
                    program,
                    facts,
                    target,
                    events,
                    &maps,
                    &mut Vec::new(),
                    received_path,
                    &mut diagnostics,
                );
                prepared_frontiers.push(PreparedCalleeFrontier {
                    state: target.symbol,
                    path: received_path.to_vec(),
                    frontiers,
                    diagnostics,
                });
                index
            }
        };
        let prepared = &prepared_frontiers[frontier_index];
        let Some(frontiers) = prepared.frontiers.as_deref() else {
            if let Some(failures) = failures.as_deref_mut() {
                failures.push(failure(
                    "callee return frontier correspondence is unavailable",
                    prepared.diagnostics.failures.clone(),
                ));
            }
            continue;
        };
        let path = ownership.segments.span_or_empty(event.segments);
        let mut alternatives = Vec::new();
        let mut complete = true;
        for frontier in frontiers {
            let mut entries = frontier
                .entries
                .iter()
                .filter(|entry| entry.output_path == received_path);
            let Some(entry) = entries.next() else {
                if frontier
                    .inactive_paths
                    .iter()
                    .any(|inactive| inactive == received_path)
                {
                    alternatives.push(Alternative {
                        captures: frontier.captures.clone(),
                        exits: frontier.exits.clone(),
                        source: None,
                        excluded_input: frontier.excluded_input.clone(),
                    });
                    continue;
                }
                complete = false;
                break;
            };
            if entries.next().is_some() {
                complete = false;
                break;
            }
            let bound_sources = match bound_sources_at_call(
                program,
                facts,
                event.machine_symbol,
                state,
                statement_index,
                ordinal,
                expression,
                call,
                target,
                &entry.source,
                events,
            ) {
                Ok(sources) => sources,
                Err(reason) => {
                    if let Some(failures) = failures.as_deref_mut() {
                        failures.push(failure(
                            "completed caller argument cannot bind this callee return source",
                            frontier
                                .exits
                                .iter()
                                .map(|exit| ReturnFrontierFailure {
                                    exit: *exit,
                                    reason,
                                    source: Some(entry.source.clone()),
                                })
                                .collect(),
                        ));
                    }
                    complete = false;
                    break;
                }
            };
            if trace_derivation {
                eprintln!(
                    "CLAIM_JOIN_DERIVATION_BOUND receiver_identity={:?} callee_source={:?} bound_sources={bound_sources:?}",
                    event.claim_identity, entry.source
                );
            }
            for bound in bound_sources {
                let mut captures = frontier.captures.clone();
                captures.extend(bound.captures);
                alternatives.push(Alternative {
                    captures,
                    exits: frontier.exits.clone(),
                    source: bound.source.map(|bound| (entry.source.clone(), bound)),
                    excluded_input: bound
                        .excluded_input
                        .or_else(|| frontier.excluded_input.clone()),
                });
            }
        }
        if !complete || alternatives.is_empty() {
            if trace_derivation {
                eprintln!(
                    "CLAIM_JOIN_DERIVATION_DROP gate=incomplete-alternatives identity={:?} complete={complete} alternatives={alternatives:?}",
                    event.claim_identity
                );
            }
            continue;
        }
        // An available invariant whole-result map has already forwarded its
        // input identities before this query. A path can nevertheless have an
        // invariant input while a sibling has differing returning sources, so
        // no whole-result map exists. Retain that path's exact receipt too:
        // the local receiving name continues its authenticated input rather
        // than acquiring authority from an unrelated sibling's summary.
        // Only the result occurrence allocated at this call can name its join.
        // Rewritten identities from another call cannot borrow this receipt.
        let PermissionClaimIdentity::Established {
            machine_symbol,
            state_symbol,
            source,
            ..
        } = event.claim_identity
        else {
            if trace_derivation {
                eprintln!(
                    "CLAIM_JOIN_DERIVATION_DROP gate=nonlocal-identity identity={:?} provenance={:?}",
                    event.claim_identity, event.provenance
                );
            }
            continue;
        };
        if machine_symbol != event.machine_symbol
            || state_symbol != state.symbol
            || source != event.source
        {
            if trace_derivation {
                eprintln!(
                    "CLAIM_JOIN_DERIVATION_DROP gate=identity-owner identity={:?} event_machine={:?} event_state={:?} event_source={:?}",
                    event.claim_identity, event.machine_symbol, event.state_symbol, event.source
                );
            }
            continue;
        }
        if trace_derivation {
            eprintln!(
                "CLAIM_JOIN_DERIVATION_RETAIN identity={:?} provenance={:?} alternatives={alternatives:?}",
                event.claim_identity, event.provenance
            );
        }
        receipts.push(Receipt {
            machine: event.machine_symbol,
            state: state.symbol,
            statement: statement_index,
            ordinal,
            target: call.target_symbol,
            expression,
            root: event.root,
            path: path.to_vec(),
            identity: event.claim_identity,
            alternatives,
        });
    }
    Ok(receipts)
}

fn join_provenance(receipt: &Receipt) -> Option<PermissionProvenance> {
    let PermissionClaimIdentity::Established { ordinal, .. } = receipt.identity else {
        return None;
    };
    Some(PermissionProvenance::Joined {
        machine_symbol: receipt.machine,
        state_symbol: receipt.state,
        source: PermissionEventSource::Call {
            statement_index: receipt.statement,
            call_ordinal: receipt.ordinal,
            target_symbol: receipt.target,
        },
        ordinal,
    })
}

fn store_source(
    ownership: &mut FlowOwnershipFacts,
    source: CheckedClaimOutcomeSource,
) -> FlowClaimOutcomeSource {
    match source {
        CheckedClaimOutcomeSource::Input {
            parameter_symbol,
            path,
        } => FlowClaimOutcomeSource::Input {
            parameter_symbol,
            segments: ownership.segments.insert_many(path),
        },
        CheckedClaimOutcomeSource::Established {
            claim_identity,
            provenance,
        } => FlowClaimOutcomeSource::Established {
            claim_identity,
            provenance,
        },
    }
}

pub(crate) fn publish_call_claim_joins(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
) -> Result<Vec<PreparedCallClaimJoin>, &'static str> {
    let mut events = facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event.clone())
        .collect::<Vec<_>>();
    // Earlier call joins can feed later calls, but no receipt cites itself.
    let mut converged = false;
    let trace_derivation = std::env::var_os("OMEGA_LOCAL_PRODUCTION_TRACE").is_some();
    for _ in 0..=events.len() {
        // Failure collection is diagnostic payload only; the same derivation
        // and original rewrite/convergence decisions remain authoritative.
        let mut failures = Vec::new();
        let receipts = derive_receipts(
            program,
            facts,
            &events,
            if trace_derivation {
                Some(&mut failures)
            } else {
                None
            },
        )?;
        let mut rewrites = Vec::new();
        for receipt in &receipts {
            let Some(provenance) = join_provenance(receipt) else {
                continue;
            };
            for event in &events {
                if event.claim_identity == receipt.identity && event.provenance != provenance {
                    let rewrite = (
                        event.provenance,
                        event.claim_identity,
                        provenance,
                        receipt.identity,
                    );
                    if !rewrites.contains(&rewrite) {
                        rewrites.push(rewrite);
                    }
                }
            }
        }
        if !super::apply_claim_origin_rewrites(&mut events, &mut facts.flow.ownership, &rewrites) {
            converged = true;
            break;
        }
    }
    if !converged {
        return Err("call-result claim joins did not converge");
    }
    let mut failures = Vec::new();
    let receipts = derive_receipts(
        program,
        facts,
        &events,
        if trace_derivation {
            Some(&mut failures)
        } else {
            None
        },
    )?;
    let ownership = &mut facts.flow.ownership;
    ownership.permissions = Default::default();
    ownership.permissions.insert_many(events);
    ownership.claim_join_receipts = Default::default();
    ownership.claim_join_alternatives = Default::default();
    ownership.claim_join_exits = Default::default();
    let mut prepared = Vec::new();
    ownership.claim_join_captures = Default::default();
    for receipt in receipts {
        let PermissionClaimIdentity::Established { source, .. } = receipt.identity else {
            return Err("call-result join lacks an exact receiving identity");
        };
        let provenance = join_provenance(&receipt)
            .ok_or("call-result join lacks exact invocation provenance")?;
        prepared.push(PreparedCallClaimJoin {
            receiver: ReceivingClaim {
                machine: receipt.machine,
                state: receipt.state,
                source,
                expression: receipt.expression,
                root: receipt.root,
                path: receipt.path.clone(),
                identity: receipt.identity,
            },
            provenance,
        });
        let mut alternatives = Vec::new();
        for alternative in receipt.alternatives {
            let exits = ownership.claim_join_exits.insert_many(alternative.exits);
            let source = match (alternative.source, alternative.excluded_input) {
                (None, None) => FlowClaimJoinAlternativeSource::Inactive,
                (_, Some(excluded)) => FlowClaimJoinAlternativeSource::ExcludedInput {
                    source: store_source(ownership, excluded.source),
                    operand: excluded.operand,
                    segments: ownership.segments.insert_many(excluded.path),
                },
                (Some((source, bound_source)), None) => FlowClaimJoinAlternativeSource::Claim {
                    source: store_source(ownership, source),
                    bound_source: store_source(ownership, bound_source),
                },
            };
            let captures = alternative
                .captures
                .into_iter()
                .map(|capture| checked_trees::FlowClaimJoinCapture {
                    operand: capture.operand,
                    path: ownership.segments.insert_many(capture.path),
                    identity: capture.identity,
                })
                .collect::<Vec<_>>();
            let captures = ownership.claim_join_captures.insert_many(captures);
            alternatives.push(FlowClaimJoinAlternative {
                exits,
                source,
                captures,
            });
        }
        let alternatives = ownership.claim_join_alternatives.insert_many(alternatives);
        let result_segments = ownership.segments.insert_many(receipt.path);
        ownership.claim_join_receipts.insert(FlowClaimJoinReceipt {
            machine_symbol: receipt.machine,
            state_symbol: receipt.state,
            statement_index: receipt.statement,
            call_ordinal: receipt.ordinal,
            target_symbol: receipt.target,
            expression: receipt.expression,
            result_root: receipt.root,
            result_segments,
            claim_identity: receipt.identity,
            alternatives,
        });
    }
    Ok(prepared)
}

fn read_source(
    ownership: &FlowOwnershipFacts,
    source: FlowClaimOutcomeSource,
) -> Option<CheckedClaimOutcomeSource> {
    Some(match source {
        FlowClaimOutcomeSource::Unknown => return None,
        FlowClaimOutcomeSource::Input {
            parameter_symbol,
            segments,
        } => CheckedClaimOutcomeSource::Input {
            parameter_symbol,
            path: ownership.segments.span(segments)?.to_vec(),
        },
        FlowClaimOutcomeSource::Established {
            claim_identity,
            provenance,
        } => CheckedClaimOutcomeSource::Established {
            claim_identity,
            provenance,
        },
    })
}

pub(crate) fn validate_call_claim_joins(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<CallClaimJoinFailure> {
    let ownership = &facts.flow.ownership;
    let events = ownership
        .permissions
        .iter()
        .map(|(_, event)| event.clone())
        .collect::<Vec<_>>();
    let mut failures = Vec::new();
    let expected = match derive_receipts(program, facts, &events, Some(&mut failures)) {
        Ok(receipts) => receipts,
        Err(message) => {
            diagnostics.push(Diagnostic::error(message));
            return failures;
        }
    };
    let recorded = ownership
        .claim_join_receipts
        .iter()
        .map(|(_, receipt)| {
            let alternatives = ownership
                .claim_join_alternatives
                .span(receipt.alternatives)?
                .iter()
                .map(|alternative| {
                    Some(Alternative {
                        captures: ownership
                            .claim_join_captures
                            .span(alternative.captures)?
                            .iter()
                            .map(|capture| {
                                Some(Capture {
                                    operand: capture.operand,
                                    path: ownership.segments.span(capture.path)?.to_vec(),
                                    identity: capture.identity,
                                })
                            })
                            .collect::<Option<Vec<_>>>()?,
                        exits: ownership.claim_join_exits.span(alternative.exits)?.to_vec(),
                        source: match alternative.source {
                            FlowClaimJoinAlternativeSource::Inactive
                            | FlowClaimJoinAlternativeSource::ExcludedInput { .. } => None,
                            FlowClaimJoinAlternativeSource::Claim {
                                source,
                                bound_source,
                            } => Some((
                                read_source(ownership, source)?,
                                read_source(ownership, bound_source)?,
                            )),
                        },
                        excluded_input: match alternative.source {
                            FlowClaimJoinAlternativeSource::ExcludedInput {
                                source,
                                operand,
                                segments,
                            } => Some(ExcludedInput {
                                source: read_source(ownership, source)?,
                                operand,
                                path: ownership.segments.span(segments)?.to_vec(),
                            }),
                            _ => None,
                        },
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            Some(Receipt {
                machine: receipt.machine_symbol,
                state: receipt.state_symbol,
                statement: receipt.statement_index,
                ordinal: receipt.call_ordinal,
                target: receipt.target_symbol,
                expression: receipt.expression,
                root: receipt.result_root,
                path: ownership.segments.span(receipt.result_segments)?.to_vec(),
                identity: receipt.claim_identity,
                alternatives,
            })
        })
        .collect::<Option<Vec<_>>>();
    if recorded.as_ref() != Some(&expected) {
        let detail = if let Some(recorded) = &recorded {
            (0..recorded.len().max(expected.len()))
                .find(|&position| recorded.get(position) != expected.get(position))
                .and_then(|position| expected.get(position).or_else(|| recorded.get(position)))
                .map(|receipt| format!(
                    "in machine `{}` state `{}` statement {} call {} to `{}`: result receipt differs",
                    program.symbols.name(receipt.machine),
                    program.symbols.name(receipt.state),
                    receipt.statement,
                    receipt.ordinal,
                    program.symbols.name(receipt.target),
                ))
                .unwrap_or_else(|| "receipt roster differs".to_owned())
        } else {
            "stored receipt contains an invalid source or span".to_owned()
        };
        diagnostics.push(Diagnostic::error(format!(
            "call claim joins differ from exact source exit and call replay: {detail}"
        )));
    }
    let invalid_event = events.iter().find(|event| {
        let valid = if matches!(
            event.provenance,
            PermissionProvenance::Joined {
                source: PermissionEventSource::Statement { .. },
                ..
            }
        ) {
            super::super::selected_evaluation::place_bindings::claim_origin(
                ownership,
                event.claim_identity,
                event.provenance,
            )
        } else if matches!(event.provenance, PermissionProvenance::Joined { .. }) {
            expected.iter().any(|receipt| {
                receipt.identity == event.claim_identity
                    && join_provenance(receipt) == Some(event.provenance)
            })
        } else {
            !expected
                .iter()
                .any(|receipt| receipt.identity == event.claim_identity)
        };
        !valid
    });
    if let Some(event) = invalid_event {
        diagnostics.push(Diagnostic::error(format!(
            "call claim joins differ from exact source exit and call replay in machine `{}` state `{}` at {:?}: {:?} has no matching claim origin",
            program.symbols.name(event.machine_symbol),
            program.symbols.name(event.state_symbol),
            event.source,
            event.kind,
        )));
    }
    failures
}
