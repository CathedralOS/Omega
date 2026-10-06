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
use typed_trees::{
    TypedTrees,
    statement::{StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode},
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
    program: &TypedTrees,
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
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) -> Option<CheckedStructuralControlStateCleanupPlan> {
    let parameters = program.state_parameters(state);
    let result_locals = state_exit_result_locals(program, facts, machine, state, None)?;
    let discard_parameters = checked_whole_affine_discard_parameters_excluding_results(
        program,
        facts,
        machine.symbol,
        state,
        &result_locals,
    )?;

    let statements = program.statement_table.statements(state.statement_nodes);
    let has_structural_control = statements.iter().any(|statement| {
        let StatementNode::Transition(transition) = statement else {
            return false;
        };
        transition.exit == TransitionExit::Ordinary
            && matches!(
                program.statement_table.transition_target(transition.target),
                TransitionTargetNode::Named { path, .. }
                    if crate::checks::termination::named_transition_target_state_index(
                        program, machine, path.symbol,
                    ).is_some()
                        || crate::semantic::calls::find_machine_by_entry_state(
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
        let StatementNode::Transition(transition) = statement else {
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
        let target = match crate::checks::termination::named_transition_target_state_index(
            program,
            machine,
            path.symbol,
        ) {
            Some(target_index) => &program.machine_states(machine)[target_index],
            None => match crate::semantic::calls::find_machine_by_entry_state(program, path.symbol)
            {
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
                } if source_index == statement_index && target_symbol == path.symbol
            )
        }) {
            let facts::PlaceRoot::Symbol(root) = event.root else {
                continue;
            };
            let Some((_, position)) = discard_parameters
                .iter()
                .find(|(candidate, _)| *candidate == root)
            else {
                continue;
            };
            if !segments.span_or_empty(event.segments).is_empty() {
                // The case-payload operation materializes a fresh carrier
                // before the edge retires its parent storage. It is not a
                // transfer of the whole parent. Keep that parent in the
                // ordinary discard roster; actual partial moves still
                // require their separate residual-complement evidence.
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
/// affine subtree is different — that read is a partial move, and this lane
/// has no residual vocabulary for it — with one exception the physical route
/// already provides: a proven case test on this edge extracts the selected
/// payload into a fresh carrier before the edge retires the parent. Crossing
/// a case also needs that proof, because an unproven case path names storage
/// the edge never selected. So the selection proof is required exactly where
/// it is load-bearing rather than on every projected read, which is what made
/// a copyable field or element read look like an unaccountable partial move.
/// Claims and nominal drops still cannot use the no-code retirement.
fn projected_read_retains_parent(
    program: &TypedTrees,
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
    let parameter = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == root)?;
    let payload = crate::flow::project_type_reference_from_segments(
        program,
        parameter.type_reference,
        segments,
    )?;
    let multiplicity = program.type_multiplicity(payload);
    if !matches!(
        multiplicity,
        Multiplicity::Affine | Multiplicity::Unrestricted
    ) || !(validation::has_plain_owned_contents_with_numeric_constraints(program, payload)
        || validation::has_owned_or_shared_view_contents(program, payload))
        || super::terminal_unit::types::type_graph_requires_nominal_drop(program, payload)
    {
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
    let subject = crate::flow::contextual_canonical_place_from_expression(
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
    // The extraction is the dispatch's leaf copy: the contents judgment above
    // already admitted plain contents or a shared-view carrier whose
    // descriptor loans keep their lineage on the fresh carrier.
    Some(())
}

pub(super) fn checked_whole_affine_discard_parameters(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: symbols::SymbolHandle,
    state: &typed_trees::state::State,
) -> Option<Vec<(symbols::SymbolHandle, u32)>> {
    checked_whole_affine_discard_parameters_excluding_results(program, facts, machine, state, &[])
}

fn checked_whole_affine_discard_parameters_excluding_results(
    program: &TypedTrees,
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
                edge_target != Some(target_symbol)
                    && program
                        .machines()
                        .iter()
                        .any(|candidate| candidate.symbol == target_symbol)
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
        let facts::PlaceRoot::Symbol(root) = event.root else {
            return None;
        };
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
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    dispatch_subject_local: Option<symbols::SymbolHandle>,
) -> Option<Vec<symbols::SymbolHandle>> {
    let parameters = program.state_parameters(state);
    let statements = program.statement_table.statements(state.statement_nodes);
    let permissions = &facts.flow.ownership;
    let mut results = Vec::new();
    for drop in facts
        .flow
        .ownership
        .state_exit_cleanup_roots(machine.symbol, state.symbol)?
    {
        let facts::PlaceRoot::Symbol(symbol) = drop.root else {
            return None;
        };
        // A closed-sum case dispatch consumes its subject directly — the
        // terminator build already proved the subject's exit drop, and a
        // `(expression)` arm needs no named successor to account for it.
        if dispatch_subject_local == Some(symbol) {
            continue;
        }
        if parameters
            .iter()
            .any(|parameter| parameter.symbol == symbol)
        {
            continue;
        }
        if drop.multiplicity != Multiplicity::Affine
            || drop.access != PermissionAccess::Owned
            || drop.claim_identity != PermissionClaimIdentity::Unknown
            || drop.obligation_live
            || !drop.segments.is_empty()
            || results.contains(&symbol)
        {
            return None;
        }
        let exact = |event: &checked_trees::FlowPermissionEventFact| {
            event.machine_symbol == machine.symbol
                && event.state_symbol == state.symbol
                && event.root == drop.root
                && event.multiplicity == drop.multiplicity
                && event.access == drop.access
                && event.claim_identity == drop.claim_identity
                && event.provenance == drop.provenance
                && !event.obligation_live
                && event.segments.is_empty()
        };
        // The establishment occurrence identifies this binding. Conserved
        // ownership provenance can name a child moved into its initializer,
        // so it need not name this local's declaration.
        let mut establishments = permissions.permissions.iter().filter(|(_, event)| {
            event.machine_symbol == machine.symbol
                && event.state_symbol == state.symbol
                && event.root == drop.root
                && event.kind == PermissionEventKind::Establish
                && event.access == PermissionAccess::Owned
        });
        let statement_index = if let Some((_, establish)) = establishments.next() {
            if establishments.next().is_some() || !exact(establish) {
                return None;
            }
            let PermissionEventSource::Statement { statement_index } = establish.source else {
                return None;
            };
            statement_index
        } else {
            // An owned selection is established by its arm-transfer receipt,
            // rather than an ordinary Establish row.
            if drop.provenance != PermissionProvenance::Unknown {
                return None;
            }
            let receipt = permissions
                .owned_selections
                .iter()
                .map(|(_, receipt)| receipt)
                .find(|receipt| {
                    receipt.machine == machine.symbol
                        && receipt.state == state.symbol
                        && receipt.destination == symbol
                        && receipt.death == PermissionEventSource::StateExit
                })?;
            receipt.statement_ordinal as usize
        };
        let StatementNode::LocalData(local) = statements.get(statement_index)? else {
            return None;
        };
        if local.symbol != symbol
            || program.type_multiplicity(local.type_reference) != Multiplicity::Affine
            // Disposal preserves the established value until its death. Its
            // represented predicates do not add a cleanup operation; the exact
            // permission ledger owns that decision. Normal-edge planning must
            // retain the selected nominal action and its prerequisites before
            // publishing the graph; a hook does not change the local's custody.
            || !(validation::has_structural_construction_contents(
                program,
                local.type_reference,
            ) || validation::has_owned_or_shared_view_contents(program, local.type_reference)
                // Stored reference leaves own descriptors, not their backing.
                // Their exact loan roster supports the same whole disposal.
                || validation::reference_result_custody::is_reference_record(program, local.type_reference))
        {
            return None;
        }
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
