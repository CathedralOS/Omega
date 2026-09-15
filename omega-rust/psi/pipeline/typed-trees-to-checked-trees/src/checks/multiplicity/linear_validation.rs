//! Validating linear permission events statement by statement: the claim
//! frontier, borrow events, permission production and affine cleanup.

use crate::checks::multiplicity::linear_obligations::{
    ClaimIdentityAllocator, LinearClaimTemplate, LinearPlace, WrittenLinearTarget,
};
use crate::checks::multiplicity::owned_selection;
use crate::checks::multiplicity::permission_events::{
    case_transition_run_is_exhaustive, collect_positive_case_tests, exact_positive_case_test,
    exclude_case_alternative, select_case_alternative_from_guard,
};
use crate::checks::multiplicity::projected_affine;
use crate::checks::multiplicity::temporary_results;
use crate::checks::multiplicity::type_multiplicity::{
    data_field_name, expression_establishes_obligation, find_data_definition, literal_variant,
    type_carries_linear_obligation, type_multiplicity, type_multiplicity_with_substitutions,
};
use crate::flow::FlowOwnershipEventSource;
use arena::HandleSpan;
use checked_trees::{CheckFacts, FlowClaimOutcomeSource, FlowPermissionEventFact};
use diagnostics::Diagnostic;
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource, PermissionProvenance,
};
use symbols::SymbolHandle;
use typed_trees::statement::StatementNode;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn validate_linear_permission_events(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut selected_replay = CheckFacts::default();
    // `record_statement` consults the borrow ledger and the statement-entry
    // constraint sets to decide whether a once-borrowed source may join a
    // selection. Those are inputs recorded by earlier passes, so the replay
    // seeds them unchanged while the ownership arenas it fills stay fresh.
    selected_replay.borrow = facts.borrow.clone();
    selected_replay.flow.contexts = facts.flow.contexts.clone();
    selected_replay.flow.control = facts.flow.control.clone();

    for (_, state_flow) in facts.flow.control.states.iter() {
        let Some(state) = crate::find_state(program, state_flow.state_symbol) else {
            continue;
        };
        let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == state_flow.machine_symbol)
        else {
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
            &facts.flow.ownership,
            &mut diagnostics,
        );

        let first_transition = statements
            .iter()
            .position(|statement| matches!(statement, StatementNode::Transition(_)));
        let prefix_end = first_transition.unwrap_or(statements.len());
        for (statement_index, statement) in statements[..prefix_end].iter().enumerate() {
            match owned_selection::record_statement(
                program,
                &mut selected_replay,
                machine,
                state,
                statement_index,
                statement,
                &mut places,
            ) {
                Ok(true) => continue,
                Ok(false) => {}
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            }
            apply_recorded_statement_events(
                statement_index,
                &events,
                &facts.flow.ownership.segments,
                &mut places,
                &mut diagnostics,
            );
        }

        let mut mixed_places = Vec::new();
        let mut has_ordinary_exit = first_transition.is_none();
        if let Some(first_transition) = first_transition {
            let entry = places.clone();
            let arm_indices = (first_transition..statements.len())
                .filter(|index| matches!(statements[*index], StatementNode::Transition(_)))
                .collect::<Vec<_>>();
            let mut outcomes = Vec::new();
            let mut excluded_case_tests = Vec::new();
            for statement_index in arm_indices.iter().copied() {
                let mut outcome = entry.clone();
                if let Err(diagnostic) = owned_selection::record_statement(
                    program,
                    &mut selected_replay,
                    machine,
                    state,
                    statement_index,
                    &statements[statement_index],
                    &mut outcome,
                ) {
                    diagnostics.push(diagnostic);
                    continue;
                }
                for &(subject, variant) in &excluded_case_tests {
                    exclude_case_alternative(
                        program,
                        state.symbol,
                        statement_index,
                        subject,
                        variant,
                        &mut outcome,
                    );
                }
                let StatementNode::Transition(transition) = &statements[statement_index] else {
                    unreachable!("transition indices contain only transitions")
                };
                if let typed_trees::statement::TransitionGuardNode::When(guard) = transition.guard {
                    let mut selected = Vec::new();
                    collect_positive_case_tests(program, guard, &mut selected);
                    for (subject, variant) in selected {
                        select_case_alternative_from_guard(
                            program,
                            state.symbol,
                            statement_index,
                            subject,
                            variant,
                            &mut outcome,
                        );
                    }
                }
                apply_recorded_statement_events(
                    statement_index,
                    &events,
                    &facts.flow.ownership.segments,
                    &mut outcome,
                    &mut diagnostics,
                );
                if transition.exit == typed_trees::statement::TransitionExit::Ordinary {
                    outcomes.push(outcome);
                }
                if let typed_trees::statement::TransitionGuardNode::When(guard) = transition.guard
                    && let Some(case_test) = exact_positive_case_test(program, guard)
                {
                    excluded_case_tests.push(case_test);
                }
            }
            let exhaustive =
                arm_indices.last().is_some_and(|index| {
                    matches!(
                        statements[*index],
                        StatementNode::Transition(typed_trees::statement::TableTransition {
                            guard: typed_trees::statement::TransitionGuardNode::Always,
                            ..
                        })
                    )
                }) || case_transition_run_is_exhaustive(program, statements, &arm_indices);
            if !exhaustive {
                outcomes.push(entry);
            }
            has_ordinary_exit = !outcomes.is_empty();

            if let Some(first) = outcomes.first() {
                for place_index in 0..places.len() {
                    let live = first[place_index].live;
                    if places[place_index].multiplicity == Multiplicity::Linear
                        && outcomes
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

        if has_ordinary_exit {
            for place in places.iter().filter(|place| {
                place.multiplicity == Multiplicity::Linear
                    && place.live
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
    }

    if !owned_selections_match(&selected_replay.flow.ownership, &facts.flow.ownership) {
        diagnostics.push(Diagnostic::error(
            "owned selection receipts differ from source ownership replay",
        ));
    }
    for (_, receipt) in selected_replay.flow.ownership.owned_selections.iter() {
        let sources = selected_replay
            .flow
            .ownership
            .selection_sources
            .span_or_empty(receipt.sources);
        if facts.flow.ownership.permissions.iter().any(|(_, event)| {
            if event.machine_symbol != receipt.machine
                || event.state_symbol != receipt.state
                || event.access != PermissionAccess::Owned
            {
                return false;
            }
            let statement = permission_event_statement_index(event.source);
            statement == Some(receipt.statement_ordinal as usize)
                || (sources
                    .iter()
                    .any(|source| event.root == facts::PlaceRoot::Symbol(source.symbol))
                    && (event.source == PermissionEventSource::StateExit
                        || statement.is_some_and(|statement| {
                            statement > receipt.statement_ordinal as usize
                        })))
        }) {
            diagnostics.push(Diagnostic::error("owned selection acquired unconditional transfer, establishment, or residual cleanup events"));
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// The replay populates a fresh ownership arena, so span offsets into
/// `segments`, `selection_sources`, and `selection_transfers` are positional
/// rather than semantic. Compare each recorded receipt field-for-field,
/// resolving spans through the side that produced them.
fn owned_selections_match(
    replay: &checked_trees::FlowOwnershipFacts,
    recorded: &checked_trees::FlowOwnershipFacts,
) -> bool {
    let replay_receipts = replay
        .owned_selections
        .iter()
        .map(|(_, receipt)| receipt)
        .collect::<Vec<_>>();
    let recorded_receipts = recorded
        .owned_selections
        .iter()
        .map(|(_, receipt)| receipt)
        .collect::<Vec<_>>();
    if replay_receipts.len() != recorded_receipts.len() {
        return false;
    }
    for (replayed_receipt, recorded_receipt) in replay_receipts.into_iter().zip(recorded_receipts) {
        if replayed_receipt.machine != recorded_receipt.machine
            || replayed_receipt.state != recorded_receipt.state
            || replayed_receipt.statement_ordinal != recorded_receipt.statement_ordinal
            || replayed_receipt.expression != recorded_receipt.expression
            || replayed_receipt.destination != recorded_receipt.destination
            || replayed_receipt.type_reference != recorded_receipt.type_reference
            || replayed_receipt.death != recorded_receipt.death
            || replay
                .selection_sources
                .span_or_empty(replayed_receipt.sources)
                != recorded
                    .selection_sources
                    .span_or_empty(recorded_receipt.sources)
        {
            return false;
        }
        let replayed_transfers = replay
            .selection_transfers
            .span_or_empty(replayed_receipt.transfers);
        let recorded_transfers = recorded
            .selection_transfers
            .span_or_empty(recorded_receipt.transfers);
        if replayed_transfers.len() != recorded_transfers.len() {
            return false;
        }
        for (replayed_transfer, recorded_transfer) in
            replayed_transfers.iter().zip(recorded_transfers)
        {
            if replayed_transfer.expression != recorded_transfer.expression
                || replayed_transfer.source_arm != recorded_transfer.source_arm
                || replayed_transfer.source != recorded_transfer.source
                || replay.segments.span_or_empty(replayed_transfer.path)
                    != recorded.segments.span_or_empty(recorded_transfer.path)
            {
                return false;
            }
        }
    }
    true
}

fn append_unresolved_state_result_mapping_diagnostics(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    events: &[&FlowPermissionEventFact],
    ownership: &checked_trees::FlowOwnershipFacts,
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
        let typed_trees::expression::ExpressionNode::Call(call) =
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
        let mapped_from_call = ownership.permissions.iter().any(|(_, candidate)| {
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
        let receiving_path = ownership.segments.span_or_empty(event.segments);
        let mapped_by_checked_outcome = ownership
            .claim_outcome_maps
            .iter()
            .filter(|(_, map)| map.state_symbol == target_state.symbol)
            .flat_map(|(_, map)| ownership.claim_outcome_entries.span_or_empty(map.entries))
            .any(|entry| {
                ownership.segments.span_or_empty(entry.output_segments) == receiving_path
                    && matches!(
                        entry.source,
                        FlowClaimOutcomeSource::Established {
                            claim_identity,
                            provenance,
                        } if claim_identity == event.claim_identity
                            && provenance == event.provenance
                    )
            });
        if mapped_from_call || mapped_by_checked_outcome {
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

pub(crate) fn initial_linear_places(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
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
        let claims = linear_claim_frontier(program, parameter.type_reference);
        // Whole claim-free affine parameters are live on entry, but establish
        // no linear claim. Track their moves so a transferred parameter cannot
        // be moved again or disposed by the caller at exit.
        if claims.is_empty()
            && type_multiplicity(program, parameter.type_reference) == Multiplicity::Affine
            && matches!(
                program
                    .type_reference_table
                    .type_reference(parameter.type_reference),
                TypeReferenceNode::Named { .. }
                    | TypeReferenceNode::Generic { .. }
                    | TypeReferenceNode::FixedArray { .. }
            )
        {
            places.push(LinearPlace {
                symbol: parameter.symbol,
                name: parameter.name.as_str().to_owned(),
                path: Vec::new(),
                multiplicity: Multiplicity::Affine,
                claim_identity: None,
                provenance: None,
                live: true,
                ever_established: true,
                conditional: false,
            });
        }
        for claim in claims {
            places.push(LinearPlace {
                symbol: parameter.symbol,
                name: claim_place_name(program, parameter.name.as_str(), &claim.path),
                path: claim.path,
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
        let claims = linear_claim_frontier(program, local.type_reference);
        for claim in &claims {
            places.push(LinearPlace {
                symbol: local.symbol,
                name: claim_place_name(program, local.name.as_str(), &claim.path),
                path: claim.path.clone(),
                multiplicity: claim.multiplicity,
                claim_identity: None,
                provenance: None,
                live: false,
                ever_established: false,
                conditional: claim.conditional,
            });
        }
        // An explicitly initialized affine destination owns one whole value,
        // independently of which validated expression produced it. The ordinary
        // LocalData write establishes this root; later replacement settles the
        // old value before establishing the new one, including mutable locals.
        // Absent initializers remain outside this roster: partial zero-fill
        // construction needs its own initialization judgment.
        if claims.is_empty()
            && type_multiplicity(program, local.type_reference) == Multiplicity::Affine
            && local.initial_value.is_valid()
            && matches!(
                program
                    .type_reference_table
                    .type_reference(local.type_reference),
                typed_trees::types::TypeReferenceNode::Named { .. }
                    | typed_trees::types::TypeReferenceNode::Generic { .. }
                    | typed_trees::types::TypeReferenceNode::FixedArray { .. }
            )
        {
            places.push(LinearPlace {
                symbol: local.symbol,
                name: local.name.as_str().to_owned(),
                path: Vec::new(),
                multiplicity: Multiplicity::Affine,
                claim_identity: None,
                provenance: None,
                live: false,
                ever_established: false,
                conditional: false,
            });
        }
    }
    places
}

pub(crate) fn linear_claim_frontier(
    program: &typed_trees::TypedTrees,
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
    program: &typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    path: &[facts::PlaceSegment],
    visiting: &mut Vec<SymbolHandle>,
    claims: &mut Vec<LinearClaimTemplate>,
) {
    if !type_reference.is_valid() {
        return;
    }
    let multiplicity = type_multiplicity_with_substitutions(program, type_reference, substitutions);
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            if multiplicity == Multiplicity::Linear {
                claims.push(LinearClaimTemplate {
                    path: path.to_vec(),
                    type_reference,
                    multiplicity,
                    conditional: path
                        .iter()
                        .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. })),
                });
                return;
            }
            append_linear_claim_frontier(
                program,
                *base_type,
                substitutions,
                path,
                visiting,
                claims,
            );
            return;
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: typed_trees::types::FixedArrayLength::Literal(length),
        } => {
            for index in 0..*length {
                let mut element_path = path.to_vec();
                element_path.push(facts::PlaceSegment::FixedIndex { index });
                append_linear_claim_frontier(
                    program,
                    *element_type,
                    substitutions,
                    &element_path,
                    visiting,
                    claims,
                );
            }
            return;
        }
        TypeReferenceNode::Named { symbol, .. } => {
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
        }
        _ => {}
    }

    if multiplicity == Multiplicity::Linear {
        claims.push(LinearClaimTemplate {
            path: path.to_vec(),
            type_reference,
            multiplicity,
            conditional: path
                .iter()
                .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. })),
        });
        return;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { .. } => unreachable!("handled before multiplicity"),
        TypeReferenceNode::Named { symbol, name } => {
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
                        matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
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
        TypeReferenceNode::FixedArray { .. } => {
            // Const-parameter lengths must become literal before this stage
            // can enumerate the complete fixed-index ownership frontier.
        }
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Unit => {}
    }
}

fn append_data_linear_claim_frontier(
    program: &typed_trees::TypedTrees,
    definition: &typed_trees::data::DataDefinition,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    path: &[facts::PlaceSegment],
    visiting: &mut Vec<SymbolHandle>,
    claims: &mut Vec<LinearClaimTemplate>,
) {
    if visiting.contains(&definition.symbol) {
        return;
    }
    visiting.push(definition.symbol);
    for member in program.data_members(definition) {
        match member {
            typed_trees::data::DataMember::Field(field) => {
                let mut field_path = path.to_vec();
                field_path.push(facts::PlaceSegment::Field {
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
            typed_trees::data::DataMember::Variant(variant) => {
                let mut case_path = path.to_vec();
                case_path.push(facts::PlaceSegment::Case {
                    variant: variant.symbol,
                });
                for field in program.data_payload_fields(variant) {
                    let mut field_path = case_path.clone();
                    field_path.push(facts::PlaceSegment::Field {
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
            }
        }
    }
    visiting.pop();
}

fn claim_place_name(
    program: &typed_trees::TypedTrees,
    root: &str,
    path: &[facts::PlaceSegment],
) -> String {
    let mut name = root.to_owned();
    for segment in path {
        match segment {
            facts::PlaceSegment::Case { variant } => {
                let case = program.data_definitions().iter().find_map(|definition| {
                    program.data_members(definition).iter().find_map(|member| {
                        let typed_trees::data::DataMember::Variant(candidate) = member else {
                            return None;
                        };
                        (candidate.symbol == *variant).then_some(candidate.name.as_str())
                    })
                });
                name.push_str("::");
                name.push_str(case.unwrap_or("<case>"));
            }
            facts::PlaceSegment::Field { symbol } => {
                let field = data_field_name(program, *symbol);
                name.push('.');
                name.push_str(field.unwrap_or("<field>"));
            }
            facts::PlaceSegment::FixedIndex { index } => {
                name.push('[');
                name.push_str(&index.to_string());
                name.push(']');
            }
            facts::PlaceSegment::FixedRange { start, end } => {
                name.push('[');
                name.push_str(&start.to_string());
                name.push_str("..");
                name.push_str(&end.to_string());
                name.push(']');
            }
            facts::PlaceSegment::Index { .. } => name.push_str("[<index>]"),
        }
    }
    name
}

pub(crate) fn apply_recorded_state_entry_events(
    events: &[&FlowPermissionEventFact],
    segments: &arena::Arena<facts::PlaceSegment>,
    places: &mut [LinearPlace],
) {
    for event in events.iter().copied().filter(|event| {
        event.source == PermissionEventSource::StateEntry
            && event.kind == PermissionEventKind::Establish
    }) {
        let facts::PlaceRoot::Symbol(symbol) = event.root else {
            continue;
        };
        let event_path = segments.span_or_empty(event.segments);
        let Some(place) = places
            .iter_mut()
            .find(|place| place.symbol == symbol && place.path.as_slice() == event_path)
        else {
            continue;
        };
        place.live = event.obligation_live || place.multiplicity == Multiplicity::Affine;
        place.ever_established = true;
        place.claim_identity = Some(event.claim_identity);
        place.provenance = Some(event.provenance);
    }
}

pub(crate) fn apply_recorded_statement_events(
    statement_index: usize,
    events: &[&FlowPermissionEventFact],
    segments: &arena::Arena<facts::PlaceSegment>,
    places: &mut [LinearPlace],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for event in events.iter().copied().filter(|event| {
        permission_event_statement_index(event.source) == Some(statement_index)
            && event.kind != PermissionEventKind::AffineDrop
    }) {
        let facts::PlaceRoot::Symbol(symbol) = event.root else {
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
                } else if !place.live {
                    let ownership = if place.multiplicity == Multiplicity::Affine {
                        "affine"
                    } else {
                        "linear"
                    };
                    diagnostics.push(Diagnostic::error(format!(
                        "{ownership} value `{}` was already transferred or consumed; it cannot be moved here",
                        place.name
                    )));
                } else {
                    place.live = false;
                }
            }
            PermissionEventKind::Establish => {
                if place.live && place.multiplicity == Multiplicity::Linear {
                    diagnostics.push(Diagnostic::error(format!(
                        "assignment would overwrite live linear value `{}`; consume or transfer the existing obligation first",
                        place.name
                    )));
                }
                place.live = event.obligation_live || place.multiplicity == Multiplicity::Affine;
                place.ever_established = true;
                place.claim_identity = Some(event.claim_identity);
                place.provenance = Some(event.provenance);
            }
            PermissionEventKind::AffineDrop => {}
        }
    }
}

pub(crate) fn permission_event_statement_index(source: PermissionEventSource) -> Option<usize> {
    match source {
        PermissionEventSource::Statement { statement_index }
        | PermissionEventSource::Call {
            statement_index, ..
        } => Some(statement_index),
        PermissionEventSource::StateEntry | PermissionEventSource::StateExit => None,
    }
}

pub(crate) fn append_borrow_permission_events(
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
            let source = if weakening.reason == checked_trees::FlowBorrowWeakeningReason::StateExit
            {
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
    loan_handle: arena::Handle<checked_trees::BorrowLoanFact>,
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
        checked_trees::BorrowAccessKind::Read => {
            (Multiplicity::Unrestricted, PermissionAccess::Shared)
        }
        checked_trees::BorrowAccessKind::Mutable => {
            (Multiplicity::Affine, PermissionAccess::Exclusive)
        }
        checked_trees::BorrowAccessKind::WriteOnly => {
            // A write-only borrow is exclusive and therefore follows the same
            // affine use discipline as a mutable borrow. Its observation
            // restriction is validated separately.
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
        root: facts::PlaceRoot::Symbol(loan.root_symbol),
        segments,
        obligation_live: false,
    });
}

fn permission_source_from_invalidation(
    source: checked_trees::FlowInvalidationSource,
) -> PermissionEventSource {
    match source {
        checked_trees::FlowInvalidationSource::Statement { statement_index } => {
            PermissionEventSource::Statement { statement_index }
        }
        checked_trees::FlowInvalidationSource::Call {
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
pub(crate) fn apply_statement_permission_production(
    program: &typed_trees::TypedTrees,
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
    let calls = facts
        .flow
        .control
        .states
        .iter()
        .find_map(|(_, state)| {
            (state.machine_symbol == machine_symbol && state.state_symbol == state_symbol)
                .then(|| facts.flow.control.calls.span_or_empty(state.calls))
        })
        .unwrap_or_default()
        .to_vec();
    let mut statement_moves = moves
        .iter()
        .filter(|event| {
            !event.source_arm.is_valid()
                && event_statement_index(event.source) == Some(statement_index)
        })
        .collect::<Vec<_>>();
    // Call ordinals are authored preorder coordinates, not evaluation order.
    // Reuse the captured control-flow order: nested producers execute before
    // their consumer; an initializer's destination is established afterwards.
    statement_moves.sort_by_key(|event| match event.source {
        FlowOwnershipEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => calls
            .iter()
            .position(|call| {
                call.statement_index == statement_index
                    && call.call_ordinal == call_ordinal
                    && call.target_symbol == target_symbol
            })
            .unwrap_or(calls.len()),
        FlowOwnershipEventSource::Statement { .. } => calls.len(),
    });
    for event in statement_moves {
        if matches!(event.root, facts::PlaceRoot::Expression(_)) {
            temporary_results::append_whole_affine_transfer(
                program,
                facts,
                machine_symbol,
                state_symbol,
                &calls,
                event,
                permission_events,
            );
            continue;
        }
        let facts::PlaceRoot::Symbol(symbol) = event.root else {
            continue;
        };
        let event_path = facts
            .flow
            .ownership
            .segments
            .span_or_empty(event.segments)
            .to_vec();
        select_static_case_alternative(symbol, &event_path, places);
        if matches!(event.source, FlowOwnershipEventSource::Statement { .. })
            && written_targets.iter().any(|target| {
                target.root == event.root && target.destination_path.as_slice() == event_path
            })
        {
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
            projected_affine::append_transfer(
                program,
                facts,
                machine_symbol,
                state_symbol,
                event,
                &event_path,
                places,
                permission_events,
            );
            continue;
        }
        let kind = permission_kind_for_move(program, facts, machine_symbol, state_symbol, event);
        for index in matching {
            let claim_path = places[index].path.clone();
            if places[index].ever_established
                && !places[index].live
                && places[index].conditional
                && !event_path
                    .iter()
                    .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }))
            {
                continue;
            }
            let segments = facts.flow.ownership.segments.insert_many(claim_path);
            let place = &mut places[index];
            let obligation_live = place.live && place.multiplicity == Multiplicity::Linear;
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
        let facts::PlaceRoot::Symbol(symbol) = target.root else {
            continue;
        };
        let place_index = target.place_index;
        let obligation_live = target.obligation_live;
        let claim_identity = target.claim_identity.unwrap_or_else(|| {
            if obligation_live {
                {
                    claim_identities.mint(
                        machine_symbol,
                        state_symbol,
                        PermissionEventSource::Statement { statement_index },
                    )
                }
            } else {
                PermissionClaimIdentity::Unknown
            }
        });
        let provenance = target.provenance;
        let claim_path = places[place_index].path.clone();
        let segments = facts.flow.ownership.segments.insert_many(claim_path);
        let place = &mut places[place_index];
        debug_assert_eq!(place.symbol, symbol);
        // Replacement settles the old affine value before establishing the
        // new one. A source transferred into the replacement call is already
        // dead here and must not also receive a caller-side drop.
        if place.live && place.multiplicity == Multiplicity::Affine {
            permission_events.push(FlowPermissionEventFact {
                machine_symbol,
                state_symbol,
                source: PermissionEventSource::Statement { statement_index },
                kind: PermissionEventKind::AffineDrop,
                multiplicity: Multiplicity::Affine,
                access: PermissionAccess::Owned,
                claim_identity: place
                    .claim_identity
                    .unwrap_or(PermissionClaimIdentity::Unknown),
                provenance: place.provenance.unwrap_or(PermissionProvenance::Unknown),
                root: facts::PlaceRoot::Symbol(symbol),
                segments,
                obligation_live: false,
            });
        }
        place.live = obligation_live || place.multiplicity == Multiplicity::Affine;
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
            root: facts::PlaceRoot::Symbol(symbol),
            segments,
            obligation_live,
        });
    }
}

fn move_selects_claim(event_path: &[facts::PlaceSegment], claim: &LinearPlace) -> bool {
    claim.path.starts_with(event_path)
        || (claim.conditional && event_path.starts_with(claim.path.as_slice()))
}

pub(crate) fn select_static_case_alternative(
    symbol: SymbolHandle,
    event_path: &[facts::PlaceSegment],
    places: &mut [LinearPlace],
) {
    let Some((case_index, selected_variant)) =
        event_path
            .iter()
            .enumerate()
            .find_map(|(index, segment)| match segment {
                facts::PlaceSegment::Case { variant } => Some((index, *variant)),
                _ => None,
            })
    else {
        return;
    };
    let prefix = &event_path[..case_index];
    for place in places {
        if place.symbol != symbol
            || place.path.get(..case_index) != Some(prefix)
            || place.path.get(case_index)
                == Some(&facts::PlaceSegment::Case {
                    variant: selected_variant,
                })
        {
            continue;
        }
        if matches!(
            place.path.get(case_index),
            Some(facts::PlaceSegment::Case { .. })
        ) {
            place.live = false;
        }
    }
}

pub(crate) fn established_provenance(
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

pub(crate) fn event_statement_index(source: FlowOwnershipEventSource) -> Option<usize> {
    match source {
        FlowOwnershipEventSource::Statement { statement_index }
        | FlowOwnershipEventSource::Call {
            statement_index, ..
        } => Some(statement_index),
    }
}

pub(crate) fn permission_source(source: FlowOwnershipEventSource) -> PermissionEventSource {
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
    }
}

pub(crate) fn permission_kind_for_move(
    program: &typed_trees::TypedTrees,
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
    let event_segments = facts.flow.ownership.segments.span_or_empty(event.segments);
    if arguments.len() == parameters.len() {
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
    } else if let Some(place) = crate::flow::owned_method_receiver_place(
        program,
        state_symbol,
        statement_index,
        &call_site,
        parameters,
        SymbolHandle::invalid(),
    ) && place.root == event.root
        && place.segments.as_slice() == event_segments
    {
        return if type_carries_linear_obligation(program, target_state.return_type) {
            PermissionEventKind::Transfer
        } else {
            PermissionEventKind::Consume
        };
    }
    PermissionEventKind::Transfer
}

fn written_linear_targets(
    program: &typed_trees::TypedTrees,
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
                    root: facts::PlaceRoot::Symbol(local.symbol),
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
    let facts::PlaceRoot::Symbol(symbol) = target.root else {
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
                obligation_live: tracked.multiplicity != Multiplicity::Affine
                    && expression_establishes_obligation(
                        program,
                        state_symbol,
                        statement_index,
                        value,
                        relative_path,
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
    program: &typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
    relative_path: &[facts::PlaceSegment],
    places: &[LinearPlace],
) -> Option<PermissionClaimIdentity> {
    if relative_path.is_empty() {
        match program.expression_table.expression(expression) {
            typed_trees::expression::ExpressionNode::Call(call) => {
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
            typed_trees::expression::ExpressionNode::StructLiteral(literal) => {
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
            typed_trees::expression::ExpressionNode::ArrayLiteral(values) => {
                return common_permission_claim_identity(
                    program
                        .expression_table
                        .expression_handles(*values)
                        .iter()
                        .filter_map(|value| {
                            expression_permission_claim_identity_for_claim(
                                program,
                                state_symbol,
                                statement_index,
                                *value,
                                &[],
                                places,
                            )
                        }),
                );
            }
            _ => {}
        }
    }

    if let typed_trees::expression::ExpressionNode::StructLiteral(literal) =
        program.expression_table.expression(expression)
        && let Some(facts::PlaceSegment::Case { variant }) = relative_path.first()
    {
        if literal_variant(program, literal).map(|candidate| candidate.symbol) != Some(*variant) {
            return None;
        }
        return expression_permission_claim_identity_for_claim(
            program,
            state_symbol,
            statement_index,
            expression,
            &relative_path[1..],
            places,
        );
    }

    if let typed_trees::expression::ExpressionNode::ArrayLiteral(values) =
        program.expression_table.expression(expression)
        && let Some(facts::PlaceSegment::FixedIndex { index }) = relative_path.first()
    {
        let value = *program
            .expression_table
            .expression_handles(*values)
            .get(*index)?;
        return expression_permission_claim_identity_for_claim(
            program,
            state_symbol,
            statement_index,
            value,
            &relative_path[1..],
            places,
        );
    }

    if let typed_trees::expression::ExpressionNode::StructLiteral(literal) =
        program.expression_table.expression(expression)
        && let Some(facts::PlaceSegment::Field { symbol }) = relative_path.first()
    {
        let field_name = data_field_name(program, *symbol)?;
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
            typed_trees::expression::ExpressionNode::Call(_)
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
    let facts::PlaceRoot::Symbol(symbol) = source.root else {
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
    program: &typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
    relative_path: &[facts::PlaceSegment],
    places: &[LinearPlace],
) -> Option<PermissionProvenance> {
    if relative_path.is_empty()
        && matches!(
            program.expression_table.expression(expression),
            typed_trees::expression::ExpressionNode::Call(_)
                | typed_trees::expression::ExpressionNode::StructLiteral(_)
                | typed_trees::expression::ExpressionNode::ArrayLiteral(_)
        )
    {
        return validation::expression_permission_provenance(
            program,
            expression,
            &mut |candidate| {
                Ok(expression_permission_provenance_for_claim(
                    program,
                    state_symbol,
                    statement_index,
                    candidate,
                    &[],
                    places,
                ))
            },
        )
        .ok()
        .flatten();
    }

    if let typed_trees::expression::ExpressionNode::StructLiteral(literal) =
        program.expression_table.expression(expression)
        && let Some(facts::PlaceSegment::Case { variant }) = relative_path.first()
    {
        if literal_variant(program, literal).map(|candidate| candidate.symbol) != Some(*variant) {
            return None;
        }
        return expression_permission_provenance_for_claim(
            program,
            state_symbol,
            statement_index,
            expression,
            &relative_path[1..],
            places,
        );
    }

    if let typed_trees::expression::ExpressionNode::ArrayLiteral(values) =
        program.expression_table.expression(expression)
        && let Some(facts::PlaceSegment::FixedIndex { index }) = relative_path.first()
    {
        let value = *program
            .expression_table
            .expression_handles(*values)
            .get(*index)?;
        return expression_permission_provenance_for_claim(
            program,
            state_symbol,
            statement_index,
            value,
            &relative_path[1..],
            places,
        );
    }

    if let typed_trees::expression::ExpressionNode::StructLiteral(literal) =
        program.expression_table.expression(expression)
        && let Some(facts::PlaceSegment::Field { symbol }) = relative_path.first()
    {
        let field_name = data_field_name(program, *symbol)?;
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
            typed_trees::expression::ExpressionNode::Call(_)
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
    let facts::PlaceRoot::Symbol(symbol) = source.root else {
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

/// Discover ordinary affine cleanup directly from typed ownership. Locals drop in
/// reverse declaration order, followed by owned by-value parameters in reverse
/// declaration order, exactly matching the language's cleanup order. Linear
/// and conditional roots are excluded because their path-sensitive settlement
/// is represented by the permission events produced above.
pub(crate) fn append_affine_cleanup_permission_events(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    machine_symbol: SymbolHandle,
    tracked_places: &[LinearPlace],
    permission_events: &mut Vec<FlowPermissionEventFact>,
) {
    let mut append = |symbol: SymbolHandle, type_reference: TypeReferenceHandle| {
        let tracked_root = tracked_places
            .iter()
            .find(|place| place.symbol == symbol && place.path.is_empty());
        if let Some(place) = tracked_root {
            if place.multiplicity != Multiplicity::Affine || !place.live {
                return;
            }
        } else if tracked_places.iter().any(|place| place.symbol == symbol)
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
            provenance: tracked_root
                .and_then(|place| place.provenance)
                .unwrap_or(PermissionProvenance::Unknown),
            root: facts::PlaceRoot::Symbol(symbol),
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
