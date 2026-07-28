use omega_checked_trees::{CheckFacts, FlowOwnershipEventSource, FlowPermissionEventFact};
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_core::semantics::{
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource, PermissionProvenance,
};
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::statement::StatementNode;
use omega_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

#[derive(Debug, Clone)]
struct LinearPlace {
    symbol: SymbolHandle,
    name: String,
    /// Canonical claim path below `symbol`. An empty path is one nominal or
    /// conditional root claim; transparent records contribute one entry per
    /// contained linear claim instead of inventing an aggregate root.
    path: Vec<omega_facts::PlaceSegment>,
    type_reference: TypeReferenceHandle,
    multiplicity: Multiplicity,
    claim_identity: Option<PermissionClaimIdentity>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct WrittenLinearTarget {
    root: omega_facts::PlaceRoot,
    destination_path: Vec<omega_facts::PlaceSegment>,
    place_index: usize,
    obligation_live: bool,
    claim_identity: Option<PermissionClaimIdentity>,
    provenance: Option<PermissionProvenance>,
}

#[derive(Debug, Clone)]
struct LinearClaimTemplate {
    path: Vec<omega_facts::PlaceSegment>,
    type_reference: TypeReferenceHandle,
    multiplicity: Multiplicity,
    conditional: bool,
}

#[derive(Debug, Default)]
struct ClaimIdentityAllocator {
    next_ordinal: u32,
}

impl ClaimIdentityAllocator {
    fn mint(
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
    let mut claim_identities = ClaimIdentityAllocator::default();

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
        let mut places =
            initial_linear_places(program, state, state_flow.machine_symbol, state.symbol);

        for place in places.iter_mut().filter(|place| place.ever_established) {
            let claim_identity = claim_identities.mint(
                state_flow.machine_symbol,
                state.symbol,
                PermissionEventSource::StateEntry,
            );
            place.claim_identity = Some(claim_identity);
            let segments = facts
                .flow
                .ownership
                .segments
                .insert_many(place.path.iter().copied());
            permission_events.push(FlowPermissionEventFact {
                machine_symbol: state_flow.machine_symbol,
                state_symbol: state.symbol,
                source: PermissionEventSource::StateEntry,
                kind: PermissionEventKind::Establish,
                multiplicity: place.multiplicity,
                access: PermissionAccess::Owned,
                claim_identity,
                provenance: place.provenance.expect("entry place has provenance"),
                root: omega_facts::PlaceRoot::Symbol(place.symbol),
                segments,
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
                &mut claim_identities,
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
                    &mut claim_identities,
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

    append_borrow_permission_events(facts, &mut permission_events, &mut claim_identities);
    reconcile_state_call_result_origins(
        program,
        &facts.flow.ownership.segments,
        &mut permission_events,
    );

    facts.flow.ownership.permissions = omega_core::arena::Arena::default();
    facts
        .flow
        .ownership
        .permissions
        .insert_many(permission_events);
}

/// Join a state call's receiving establishment to the unique claim and
/// root-lineage provenance that the target state transferred through its
/// result.
///
/// Intra-state production can propagate provenance directly through source
/// expressions. A zero-argument state call has no caller-side source place,
/// though: without this join, binding a locally-created linear result in the
/// caller would mint a second identity and origin even when the target has one
/// unambiguous outgoing obligation. Ambiguous/multi-resource results remain
/// conservative until the general resource algebra can publish an explicit
/// result mapping.
fn reconcile_state_call_result_origins(
    program: &omega_typed_trees::TypedTrees,
    segments: &omega_core::arena::Arena<omega_facts::PlaceSegment>,
    permission_events: &mut [FlowPermissionEventFact],
) {
    let mut rewrites = Vec::new();

    for event in permission_events.iter() {
        if event.kind != PermissionEventKind::Establish
            || event.access != PermissionAccess::Owned
            || !event.obligation_live
        {
            continue;
        }
        let PermissionEventSource::Statement { statement_index } = event.source else {
            continue;
        };
        let locally_minted =
            established_provenance(event.machine_symbol, event.state_symbol, event.source);
        if event.provenance != locally_minted
            || event.claim_identity == PermissionClaimIdentity::Unknown
        {
            continue;
        }
        let Some(state) = crate::find_state(program, event.state_symbol) else {
            continue;
        };
        let Some(statement) = program
            .statement_table
            .statements(state.statement_nodes)
            .get(statement_index)
        else {
            continue;
        };
        let result_expression = match statement {
            StatementNode::LocalData(local) => local.initial_value,
            StatementNode::Assignment(assignment) => assignment.value,
            _ => continue,
        };
        let omega_typed_trees::expression::ExpressionNode::Call(call) =
            program.expression_table.expression(result_expression)
        else {
            continue;
        };
        let Some(target_state) = crate::find_state(program, call.target_symbol) else {
            continue;
        };
        if !type_carries_linear_obligation(program, target_state.return_type) {
            continue;
        }
        let receiving_path = segments.span_or_empty(event.segments);

        let target_statements = program
            .statement_table
            .statements(target_state.statement_nodes);
        let origins = permission_events
            .iter()
            .filter_map(|candidate| {
                if candidate.state_symbol != target_state.symbol
                    || candidate.kind != PermissionEventKind::Transfer
                    || candidate.access != PermissionAccess::Owned
                    || !candidate.obligation_live
                    || candidate.provenance == PermissionProvenance::Unknown
                    || candidate.claim_identity == PermissionClaimIdentity::Unknown
                {
                    return None;
                }
                let PermissionEventSource::Statement { statement_index } = candidate.source else {
                    return None;
                };
                if !statement_transfers_state_result(program, target_statements, statement_index) {
                    return None;
                }
                let output_path = state_result_relative_path_for_event(
                    program,
                    target_state.symbol,
                    target_statements,
                    statement_index,
                    candidate,
                    segments,
                );
                Some((output_path, candidate.provenance, candidate.claim_identity))
            })
            .fold(Vec::new(), |mut origins, origin| {
                if !origins.contains(&origin) {
                    origins.push(origin);
                }
                origins
            });
        let matching = origins
            .iter()
            .filter(|(output_path, _, _)| {
                output_path
                    .as_deref()
                    .is_some_and(|output_path| output_path == receiving_path)
            })
            .collect::<Vec<_>>();
        let origin = if let [origin] = matching.as_slice() {
            Some((origin.1, origin.2))
        } else if receiving_path.is_empty() {
            let distinct = origins
                .iter()
                .map(|(_, provenance, identity)| (*provenance, *identity))
                .fold(Vec::new(), |mut distinct, origin| {
                    if !distinct.contains(&origin) {
                        distinct.push(origin);
                    }
                    distinct
                });
            let [origin] = distinct.as_slice() else {
                continue;
            };
            Some(*origin)
        } else {
            None
        };
        if let Some((provenance, identity)) = origin {
            rewrites.push((locally_minted, event.claim_identity, provenance, identity));
        }
    }

    for event in permission_events
        .iter_mut()
        .filter(|event| event.access == PermissionAccess::Owned)
    {
        let mut provenance = event.provenance;
        let mut claim_identity = event.claim_identity;
        for _ in 0..rewrites.len() {
            let Some((_, _, replacement_provenance, replacement_identity)) =
                rewrites
                    .iter()
                    .find(|(source_provenance, source_identity, _, _)| {
                        *source_provenance == provenance && *source_identity == claim_identity
                    })
            else {
                break;
            };
            if *replacement_provenance == provenance && *replacement_identity == claim_identity {
                break;
            }
            provenance = *replacement_provenance;
            claim_identity = *replacement_identity;
        }
        event.provenance = provenance;
        event.claim_identity = claim_identity;
    }
}

fn state_result_relative_path_for_event(
    program: &omega_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statements: &[StatementNode],
    statement_index: usize,
    event: &FlowPermissionEventFact,
    segments: &omega_core::arena::Arena<omega_facts::PlaceSegment>,
) -> Option<Vec<omega_facts::PlaceSegment>> {
    let statement = statements.get(statement_index)?;
    let result_expressions = match statement {
        StatementNode::Expression(expression) if statement_index + 1 == statements.len() => {
            vec![*expression]
        }
        StatementNode::Transition(transition) => [transition.target, transition.continuation]
            .into_iter()
            .filter(|handle| handle.is_valid())
            .filter_map(|handle| {
                let omega_typed_trees::statement::TransitionTargetNode::Value(expression) =
                    program.statement_table.transition_target(handle)
                else {
                    return None;
                };
                Some(*expression)
            })
            .collect(),
        _ => return None,
    };
    let event_path = segments.span_or_empty(event.segments);
    let mut paths = result_expressions
        .into_iter()
        .filter_map(|expression| {
            let place = crate::flow::canonical_place_from_expression_in_state(
                program,
                state_symbol,
                statement_index,
                expression,
            )?;
            (place.root == event.root && event_path.starts_with(place.segments.as_slice()))
                .then(|| event_path[place.segments.len()..].to_vec())
        })
        .fold(Vec::new(), |mut paths, path| {
            if !paths.contains(&path) {
                paths.push(path);
            }
            paths
        });
    (paths.len() == 1).then(|| paths.pop().expect("one result-relative path"))
}

fn statement_transfers_state_result(
    program: &omega_typed_trees::TypedTrees,
    statements: &[StatementNode],
    statement_index: usize,
) -> bool {
    let Some(statement) = statements.get(statement_index) else {
        return false;
    };
    match statement {
        StatementNode::Expression(_) => statement_index + 1 == statements.len(),
        StatementNode::Transition(transition) => [transition.target, transition.continuation]
            .into_iter()
            .filter(|handle| handle.is_valid())
            .any(|handle| {
                matches!(
                    program.statement_table.transition_target(handle),
                    omega_typed_trees::statement::TransitionTargetNode::Value(_)
                )
            }),
        _ => false,
    }
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
        let mut places =
            initial_linear_places(program, state, state_flow.machine_symbol, state.symbol);
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
        append_unresolved_state_result_mapping_diagnostics(
            program,
            state,
            &events,
            &facts.flow.ownership.permissions,
            &mut diagnostics,
        );

        let first_transition = statements
            .iter()
            .position(|statement| matches!(statement, StatementNode::Transition(_)));
        let prefix_end = first_transition.unwrap_or(statements.len());
        for statement_index in 0..prefix_end {
            apply_recorded_statement_events(
                statement_index,
                &events,
                &facts.flow.ownership.segments,
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
                    &facts.flow.ownership.segments,
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
                        mixed_places
                            .push((places[place_index].symbol, places[place_index].path.clone()));
                    } else {
                        places[place_index].live = live;
                        places[place_index].ever_established = outcomes
                            .iter()
                            .any(|outcome| outcome[place_index].ever_established);
                    }
                }
            }
        }

        for place in places.iter().filter(|place| {
            place.live
                && !mixed_places
                    .iter()
                    .any(|(symbol, path)| *symbol == place.symbol && *path == place.path)
        }) {
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

fn append_unresolved_state_result_mapping_diagnostics(
    program: &omega_typed_trees::TypedTrees,
    state: &omega_typed_trees::state::State,
    events: &[&FlowPermissionEventFact],
    all_events: &omega_core::arena::Arena<FlowPermissionEventFact>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let statements = program.statement_table.statements(state.statement_nodes);
    let mut unresolved_statements = Vec::new();
    for event in events {
        if event.kind != PermissionEventKind::Establish
            || event.access != PermissionAccess::Owned
            || !event.obligation_live
        {
            continue;
        }
        let PermissionEventSource::Statement { statement_index } = event.source else {
            continue;
        };
        let Some(statement) = statements.get(statement_index) else {
            continue;
        };
        let result_expression = match statement {
            StatementNode::LocalData(local) => local.initial_value,
            StatementNode::Assignment(assignment) => assignment.value,
            _ => continue,
        };
        let omega_typed_trees::expression::ExpressionNode::Call(call) =
            program.expression_table.expression(result_expression)
        else {
            continue;
        };
        let Some(target_state) = crate::find_state(program, call.target_symbol) else {
            continue;
        };
        if program
            .statement_table
            .statements(target_state.statement_nodes)
            .is_empty()
            || !type_carries_linear_obligation(program, target_state.return_type)
        {
            continue;
        }
        let mapped_from_call = all_events.iter().any(|(_, candidate)| {
            candidate.kind == PermissionEventKind::Transfer
                && candidate.access == PermissionAccess::Owned
                && candidate.obligation_live
                && candidate.claim_identity != PermissionClaimIdentity::Unknown
                && candidate.claim_identity == event.claim_identity
                && (candidate.state_symbol == target_state.symbol
                    || (candidate.state_symbol == state.symbol
                        && permission_event_statement_index(candidate.source)
                            == Some(statement_index)))
        });
        if mapped_from_call {
            continue;
        }
        if !unresolved_statements.contains(&statement_index) {
            unresolved_statements.push(statement_index);
        }
    }

    for statement_index in unresolved_statements {
        diagnostics.push(Diagnostic::error(format!(
            "linear state-call result at statement {statement_index} has no unique conserved claim mapping; return a path-aligned source place or publish an explicit outcome mapping"
        )));
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
        for claim in linear_claim_frontier(program, parameter.type_reference) {
            places.push(LinearPlace {
                symbol: parameter.symbol,
                name: claim_place_name(program, parameter.name.as_str(), &claim.path),
                path: claim.path,
                type_reference: claim.type_reference,
                multiplicity: claim.multiplicity,
                claim_identity: None,
                provenance: Some(established_provenance(
                    machine_symbol,
                    state_symbol,
                    PermissionEventSource::StateEntry,
                )),
                live: true,
                ever_established: true,
                conditional: claim.conditional,
            });
        }
    }
    for statement in program.statement_table.statements(state.statement_nodes) {
        let StatementNode::LocalData(local) = statement else {
            continue;
        };
        for claim in linear_claim_frontier(program, local.type_reference) {
            places.push(LinearPlace {
                symbol: local.symbol,
                name: claim_place_name(program, local.name.as_str(), &claim.path),
                path: claim.path,
                type_reference: claim.type_reference,
                multiplicity: claim.multiplicity,
                claim_identity: None,
                provenance: None,
                live: false,
                ever_established: false,
                conditional: claim.conditional,
            });
        }
    }
    places
}

fn linear_claim_frontier(
    program: &omega_typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Vec<LinearClaimTemplate> {
    let mut claims = Vec::new();
    append_linear_claim_frontier(
        program,
        type_reference,
        &[],
        &[],
        &mut Vec::new(),
        &mut claims,
    );
    claims
}

fn append_linear_claim_frontier(
    program: &omega_typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    path: &[omega_facts::PlaceSegment],
    visiting: &mut Vec<SymbolHandle>,
    claims: &mut Vec<LinearClaimTemplate>,
) {
    if !type_reference.is_valid() {
        return;
    }
    let multiplicity = type_multiplicity_with_substitutions(program, type_reference, substitutions);
    if multiplicity == Multiplicity::Linear {
        claims.push(LinearClaimTemplate {
            path: path.to_vec(),
            type_reference,
            multiplicity,
            conditional: false,
        });
        return;
    }
    if conditional_linear_payload_inner(program, type_reference, substitutions, &mut Vec::new()) {
        claims.push(LinearClaimTemplate {
            path: path.to_vec(),
            type_reference,
            multiplicity,
            conditional: true,
        });
        return;
    }

    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            append_linear_claim_frontier(program, *base_type, substitutions, path, visiting, claims)
        }
        TypeReferenceNode::Named { symbol, name } => {
            if let Some(replacement) =
                substitutions
                    .iter()
                    .rev()
                    .find_map(|(parameter, replacement)| {
                        (*parameter == *symbol).then_some(*replacement)
                    })
                && replacement != type_reference
            {
                append_linear_claim_frontier(
                    program,
                    replacement,
                    substitutions,
                    path,
                    visiting,
                    claims,
                );
                return;
            }
            let Some(definition) = find_data_definition(program, *symbol, name.as_str()) else {
                return;
            };
            append_data_linear_claim_frontier(
                program,
                definition,
                substitutions,
                path,
                visiting,
                claims,
            );
        }
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            arguments,
            ..
        } => {
            let Some(definition) = find_data_definition(program, *base_symbol, base_name.as_str())
            else {
                return;
            };
            let mut instantiated = substitutions.to_vec();
            instantiated.extend(
                program
                    .data_type_parameters(definition)
                    .iter()
                    .zip(
                        program
                            .type_reference_table
                            .type_reference_handles(*arguments),
                    )
                    .filter_map(|(parameter, argument)| {
                        matches!(
                            parameter.kind,
                            omega_typed_trees::data::TypeParameterKind::Type
                        )
                        .then_some((parameter.symbol, *argument))
                    }),
            );
            append_data_linear_claim_frontier(
                program,
                definition,
                &instantiated,
                path,
                visiting,
                claims,
            );
        }
        // Literal fixed-array indices and active sum cases join this same
        // frontier once their canonical path identities are available.
        TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Unit => {}
    }
}

fn append_data_linear_claim_frontier(
    program: &omega_typed_trees::TypedTrees,
    definition: &omega_typed_trees::data::DataDefinition,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    path: &[omega_facts::PlaceSegment],
    visiting: &mut Vec<SymbolHandle>,
    claims: &mut Vec<LinearClaimTemplate>,
) {
    if visiting.contains(&definition.symbol) {
        return;
    }
    visiting.push(definition.symbol);
    for member in program.data_members(definition) {
        let omega_typed_trees::data::DataMember::Field(field) = member else {
            continue;
        };
        let mut field_path = path.to_vec();
        field_path.push(omega_facts::PlaceSegment::Field {
            symbol: field.symbol,
        });
        append_linear_claim_frontier(
            program,
            field.type_reference,
            substitutions,
            &field_path,
            visiting,
            claims,
        );
    }
    visiting.pop();
}

fn claim_place_name(
    program: &omega_typed_trees::TypedTrees,
    root: &str,
    path: &[omega_facts::PlaceSegment],
) -> String {
    let mut name = root.to_owned();
    for segment in path {
        match segment {
            omega_facts::PlaceSegment::Field { symbol } => {
                let field = program.data_definitions().iter().find_map(|definition| {
                    program.data_members(definition).iter().find_map(|member| {
                        let omega_typed_trees::data::DataMember::Field(field) = member else {
                            return None;
                        };
                        (field.symbol == *symbol).then_some(field.name.as_str())
                    })
                });
                name.push('.');
                name.push_str(field.unwrap_or("<field>"));
            }
            omega_facts::PlaceSegment::Index { .. } => name.push_str("[<index>]"),
        }
    }
    name
}

fn apply_recorded_statement_events(
    statement_index: usize,
    events: &[&FlowPermissionEventFact],
    segments: &omega_core::arena::Arena<omega_facts::PlaceSegment>,
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
        let event_path = segments.span_or_empty(event.segments);
        let Some(place) = places
            .iter_mut()
            .find(|place| place.symbol == symbol && place.path.as_slice() == event_path)
        else {
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
                place.claim_identity = Some(event.claim_identity);
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
    claim_identities: &mut ClaimIdentityAllocator,
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

    let mut loan_claim_identities = Vec::new();
    for (machine_symbol, state_symbol, activations, weakenings) in states {
        for activation in facts
            .flow
            .borrow_lifetimes
            .activations
            .span_or_empty(activations)
            .to_vec()
        {
            let claim_identity = claim_identities.mint(
                machine_symbol,
                state_symbol,
                permission_source_from_invalidation(activation.source),
            );
            loan_claim_identities.push((activation.loan, claim_identity));
            append_borrow_permission_event(
                facts,
                permission_events,
                machine_symbol,
                state_symbol,
                activation.loan,
                permission_source_from_invalidation(activation.source),
                PermissionEventKind::Establish,
                claim_identity,
            );
        }
        for weakening in facts
            .flow
            .borrow_lifetimes
            .weakenings
            .span_or_empty(weakenings)
            .to_vec()
        {
            let source =
                if weakening.reason == omega_checked_trees::FlowBorrowWeakeningReason::StateExit {
                    PermissionEventSource::StateExit
                } else {
                    permission_source_from_invalidation(weakening.source)
                };
            let claim_identity = loan_claim_identities
                .iter()
                .rev()
                .find_map(|(loan, identity)| (*loan == weakening.loan).then_some(*identity))
                .unwrap_or(PermissionClaimIdentity::Unknown);
            append_borrow_permission_event(
                facts,
                permission_events,
                machine_symbol,
                state_symbol,
                weakening.loan,
                source,
                PermissionEventKind::Consume,
                claim_identity,
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
    claim_identity: PermissionClaimIdentity,
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
        claim_identity,
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
    facts: &mut CheckFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    moves: &[crate::flow::DiscoveredMoveEvent],
    statement_index: usize,
    statement: &StatementNode,
    places: &mut [LinearPlace],
    permission_events: &mut Vec<FlowPermissionEventFact>,
    claim_identities: &mut ClaimIdentityAllocator,
) {
    // Destructure coverage markers are proof-only reads synthesized by the
    // parser; they neither transfer a value nor establish user storage.
    if matches!(statement, StatementNode::LocalData(local) if local.name.as_str().starts_with("__arm_destructure#"))
    {
        return;
    }

    let written_targets =
        written_linear_targets(program, state_symbol, statement_index, statement, places);

    // Moves out of initializer/assignment sources happen before the
    // destination becomes established. The old move-only summary also
    // contains a production event *at* the destination; exclude that
    // compatibility event here rather than mistaking creation for use.
    for event in moves
        .iter()
        .filter(|event| event_statement_index(event.source) == Some(statement_index))
    {
        let omega_facts::PlaceRoot::Symbol(symbol) = event.root else {
            continue;
        };
        let event_path = facts
            .flow
            .ownership
            .segments
            .span_or_empty(event.segments)
            .to_vec();
        if written_targets.iter().any(|target| {
            target.root == event.root && target.destination_path.as_slice() == event_path
        }) {
            continue;
        }
        let matching = places
            .iter()
            .enumerate()
            .filter_map(|(index, place)| {
                (place.symbol == symbol && move_selects_claim(&event_path, place)).then_some(index)
            })
            .collect::<Vec<_>>();
        if matching.is_empty() {
            continue;
        }
        let kind = permission_kind_for_move(program, facts, machine_symbol, state_symbol, event);
        for index in matching {
            let claim_path = places[index].path.clone();
            let segments = facts.flow.ownership.segments.insert_many(claim_path);
            let place = &mut places[index];
            let obligation_live = place.live;
            permission_events.push(FlowPermissionEventFact {
                machine_symbol,
                state_symbol,
                source: permission_source(event.source),
                kind,
                multiplicity: place.multiplicity,
                access: PermissionAccess::Owned,
                claim_identity: place
                    .claim_identity
                    .unwrap_or(PermissionClaimIdentity::Unknown),
                provenance: place.provenance.unwrap_or(PermissionProvenance::Unknown),
                root: event.root,
                segments,
                obligation_live,
            });
            place.live = false;
        }
    }

    for target in written_targets {
        let omega_facts::PlaceRoot::Symbol(symbol) = target.root else {
            continue;
        };
        let place_index = target.place_index;
        let obligation_live = target.obligation_live;
        let claim_identity = target.claim_identity.unwrap_or_else(|| {
            obligation_live
                .then(|| {
                    claim_identities.mint(
                        machine_symbol,
                        state_symbol,
                        PermissionEventSource::Statement { statement_index },
                    )
                })
                .unwrap_or(PermissionClaimIdentity::Unknown)
        });
        let provenance = target.provenance;
        let claim_path = places[place_index].path.clone();
        let segments = facts.flow.ownership.segments.insert_many(claim_path);
        let place = &mut places[place_index];
        debug_assert_eq!(place.symbol, symbol);
        place.live = obligation_live;
        place.ever_established = true;
        place.claim_identity = Some(claim_identity);
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
            claim_identity,
            provenance: place
                .provenance
                .expect("an established place has explicit provenance"),
            root: omega_facts::PlaceRoot::Symbol(symbol),
            segments,
            obligation_live,
        });
    }
}

fn move_selects_claim(event_path: &[omega_facts::PlaceSegment], claim: &LinearPlace) -> bool {
    claim.path.starts_with(event_path)
        || (claim.conditional && event_path.starts_with(claim.path.as_slice()))
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
    let event_segments = facts.flow.ownership.segments.span_or_empty(event.segments);
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

fn written_linear_targets(
    program: &omega_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
    places: &[LinearPlace],
) -> Vec<WrittenLinearTarget> {
    let (target, value) = match statement {
        StatementNode::LocalData(local) => {
            if !local.initial_value.is_valid() {
                return Vec::new();
            }
            (
                crate::flow::CanonicalPlace {
                    root: omega_facts::PlaceRoot::Symbol(local.symbol),
                    segments: Vec::new(),
                },
                local.initial_value,
            )
        }
        StatementNode::Assignment(assignment) => {
            let Some(place) = crate::flow::canonical_place_from_expression_in_state(
                program,
                state_symbol,
                statement_index,
                assignment.target,
            ) else {
                return Vec::new();
            };
            (place, assignment.value)
        }
        _ => return Vec::new(),
    };
    let omega_facts::PlaceRoot::Symbol(symbol) = target.root else {
        return Vec::new();
    };

    places
        .iter()
        .enumerate()
        .filter_map(|(place_index, tracked)| {
            if tracked.symbol != symbol || !tracked.path.starts_with(target.segments.as_slice()) {
                return None;
            }
            let relative_path = &tracked.path[target.segments.len()..];
            Some(WrittenLinearTarget {
                root: target.root,
                destination_path: target.segments.clone(),
                place_index,
                obligation_live: expression_establishes_obligation(
                    program,
                    state_symbol,
                    statement_index,
                    value,
                    tracked.conditional,
                    tracked.type_reference,
                    places,
                ),
                claim_identity: expression_permission_claim_identity_for_claim(
                    program,
                    state_symbol,
                    statement_index,
                    value,
                    relative_path,
                    places,
                ),
                provenance: expression_permission_provenance_for_claim(
                    program,
                    state_symbol,
                    statement_index,
                    value,
                    relative_path,
                    places,
                ),
            })
        })
        .collect()
}

fn expression_permission_claim_identity_for_claim(
    program: &omega_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: omega_typed_trees::expression::ExpressionHandle,
    relative_path: &[omega_facts::PlaceSegment],
    places: &[LinearPlace],
) -> Option<PermissionClaimIdentity> {
    if relative_path.is_empty() {
        match program.expression_table.expression(expression) {
            omega_typed_trees::expression::ExpressionNode::Call(call) => {
                let mut candidates = Vec::new();
                if call.receiver.is_valid() {
                    candidates.push(call.receiver);
                }
                candidates
                    .extend_from_slice(program.expression_table.expression_handles(call.arguments));
                return common_permission_claim_identity(candidates.into_iter().filter_map(
                    |candidate| {
                        expression_permission_claim_identity_for_claim(
                            program,
                            state_symbol,
                            statement_index,
                            candidate,
                            &[],
                            places,
                        )
                    },
                ));
            }
            omega_typed_trees::expression::ExpressionNode::StructLiteral(literal) => {
                return common_permission_claim_identity(
                    program
                        .expression_table
                        .struct_fields(literal.fields)
                        .iter()
                        .filter_map(|field| {
                            expression_permission_claim_identity_for_claim(
                                program,
                                state_symbol,
                                statement_index,
                                field.value,
                                &[],
                                places,
                            )
                        }),
                );
            }
            _ => {}
        }
    }

    if let omega_typed_trees::expression::ExpressionNode::StructLiteral(literal) =
        program.expression_table.expression(expression)
        && let Some(omega_facts::PlaceSegment::Field { symbol }) = relative_path.first()
    {
        let field_name = program.data_definitions().iter().find_map(|definition| {
            program.data_members(definition).iter().find_map(|member| {
                let omega_typed_trees::data::DataMember::Field(field) = member else {
                    return None;
                };
                (field.symbol == *symbol).then_some(field.name.as_str())
            })
        })?;
        let field = program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .find(|field| field.name.as_str() == field_name)?;
        return expression_permission_claim_identity_for_claim(
            program,
            state_symbol,
            statement_index,
            field.value,
            &relative_path[1..],
            places,
        );
    }

    if !relative_path.is_empty()
        && matches!(
            program.expression_table.expression(expression),
            omega_typed_trees::expression::ExpressionNode::Call(_)
        )
    {
        return None;
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
    let mut source_path = source.segments;
    source_path.extend_from_slice(relative_path);
    let matches = places
        .iter()
        .filter(|place| {
            place.symbol == symbol
                && place.live
                && (place.path == source_path
                    || (place.conditional && source_path.starts_with(place.path.as_slice())))
        })
        .filter_map(|place| place.claim_identity);
    common_permission_claim_identity(matches)
}

fn expression_permission_provenance_for_claim(
    program: &omega_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: omega_typed_trees::expression::ExpressionHandle,
    relative_path: &[omega_facts::PlaceSegment],
    places: &[LinearPlace],
) -> Option<PermissionProvenance> {
    if relative_path.is_empty() {
        match program.expression_table.expression(expression) {
            omega_typed_trees::expression::ExpressionNode::Call(call) => {
                let mut candidates = Vec::new();
                if call.receiver.is_valid() {
                    candidates.push(call.receiver);
                }
                candidates
                    .extend_from_slice(program.expression_table.expression_handles(call.arguments));
                return common_permission_provenance(candidates.into_iter().filter_map(
                    |candidate| {
                        expression_permission_provenance_for_claim(
                            program,
                            state_symbol,
                            statement_index,
                            candidate,
                            &[],
                            places,
                        )
                    },
                ));
            }
            omega_typed_trees::expression::ExpressionNode::StructLiteral(literal) => {
                return common_permission_provenance(
                    program
                        .expression_table
                        .struct_fields(literal.fields)
                        .iter()
                        .filter_map(|field| {
                            expression_permission_provenance_for_claim(
                                program,
                                state_symbol,
                                statement_index,
                                field.value,
                                &[],
                                places,
                            )
                        }),
                );
            }
            _ => {}
        }
    }

    if let omega_typed_trees::expression::ExpressionNode::StructLiteral(literal) =
        program.expression_table.expression(expression)
        && let Some(omega_facts::PlaceSegment::Field { symbol }) = relative_path.first()
    {
        let field_name = program.data_definitions().iter().find_map(|definition| {
            program.data_members(definition).iter().find_map(|member| {
                let omega_typed_trees::data::DataMember::Field(field) = member else {
                    return None;
                };
                (field.symbol == *symbol).then_some(field.name.as_str())
            })
        })?;
        let field = program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .find(|field| field.name.as_str() == field_name)?;
        return expression_permission_provenance_for_claim(
            program,
            state_symbol,
            statement_index,
            field.value,
            &relative_path[1..],
            places,
        );
    }

    if !relative_path.is_empty()
        && matches!(
            program.expression_table.expression(expression),
            omega_typed_trees::expression::ExpressionNode::Call(_)
        )
    {
        // Multi-output call mappings need the explicit P1c outcome map. Do
        // not guess a field origin from argument order.
        return None;
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
    let mut source_path = source.segments;
    source_path.extend_from_slice(relative_path);
    let matches = places
        .iter()
        .filter(|place| {
            place.symbol == symbol
                && place.live
                && (place.path == source_path
                    || (place.conditional && source_path.starts_with(place.path.as_slice())))
        })
        .filter_map(|place| place.provenance);
    common_permission_provenance(matches)
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
            claim_identity: PermissionClaimIdentity::Unknown,
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

fn common_permission_claim_identity(
    mut identities: impl Iterator<Item = PermissionClaimIdentity>,
) -> Option<PermissionClaimIdentity> {
    let first = identities.next()?;
    identities
        .all(|identity| identity == first)
        .then_some(first)
}

pub(crate) fn type_carries_linear_obligation(
    program: &omega_typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    !linear_claim_frontier(program, type_reference).is_empty()
}

fn expression_establishes_obligation(
    program: &omega_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: omega_typed_trees::expression::ExpressionHandle,
    conditional: bool,
    target_type_reference: TypeReferenceHandle,
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
            program
                .data_members(definition)
                .iter()
                .find_map(|member| match member {
                    omega_typed_trees::data::DataMember::Variant(variant)
                        if variant.symbol == path.symbol =>
                    {
                        Some(variant)
                    }
                    _ => None,
                })
        })
    {
        return variant_carries_linear_obligation(program, variant, target_type_reference);
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
    let Some(variant) = program
        .data_members(definition)
        .iter()
        .find_map(|member| match member {
            omega_typed_trees::data::DataMember::Variant(variant)
                if variant.name.as_str() == case_name.as_str() =>
            {
                Some(variant)
            }
            _ => None,
        })
    else {
        return true;
    };

    variant_carries_linear_obligation(program, variant, target_type_reference)
}

fn variant_carries_linear_obligation(
    program: &omega_typed_trees::TypedTrees,
    variant: &omega_typed_trees::data::DataVariant,
    instantiated_type: TypeReferenceHandle,
) -> bool {
    let substitutions = substitutions_for_instantiated_data(program, instantiated_type);
    program.data_payload_fields(variant).iter().any(|field| {
        type_multiplicity_with_substitutions(program, field.type_reference, &substitutions)
            == Multiplicity::Linear
            || conditional_linear_payload_inner(
                program,
                field.type_reference,
                &substitutions,
                &mut Vec::new(),
            )
    })
}

pub(crate) fn type_multiplicity(
    program: &omega_typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Multiplicity {
    if !type_reference.is_valid() {
        return Multiplicity::Affine;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { .. } | TypeReferenceNode::Unit => Multiplicity::Unrestricted,
        TypeReferenceNode::Constrained { base_type, .. } => type_multiplicity(program, *base_type),
        TypeReferenceNode::FixedArray { element_type, .. } => {
            type_multiplicity(program, *element_type)
        }
        TypeReferenceNode::Named { symbol, name } => {
            if let Some(parameter) = program
                .data_type_parameters
                .iter()
                .find_map(|(_, parameter)| (parameter.symbol == *symbol).then_some(parameter))
            {
                return parameter.bounds.multiplicity;
            }
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

fn conditional_linear_payload_inner(
    program: &omega_typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    visiting: &mut Vec<SymbolHandle>,
) -> bool {
    if !type_reference.is_valid() {
        return false;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            conditional_linear_payload_inner(program, *base_type, substitutions, visiting)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            conditional_linear_payload_inner(program, *element_type, substitutions, visiting)
        }
        TypeReferenceNode::Named { symbol, name } => {
            if let Some(replacement) =
                substitutions
                    .iter()
                    .rev()
                    .find_map(|(parameter, replacement)| {
                        (*parameter == *symbol).then_some(*replacement)
                    })
                && replacement != type_reference
            {
                return conditional_linear_payload_inner(
                    program,
                    replacement,
                    substitutions,
                    visiting,
                );
            }
            conditional_linear_payload_named(
                program,
                *symbol,
                name.as_str(),
                substitutions,
                visiting,
            )
        }
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            arguments,
            ..
        } => conditional_linear_payload_generic(
            program,
            *base_symbol,
            base_name.as_str(),
            *arguments,
            substitutions,
            visiting,
        ),
        _ => false,
    }
}

fn conditional_linear_payload_named(
    program: &omega_typed_trees::TypedTrees,
    symbol: SymbolHandle,
    name: &str,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    visiting: &mut Vec<SymbolHandle>,
) -> bool {
    let Some(definition) = find_data_definition(program, symbol, name) else {
        return false;
    };
    conditional_linear_payload_definition(program, definition, substitutions, visiting)
}

fn conditional_linear_payload_generic(
    program: &omega_typed_trees::TypedTrees,
    symbol: SymbolHandle,
    name: &str,
    arguments: HandleSpan<TypeReferenceHandle>,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    visiting: &mut Vec<SymbolHandle>,
) -> bool {
    let Some(definition) = find_data_definition(program, symbol, name) else {
        return false;
    };
    let mut instantiated = substitutions.to_vec();
    instantiated.extend(
        program
            .data_type_parameters(definition)
            .iter()
            .zip(
                program
                    .type_reference_table
                    .type_reference_handles(arguments),
            )
            .filter_map(|(parameter, argument)| {
                matches!(
                    parameter.kind,
                    omega_typed_trees::data::TypeParameterKind::Type
                )
                .then_some((parameter.symbol, *argument))
            }),
    );
    conditional_linear_payload_definition(program, definition, &instantiated, visiting)
}

fn conditional_linear_payload_definition(
    program: &omega_typed_trees::TypedTrees,
    definition: &omega_typed_trees::data::DataDefinition,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    visiting: &mut Vec<SymbolHandle>,
) -> bool {
    if visiting.contains(&definition.symbol) {
        return false;
    }
    if definition.properties.multiplicity == Multiplicity::Linear {
        return false;
    }
    visiting.push(definition.symbol);
    let result = program.data_members(definition).iter().any(|member| {
        let omega_typed_trees::data::DataMember::Variant(variant) = member else {
            return false;
        };
        program.data_payload_fields(variant).iter().any(|field| {
            type_multiplicity_with_substitutions(program, field.type_reference, substitutions)
                == Multiplicity::Linear
                || conditional_linear_payload_inner(
                    program,
                    field.type_reference,
                    substitutions,
                    visiting,
                )
        })
    });
    visiting.pop();
    result
}

fn type_multiplicity_with_substitutions(
    program: &omega_typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> Multiplicity {
    if !type_reference.is_valid() {
        return Multiplicity::Affine;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_multiplicity_with_substitutions(program, *base_type, substitutions)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            type_multiplicity_with_substitutions(program, *element_type, substitutions)
        }
        TypeReferenceNode::Named { symbol, .. } => substitutions
            .iter()
            .rev()
            .find_map(|(parameter, replacement)| {
                (*parameter == *symbol && *replacement != type_reference).then_some(*replacement)
            })
            .map(|replacement| {
                type_multiplicity_with_substitutions(program, replacement, substitutions)
            })
            .unwrap_or_else(|| type_multiplicity(program, type_reference)),
        _ => type_multiplicity(program, type_reference),
    }
}

fn substitutions_for_instantiated_data(
    program: &omega_typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Vec<(SymbolHandle, TypeReferenceHandle)> {
    let TypeReferenceNode::Generic {
        base_symbol,
        base_name,
        arguments,
        ..
    } = program.type_reference_table.type_reference(type_reference)
    else {
        return Vec::new();
    };
    let Some(definition) = find_data_definition(program, *base_symbol, base_name.as_str()) else {
        return Vec::new();
    };
    program
        .data_type_parameters(definition)
        .iter()
        .zip(
            program
                .type_reference_table
                .type_reference_handles(*arguments),
        )
        .filter_map(|(parameter, argument)| {
            matches!(
                parameter.kind,
                omega_typed_trees::data::TypeParameterKind::Type
            )
            .then_some((parameter.symbol, *argument))
        })
        .collect()
}

fn find_data_definition<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    symbol: SymbolHandle,
    name: &str,
) -> Option<&'program omega_typed_trees::data::DataDefinition> {
    program.data_definitions().iter().find(|definition| {
        (symbol.is_valid() && definition.symbol == symbol) || definition.name.as_str() == name
    })
}

#[cfg(test)]
mod generic_substitution_tests {
    use super::*;
    use omega_source_files_to_tokens::Lexer;
    use omega_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use omega_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use omega_tokens_to_syntax_trees::parse_syntax_trees;

    #[test]
    fn linear_generic_bound_classifies_the_parameter_type() {
        let source = r#"
            data Main {}
            machine Main::identity<T [linear]>(value: T) -> T {
                value
            }
        "#;
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::identity")
            .expect("generic identity machine");
        let state = typed
            .machine_states(machine)
            .first()
            .expect("generic identity state");
        let parameter = typed
            .state_parameters(state)
            .iter()
            .find(|parameter| !parameter.is_self)
            .expect("linear generic value parameter");
        assert_eq!(
            type_multiplicity(&typed, parameter.type_reference),
            Multiplicity::Linear
        );
    }
}
