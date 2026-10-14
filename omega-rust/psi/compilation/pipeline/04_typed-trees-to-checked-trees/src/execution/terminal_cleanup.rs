//! Whole-root cleanup for the older structural-control consumers, plus lexical
//! result identity for composed control. Composed execution gets its ordered
//! whole/residual/nominal actions from `terminal_unit::cleanup::normal_edges`.
//! Do not rebuild a parallel projected-edge table here: it has no executable
//! consumer, and requiring it before normal-edge planning prevents partial
//! transfers from reaching the operation that already owns their cleanup.

use std::collections::BTreeSet;

use checked_trees::{
    CheckFacts, CheckedStructuralControlCleanupPlans, CheckedStructuralControlEdgeCleanupPlan,
    CheckedStructuralControlStateCleanupPlan,
};
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource, PermissionProvenance,
};
use typed_trees::statement::{
    StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode,
};

use crate::flow::FlowOwnershipEventSource;

/// Retain the first source-handle-free structural control-edge cleanup slice.
///
/// The permission ledger is authoritative for which whole affine parameters
/// are eligible for no-code state-exit disposal. Per-arm move discovery is the
/// same checked ownership input used by multiplicity validation. Anything that
/// needs projected, nominal, or claim-bearing cleanup omits the complete
/// state plan rather than publishing partial evidence. Exact local result
/// obligations stay separate: ordinary completion disposes the local, while
/// a named successor must account for its selected transfer or disposal.
pub(crate) fn build_checked_structural_control_cleanup_plans(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
) -> CheckedStructuralControlCleanupPlans {
    let mut states = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            if let Some(plan) = build_state_plan(program, facts, machine, state) {
                states.push(plan);
            }
        }
    }
    CheckedStructuralControlCleanupPlans { states }
}

fn build_state_plan(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) -> Option<CheckedStructuralControlStateCleanupPlan> {
    let parameters = program.state_parameters(state);
    let result_locals = state_exit_result_locals(program, facts, machine, state)?;
    let discard_parameters = checked_whole_affine_discard_parameters_excluding_results(
        program,
        facts,
        machine.symbol,
        state,
        &result_locals,
    )?;

    let statements = program.statement_table.statements(state.statement_nodes);
    let has_structural_control = statements.iter().any(|statement| {
        let Some(transition) = statement.as_transition() else {
            return false;
        };
        transition.exit == TransitionExit::Ordinary
            && matches!(
                program.statement_table.transition_target(transition.target),
                TransitionTargetNode::Named { path, .. }
                    if validation::activation_target_state_index(
                        program, machine, path.symbol,
                    ).is_some()
                        || validation::find_machine_by_entry_state(
                            program,
                            path.symbol,
                        )
                        .is_some()
            )
    });
    if !has_structural_control {
        return None;
    }

    let mut segments = facts.flow.ownership.segments.clone();
    let moves = crate::flow::discover_state_move_events(
        program,
        &facts.borrow,
        &facts.operators,
        machine,
        state,
        &mut segments,
    );
    let mut edges = Vec::new();
    for (statement_index, statement) in statements.iter().enumerate() {
        let Some(transition) = statement.as_transition() else {
            continue;
        };
        if transition.exit != TransitionExit::Ordinary {
            continue;
        }
        let TransitionTargetNode::Named { path, .. } =
            program.statement_table.transition_target(transition.target)
        else {
            continue;
        };
        // A named target either spells one of this machine's own states or
        // another machine's entry; the fused Unit graph keeps both inside
        // the same composed roster, so the edge row names whichever state
        // the transition actually transfers to. Anything else has no
        // composed edge and keeps the omitted-edge behavior.
        let target = match validation::activation_target_state_index(program, machine, path.symbol)
        {
            Some(target_index) => &program.machine_states(machine)[target_index],
            None => match validation::find_machine_by_entry_state(program, path.symbol) {
                Some((_, entry)) => entry,
                None => continue,
            },
        };

        let mut transferred = BTreeSet::new();
        for event in moves.iter().filter(|event| {
            matches!(
                event.source,
                FlowOwnershipEventSource::Statement {
                    statement_index: source_index,
                } if source_index == statement_index
            ) || matches!(
                event.source,
                FlowOwnershipEventSource::Call {
                    statement_index: source_index,
                    target_symbol,
                    ..
                } if source_index == statement_index && target_symbol == validation::call_target_state(program, path.symbol)
                    .map_or(path.symbol, |state| state.symbol)
            )
        }) {
            let Some(root) = event.root.as_symbol() else {
                continue;
            };
            let Some((_, position)) = discard_parameters
                .iter()
                .find(|(candidate, _)| *candidate == root)
            else {
                continue;
            };
            if !segments.span_or_empty(event.segments).is_empty() {
                // A projected argument does not transfer the whole parent.
                // Keep its lexical death in this roster; selected-edge
                // emission replaces whole retirement with the actual residual
                // complement when an affine subtree moves. Copyable reads
                // leave the parent's custody whole.
                projected_read_retains_parent(
                    program,
                    state,
                    statement_index,
                    transition,
                    root,
                    segments.span_or_empty(event.segments),
                )?;
                continue;
            }
            transferred.insert(*position);
        }
        // Selected operand calls consume their inputs before the successor
        // receives their results. The input still dies on sibling edges, so
        // conserve its lexical exit row and account this exact call only here.
        for (symbol, position) in &discard_parameters {
            let operand_moves = validation::transition_operand_moves(
                program,
                facts,
                machine.symbol,
                state,
                statement_index,
                *symbol,
            )?;
            if operand_moves.len() > 1
                || (!operand_moves.is_empty() && transferred.contains(position))
            {
                return None;
            }
            for source in operand_moves {
                let mut transfers = facts
                    .flow
                    .ownership
                    .permissions
                    .iter()
                    .filter(|(_, event)| {
                        event.machine_symbol == machine.symbol
                            && event.state_symbol == state.symbol
                            && event.root == facts::PlaceRoot::Symbol(*symbol)
                            && event.kind == PermissionEventKind::Transfer
                            && event.source == source
                    });
                let (_, transfer) = transfers.next()?;
                if transfers.next().is_some()
                    || transfer.access != PermissionAccess::Owned
                    || transfer.multiplicity != Multiplicity::Affine
                    || transfer.claim_identity != PermissionClaimIdentity::Unknown
                    || transfer.provenance != PermissionProvenance::Unknown
                    || transfer.obligation_live
                    || !facts
                        .flow
                        .ownership
                        .segments
                        .span_or_empty(transfer.segments)
                        .is_empty()
                {
                    return None;
                }
                transferred.insert(*position);
            }
        }
        let trivial_affine_discard_parameter_positions = discard_parameters
            .iter()
            .filter_map(|(symbol, position)| {
                (!transferred.contains(position)).then_some((*symbol, *position))
            })
            .map(|(symbol, position)| {
                let parameter = parameters
                    .iter()
                    .find(|parameter| parameter.symbol == symbol)?;
                (!super::terminal_unit::types::type_graph_requires_nominal_drop(
                    program,
                    parameter.type_reference,
                    &[],
                    &mut std::collections::BTreeSet::new(),
                ))
                .then_some(position)
            })
            .collect::<Option<Vec<_>>>()?;
        edges.push(CheckedStructuralControlEdgeCleanupPlan {
            statement_ordinal: u32::try_from(statement_index).ok()?,
            // Entry back-edges name the machine in source; consumers join the
            // cleanup to the normalized state, while move events above retain
            // their authored call target identity.
            target_state: target.symbol,
            trivial_affine_discard_parameter_positions,
        });
    }
    (!edges.is_empty()).then_some(CheckedStructuralControlStateCleanupPlan {
        machine: machine.symbol,
        state: state.symbol,
        edges,
    })
}

/// Whether a projected read out of `root` leaves the parent's storage intact,
/// so the parent keeps its ordinary state-exit discard row.
///
/// Reading a copyable subtree is an observation: the read materializes a
/// fresh value and the parent survives it, at any field or index path. An
/// affine subtree is different — that read is a partial move, and this lexical
/// roster does not enumerate its complement. Selected-edge emission retains
/// original projected custody and completes that complement separately. Crossing
/// a case also needs that proof, because an unproven case path names storage
/// the edge never selected. So the selection proof is required exactly where
/// it is load-bearing rather than on every projected read, which is what made
/// a copyable field or element read look like an unaccountable partial move.
/// Claims and nominal drops still cannot use the no-code retirement.
fn projected_read_retains_parent(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    transition: &typed_trees::statement::TableTransition,
    root: symbols::SymbolHandle,
    segments: &[facts::PlaceSegment],
) -> Option<()> {
    // A segment carrying no declared field symbol names derived metadata
    // rather than stored contents — `view.len` is the common one. Metadata is
    // computed from the storage it describes, so reading it transfers nothing
    // at any depth and there is no subtree to classify below it.
    if segments.iter().any(
        |segment| matches!(segment, facts::PlaceSegment::Field { symbol } if !symbol.is_valid()),
    ) {
        return Some(());
    }
    // Otherwise the projected subtree is the authority on whether the read
    // copies: a plain or shared-view leaf with no nominal drop is observable,
    // and only an unrestricted one leaves the parent's custody whole by
    // itself.
    let parameter = program.state_parameter_by_symbol(state, root)?;
    let payload = validation::project_type_reference_from_segments(
        program,
        parameter.type_reference,
        segments,
    )?;
    let multiplicity = program.type_multiplicity(payload);
    if !matches!(
        multiplicity,
        Multiplicity::Affine | Multiplicity::Unrestricted
    ) || !(validation::has_contents(
        program,
        payload,
        &[],
        validation::ContentsRequirement::NumericOwned,
    ) || validation::has_contents(
        program,
        payload,
        &[],
        validation::ContentsRequirement::OwnedOrSharedView,
    )) || super::terminal_unit::types::type_graph_requires_nominal_drop(
        program,
        payload,
        &[],
        &mut std::collections::BTreeSet::new(),
    ) {
        return None;
    }
    let crosses_case = segments
        .iter()
        .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }));
    if multiplicity == Multiplicity::Unrestricted && !crosses_case {
        return Some(());
    }
    let TransitionGuardNode::When(guard) = transition.guard else {
        return None;
    };
    let (subject, selected_case) = crate::proof::exact_outcome_case_test(program, guard)?;
    let subject = validation::contextual_canonical_place_from_expression(
        program,
        state.symbol,
        statement_index,
        subject,
    )?;
    if subject.root != facts::PlaceRoot::Symbol(root) || !subject.segments.is_empty() {
        return None;
    }
    let [
        facts::PlaceSegment::Case { variant },
        facts::PlaceSegment::Field { symbol },
    ] = segments
    else {
        return None;
    };
    if *variant != selected_case || !program.data_definitions().iter().any(|definition| {
        program.data_members(definition).iter().any(|member| {
            matches!(member, typed_trees::data::DataMember::Variant(variant)
                if variant.symbol == selected_case && program.data_payload_fields(variant).iter().any(|field| field.symbol == *symbol))
        })
    }) {
        return None;
    }
    // Membership makes this projected death accountable. It does not grant
    // copyability: affine emission keeps original subtree custody and supplies
    // the residual complement; unrestricted observations may copy instead.
    Some(())
}

fn checked_whole_affine_discard_parameters_excluding_results(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: symbols::SymbolHandle,
    state: &typed_trees::state::State,
    result_locals: &[symbols::SymbolHandle],
) -> Option<Vec<(symbols::SymbolHandle, u32)>> {
    let parameters = program.state_parameters(state);
    let entry_claim_roots = facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter_map(|(_, event)| {
            (event.machine_symbol == machine
                && event.state_symbol == state.symbol
                && event.source == PermissionEventSource::StateEntry
                && event.kind == PermissionEventKind::Establish
                && event.access == PermissionAccess::Owned)
                .then_some(event.root)
        })
        .collect::<Vec<_>>();
    // A selected source parameter keeps no exit drop: its conditional custody
    // and residual death ride the owned-selection receipt instead.
    let selection_source_roots = facts
        .flow
        .ownership
        .owned_selections
        .iter()
        .filter(|(_, receipt)| receipt.machine == machine && receipt.state == state.symbol)
        .flat_map(|(_, receipt)| {
            facts
                .flow
                .ownership
                .selection_sources
                .span_or_empty(receipt.sources)
                .iter()
                .map(|source| facts::PlaceRoot::Symbol(source.symbol))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    // A parameter whose whole value already left mid-body — moved into a
    // store's hole or consumed by a call — keeps no exit drop either; the
    // transfer event is its custody, not the state's exit roster. Moves
    // attributed to a terminator edge are different: the source param's
    // slot still dies at state exit (its AffineDrop stays on the roster)
    // while the edge mints the successor's fresh binding, so a
    // statement-indexed move on a transition row, or a call-sourced move
    // targeting a state rather than a machine, is an edge forward and
    // stays excluded from this set.
    let statements = program.statement_table.statements(state.statement_nodes);
    let mid_body_move = |source: PermissionEventSource| -> bool {
        match source {
            PermissionEventSource::Statement { statement_index } => !matches!(
                statements.get(statement_index),
                Some(StatementNode::Transition(_))
            ),
            PermissionEventSource::Call {
                statement_index,
                target_symbol,
                ..
            } => {
                // A call-sourced move on a transition statement whose target
                // IS the transition's named target is an edge forward — a
                // machine re-entry or a foreign machine jump — exactly like a
                // state-targeting edge call. Only a different machine's call
                // (an arm operand) consumed the value before the edge.
                let edge_target = match statements.get(statement_index) {
                    Some(StatementNode::Transition(transition))
                        if transition.exit == TransitionExit::Ordinary =>
                    {
                        match program.statement_table.transition_target(transition.target) {
                            TransitionTargetNode::Named { path, .. } => Some(path.symbol),
                            _ => None,
                        }
                    }
                    _ => None,
                };
                edge_target != Some(target_symbol) && program.machine(target_symbol).is_some()
            }
            _ => false,
        }
    };
    let moved_roots = facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter_map(|(_, event)| {
            (event.machine_symbol == machine
                && event.state_symbol == state.symbol
                && matches!(
                    event.kind,
                    PermissionEventKind::Transfer | PermissionEventKind::Consume
                )
                && event.access == PermissionAccess::Owned
                && mid_body_move(event.source)
                && facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(event.segments)
                    .is_empty())
            .then_some(event.root)
        })
        .collect::<Vec<_>>();
    let expected_discard_parameters = parameters
        .iter()
        .enumerate()
        .rev()
        .filter_map(|(position, parameter)| {
            if (crate::checks::type_multiplicity(program, parameter.type_reference)
                != Multiplicity::Affine)
                || entry_claim_roots.contains(&facts::PlaceRoot::Symbol(parameter.symbol))
                || selection_source_roots.contains(&facts::PlaceRoot::Symbol(parameter.symbol))
                || moved_roots.contains(&facts::PlaceRoot::Symbol(parameter.symbol))
            {
                return None;
            }
            Some((parameter.symbol, u32::try_from(position).ok()?))
        })
        .collect::<Vec<_>>();
    let mut discard_parameters = Vec::new();
    for event in facts
        .flow
        .ownership
        .state_exit_cleanup_roots(machine, state.symbol)?
    {
        if matches!(event.root, facts::PlaceRoot::Symbol(symbol) if result_locals.contains(&symbol))
        {
            continue;
        }
        if event.access != PermissionAccess::Owned
            || event.multiplicity != Multiplicity::Affine
            || event.obligation_live
            || event.claim_identity != PermissionClaimIdentity::Unknown
            || event.provenance != PermissionProvenance::Unknown
            || !facts
                .flow
                .ownership
                .segments
                .span_or_empty(event.segments)
                .is_empty()
        {
            return None;
        }
        let root = event.root.as_symbol()?;
        let position = parameters.iter().position(|parameter| {
            (parameter.symbol == root)
                && crate::checks::type_multiplicity(program, parameter.type_reference)
                    == Multiplicity::Affine
        })?;
        let position = u32::try_from(position).ok()?;
        if discard_parameters.contains(&(root, position)) {
            return None;
        }
        discard_parameters.push((root, position));
    }
    // This is a union of owners across selected exits, not one exit's drop
    // stream. Forwarding a later parameter on the first edge can make an
    // earlier parameter appear first in that union. Derive cleanup order
    // from declarations, then let each edge subtract its own transfers.
    // The permission query retains per-exit duplicate and custody checks;
    // Terminal checks the resulting edge's live frontier and disposal order.
    discard_parameters.sort_unstable_by_key(|(_, position)| std::cmp::Reverse(*position));
    if discard_parameters != expected_discard_parameters {
        return None;
    }
    Some(discard_parameters)
}

/// Separate exact local result obligations from the parameter cleanup roster.
/// The graph producer joins candidate cleanup roots to actual result bindings;
/// selected normal-edge actions own the whole or partial disposal. This roster
/// does not turn local results into parameters or reconstruct outgoing syntax.
pub(super) fn state_exit_result_locals(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) -> Option<Vec<symbols::SymbolHandle>> {
    let parameters = program.state_parameters(state);
    let statements = program.statement_table.statements(state.statement_nodes);
    let mut results = Vec::new();
    for drop in facts
        .flow
        .ownership
        .state_exit_cleanup_roots(machine.symbol, state.symbol)?
        .into_iter()
        .chain(
            facts
                .flow
                .ownership
                .permissions
                .iter()
                .filter_map(|(_, event)| {
                    // A selected linear region has an exact consumed claim
                    // and contextual disposition. An initialized child can
                    // be the entire live complement of a reserved container;
                    // its receipt also identifies that containing home.
                    (event.machine_symbol == machine.symbol
                        && event.state_symbol == state.symbol
                        && event.source == PermissionEventSource::StateExit
                        && event.kind == PermissionEventKind::Consume
                        && event.multiplicity == Multiplicity::Linear
                        && event.access == PermissionAccess::Owned
                        && event.claim_identity != PermissionClaimIdentity::Unknown
                        && event.provenance != PermissionProvenance::Unknown
                        && !event.obligation_live
                        && facts
                            .contextual_cleanup
                            .dispositions
                            .iter()
                            .any(|(_, disposition)| {
                                let evaluation = match disposition.exit.kind {
                                    checked_trees::FlowClaimJoinExitKind::TransitionTarget
                                    | checked_trees::FlowClaimJoinExitKind::TransitionContinuation => {
                                        let Some(StatementNode::Transition(transition)) =
                                            statements.get(disposition.exit.statement_index)
                                        else {
                                            return false;
                                        };
                                        checked_trees::FlowPermissionEvaluation::TransitionTarget(
                                            if disposition.exit.kind == checked_trees::FlowClaimJoinExitKind::TransitionTarget {
                                                transition.target
                                            } else {
                                                transition.continuation
                                            },
                                        )
                                    }
                                    _ => checked_trees::FlowPermissionEvaluation::Statement,
                                };
                                disposition.machine == machine.symbol
                                    && disposition.state == state.symbol
                                    && disposition.exit.state_symbol == state.symbol
                                    && disposition.source == PermissionEventSource::StateExit
                                    && event.evaluation == evaluation
                                    && disposition.root == event.root
                                    && facts.flow.ownership.segments.span_or_empty(disposition.path)
                                        == facts.flow.ownership.segments.span_or_empty(event.segments)
                            }))
                    .then_some(event)
                }),
        )
        .chain(
            facts
                .flow
                .ownership
                .selected_cleanup_permissions
                .iter()
                .filter_map(|(_, event)| {
                    (event.machine_symbol == machine.symbol && event.state_symbol == state.symbol)
                        .then_some(event)
                }),
        )
    {
        let Some(symbol) = drop.root.as_symbol() else {
            // Anonymous owners join their exact producer through the normal
            // exit cleanup roster, not through this named-local inventory.
            continue;
        };
        if parameters
            .iter()
            .any(|parameter| parameter.symbol == symbol)
        {
            continue;
        }
        if !matches!(
            drop.multiplicity,
            Multiplicity::Affine | Multiplicity::Linear
        ) || drop.access != PermissionAccess::Owned
            || (drop.multiplicity == Multiplicity::Affine
                && drop.claim_identity != PermissionClaimIdentity::Unknown)
            || drop.obligation_live
        {
            return None;
        }
        if results.contains(&symbol) {
            continue;
        }
        // This is the storage/result identity join, not a second permission
        // replay. A root may have several established-prefix deaths or stores;
        // its declaration identifies the one home normal-edge actions target.
        let (statement_index, local) = program.state_local_declaration(state, symbol)?;
        if local.symbol != symbol {
            return None;
        }
        // The receipt's multiplicity belongs to its selected region, not
        // necessarily the containing local (a linear child in an affine
        // record, or affine retirement of an array's claim-free home).
        // Source replay owns region conservation; local_results rejoins the
        // declared carrier type and multiplicity to the actual result.
        // The selected disposition names remaining custody, not permission to
        // discard the original complete type. Transferred linear children or
        // nominal hooks can make that whole-type classifier inapplicable.
        // Join the lexical home here; normal-edge action selection retains
        // the exact complement and hook prerequisites before publishing it.
        if statements[..=statement_index]
            .iter()
            .any(|statement| matches!(statement, StatementNode::Transition(_)))
        {
            return None;
        }
        // The contextual cleanup selection already owns the exact outgoing
        // partition, including projected moves and their residual complements.
        // This roster only joins a candidate local to its establishment. The
        // normal-edge planner retains the selected ordered actions; replaying
        // named transition syntax here used to reject every partial transfer.
        results.push(symbol);
    }
    Some(results)
}
