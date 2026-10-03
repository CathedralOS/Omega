use std::collections::BTreeSet;

use checked_trees::{
    CheckFacts, CheckedStructuralControlCleanupPlans, CheckedStructuralControlEdgeCleanupPlan,
    CheckedStructuralControlProjectedEdgeCleanupPlan,
    CheckedStructuralControlProjectedTransferPlan, CheckedStructuralControlStateCleanupPlan,
    CheckedUnitPartialAffineDiscardPlan, CheckedUnitStructuralPathSegment,
};
use language_semantics::{
    MachineSupplyMode, Multiplicity, PermissionAccess, PermissionClaimIdentity,
    PermissionEventKind, PermissionEventSource, PermissionProvenance,
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
    let mut projected_edges = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            if let Some(edge) = build_projected_edge_plan(program, facts, machine, state) {
                projected_edges.push(edge);
            }
            if let Some(plan) = build_state_plan(program, facts, machine, state) {
                states.push(plan);
            }
        }
    }
    CheckedStructuralControlCleanupPlans {
        states,
        projected_edges,
    }
}

fn build_projected_edge_plan(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) -> Option<CheckedStructuralControlProjectedEdgeCleanupPlan> {
    let states = program.machine_states(machine);
    if machine.supply_mode != MachineSupplyMode::CheckedBody
        || machine.attached_data.is_none()
        || !program.machine_contracts(machine).is_empty()
        || !program.state_contracts(state).is_empty()
        || !super::terminal_unit::cleanup_type_is_unit(program, state.return_type)
    {
        return None;
    }
    let [source_parameter] = program.state_parameters(state) else {
        return None;
    };
    if source_parameter.is_self {
        return None;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    let (last, prefix) = statements.split_last()?;
    let StatementNode::Transition(transition) = last else {
        return None;
    };
    let transition_ordinal = prefix.len();
    if transition.exit != TransitionExit::Ordinary
        || transition.guard != TransitionGuardNode::Always
        || transition.continuation.is_valid()
    {
        return None;
    }
    // Leading statements may establish construction-local temporaries and move
    // their exact projections through calls. Each affine local that drops at the
    // edge is a residual-bearing dying root; entry-parameter cleanup alone cannot
    // dispose its remainder. Anything outside LocalData and call expressions —
    // other statements, whole-local transfers, case/dynamic/referent paths —
    // keeps the edge fail-closed.
    let mut locals = Vec::new();
    for (index, statement) in prefix.iter().enumerate() {
        match statement {
            StatementNode::LocalData(local) => {
                if program.type_multiplicity(local.type_reference) != Multiplicity::Affine {
                    continue;
                }
                if !local.symbol.is_valid()
                    || local.is_mutable
                    || !program
                        .expression_table
                        .expression_is_valid(local.initial_value)
                    || !validation::has_plain_owned_contents_with_numeric_constraints(
                        program,
                        local.type_reference,
                    )
                    || super::terminal_unit::types::type_graph_requires_nominal_drop(
                        program,
                        local.type_reference,
                    )
                {
                    return None;
                }
                match program
                    .type_reference_table
                    .type_reference(local.type_reference)
                {
                    typed_trees::types::TypeReferenceNode::Named { symbol, .. }
                    | typed_trees::types::TypeReferenceNode::Generic {
                        base_symbol: symbol,
                        ..
                    } => {
                        let root = program
                            .data_definitions()
                            .iter()
                            .find(|data| data.symbol == *symbol)?;
                        if root.properties.carry.is_some()
                            || !root.lifetime_parameters.is_empty()
                            || !program.data_type_parameters(root).is_empty()
                        {
                            return None;
                        }
                    }
                    typed_trees::types::TypeReferenceNode::FixedArray { .. } => {}
                    _ => return None,
                }
                locals.push((local.symbol, local.type_reference, index));
            }
            StatementNode::Call(_) => {}
            StatementNode::Expression(expression)
                if matches!(
                    program.expression_table.expression(*expression),
                    typed_trees::expression::ExpressionNode::Call(_)
                ) => {}
            _ => return None,
        }
    }
    let local_roots = locals
        .iter()
        .map(|(symbol, ..)| *symbol)
        .collect::<Vec<_>>();
    let TransitionTargetNode::Named {
        path, arguments, ..
    } = program.statement_table.transition_target(transition.target)
    else {
        return None;
    };
    // A transition naming another machine's entry transfers to that
    // machine's first state exactly as a fused Unit graph resolves it.
    let target = match states.iter().find(|target| target.symbol == path.symbol) {
        Some(target) => target,
        None => crate::semantic::calls::find_machine_by_entry_state(program, path.symbol)?.1,
    };
    if target.symbol == state.symbol
        || !program.state_contracts(target).is_empty()
        || !super::terminal_unit::cleanup_type_is_unit(program, target.return_type)
    {
        return None;
    }
    let [target_parameter] = program.state_parameters(target) else {
        return None;
    };
    if target_parameter.is_self {
        return None;
    }
    let [argument] = program.statement_table.expression_handles(*arguments) else {
        return None;
    };
    let argument_place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        transition_ordinal,
        *argument,
    )?;
    let facts::PlaceRoot::Symbol(argument_root) = argument_place.root else {
        return None;
    };
    if argument_root != source_parameter.symbol {
        return None;
    }
    // Exact projected moves admit field and literal-index segments only; case
    // projections, computed indexes and referent segments keep the older
    // fail-closed behavior for the path-sensitive lane.
    let (moved_type, moved_path) = super::terminal_unit::calls::projected_argument_path(
        program,
        state.symbol,
        0,
        &argument_place,
    )?;
    if moved_path.is_empty() {
        return None;
    }
    let discard_parameters = checked_whole_affine_discard_parameters_excluding_results(
        program,
        facts,
        machine.symbol,
        state,
        &local_roots,
    )?;
    if discard_parameters != [(source_parameter.symbol, 0)]
        || facts.flow.ownership.permissions.iter().any(|(_, event)| {
            event.machine_symbol == machine.symbol
                && event.state_symbol == state.symbol
                && event.claim_identity != PermissionClaimIdentity::Unknown
        })
    {
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
    // The moved parameter's only ownership move is the transition argument;
    // construction-local roots move only through calls earlier in the body.
    // Any other rooted move is custody this lane cannot account for.
    let on_edge = |event: &crate::flow::DiscoveredMoveEvent| {
        matches!(
            event.source,
            FlowOwnershipEventSource::Statement {
                statement_index
            } if statement_index == transition_ordinal
        ) || matches!(
            event.source,
            FlowOwnershipEventSource::Call {
                statement_index,
                target_symbol,
                ..
            } if statement_index == transition_ordinal && target_symbol == path.symbol
        )
    };
    for event in &moves {
        let facts::PlaceRoot::Symbol(root) = event.root else {
            return None;
        };
        if root == source_parameter.symbol {
            if !on_edge(event) {
                return None;
            }
        } else if local_roots.contains(&root) {
            if !matches!(
                event.source,
                FlowOwnershipEventSource::Call {
                    statement_index,
                    ..
                } if statement_index < transition_ordinal
            ) {
                return None;
            }
        } else {
            return None;
        }
    }
    let edge_moves = moves
        .iter()
        .filter(|event| on_edge(event))
        .collect::<Vec<_>>();
    let [edge_move] = edge_moves.as_slice() else {
        return None;
    };
    if edge_move.root != facts::PlaceRoot::Symbol(source_parameter.symbol)
        || segments.span_or_empty(edge_move.segments) != argument_place.segments
    {
        return None;
    }

    // Each construction-local dying root contributes its own projected
    // residual on the same edge. The ledger must carry exactly one
    // establishment, one projected call consumption, and one exit drop per
    // temporary; more or less custody keeps the edge fail-closed.
    let mut local_residuals = Vec::new();
    for (symbol, type_reference, index) in locals.iter().rev() {
        let expected_provenance = PermissionProvenance::Established {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            source: PermissionEventSource::Statement {
                statement_index: *index,
            },
        };
        let mut transfers = Vec::new();
        let mut established = false;
        let mut dropped = false;
        for (_, event) in facts
            .flow
            .ownership
            .permissions
            .iter()
            .filter(|(_, event)| {
                event.machine_symbol == machine.symbol
                    && event.state_symbol == state.symbol
                    && event.root == facts::PlaceRoot::Symbol(*symbol)
            })
        {
            let exact = event.access == PermissionAccess::Owned
                && event.multiplicity == Multiplicity::Affine
                && event.claim_identity == PermissionClaimIdentity::Unknown
                && event.provenance == expected_provenance
                && !event.obligation_live;
            match event.kind {
                PermissionEventKind::Establish => {
                    if !exact
                        || established
                        || event.source
                            != (PermissionEventSource::Statement {
                                statement_index: *index,
                            })
                        || !facts
                            .flow
                            .ownership
                            .segments
                            .span_or_empty(event.segments)
                            .is_empty()
                    {
                        return None;
                    }
                    established = true;
                }
                PermissionEventKind::Transfer => {
                    let PermissionEventSource::Call {
                        statement_index, ..
                    } = event.source
                    else {
                        return None;
                    };
                    let moved_segments =
                        facts.flow.ownership.segments.span_or_empty(event.segments);
                    if !exact || statement_index >= transition_ordinal || moved_segments.is_empty()
                    {
                        return None;
                    }
                    transfers.push((statement_index, moved_segments.to_vec()));
                }
                PermissionEventKind::AffineDrop => {
                    if !exact
                        || dropped
                        || event.source != PermissionEventSource::StateExit
                        || !facts
                            .flow
                            .ownership
                            .segments
                            .span_or_empty(event.segments)
                            .is_empty()
                    {
                        return None;
                    }
                    dropped = true;
                }
                _ => return None,
            }
        }
        if !established || !dropped {
            return None;
        }
        let [(transfer_ordinal, moved_segments)] = transfers.as_slice() else {
            return None;
        };
        let place = crate::flow::CanonicalPlace {
            root: facts::PlaceRoot::Symbol(*symbol),
            segments: moved_segments.clone(),
        };
        let (moved_type, moved_path) = super::terminal_unit::calls::projected_argument_path(
            program,
            state.symbol,
            *transfer_ordinal,
            &place,
        )?;
        // The callee's parameter already typed this argument, so the moved
        // leaf's own identity stands in as the transfer target shape.
        let (_, mut residuals) = super::terminal_unit::types::projected_move_residuals(
            program,
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                symbol: *symbol,
            },
            *type_reference,
            &moved_path,
            moved_type,
            moved_type,
        )?;
        local_residuals.append(&mut residuals);
    }

    // The original direct-field cohort keeps its dedicated validator: one
    // whole-field move out of a two-field record, one maximal sibling. Wider
    // exact paths and roots rely on the shared residual complement instead.
    let narrow = match argument_place.segments.as_slice() {
        [
            facts::PlaceSegment::Field {
                symbol: moved_field,
            },
        ] => super::terminal_unit::exact_two_field_record_projection(
            program,
            source_parameter.type_reference,
            *moved_field,
            target_parameter.type_reference,
        ),
        _ => None,
    };
    let (moved_type_identity, parameter_residuals) = match narrow {
        Some((_, moved_type_identity, residual_field_identity, residual_type_identity)) => (
            moved_type_identity,
            vec![CheckedUnitPartialAffineDiscardPlan {
                source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index: 0,
                },
                path: vec![CheckedUnitStructuralPathSegment::Field(
                    residual_field_identity,
                )],
                type_identity: residual_type_identity,
            }],
        ),
        None => {
            // The root stays a closed affine record or array: generic,
            // carried, or destructor-bearing storage cannot partition its
            // residual custody under this row's exact vocabulary.
            match program
                .type_reference_table
                .type_reference(source_parameter.type_reference)
            {
                typed_trees::types::TypeReferenceNode::Named { symbol, .. }
                | typed_trees::types::TypeReferenceNode::Generic {
                    base_symbol: symbol,
                    ..
                } => {
                    let root = program
                        .data_definitions()
                        .iter()
                        .find(|data| data.symbol == *symbol)?;
                    if root.properties.carry.is_some()
                        || !root.lifetime_parameters.is_empty()
                        || !program.data_type_parameters(root).is_empty()
                    {
                        return None;
                    }
                }
                typed_trees::types::TypeReferenceNode::FixedArray { .. } => {}
                _ => return None,
            }
            if super::terminal_unit::types::type_graph_requires_nominal_drop(
                program,
                source_parameter.type_reference,
            ) {
                return None;
            }
            super::terminal_unit::types::projected_move_residuals(
                program,
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index: 0,
                },
                source_parameter.type_reference,
                &moved_path,
                moved_type,
                target_parameter.type_reference,
            )?
        }
    };
    // Residual custody dies in reverse establishment order: body temporaries
    // before the entry parameter.
    let mut residual_affine_discards = local_residuals;
    residual_affine_discards.extend(parameter_residuals);
    Some(CheckedStructuralControlProjectedEdgeCleanupPlan {
        machine: machine.symbol,
        state: state.symbol,
        statement_ordinal: u32::try_from(transition_ordinal).ok()?,
        target_state: target.symbol,
        transfer: CheckedStructuralControlProjectedTransferPlan {
            source_parameter_position: 0,
            path: moved_path,
            type_identity: moved_type_identity,
            target_parameter_position: 0,
        },
        residual_affine_discards,
    })
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
            && match program.statement_table.transition_target(transition.target) {
                TransitionTargetNode::SelfTarget => true,
                TransitionTargetNode::Named { path, .. } => {
                    crate::checks::termination::named_transition_target_state_index(
                        program,
                        machine,
                        path.symbol,
                    )
                    .is_some()
                        || crate::semantic::calls::find_machine_by_entry_state(program, path.symbol)
                            .is_some()
                }
                _ => false,
            }
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
        let (target, call_target, implicit_arguments) =
            match program.statement_table.transition_target(transition.target) {
                TransitionTargetNode::SelfTarget => (state, state.symbol, true),
                TransitionTargetNode::Named { path, .. } => {
                    let target =
                        match crate::checks::termination::named_transition_target_state_index(
                            program,
                            machine,
                            path.symbol,
                        ) {
                            Some(index) => &program.machine_states(machine)[index],
                            None => match crate::semantic::calls::find_machine_by_entry_state(
                                program,
                                path.symbol,
                            ) {
                                Some((_, entry)) => entry,
                                None => continue,
                            },
                        };
                    (target, path.symbol, false)
                }
                _ => continue,
            };

        // An implicit self edge forwards each current parameter to its same
        // declaration. Those carriers remain owned by the next activation.
        let mut transferred = if implicit_arguments {
            discard_parameters
                .iter()
                .map(|(_, position)| *position)
                .collect::<BTreeSet<_>>()
        } else {
            BTreeSet::new()
        };
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
                } if source_index == statement_index && target_symbol == call_target
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
                // The case-payload operation materializes a fresh plain
                // carrier before the edge retires its parent storage. It is
                // not a transfer of the whole parent. Keep that parent in
                // the ordinary discard roster; actual partial moves still
                // require their separate residual-complement evidence.
                plain_case_payload_copy(
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

/// Rejoin the plain whole payload the existing case-leaf operation copies.
/// A case test alone never consumes its subject: the source move chooses the
/// payload's next owner, while this physical route retains parent storage
/// until the edge's ordinary cleanup. Claims, nominal drops and general
/// projected moves cannot use that no-code retirement.
fn plain_case_payload_copy(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    statement_index: usize,
    transition: &typed_trees::statement::TableTransition,
    root: symbols::SymbolHandle,
    segments: &[facts::PlaceSegment],
) -> Option<()> {
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
    let parameter = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == root)?;
    let payload = crate::flow::project_type_reference_from_segments(
        program,
        parameter.type_reference,
        segments,
    )?;
    (matches!(
        program.type_multiplicity(payload),
        Multiplicity::Affine | Multiplicity::Unrestricted
    ) && validation::has_plain_owned_contents_with_numeric_constraints(program, payload)
        && !super::terminal_unit::types::type_graph_requires_nominal_drop(program, payload))
    .then_some(())
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
            PermissionEventSource::Call { target_symbol, .. } => program
                .machines()
                .iter()
                .any(|candidate| candidate.symbol == target_symbol),
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
            if parameter.is_self
                || crate::checks::type_multiplicity(program, parameter.type_reference)
                    != Multiplicity::Affine
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
    for (_, event) in facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == machine
                && event.state_symbol == state.symbol
                && event.source == PermissionEventSource::StateExit
                && event.kind == PermissionEventKind::AffineDrop
        })
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
            !parameter.is_self
                && parameter.symbol == root
                && crate::checks::type_multiplicity(program, parameter.type_reference)
                    == Multiplicity::Affine
        })?;
        let position = u32::try_from(position).ok()?;
        if discard_parameters.contains(&(root, position)) {
            return None;
        }
        discard_parameters.push((root, position));
    }
    if discard_parameters != expected_discard_parameters {
        return None;
    }
    Some(discard_parameters)
}

/// Separate exact local result obligations from the parameter cleanup roster.
/// Each ordinary successor either transfers the whole local or retains its
/// state-exit disposal. The graph producer rejoins that partition to the actual
/// result binding; this row does not turn local results into parameters.
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
    let scalar_result = program
        .primitive_type_reference(state.return_type)
        .is_some();
    let mut results = Vec::new();
    for (_, drop) in permissions.permissions.iter().filter(|(_, event)| {
        event.machine_symbol == machine.symbol
            && event.state_symbol == state.symbol
            && event.source == PermissionEventSource::StateExit
            && event.kind == PermissionEventKind::AffineDrop
    }) {
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
            || !(validation::has_plain_owned_contents_with_numeric_constraints(
                program,
                local.type_reference,
            ) || validation::has_owned_or_shared_view_fields(program, local.type_reference))
            || super::terminal_unit::types::type_graph_requires_nominal_drop(
                program,
                local.type_reference,
            )
        {
            return None;
        }
        if statements[..=statement_index]
            .iter()
            .any(|statement| matches!(statement, StatementNode::Transition(_)))
        {
            return None;
        }
        let mut edge_count = 0;
        let mut transfer_count = 0;
        for (ordinal, statement) in statements.iter().enumerate().skip(statement_index + 1) {
            let StatementNode::Transition(transition) = statement else {
                // A scalar tail observes locals but cannot return their
                // structural ownership. Its disposal is the same as an
                // explicit scalar transition target below.
                let scalar_tail = scalar_result && ordinal + 1 == statements.len();
                if matches!(statement, StatementNode::Expression(expression) if !scalar_tail && !matches!(program.expression_table.expression(*expression), typed_trees::expression::ExpressionNode::Call(_)))
                {
                    return None;
                }
                continue;
            };
            if transition.exit != TransitionExit::Ordinary || transition.continuation.is_valid() {
                return None;
            }
            // Unit and scalar completions retain this structural local for
            // ordinary exit disposal; only named edges may transfer it here.
            // Returning a structural value needs its own custody, not disposal.
            // Count normal completion as a real exit, keeping the exact
            // transfer-ledger comparison below: a fabricated
            // move on a returning edge must still reject the complete roster.
            let normal_completion =
                match program.statement_table.transition_target(transition.target) {
                    TransitionTargetNode::Terminal => {
                        super::terminal_unit::cleanup_type_is_unit(program, state.return_type)
                    }
                    TransitionTargetNode::Value(_) => scalar_result,
                    _ => false,
                };
            if normal_completion {
                edge_count += 1;
                continue;
            }
            let TransitionTargetNode::Named {
                path, arguments, ..
            } = program.statement_table.transition_target(transition.target)
            else {
                return None;
            };
            let target_index = crate::checks::termination::named_transition_target_state_index(
                program,
                machine,
                path.symbol,
            )?;
            let target = &program.machine_states(machine)[target_index];
            let arguments = program.statement_table.expression_handles(*arguments);
            let target_parameters = program
                .state_parameters(target)
                .iter()
                .filter(|parameter| !parameter.is_self);
            if target_parameters.clone().count() != arguments.len() {
                return None;
            }
            let mut matching = arguments
                .iter()
                .zip(target_parameters)
                .filter(|(argument, _)| {
                    let typed_trees::expression::ExpressionNode::Name(name) =
                        program.expression_table.expression(**argument)
                    else {
                        return false;
                    };
                    name.symbol == symbol
                        && name.head_symbol == symbol
                        && program
                            .expression_table
                            .name_path_members(name.members)
                            .len()
                            == 1
                });
            edge_count += 1;
            // An operand call of this edge may take the local whole on the
            // edge alone; its ledger row sits at that call's occurrence,
            // after selection. Each reconstructed occurrence must meet its
            // one exact Transfer row; sibling edges keep the exit drop.
            let operand_moves = validation::transition_operand_moves(
                program,
                facts,
                machine.symbol,
                state,
                ordinal,
                symbol,
            )?;
            for source in &operand_moves {
                let mut rows = permissions.permissions.iter().filter(|(_, event)| {
                    event.machine_symbol == machine.symbol
                        && event.state_symbol == state.symbol
                        && event.root == drop.root
                        && event.kind == PermissionEventKind::Transfer
                        && event.access == PermissionAccess::Owned
                        && event.source == *source
                });
                let (_, row) = rows.next()?;
                if rows.next().is_some() || !exact(row) {
                    return None;
                }
                transfer_count += 1;
            }
            let Some((_, parameter)) = matching.next() else {
                continue;
            };
            if !operand_moves.is_empty() {
                return None;
            }
            if matching.next().is_some()
                || program.normalized_type_identity(parameter.type_reference)
                    != program.normalized_type_identity(local.type_reference)
            {
                return None;
            }
            let mut transfers = permissions.permissions.iter().filter(|(_, event)| {
                event.machine_symbol == machine.symbol
                    && event.state_symbol == state.symbol
                    && event.root == drop.root
                    && event.kind == PermissionEventKind::Transfer
                    && event.access == PermissionAccess::Owned
                    && matches!(event.source,
                        PermissionEventSource::Call { statement_index, .. }
                        | PermissionEventSource::Statement { statement_index }
                        if statement_index == ordinal)
            });
            let (_, transfer) = transfers.next()?;
            if transfers.next().is_some()
                || !exact(transfer)
                || transfer.source
                    != (PermissionEventSource::Call {
                        statement_index: ordinal,
                        call_ordinal: 0,
                        target_symbol: path.symbol,
                    })
            {
                return None;
            }
            transfer_count += 1;
        }
        // Shared and exclusive loans record Transfer events too, but they
        // lend access rather than move the owned home — only Owned transfers
        // consume the local before its exit drop.
        let retained_transfers = permissions
            .permissions
            .iter()
            .filter(|(_, event)| {
                event.machine_symbol == machine.symbol
                    && event.state_symbol == state.symbol
                    && event.root == drop.root
                    && event.kind == PermissionEventKind::Transfer
                    && event.access == PermissionAccess::Owned
            })
            .count();
        // Unit and scalar body completions retire the exact lexical remainder
        // without transferring it to a successor. Structural returns do not
        // acquire disposal authority from the absence of a transition.
        let normal_body_return = (matches!(
            program
                .type_reference_table
                .type_reference(state.return_type),
            typed_trees::types::TypeReferenceNode::Unit
        ) || (scalar_result
            && matches!(statements.last(), Some(StatementNode::Expression(_)))))
            && !statements
                .iter()
                .any(|statement| matches!(statement, StatementNode::Transition(_)));
        if (edge_count == 0 && !normal_body_return) || retained_transfers != transfer_count {
            return None;
        }
        if permissions.permissions.iter().any(|(_, event)| {
            event.machine_symbol == machine.symbol
                && event.state_symbol == state.symbol
                && event.root == drop.root
                // Establishing, transferring and ending a loan may name this
                // same storage root. Those access claims are checked by the
                // borrow ledger; they neither establish nor consume the owned
                // local whose exit disposal this roster accounts for.
                && event.access == PermissionAccess::Owned
                && (!exact(event)
                    || (event.kind == PermissionEventKind::AffineDrop
                        && event.source != PermissionEventSource::StateExit)
                    || !matches!(
                        event.kind,
                        PermissionEventKind::Establish
                            | PermissionEventKind::Transfer
                            | PermissionEventKind::AffineDrop
                    ))
        }) {
            return None;
        }
        results.push(symbol);
    }
    Some(results)
}
