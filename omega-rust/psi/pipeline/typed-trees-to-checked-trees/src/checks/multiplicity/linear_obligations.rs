//! Linear obligations: the places and claims a linear value establishes,
//! the claim outcome maps, and the checks that every partial move and
//! nominal drop respects them.

use super::borrowed_windows;
use crate::checks::multiplicity::linear_validation::{
    event_statement_index, validate_linear_permission_events,
};
use crate::checks::multiplicity::permission_events::record_permission_events_with_incoming_guards;
use crate::checks::multiplicity::projected_affine;
use crate::checks::multiplicity::temporary_results;
use crate::checks::multiplicity::type_multiplicity::find_data_definition;
use checked_trees::CheckFacts;
use diagnostics::Diagnostic;
use language_semantics::{
    Multiplicity, PermissionClaimIdentity, PermissionEventSource, PermissionProvenance,
};
use symbols::SymbolHandle;
use typed_trees::statement::StatementNode;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

#[derive(Debug, Clone)]
pub(crate) struct LinearPlace {
    pub(crate) symbol: SymbolHandle,
    pub(crate) name: String,
    /// Canonical claim path below `symbol`. An empty path is one nominal claim;
    /// transparent records, active cases, and fixed arrays contribute one
    /// entry per contained linear claim instead of inventing an aggregate root.
    pub(crate) path: Vec<facts::PlaceSegment>,
    pub(crate) multiplicity: Multiplicity,
    pub(crate) claim_identity: Option<PermissionClaimIdentity>,
    pub(crate) provenance: Option<PermissionProvenance>,
    pub(crate) live: bool,
    /// Parameters are established on entry. A local is established only by an
    /// explicit initializer/assignment; implicit zero-fill creates no debt.
    pub(crate) ever_established: bool,
    /// An affine sum can carry a linear payload only in selected cases. When
    /// false, `live` is unconditional for every established value; when true,
    /// `live` follows the active case.
    pub(crate) conditional: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WrittenLinearTarget {
    pub(crate) root: facts::PlaceRoot,
    pub(crate) destination_path: Vec<facts::PlaceSegment>,
    pub(crate) place_index: usize,
    pub(crate) obligation_live: bool,
    pub(crate) claim_identity: Option<PermissionClaimIdentity>,
    pub(crate) provenance: Option<PermissionProvenance>,
}

#[derive(Debug, Clone)]
pub(crate) struct LinearClaimTemplate {
    pub(crate) path: Vec<facts::PlaceSegment>,
    pub(crate) type_reference: TypeReferenceHandle,
    pub(super) multiplicity: Multiplicity,
    pub(super) conditional: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CheckedClaimOutcomeMap {
    pub(crate) machine_symbol: SymbolHandle,
    pub(crate) state_symbol: SymbolHandle,
    pub(crate) entries: Vec<CheckedClaimOutcomeEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CheckedClaimOutcomeEntry {
    pub(crate) output_path: Vec<facts::PlaceSegment>,
    pub(crate) source: CheckedClaimOutcomeSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CheckedClaimOutcomeSource {
    Input {
        parameter_symbol: SymbolHandle,
        path: Vec<facts::PlaceSegment>,
    },
    Established {
        claim_identity: PermissionClaimIdentity,
        provenance: PermissionProvenance,
    },
}

#[derive(Debug, Default)]
pub(crate) struct ClaimIdentityAllocator {
    next_ordinal: u32,
}

impl ClaimIdentityAllocator {
    pub(crate) fn mint(
        &mut self,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        source: PermissionEventSource,
    ) -> PermissionClaimIdentity {
        let ordinal = self.next_ordinal;
        self.next_ordinal = self
            .next_ordinal
            .checked_add(1)
            .expect("permission claim identity ordinal overflow");
        PermissionClaimIdentity::Established {
            machine_symbol,
            state_symbol,
            source,
            ordinal,
        }
    }
}

pub(crate) fn check_linear_obligations(
    program: &typed_trees::TypedTrees,
    facts: &mut CheckFacts,
    incoming_guards: &crate::checks::ranges::incoming_guards::IncomingGuardIndex,
) -> Result<(), Vec<Diagnostic>> {
    validate_partial_moves(program, facts)?;
    record_permission_events_with_incoming_guards(program, facts, incoming_guards)?;
    validate_linear_permission_events(program, facts)
}

/// A nominal cleanup machine is entitled to one whole valid value. Reject a
/// move below any prefix carrying that entitlement; structural records without
/// nominal cleanup remain decomposable, and moving the entitled value itself
/// remains legal. Temporary partial moves must also retain every linear claim
/// because there is no remaining local owner for an unselected sibling.
fn validate_partial_moves(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut segments = arena::Arena::default();
            let moves = crate::flow::discover_state_move_events(
                program,
                &facts.borrow,
                &facts.operators,
                machine,
                state,
                &mut segments,
            );
            let statements = program.statement_table.statements(state.statement_nodes);
            let state_calls = facts
                .flow
                .control
                .states
                .iter()
                .find_map(|(_, flow)| {
                    (flow.machine_symbol == machine.symbol && flow.state_symbol == state.symbol)
                        .then(|| facts.flow.control.calls.span_or_empty(flow.calls))
                })
                .unwrap_or_default();
            // A state body is a linear fall-through: each `transition` arm is
            // a guarded exit edge, and a missed arm continues to the next
            // statement. Every taken edge and the implicit return must see a
            // discharged window; the fall-through path may still repair it.
            let mut windows = borrowed_windows::BorrowedStorageWindows::default();
            for (statement_index, statement) in statements.iter().enumerate() {
                let is_transition = matches!(statement, StatementNode::Transition(_));
                for event in moves
                    .iter()
                    .filter(|event| event_statement_index(event.source) == Some(statement_index))
                {
                    let path = segments.span_or_empty(event.segments);
                    if move_event_is_production_target(program, state, event, path) {
                        continue;
                    }
                    if !path.is_empty()
                        && projected_affine::is_borrowed_place_transfer(
                            program,
                            machine.symbol,
                            state,
                            event,
                            path,
                        )
                    {
                        // Moves evaluated on a transition edge cannot be
                        // repaired before the edge leaves; match-arm moves
                        // open a hole on only one joining edge; a nominal-drop
                        // owner is entitled to a whole value. All keep the
                        // plain rejection.
                        if is_transition
                            || event.source_arm.is_valid()
                            || borrowed_move_crosses_nominal_drop(program, state, event, path)
                        {
                            diagnostics.push(borrowed_windows::borrowed_transfer_diagnostic(
                                machine,
                                state,
                                event_statement_index(event.source).unwrap_or(0),
                            ));
                            continue;
                        }
                        if let Some(diagnostic) = windows.open(
                            program,
                            machine,
                            state,
                            statements,
                            statement_index,
                            event,
                            path,
                        ) {
                            diagnostics.push(diagnostic);
                        }
                        continue;
                    }
                    if !path.is_empty() {
                        temporary_results::check_unselected_claims(
                            program,
                            state.symbol,
                            event,
                            path,
                            &mut diagnostics,
                        );
                        for prefix_len in 0..path.len() {
                            if prefix_len == 0
                                && event_is_owned_self_projection(program, state, event)
                            {
                                continue;
                            }
                            let prefix = crate::flow::CanonicalPlace {
                                root: event.root,
                                segments: path[..prefix_len].to_vec(),
                            };
                            let Some(type_name) = nominal_drop_place_name(
                                program,
                                state.symbol,
                                event_statement_index(event.source).unwrap_or(0),
                                &prefix,
                            ) else {
                                continue;
                            };
                            diagnostics.push(Diagnostic::error(format!(
                                "cannot partially move a value of `{type_name}` because `{type_name}::drop` requires the whole value"
                            )));
                            break;
                        }
                    }
                    // Even an ordinary whole-owner move may not carry away a
                    // place with an absent borrowed subtree.
                    windows.refuse_move_over_open(
                        program,
                        machine,
                        state,
                        statements,
                        statement_index,
                        event,
                        path,
                        &mut diagnostics,
                    );
                }
                let moved = borrowed_windows::BorrowedStorageWindows::statement_moved_places(
                    program,
                    machine,
                    state,
                    statements,
                    statement_index,
                    &moves,
                    &segments,
                );
                windows.check_statement(
                    program,
                    machine,
                    state,
                    statements,
                    statement_index,
                    statement,
                    &moved,
                    &facts.flow.control,
                    state_calls,
                    &facts.service_reaches,
                    &facts.operators,
                    &mut diagnostics,
                );
                // A taken transition edge leaves the state; a window open on
                // it can never be repaired. A crash edge abandons the window
                // outright under the existing survivor contract — no hole is
                // observed by anything that could survive. The guard-miss
                // path falls through and may still discharge the debt below.
                let is_crash = matches!(
                    statement,
                    StatementNode::Transition(transition)
                        if matches!(
                            transition.exit,
                            typed_trees::statement::TransitionExit::Crash(_)
                        )
                );
                if is_transition && !is_crash {
                    windows.refuse_open_at_exit(machine, state, &mut diagnostics);
                }
            }
            // The last statement's fall-through — and a state with no
            // transitions at all — ends in the implicit return.
            if !matches!(statements.last(), Some(StatementNode::Transition(_))) {
                windows.refuse_open_at_exit(machine, state, &mut diagnostics);
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// Nominal cleanup is entitled to one whole value: a move out of borrowed
/// storage that crosses an entitled prefix stays rejected even when a later
/// repair would reseat the hole.
fn borrowed_move_crosses_nominal_drop(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    event: &crate::flow::DiscoveredMoveEvent,
    path: &[facts::PlaceSegment],
) -> bool {
    (0..path.len()).any(|prefix_len| {
        let prefix = crate::flow::CanonicalPlace {
            root: event.root,
            segments: path[..prefix_len].to_vec(),
        };
        nominal_drop_place_name(
            program,
            state.symbol,
            event_statement_index(event.source).unwrap_or(0),
            &prefix,
        )
        .is_some()
    })
}

fn event_is_owned_self_projection(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    event: &crate::flow::DiscoveredMoveEvent,
) -> bool {
    let facts::PlaceRoot::Symbol(event_root) = event.root else {
        return false;
    };
    if !program.machines().iter().any(|machine| {
        machine.symbol == event_root
            && program
                .machine_states(machine)
                .iter()
                .any(|candidate| candidate.symbol == state.symbol)
    }) {
        return false;
    }
    program.state_parameters(state).iter().any(|parameter| {
        parameter.is_self && !type_reference_is_reference(program, parameter.type_reference)
    })
}

pub(crate) fn type_reference_is_reference(
    program: &typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { .. } => true,
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_is_reference(program, *base_type)
        }
        _ => false,
    }
}

fn nominal_drop_place_name<'program>(
    program: &'program typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    place: &crate::flow::CanonicalPlace,
) -> Option<&'program str> {
    if place.segments.is_empty()
        && let facts::PlaceRoot::Symbol(root) = place.root
        && let Some(attached) = program.machines().iter().find_map(|machine| {
            (machine.symbol == root
                && program
                    .machine_states(machine)
                    .iter()
                    .any(|state| state.symbol == state_symbol))
            .then_some(machine.attached_data.as_deref())
            .flatten()
        })
    {
        return data_name_with_nominal_drop(program, attached);
    }
    let type_reference =
        crate::flow::canonical_place_type_reference(program, state_symbol, statement_index, place)?;
    nominal_drop_type_name(program, type_reference)
}

fn data_name_with_nominal_drop<'program>(
    program: &'program typed_trees::TypedTrees,
    name: &str,
) -> Option<&'program str> {
    let definition = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == name)?;
    nominal_drop_machine_symbol(program, definition.symbol)
        .is_some()
        .then_some(definition.name.as_str())
}

/// The one reserved cleanup machine attached to an exact nominal declaration.
/// Presentation names never choose the carrier: the retained attachment symbol
/// must agree first, preventing an unrelated same-named machine from becoming
/// automatic cleanup authority.
pub(crate) fn nominal_drop_machine_symbol(
    program: &typed_trees::TypedTrees,
    data_symbol: SymbolHandle,
) -> Option<SymbolHandle> {
    let mut matches = program.machines().iter().filter(|machine| {
        machine.attached_data_symbol == data_symbol
            && machine.name.as_str().rsplit("::").next() == Some("drop")
    });
    let selected = matches.next()?;
    matches.next().is_none().then_some(selected.symbol)
}

fn move_event_is_production_target(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    event: &crate::flow::DiscoveredMoveEvent,
    path: &[facts::PlaceSegment],
) -> bool {
    let Some(statement_index) = event_statement_index(event.source) else {
        return false;
    };
    let Some(statement) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)
    else {
        return false;
    };
    let target = match statement {
        StatementNode::LocalData(local) => crate::flow::CanonicalPlace {
            root: facts::PlaceRoot::Symbol(local.symbol),
            segments: Vec::new(),
        },
        StatementNode::Assignment(assignment) => {
            let Some(target) = crate::flow::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                statement_index,
                assignment.target,
            ) else {
                return false;
            };
            target
        }
        _ => return false,
    };
    crate::flow::normalized_event_place_root(program, target.root) == event.root
        && target.segments.as_slice() == path
}

fn nominal_drop_type_name(
    program: &typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<&str> {
    let (symbol, name) = match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. }
        | TypeReferenceNode::Reference {
            referee: base_type, ..
        } => return nominal_drop_type_name(program, *base_type),
        TypeReferenceNode::Named { symbol, name } => (*symbol, name.as_str()),
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            ..
        } => (*base_symbol, base_name.as_str()),
        _ => return None,
    };
    let definition = find_data_definition(program, symbol, name)?;
    data_name_with_nominal_drop(program, definition.name.as_str())
}
