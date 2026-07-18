use omega_checked_trees::{
    CheckFacts, FlowOwnershipEventSource, FlowPermissionEventFact, FlowPermissionEventKind,
    FlowPermissionEventSource,
};
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_core::semantics::Multiplicity;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::statement::StatementNode;
use omega_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

#[derive(Debug, Clone)]
struct LinearPlace {
    symbol: SymbolHandle,
    name: String,
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
}

pub(crate) fn check_linear_obligations(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut permission_events = Vec::new();

    for (_, state_flow) in facts.flow.control.states.iter() {
        let Some(state) = crate::find_state(program, state_flow.state_symbol) else {
            continue;
        };
        let statements = program.statement_table.statements(state.statement_nodes);
        let mut places = Vec::<LinearPlace>::new();

        for parameter in program.state_parameters(state) {
            // A by-value `self` parameter is the language's terminal-consumer
            // form. The caller transfers the obligation into the call; an
            // outcome carrying it would instead establish a new linear result.
            if parameter.is_self {
                continue;
            }
            let multiplicity = type_multiplicity(program, parameter.type_reference);
            let conditional = type_has_conditional_linear_payload(program, parameter.type_reference);
            if multiplicity == Multiplicity::Linear || conditional {
                permission_events.push(FlowPermissionEventFact {
                    machine_symbol: state_flow.machine_symbol,
                    state_symbol: state.symbol,
                    source: FlowPermissionEventSource::StateEntry,
                    kind: FlowPermissionEventKind::Establish,
                    root: omega_facts::PlaceRoot::Symbol(parameter.symbol),
                    segments: HandleSpan::empty(),
                    obligation_live: true,
                });
                places.push(LinearPlace {
                    symbol: parameter.symbol,
                    name: parameter.name.as_str().to_owned(),
                    live: true,
                    ever_established: true,
                    conditional,
                });
            }
        }

        for statement in statements {
            if let StatementNode::LocalData(local) = statement
            {
                let multiplicity = type_multiplicity(program, local.type_reference);
                let conditional =
                    type_has_conditional_linear_payload(program, local.type_reference);
                if multiplicity == Multiplicity::Linear || conditional {
                    places.push(LinearPlace {
                        symbol: local.symbol,
                        name: local.name.as_str().to_owned(),
                        live: false,
                        ever_established: false,
                        conditional,
                    });
                }
            }
        }

        let moves = facts.flow.ownership.moves.span_or_empty(state_flow.moves);
        let first_transition = statements
            .iter()
            .position(|statement| matches!(statement, StatementNode::Transition(_)));
        let prefix_end = first_transition.unwrap_or(statements.len());
        for (statement_index, statement) in statements[..prefix_end].iter().enumerate() {
            apply_statement_permission_flow(
                program,
                facts,
                state_flow.machine_symbol,
                state.symbol,
                moves,
                statement_index,
                statement,
                &mut places,
                &mut diagnostics,
                &mut permission_events,
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
                apply_statement_permission_flow(
                    program,
                    facts,
                    state_flow.machine_symbol,
                    state.symbol,
                    moves,
                    statement_index,
                    &statements[statement_index],
                    &mut outcome,
                    &mut diagnostics,
                    &mut permission_events,
                );
                outcomes.push(outcome);
            }

            // A non-fallthrough final guard leaves an implicit path that takes
            // no arm. Other validation normally rejects it as non-exhaustive;
            // retaining it here keeps the resource judgment independently
            // conservative.
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

        // Once linear/conditional roots are removed, the old state-exit drops
        // are precisely the affine cleanup events. Preserve them explicitly so
        // later consumers never have to infer semantic kind from `drops`.
        for drop in facts.flow.ownership.drops.span_or_empty(state_flow.drops) {
            let tracked_linear = matches!(drop.root, omega_facts::PlaceRoot::Symbol(symbol) if places.iter().any(|place| place.symbol == symbol));
            if tracked_linear {
                continue;
            }
            permission_events.push(FlowPermissionEventFact {
                machine_symbol: state_flow.machine_symbol,
                state_symbol: state.symbol,
                source: FlowPermissionEventSource::StateExit,
                kind: FlowPermissionEventKind::AffineDrop,
                root: drop.root,
                segments: drop.segments,
                obligation_live: false,
            });
        }
    }

    facts.flow.ownership.permissions = omega_core::arena::Arena::default();
    facts
        .flow
        .ownership
        .permissions
        .insert_many(permission_events);

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_statement_permission_flow(
    program: &omega_typed_trees::TypedTrees,
    facts: &CheckFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    moves: &[omega_checked_trees::FlowMoveEventFact],
    statement_index: usize,
    statement: &StatementNode,
    places: &mut [LinearPlace],
    diagnostics: &mut Vec<Diagnostic>,
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
            && facts
                .flow
                .ownership
                .segments
                .span_or_empty(event.segments)
                .is_empty()
            && written_target.map(|target| target.root) != Some(event.root)
    }) {
        let omega_facts::PlaceRoot::Symbol(symbol) = event.root else {
            continue;
        };
        let Some(place) = places.iter_mut().find(|place| place.symbol == symbol) else {
            continue;
        };
        let obligation_live = place.live;
        permission_events.push(FlowPermissionEventFact {
            machine_symbol,
            state_symbol,
            source: permission_source(event.source),
            kind: permission_kind_for_move(program, facts, machine_symbol, state_symbol, event),
            root: event.root,
            segments: event.segments,
            obligation_live,
        });
        if !place.live && !place.conditional {
            let reason = if place.ever_established {
                "was already transferred or consumed"
            } else {
                "has not been established (implicit zero-fill creates no linear obligation)"
            };
            diagnostics.push(Diagnostic::error(format!(
                "linear value `{}` {reason}; it cannot be moved here",
                place.name
            )));
        } else {
            place.live = false;
        }
    }

    if let Some(WrittenLinearTarget {
        root: omega_facts::PlaceRoot::Symbol(symbol),
        obligation_live,
    }) = written_target
    {
        let place = places
            .iter_mut()
            .find(|place| place.symbol == symbol)
            .expect("written linear target came from the tracked place set");
        if place.live {
            diagnostics.push(Diagnostic::error(format!(
                "assignment would overwrite live linear value `{}`; consume or transfer the existing obligation first",
                place.name
            )));
        }
        place.live = obligation_live;
        place.ever_established = true;
        permission_events.push(FlowPermissionEventFact {
            machine_symbol,
            state_symbol,
            source: FlowPermissionEventSource::Statement { statement_index },
            kind: FlowPermissionEventKind::Establish,
            root: omega_facts::PlaceRoot::Symbol(symbol),
            segments: HandleSpan::empty(),
            obligation_live,
        });
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

fn permission_source(source: FlowOwnershipEventSource) -> FlowPermissionEventSource {
    match source {
        FlowOwnershipEventSource::Statement { statement_index } => {
            FlowPermissionEventSource::Statement { statement_index }
        }
        FlowOwnershipEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => FlowPermissionEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        },
        FlowOwnershipEventSource::StateExit => FlowPermissionEventSource::StateExit,
    }
}

fn permission_kind_for_move(
    program: &omega_typed_trees::TypedTrees,
    facts: &CheckFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    event: &omega_checked_trees::FlowMoveEventFact,
) -> FlowPermissionEventKind {
    let FlowOwnershipEventSource::Call {
        statement_index,
        call_ordinal,
        target_symbol,
    } = event.source
    else {
        return FlowPermissionEventKind::Transfer;
    };
    let Some(call_site) = crate::find_call_site(
        program,
        machine_symbol,
        state_symbol,
        statement_index,
        call_ordinal,
    ) else {
        return FlowPermissionEventKind::Transfer;
    };
    let Some(target_state) = crate::find_state(program, target_symbol) else {
        return FlowPermissionEventKind::Transfer;
    };
    let arguments = crate::call_site_argument_expressions(program, &call_site);
    let parameters = program.state_parameters(target_state);
    if arguments.len() != parameters.len() {
        return FlowPermissionEventKind::Transfer;
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
            return FlowPermissionEventKind::Consume;
        }
    }
    FlowPermissionEventKind::Transfer
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
            })
        }
        _ => None,
    }
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
