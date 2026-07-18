use omega_checked_trees::{CheckFacts, FlowOwnershipEventSource, FlowPermissionEventFact};
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_core::semantics::{
    Multiplicity, PermissionAccess, PermissionEventKind, PermissionEventSource,
    PermissionProvenance,
};
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::statement::StatementNode;
use omega_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

#[derive(Debug, Clone)]
struct LinearPlace {
    symbol: SymbolHandle,
    name: String,
    multiplicity: Multiplicity,
    provenance: Option<PermissionProvenance>,
    live: bool,
    /// Parameters are established on entry. A local is established only by an
    /// explicit initializer/assignment; implicit zero-fill creates no debt.
    ever_established: bool,
    /// An affine sum can carry a linear payload only in selected cases. When
    /// false, `live` is unconditional for every established value; when true,
    /// `live` follows the active case.
    conditional: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WrittenLinearTarget {
    root: omega_facts::PlaceRoot,
    obligation_live: bool,
    provenance: Option<PermissionProvenance>,
}

pub(crate) fn check_linear_obligations(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    record_permission_events(program, facts);
    validate_linear_permission_events(program, facts)
}

pub(crate) fn record_permission_events(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut CheckFacts,
) {
    let mut permission_events = Vec::new();

    let state_flows = facts
        .flow
        .control
        .states
        .iter()
        .map(|(_, state)| state.clone())
        .collect::<Vec<_>>();
    for state_flow in state_flows {
        let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == state_flow.machine_symbol)
        else {
            continue;
        };
        let Some(state) = crate::find_state(program, state_flow.state_symbol) else {
            continue;
        };
        let statements = program.statement_table.statements(state.statement_nodes);
        let mut places = initial_linear_places(
            program,
            state,
            state_flow.machine_symbol,
            state.symbol,
        );

        for place in places.iter().filter(|place| place.ever_established) {
                permission_events.push(FlowPermissionEventFact {
                    machine_symbol: state_flow.machine_symbol,
                    state_symbol: state.symbol,
                    source: PermissionEventSource::StateEntry,
                    kind: PermissionEventKind::Establish,
                    multiplicity: place.multiplicity,
                    access: PermissionAccess::Owned,
                    provenance: place.provenance.expect("entry place has provenance"),
                    root: omega_facts::PlaceRoot::Symbol(place.symbol),
                    segments: HandleSpan::empty(),
                    obligation_live: true,
                });
        }

        let moves = crate::flow::discover_state_move_events(
            program,
            &facts.borrow,
            machine,
            state,
            &mut facts.flow.ownership.segments,
        );
        let first_transition = statements
            .iter()
            .position(|statement| matches!(statement, StatementNode::Transition(_)));
        let prefix_end = first_transition.unwrap_or(statements.len());
        for (statement_index, statement) in statements[..prefix_end].iter().enumerate() {
            apply_statement_permission_production(
                program,
                facts,
                state_flow.machine_symbol,
                state.symbol,
                &moves,
                statement_index,
                statement,
                &mut places,
                &mut permission_events,
            );
        }

        if let Some(first_transition) = first_transition {
            let entry = places.clone();
            let arm_indices = (first_transition..statements.len())
                .filter(|index| matches!(statements[*index], StatementNode::Transition(_)))
                .collect::<Vec<_>>();
            for statement_index in arm_indices.iter().copied() {
                let mut outcome = entry.clone();
                apply_statement_permission_production(
                    program,
                    facts,
                    state_flow.machine_symbol,
                    state.symbol,
                    &moves,
                    statement_index,
                    &statements[statement_index],
                    &mut outcome,
                    &mut permission_events,
                );
            }
        }

        append_affine_cleanup_permission_events(
            program,
            state,
            state_flow.machine_symbol,
            &places,
            &mut permission_events,
        );
    }

    append_borrow_permission_events(facts, &mut permission_events);

    facts.flow.ownership.permissions = omega_core::arena::Arena::default();
    facts
        .flow
        .ownership
        .permissions
        .insert_many(permission_events);
}

pub(crate) fn validate_linear_permission_events(
    program: &omega_typed_trees::TypedTrees,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    for (_, state_flow) in facts.flow.control.states.iter() {
        let Some(state) = crate::find_state(program, state_flow.state_symbol) else {
            continue;
        };
        let statements = program.statement_table.statements(state.statement_nodes);
        let mut places = initial_linear_places(
            program,
            state,
            state_flow.machine_symbol,
            state.symbol,
        );
        let events = facts
            .flow
            .ownership
            .permissions
            .iter()
            .filter_map(|(_, event)| {
                (event.machine_symbol == state_flow.machine_symbol
                    && event.state_symbol == state.symbol
                    && event.access == PermissionAccess::Owned)
                    .then_some(event)
            })
            .collect::<Vec<_>>();

        let first_transition = statements
            .iter()
            .position(|statement| matches!(statement, StatementNode::Transition(_)));
        let prefix_end = first_transition.unwrap_or(statements.len());
        for statement_index in 0..prefix_end {
            apply_recorded_statement_events(
                statement_index,
                &events,
                &mut places,
                &mut diagnostics,
            );
        }

        let mut mixed_places = Vec::new();
        if let Some(first_transition) = first_transition {
            let entry = places.clone();
            let arm_indices = (first_transition..statements.len())
                .filter(|index| matches!(statements[*index], StatementNode::Transition(_)))
                .collect::<Vec<_>>();
            let mut outcomes = Vec::new();
            for statement_index in arm_indices.iter().copied() {
                let mut outcome = entry.clone();
                apply_recorded_statement_events(
                    statement_index,
                    &events,
                    &mut outcome,
                    &mut diagnostics,
                );
                outcomes.push(outcome);
            }
            let exhaustive = arm_indices.last().is_some_and(|index| {
                matches!(
                    statements[*index],
                    StatementNode::Transition(omega_typed_trees::statement::TableTransition {
                        guard: omega_typed_trees::statement::TransitionGuardNode::Always,
                        ..
                    })
                )
            });
            if !exhaustive {
                outcomes.push(entry);
            }

            if let Some(first) = outcomes.first() {
                for place_index in 0..places.len() {
                    let live = first[place_index].live;
                    if outcomes
                        .iter()
                        .skip(1)
                        .any(|outcome| outcome[place_index].live != live)
                    {
                        diagnostics.push(Diagnostic::error(format!(
                            "linear value `{}` has inconsistent treatment across transition arms; every path must consume/transfer it or every path must preserve the same live obligation",
                            places[place_index].name
                        )));
                        mixed_places.push(places[place_index].symbol);
                    } else {
                        places[place_index].live = live;
                        places[place_index].ever_established = outcomes
                            .iter()
                            .any(|outcome| outcome[place_index].ever_established);
                    }
                }
            }
        }

        for place in places
            .iter()
            .filter(|place| place.live && !mixed_places.contains(&place.symbol))
        {
            diagnostics.push(Diagnostic::error(format!(
                "linear value `{}` reaches scope exit without being consumed or transferred",
                place.name
            )));
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn initial_linear_places(
    program: &omega_typed_trees::TypedTrees,
    state: &omega_typed_trees::state::State,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> Vec<LinearPlace> {
    let mut places = Vec::new();
    for parameter in program.state_parameters(state) {
        // A by-value `self` parameter is the language's terminal-consumer
        // form. The caller owns the consumption judgment.
        if parameter.is_self {
            continue;
        }
        let multiplicity = type_multiplicity(program, parameter.type_reference);
        let conditional = type_has_conditional_linear_payload(program, parameter.type_reference);
        if multiplicity == Multiplicity::Linear || conditional {
            places.push(LinearPlace {
                symbol: parameter.symbol,
                name: parameter.name.as_str().to_owned(),
                multiplicity,
                provenance: Some(established_provenance(
                    machine_symbol,
                    state_symbol,
                    PermissionEventSource::StateEntry,
                )),
                live: true,
                ever_established: true,
                conditional,
            });
        }
    }
    for statement in program.statement_table.statements(state.statement_nodes) {
        let StatementNode::LocalData(local) = statement else {
            continue;
        };
        let multiplicity = type_multiplicity(program, local.type_reference);
        let conditional = type_has_conditional_linear_payload(program, local.type_reference);
        if multiplicity == Multiplicity::Linear || conditional {
            places.push(LinearPlace {
                symbol: local.symbol,
                name: local.name.as_str().to_owned(),
                multiplicity,
                provenance: None,
                live: false,
                ever_established: false,
                conditional,
            });
        }
    }
    places
}

fn apply_recorded_statement_events(
    statement_index: usize,
    events: &[&FlowPermissionEventFact],
    places: &mut [LinearPlace],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for event in events.iter().copied().filter(|event| {
        permission_event_statement_index(event.source) == Some(statement_index)
            && event.kind != PermissionEventKind::AffineDrop
    }) {
        let omega_facts::PlaceRoot::Symbol(symbol) = event.root else {
            continue;
        };
        let Some(place) = places.iter_mut().find(|place| place.symbol == symbol) else {
            continue;
        };
        match event.kind {
            PermissionEventKind::Transfer | PermissionEventKind::Consume => {
                if !place.ever_established {
                    diagnostics.push(Diagnostic::error(format!(
                        "linear value `{}` has not been established (implicit zero-fill creates no linear obligation); it cannot be moved here",
                        place.name
                    )));
                } else if !place.live && !place.conditional {
                    diagnostics.push(Diagnostic::error(format!(
                        "linear value `{}` was already transferred or consumed; it cannot be moved here",
                        place.name
                    )));
                } else {
                    place.live = false;
                }
            }
            PermissionEventKind::Establish => {
                if place.live {
                    diagnostics.push(Diagnostic::error(format!(
                        "assignment would overwrite live linear value `{}`; consume or transfer the existing obligation first",
                        place.name
                    )));
                }
                place.live = event.obligation_live;
                place.ever_established = true;
                place.provenance = Some(event.provenance);
            }
            PermissionEventKind::AffineDrop => {}
        }
    }
}

fn permission_event_statement_index(source: PermissionEventSource) -> Option<usize> {
    match source {
        PermissionEventSource::Statement { statement_index }
        | PermissionEventSource::Call {
            statement_index, ..
        } => Some(statement_index),
        PermissionEventSource::StateEntry | PermissionEventSource::StateExit => None,
    }
}

fn append_borrow_permission_events(
    facts: &mut CheckFacts,
    permission_events: &mut Vec<FlowPermissionEventFact>,
) {
    // Clone only the small state/span index so the ownership-segment arena can
    // be extended while the already-built borrow facts remain immutable.
    let states = facts
        .flow
        .control
        .states
        .iter()
        .map(|(_, state)| {
            (
                state.machine_symbol,
                state.state_symbol,
                state.borrow_activations,
                state.borrow_weakenings,
            )
        })
        .collect::<Vec<_>>();

    for (machine_symbol, state_symbol, activations, weakenings) in states {
        for activation in facts
            .flow
            .borrow_lifetimes
            .activations
            .span_or_empty(activations)
            .to_vec()
        {
            append_borrow_permission_event(
                facts,
                permission_events,
                machine_symbol,
                state_symbol,
                activation.loan,
                permission_source_from_invalidation(activation.source),
                PermissionEventKind::Establish,
            );
        }
        for weakening in facts
            .flow
            .borrow_lifetimes
            .weakenings
            .span_or_empty(weakenings)
            .to_vec()
        {
            let source = if weakening.reason
                == omega_checked_trees::FlowBorrowWeakeningReason::StateExit
            {
                PermissionEventSource::StateExit
            } else {
                permission_source_from_invalidation(weakening.source)
            };
            append_borrow_permission_event(
                facts,
                permission_events,
                machine_symbol,
                state_symbol,
                weakening.loan,
                source,
                PermissionEventKind::Consume,
            );
        }
    }
}

fn append_borrow_permission_event(
    facts: &mut CheckFacts,
    permission_events: &mut Vec<FlowPermissionEventFact>,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    loan_handle: omega_core::arena::Handle<omega_checked_trees::BorrowLoanFact>,
    source: PermissionEventSource,
    kind: PermissionEventKind,
) {
    let loan = facts.borrow.loans.get(loan_handle).clone();
    let segments = facts
        .flow
        .ownership
        .segments
        .insert_many(facts.borrow.loan_segments(&loan).iter().copied());
    let (multiplicity, access) = match loan.kind {
        omega_checked_trees::BorrowAccessKind::Read => {
            (Multiplicity::Unrestricted, PermissionAccess::Shared)
        }
        omega_checked_trees::BorrowAccessKind::Mutable => {
            (Multiplicity::Affine, PermissionAccess::Exclusive)
        }
    };
    permission_events.push(FlowPermissionEventFact {
        machine_symbol,
        state_symbol,
        source,
        kind,
        multiplicity,
        access,
        provenance: established_provenance(
            machine_symbol,
            state_symbol,
            PermissionEventSource::Statement {
                statement_index: loan.statement_index,
            },
        ),
        root: omega_facts::PlaceRoot::Symbol(loan.root_symbol),
        segments,
        obligation_live: false,
    });
}

fn permission_source_from_invalidation(
    source: omega_checked_trees::FlowInvalidationSource,
) -> PermissionEventSource {
    match source {
        omega_checked_trees::FlowInvalidationSource::Statement { statement_index } => {
            PermissionEventSource::Statement { statement_index }
        }
        omega_checked_trees::FlowInvalidationSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => PermissionEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_statement_permission_production(
    program: &omega_typed_trees::TypedTrees,
    facts: &CheckFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    moves: &[crate::flow::DiscoveredMoveEvent],
    statement_index: usize,
    statement: &StatementNode,
    places: &mut [LinearPlace],
    permission_events: &mut Vec<FlowPermissionEventFact>,
) {
    // Destructure coverage markers are proof-only reads synthesized by the
    // parser; they neither transfer a value nor establish user storage.
    if matches!(statement, StatementNode::LocalData(local) if local.name.as_str().starts_with("__arm_destructure#"))
    {
        return;
    }

    let written_target = written_whole_linear_target(
        program,
        state_symbol,
        statement_index,
        statement,
        places,
    );

    // Moves out of initializer/assignment sources happen before the
    // destination becomes established. The old move-only summary also
    // contains a production event *at* the destination; exclude that
    // compatibility event here rather than mistaking creation for use.
    for event in moves.iter().filter(|event| {
        event_statement_index(event.source) == Some(statement_index)
            && written_target.map(|target| target.root) != Some(event.root)
    }) {
        let omega_facts::PlaceRoot::Symbol(symbol) = event.root else {
            continue;
        };
        let Some(place) = places.iter_mut().find(|place| place.symbol == symbol) else {
            continue;
        };
        let segments = facts.flow.ownership.segments.span_or_empty(event.segments);
        // Whole-place tracking can soundly settle a nested move only for a
        // conditional sum: its live case carries the single payload debt, so
        // extracting that payload transfers the root's obligation. Ordinary
        // linear aggregates need a future per-field resource algebra rather
        // than pretending one field move consumed every sibling.
        if !segments.is_empty() && (!place.conditional || written_target.is_none()) {
            continue;
        }
        let obligation_live = place.live;
        permission_events.push(FlowPermissionEventFact {
            machine_symbol,
            state_symbol,
            source: permission_source(event.source),
            kind: permission_kind_for_move(program, facts, machine_symbol, state_symbol, event),
            multiplicity: place.multiplicity,
            access: PermissionAccess::Owned,
            provenance: place.provenance.unwrap_or(PermissionProvenance::Unknown),
            root: event.root,
            segments: event.segments,
            obligation_live,
        });
        place.live = false;
    }

    if let Some(WrittenLinearTarget {
        root: omega_facts::PlaceRoot::Symbol(symbol),
        obligation_live,
        provenance,
    }) = written_target
    {
        let place = places
            .iter_mut()
            .find(|place| place.symbol == symbol)
            .expect("written linear target came from the tracked place set");
        place.live = obligation_live;
        place.ever_established = true;
        place.provenance = Some(provenance.unwrap_or_else(|| {
            established_provenance(
                machine_symbol,
                state_symbol,
                PermissionEventSource::Statement { statement_index },
            )
        }));
        permission_events.push(FlowPermissionEventFact {
            machine_symbol,
            state_symbol,
            source: PermissionEventSource::Statement { statement_index },
            kind: PermissionEventKind::Establish,
            multiplicity: place.multiplicity,
            access: PermissionAccess::Owned,
            provenance: place
                .provenance
                .expect("an established place has explicit provenance"),
            root: omega_facts::PlaceRoot::Symbol(symbol),
            segments: HandleSpan::empty(),
            obligation_live,
        });
    }
}

fn established_provenance(
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    source: PermissionEventSource,
) -> PermissionProvenance {
    PermissionProvenance::Established {
        machine_symbol,
        state_symbol,
        source,
    }
}

fn event_statement_index(source: FlowOwnershipEventSource) -> Option<usize> {
    match source {
        FlowOwnershipEventSource::Statement { statement_index }
        | FlowOwnershipEventSource::Call {
            statement_index, ..
        } => Some(statement_index),
        FlowOwnershipEventSource::StateExit => None,
    }
}

fn permission_source(source: FlowOwnershipEventSource) -> PermissionEventSource {
    match source {
        FlowOwnershipEventSource::Statement { statement_index } => {
            PermissionEventSource::Statement { statement_index }
        }
        FlowOwnershipEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => PermissionEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        },
        FlowOwnershipEventSource::StateExit => PermissionEventSource::StateExit,
    }
}

fn permission_kind_for_move(
    program: &omega_typed_trees::TypedTrees,
    facts: &CheckFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    event: &crate::flow::DiscoveredMoveEvent,
) -> PermissionEventKind {
    let FlowOwnershipEventSource::Call {
        statement_index,
        call_ordinal,
        target_symbol,
    } = event.source
    else {
        return PermissionEventKind::Transfer;
    };
    let Some(call_site) = crate::find_call_site(
        program,
        machine_symbol,
        state_symbol,
        statement_index,
        call_ordinal,
    ) else {
        return PermissionEventKind::Transfer;
    };
    let Some(target_state) = crate::find_state(program, target_symbol) else {
        return PermissionEventKind::Transfer;
    };
    let arguments = crate::call_site_argument_expressions(program, &call_site);
    let parameters = program.state_parameters(target_state);
    if arguments.len() != parameters.len() {
        return PermissionEventKind::Transfer;
    }
    let event_segments = facts
        .flow
        .ownership
        .segments
        .span_or_empty(event.segments);
    for (parameter, argument) in parameters.iter().zip(arguments) {
        if !parameter.is_self {
            continue;
        }
        let Some(place) = crate::flow::canonical_place_from_expression_in_state(
            program,
            state_symbol,
            statement_index,
            *argument,
        ) else {
            continue;
        };
        if place.root == event.root && place.segments.as_slice() == event_segments {
            return if type_carries_linear_obligation(program, target_state.return_type) {
                PermissionEventKind::Transfer
            } else {
                PermissionEventKind::Consume
            };
        }
    }
    PermissionEventKind::Transfer
}

fn written_whole_linear_target(
    program: &omega_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
    places: &[LinearPlace],
) -> Option<WrittenLinearTarget> {
    match statement {
        StatementNode::LocalData(local) => {
            let tracked = places.iter().find(|place| place.symbol == local.symbol)?;
            local.initial_value.is_valid().then(|| WrittenLinearTarget {
                root: omega_facts::PlaceRoot::Symbol(local.symbol),
                obligation_live: expression_establishes_obligation(
                    program,
                    state_symbol,
                    statement_index,
                    local.initial_value,
                    tracked.conditional,
                    places,
                ),
                provenance: expression_permission_provenance(
                    program,
                    state_symbol,
                    statement_index,
                    local.initial_value,
                    places,
                ),
            })
        }
        StatementNode::Assignment(assignment) => {
            let place = crate::flow::canonical_place_from_expression_in_state(
                program,
                state_symbol,
                statement_index,
                assignment.target,
            )?;
            if !place.segments.is_empty() {
                return None;
            }
            let omega_facts::PlaceRoot::Symbol(symbol) = place.root else {
                return None;
            };
            let tracked = places.iter().find(|tracked| tracked.symbol == symbol)?;
            Some(WrittenLinearTarget {
                root: place.root,
                obligation_live: expression_establishes_obligation(
                    program,
                    state_symbol,
                    statement_index,
                    assignment.value,
                    tracked.conditional,
                    places,
                ),
                provenance: expression_permission_provenance(
                    program,
                    state_symbol,
                    statement_index,
                    assignment.value,
                    places,
                ),
            })
        }
        _ => None,
    }
}

fn expression_permission_provenance(
    program: &omega_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: omega_typed_trees::expression::ExpressionHandle,
    places: &[LinearPlace],
) -> Option<PermissionProvenance> {
    match program.expression_table.expression(expression) {
        omega_typed_trees::expression::ExpressionNode::Call(call) => {
            let mut candidates = Vec::new();
            if call.receiver.is_valid() {
                candidates.push(call.receiver);
            }
            candidates
                .extend_from_slice(program.expression_table.expression_handles(call.arguments));
            let origins = candidates.into_iter().filter_map(|candidate| {
                expression_permission_provenance(
                    program,
                    state_symbol,
                    statement_index,
                    candidate,
                    places,
                )
            });
            return common_permission_provenance(origins);
        }
        omega_typed_trees::expression::ExpressionNode::StructLiteral(literal) => {
            let origins = program
                .expression_table
                .struct_fields(literal.fields)
                .iter()
                .filter_map(|field| {
                    expression_permission_provenance(
                        program,
                        state_symbol,
                        statement_index,
                        field.value,
                        places,
                    )
                });
            return common_permission_provenance(origins);
        }
        _ => {}
    }

    let source = crate::flow::canonical_place_from_expression_in_state(
        program,
        state_symbol,
        statement_index,
        expression,
    )?;
    let omega_facts::PlaceRoot::Symbol(symbol) = source.root else {
        return None;
    };
    places
        .iter()
        .find(|place| place.symbol == symbol && (source.segments.is_empty() || place.conditional))
        .and_then(|place| place.provenance)
}

/// Discover ordinary affine cleanup directly from typed ownership rather than
/// projecting it back out of the compatibility `drops` arena. Locals drop in
/// reverse declaration order, followed by owned by-value parameters in reverse
/// declaration order, exactly matching the language's cleanup order. Linear
/// and conditional roots are excluded because their path-sensitive settlement
/// is represented by the permission events produced above.
fn append_affine_cleanup_permission_events(
    program: &omega_typed_trees::TypedTrees,
    state: &omega_typed_trees::state::State,
    machine_symbol: SymbolHandle,
    tracked_places: &[LinearPlace],
    permission_events: &mut Vec<FlowPermissionEventFact>,
) {
    let mut append = |symbol: SymbolHandle, type_reference: TypeReferenceHandle| {
        if tracked_places.iter().any(|place| place.symbol == symbol)
            || type_multiplicity(program, type_reference) == Multiplicity::Unrestricted
        {
            return;
        }
        permission_events.push(FlowPermissionEventFact {
            machine_symbol,
            state_symbol: state.symbol,
            source: PermissionEventSource::StateExit,
            kind: PermissionEventKind::AffineDrop,
            multiplicity: Multiplicity::Affine,
            access: PermissionAccess::Owned,
            provenance: PermissionProvenance::Unknown,
            root: omega_facts::PlaceRoot::Symbol(symbol),
            segments: HandleSpan::empty(),
            obligation_live: false,
        });
    };

    for statement in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .rev()
    {
        if let StatementNode::LocalData(local) = statement {
            append(local.symbol, local.type_reference);
        }
    }
    for parameter in program.state_parameters(state).iter().rev() {
        if !parameter.is_self {
            append(parameter.symbol, parameter.type_reference);
        }
    }
}

fn common_permission_provenance(
    mut origins: impl Iterator<Item = PermissionProvenance>,
) -> Option<PermissionProvenance> {
    let first = origins.next()?;
    origins.all(|origin| origin == first).then_some(first)
}

fn type_carries_linear_obligation(
    program: &omega_typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    type_multiplicity(program, type_reference) == Multiplicity::Linear
        || type_has_conditional_linear_payload(program, type_reference)
}

fn expression_establishes_obligation(
    program: &omega_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: omega_typed_trees::expression::ExpressionHandle,
    conditional: bool,
    places: &[LinearPlace],
) -> bool {
    if !conditional {
        return true;
    }

    if let Some(source) = crate::flow::canonical_place_from_expression_in_state(
        program,
        state_symbol,
        statement_index,
        expression,
    ) && source.segments.is_empty()
        && let omega_facts::PlaceRoot::Symbol(symbol) = source.root
        && let Some(place) = places.iter().find(|place| place.symbol == symbol)
    {
        return place.live;
    }

    if let omega_typed_trees::expression::ExpressionNode::Name(path) =
        program.expression_table.expression(expression)
        && let Some(variant) = program.data_definitions().iter().find_map(|definition| {
            program.data_members(definition).iter().find_map(|member| match member {
                omega_typed_trees::data::DataMember::Variant(variant)
                    if variant.symbol == path.symbol =>
                {
                    Some(variant)
                }
                _ => None,
            })
        })
    {
        return variant_carries_linear_obligation(program, variant);
    }

    let omega_typed_trees::expression::ExpressionNode::StructLiteral(literal) =
        program.expression_table.expression(expression)
    else {
        // A call/boundary result has an unknown active case. Conservatively
        // retain a possible obligation until result-case narrowing lands.
        return true;
    };
    let Some(case_name) = literal.case_name.as_ref() else {
        return true;
    };
    let Some(definition) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == literal.type_name.as_str())
    else {
        return true;
    };
    let Some(variant) = program.data_members(definition).iter().find_map(|member| match member {
        omega_typed_trees::data::DataMember::Variant(variant)
            if variant.name.as_str() == case_name.as_str() =>
        {
            Some(variant)
        }
        _ => None,
    }) else {
        return true;
    };

    variant_carries_linear_obligation(program, variant)
}

fn variant_carries_linear_obligation(
    program: &omega_typed_trees::TypedTrees,
    variant: &omega_typed_trees::data::DataVariant,
) -> bool {
    program.data_payload_fields(variant).iter().any(|field| {
        type_multiplicity(program, field.type_reference) == Multiplicity::Linear
            || type_has_conditional_linear_payload(program, field.type_reference)
    })
}

fn type_multiplicity(
    program: &omega_typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Multiplicity {
    if !type_reference.is_valid() {
        return Multiplicity::Affine;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { .. } | TypeReferenceNode::Unit => {
            Multiplicity::Unrestricted
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_multiplicity(program, *base_type)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            type_multiplicity(program, *element_type)
        }
        TypeReferenceNode::Named { name, .. } => {
            if omega_typed_trees::types::PrimitiveType::from_name(name.as_str()).is_some() {
                return Multiplicity::Unrestricted;
            }
            program
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == name.as_str())
                .map(|definition| definition.properties.multiplicity)
                .unwrap_or(Multiplicity::Affine)
        }
        TypeReferenceNode::Generic { base_name, .. } => program
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == base_name.as_str())
            .map(|definition| definition.properties.multiplicity)
            .unwrap_or(Multiplicity::Affine),
        TypeReferenceNode::DynamicTrait { .. } | TypeReferenceNode::Slice { .. } => {
            Multiplicity::Affine
        }
    }
}

fn type_has_conditional_linear_payload(
    program: &omega_typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    conditional_linear_payload_inner(program, type_reference, &mut Vec::new())
}

fn conditional_linear_payload_inner(
    program: &omega_typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
    visiting: &mut Vec<String>,
) -> bool {
    if !type_reference.is_valid() {
        return false;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            conditional_linear_payload_inner(program, *base_type, visiting)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            conditional_linear_payload_inner(program, *element_type, visiting)
        }
        TypeReferenceNode::Named { name, .. } => {
            conditional_linear_payload_named(program, name.as_str(), visiting)
        }
        TypeReferenceNode::Generic { base_name, .. } => {
            conditional_linear_payload_named(program, base_name.as_str(), visiting)
        }
        _ => false,
    }
}

fn conditional_linear_payload_named(
    program: &omega_typed_trees::TypedTrees,
    name: &str,
    visiting: &mut Vec<String>,
) -> bool {
    if visiting.iter().any(|active| active == name) {
        return false;
    }
    let Some(definition) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == name)
    else {
        return false;
    };
    if definition.properties.multiplicity == Multiplicity::Linear {
        return false;
    }
    visiting.push(name.to_owned());
    let result = program.data_members(definition).iter().any(|member| {
        let omega_typed_trees::data::DataMember::Variant(variant) = member else {
            return false;
        };
        program.data_payload_fields(variant).iter().any(|field| {
            type_multiplicity(program, field.type_reference) == Multiplicity::Linear
                || conditional_linear_payload_inner(program, field.type_reference, visiting)
        })
    });
    visiting.pop();
    result
}
