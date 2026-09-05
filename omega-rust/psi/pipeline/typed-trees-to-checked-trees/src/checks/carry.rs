use diagnostics::Diagnostic;
use facts::{FactPayload, FactPlace, QualificationEvidence};
use language_semantics::{CarryPolicy, CarrySuspension};

mod activation;
mod intra_statement;

struct ClaimCarryContext<'facts> {
    semantic: &'facts facts::FactPlan,
    ownership: &'facts checked_trees::FlowOwnershipFacts,
    claim_policies: &'facts [checked_trees::ClaimCarryPolicyFact],
    state_symbol: symbols::SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
    entry_contexts: Vec<facts::FactContextHandle>,
}

impl ClaimCarryContext<'_> {
    /// A place with no claim entry follows its structural/type-wide policy.
    /// Once a compiler-owned carry fact is present, the underlying claim was
    /// born strict and only its exact positive permissions may relax it.
    /// Distinct provenances intersect so a combined claim cannot borrow a
    /// permissive axis from a sibling.
    fn effective_policy(
        &self,
        program: &typed_trees::TypedTrees,
        value_name: &str,
        structural: CarryPolicy,
        claim_identities: &[language_semantics::PermissionClaimIdentity],
    ) -> CarryPolicy {
        let claim_policies = claim_identities
            .iter()
            .filter_map(|identity| {
                self.claim_policies
                    .iter()
                    .find(|fact| fact.claim_identity == *identity)
                    .map(|fact| fact.effective)
            })
            .collect::<Vec<_>>();
        if !claim_policies.is_empty() {
            return claim_policies
                .into_iter()
                .fold(CarryPolicy::PERMISSIVE, CarryPolicy::intersect);
        }

        let mut origins = Vec::<(QualificationEvidence, CarryPolicy)>::new();

        for context_handle in &self.entry_contexts {
            let context = self.semantic.contexts.get(*context_handle);
            for fact in self.semantic.context_view(context).facts() {
                if fact.evidence.origin == language_semantics::QualificationEvidenceOrigin::None {
                    // Declaration-shaped requires/ensures facts describe an
                    // obligation. Only an established fact with retained
                    // evidence denotes a live claim entry.
                    continue;
                }
                let permission = match fact.payload {
                    FactPayload::CarryPermission { permission, .. }
                    | FactPayload::ContractCarryPermission { permission, .. } => Some(permission),
                    FactPayload::CarryOrigin { .. } => None,
                    _ => continue,
                };
                let FactPlace::Place(place) = fact.place else {
                    continue;
                };
                if crate::labels::canonical_place_label(
                    program,
                    self.semantic,
                    self.semantic.places.get(place),
                ) != value_name
                {
                    continue;
                }

                if let Some((_, policy)) = origins
                    .iter_mut()
                    .find(|(evidence, _)| *evidence == fact.evidence)
                {
                    if let Some(permission) = permission {
                        *policy = permission.relax(*policy);
                    }
                } else {
                    origins.push((
                        fact.evidence,
                        permission
                            .map(|permission| permission.relax(CarryPolicy::STRICT))
                            .unwrap_or(CarryPolicy::STRICT),
                    ));
                }
            }
        }

        if origins.is_empty() {
            structural
        } else {
            origins
                .into_iter()
                .fold(CarryPolicy::PERMISSIVE, |combined, (_, policy)| {
                    combined.intersect(policy)
                })
        }
    }

    fn live_claim_identities(
        &self,
        program: &typed_trees::TypedTrees,
        value_name: &str,
    ) -> Vec<language_semantics::PermissionClaimIdentity> {
        let mut latest_by_path = Vec::<(
            String,
            (usize, usize, usize),
            language_semantics::PermissionClaimIdentity,
        )>::new();

        for (event_index, event) in self
            .ownership
            .permissions
            .iter()
            .map(|(_, event)| event)
            .enumerate()
        {
            if event.state_symbol != self.state_symbol
                || event.access != language_semantics::PermissionAccess::Owned
                || event.kind != language_semantics::PermissionEventKind::Establish
                || !event.obligation_live
                || event.claim_identity == language_semantics::PermissionClaimIdentity::Unknown
            {
                continue;
            }
            let Some(order) = permission_event_order_before_call(
                event.source,
                self.statement_index,
                self.call_ordinal,
                event_index,
            ) else {
                continue;
            };
            let label = crate::labels::canonical_place_label_from_parts(
                program,
                event.root,
                self.ownership.segments.span_or_empty(event.segments),
            );
            if label != value_name
                && !label
                    .strip_prefix(value_name)
                    .is_some_and(|suffix| suffix.starts_with('.') || suffix.starts_with('['))
            {
                continue;
            }
            if let Some((_, previous_order, identity)) = latest_by_path
                .iter_mut()
                .find(|(candidate, _, _)| *candidate == label)
            {
                if order >= *previous_order {
                    *previous_order = order;
                    *identity = event.claim_identity;
                }
            } else {
                latest_by_path.push((label, order, event.claim_identity));
            }
        }

        latest_by_path
            .into_iter()
            .fold(Vec::new(), |mut identities, (_, _, identity)| {
                if !identities.contains(&identity) {
                    identities.push(identity);
                }
                identities
            })
    }
}

fn permission_event_order_before_call(
    source: language_semantics::PermissionEventSource,
    statement_index: usize,
    call_ordinal: usize,
    event_index: usize,
) -> Option<(usize, usize, usize)> {
    use language_semantics::PermissionEventSource;
    match source {
        PermissionEventSource::StateEntry => Some((0, 0, event_index)),
        PermissionEventSource::Statement {
            statement_index: event_statement,
        } if event_statement <= statement_index => {
            Some((event_statement.saturating_add(1), usize::MAX, event_index))
        }
        PermissionEventSource::Call {
            statement_index: event_statement,
            call_ordinal: event_call,
            ..
        } if event_statement < statement_index
            || (event_statement == statement_index && event_call <= call_ordinal) =>
        {
            Some((event_statement.saturating_add(1), event_call, event_index))
        }
        PermissionEventSource::Statement { .. }
        | PermissionEventSource::Call { .. }
        | PermissionEventSource::StateExit => None,
    }
}

struct CrossingAccumulator {
    effective: CarryPolicy,
    live_values: Vec<checked_trees::SuspensionCrossingLiveValueFact>,
}

impl Default for CrossingAccumulator {
    fn default() -> Self {
        Self {
            effective: CarryPolicy::PERMISSIVE,
            live_values: Vec::new(),
        }
    }
}

/// Reject a call that may suspend while a suspension-forbidden lexical value
/// remains live in the caller activation. This is deliberately a local check:
/// suspension joins the value policy with the callee's inferred suspension
/// possibility
/// here. The independent activation-wide analysis derives CPU/thread
/// preservation obligations without publishing a provider preemption mode.
pub(super) fn check_suspension_carry(
    program: &typed_trees::TypedTrees,
    facts: &mut checked_trees::CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    facts.carry.claim_policies = derive_claim_carry_policies(facts);
    let activation_wide_carry =
        activation::build_machine_activation_carry_facts(program, &facts.carry, &facts.semantic);
    facts.carry.activation_wide_carry = activation_wide_carry;
    let mut diagnostics = Vec::new();
    let mut suspension_crossings = Vec::new();

    for state_borrows in facts.borrow.states.iter().map(|(_, state)| state) {
        let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == state_borrows.machine_symbol)
        else {
            continue;
        };
        let Some(state) = program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == state_borrows.state_symbol)
        else {
            continue;
        };
        let Some(state_flow) = facts
            .flow
            .control
            .states
            .iter()
            .find_map(|(_, state_flow)| {
                (state_flow.machine_symbol == machine.symbol
                    && state_flow.state_symbol == state.symbol)
                    .then_some(state_flow)
            })
        else {
            continue;
        };

        for call in facts.borrow.calls.span_or_empty(state_borrows.calls) {
            let Some(call_flow) = facts
                .flow
                .control
                .calls
                .span_or_empty(state_flow.calls)
                .iter()
                .find(|flow| {
                    flow.statement_index == call.statement_index
                        && flow.call_ordinal == call.call_ordinal
                })
            else {
                continue;
            };
            if !call_flow.suspension.direct_may_suspend
                && !call_flow.suspension.transitive_may_suspend
            {
                continue;
            }

            let call_site = crate::find_call_site(
                program,
                machine.symbol,
                state.symbol,
                call.statement_index,
                call.call_ordinal,
            );
            let mut crossing = CrossingAccumulator::default();
            let claim_carry = ClaimCarryContext {
                semantic: &facts.semantic,
                ownership: &facts.flow.ownership,
                claim_policies: &facts.carry.claim_policies,
                state_symbol: state.symbol,
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
                entry_contexts: facts
                    .flow
                    .state_call_entry_semantic_contexts(
                        state_flow,
                        call.statement_index,
                        call.call_ordinal,
                        call.target_symbol,
                        call.receiver_symbol,
                    )
                    .collect(),
            };

            append_call_carried_argument_diagnostics(
                program,
                call,
                call_site.as_ref(),
                &claim_carry,
                &mut crossing,
                &mut diagnostics,
            );
            append_live_persistent_diagnostics(
                program,
                machine,
                state,
                call,
                call_site.as_ref(),
                &claim_carry,
                &mut crossing,
                &mut diagnostics,
            );
            append_live_parameter_diagnostics(
                program,
                machine,
                state,
                call,
                call_site.as_ref(),
                &claim_carry,
                &mut crossing,
                &mut diagnostics,
            );
            append_live_local_diagnostics(
                program,
                machine,
                state,
                call,
                call_site.as_ref(),
                &claim_carry,
                &mut crossing,
                &mut diagnostics,
            );
            suspension_crossings.push(checked_trees::SuspensionCrossingCarryFact {
                machine: machine.symbol,
                state: state.symbol,
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
                target: call.target_symbol,
                receiver: call
                    .receiver_symbol
                    .is_valid()
                    .then_some(call.receiver_symbol),
                effective: crossing.effective,
                live_values: crossing.live_values,
            });
        }
    }

    facts.carry.suspension_crossings = suspension_crossings;

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn derive_claim_carry_policies(
    facts: &checked_trees::CheckFacts,
) -> Vec<checked_trees::ClaimCarryPolicyFact> {
    let mut origins = Vec::<(
        language_semantics::PermissionClaimIdentity,
        QualificationEvidence,
        CarryPolicy,
    )>::new();

    for (_, fact) in facts.semantic.facts.iter() {
        if fact.evidence.origin == language_semantics::QualificationEvidenceOrigin::None {
            continue;
        }
        let permission = match fact.payload {
            FactPayload::CarryPermission { permission, .. }
            | FactPayload::ContractCarryPermission { permission, .. } => Some(permission),
            FactPayload::CarryOrigin { .. } => None,
            _ => continue,
        };
        let FactPlace::Place(place) = fact.place else {
            continue;
        };
        let place = facts.semantic.places.get(place);
        let place_segments = facts.semantic.place_segments.span_or_empty(place.segments);

        for event in facts
            .flow
            .ownership
            .permissions
            .iter()
            .map(|(_, event)| event)
            .filter(|event| {
                event.access == language_semantics::PermissionAccess::Owned
                    && event.kind == language_semantics::PermissionEventKind::Establish
                    && event.obligation_live
                    && event.claim_identity != language_semantics::PermissionClaimIdentity::Unknown
            })
        {
            if event.root != place.root
                || facts.flow.ownership.segments.span_or_empty(event.segments) != place_segments
            {
                continue;
            }
            if let Some((_, _, policy)) = origins.iter_mut().find(|(identity, evidence, _)| {
                *identity == event.claim_identity && *evidence == fact.evidence
            }) {
                if let Some(permission) = permission {
                    *policy = permission.relax(*policy);
                }
            } else {
                origins.push((
                    event.claim_identity,
                    fact.evidence,
                    permission
                        .map(|permission| permission.relax(CarryPolicy::STRICT))
                        .unwrap_or(CarryPolicy::STRICT),
                ));
            }
        }
    }

    let mut policies = Vec::<checked_trees::ClaimCarryPolicyFact>::new();
    for (claim_identity, _, origin_policy) in origins {
        if let Some(fact) = policies
            .iter_mut()
            .find(|fact| fact.claim_identity == claim_identity)
        {
            fact.effective = fact.effective.intersect(origin_policy);
            fact.contributing_origins = fact.contributing_origins.saturating_add(1);
        } else {
            policies.push(checked_trees::ClaimCarryPolicyFact {
                claim_identity,
                effective: origin_policy,
                contributing_origins: 1,
            });
        }
    }
    policies
}

fn append_call_carried_argument_diagnostics(
    program: &typed_trees::TypedTrees,
    call: &checked_trees::BorrowCallFact,
    call_site: Option<&crate::CallSite<'_>>,
    claim_carry: &ClaimCarryContext<'_>,
    crossing: &mut CrossingAccumulator,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(call_site) = call_site else {
        return;
    };
    let Some(parameters) = crate::call_target_parameters(program, call.target_symbol) else {
        return;
    };
    let arguments = crate::call_site_argument_expressions(program, call_site);
    for (position, (parameter, argument)) in parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .zip(arguments)
        .enumerate()
    {
        let display_name = program.expression_table.display_name(*argument);
        append_if_suspension_forbidden_with_type_parameters(
            program,
            crate::call_target_type_parameters(program, call.target_symbol),
            parameter.type_reference,
            &display_name,
            call,
            claim_carry,
            checked_trees::SuspensionCrossingStorage::CallArgument,
            checked_trees::SuspensionCrossingValueOrigin::CallArgument { position },
            crossing,
            diagnostics,
        );
    }
}

fn append_live_persistent_diagnostics(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    call: &checked_trees::BorrowCallFact,
    call_site: Option<&crate::CallSite<'_>>,
    claim_carry: &ClaimCarryContext<'_>,
    crossing: &mut CrossingAccumulator,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(attached_name) = machine.attached_data.as_ref()
        && let Some(attached) = program
            .data_definitions()
            .iter()
            .find(|definition| definition.name == *attached_name)
    {
        for member in program.data_members(attached) {
            match member {
                typed_trees::data::DataMember::Field(field) => {
                    append_persistent_field_if_live(
                        program,
                        machine,
                        state,
                        call,
                        call_site,
                        field.symbol,
                        field.type_reference,
                        field.name.as_str(),
                        claim_carry,
                        crossing,
                        diagnostics,
                    );
                }
                typed_trees::data::DataMember::Variant(variant) => {
                    for field in program.data_payload_fields(variant) {
                        append_persistent_field_if_live(
                            program,
                            machine,
                            state,
                            call,
                            call_site,
                            field.symbol,
                            field.type_reference,
                            field.name.as_str(),
                            claim_carry,
                            crossing,
                            diagnostics,
                        );
                    }
                }
            }
        }
    }

    for owned in program.machine_owned_data(machine) {
        append_persistent_field_if_live(
            program,
            machine,
            state,
            call,
            call_site,
            owned.symbol,
            owned.type_reference,
            owned.name.as_str(),
            claim_carry,
            crossing,
            diagnostics,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn append_persistent_field_if_live(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    call: &checked_trees::BorrowCallFact,
    call_site: Option<&crate::CallSite<'_>>,
    field_symbol: symbols::SymbolHandle,
    type_reference: typed_trees::types::TypeReferenceHandle,
    field_name: &str,
    claim_carry: &ClaimCarryContext<'_>,
    crossing: &mut CrossingAccumulator,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !persistent_symbol_is_live_after_call(
        program,
        machine,
        state,
        call,
        call_site,
        field_symbol,
        field_name,
    ) {
        return;
    }
    let display_name = format!("self.{field_name}");
    append_if_suspension_forbidden(
        program,
        machine,
        type_reference,
        &display_name,
        call,
        claim_carry,
        checked_trees::SuspensionCrossingStorage::Persistent,
        checked_trees::SuspensionCrossingValueOrigin::Persistent {
            symbol: field_symbol,
        },
        crossing,
        diagnostics,
    );
}

fn persistent_symbol_is_live_after_call(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    call: &checked_trees::BorrowCallFact,
    call_site: Option<&crate::CallSite<'_>>,
    field_symbol: symbols::SymbolHandle,
    field_name: &str,
) -> bool {
    if call_site.is_some_and(|call_site| {
        intra_statement::place_is_used_after_call(
            program,
            state,
            call.statement_index,
            call_site,
            field_symbol,
            field_name,
        )
    }) {
        return true;
    }
    if crate::borrow::place_symbol_is_used_after_statement(
        program,
        state.symbol,
        state.statement_nodes,
        call.statement_index,
        field_symbol,
    ) {
        return true;
    }

    let mut pending = Vec::new();
    append_state_successors_after_statement(
        program,
        machine,
        state,
        call.statement_index,
        &mut pending,
    );
    let mut visited = Vec::new();
    while let Some(state_symbol) = pending.pop() {
        if visited.contains(&state_symbol) {
            continue;
        }
        visited.push(state_symbol);
        let Some(reachable) = program
            .machine_states(machine)
            .iter()
            .find(|candidate| candidate.symbol == state_symbol)
        else {
            continue;
        };
        if crate::borrow::place_symbol_is_used_in_state(program, reachable, field_symbol) {
            return true;
        }
        append_all_state_successors(program, machine, reachable, &mut pending);
    }
    false
}

fn append_state_successors_after_statement(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    successors: &mut Vec<symbols::SymbolHandle>,
) {
    for statement in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .skip(statement_index)
    {
        append_statement_successors(program, machine, state, statement, successors);
    }
}

fn append_all_state_successors(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    successors: &mut Vec<symbols::SymbolHandle>,
) {
    for statement in program.statement_table.statements(state.statement_nodes) {
        append_statement_successors(program, machine, state, statement, successors);
    }
}

fn append_statement_successors(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement: &typed_trees::statement::StatementNode,
    successors: &mut Vec<symbols::SymbolHandle>,
) {
    let typed_trees::statement::StatementNode::Transition(transition) = statement else {
        return;
    };
    append_transition_target_successor(program, machine, state, transition.target, successors);
    if transition.continuation.is_valid() {
        append_transition_target_successor(
            program,
            machine,
            state,
            transition.continuation,
            successors,
        );
    }
}

fn append_transition_target_successor(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    target: typed_trees::statement::TransitionTargetHandle,
    successors: &mut Vec<symbols::SymbolHandle>,
) {
    let symbol = match program.statement_table.transition_target(target) {
        typed_trees::statement::TransitionTargetNode::Named { path, .. } => path.symbol,
        typed_trees::statement::TransitionTargetNode::SelfTarget => state.symbol,
        typed_trees::statement::TransitionTargetNode::Value(_)
        | typed_trees::statement::TransitionTargetNode::Terminal => return,
    };
    if symbol.is_valid()
        && program
            .machine_states(machine)
            .iter()
            .any(|candidate| candidate.symbol == symbol)
    {
        successors.push(symbol);
    }
}

fn append_live_parameter_diagnostics(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    call: &checked_trees::BorrowCallFact,
    call_site: Option<&crate::CallSite<'_>>,
    claim_carry: &ClaimCarryContext<'_>,
    crossing: &mut CrossingAccumulator,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (position, parameter) in program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .enumerate()
    {
        if !crate::borrow::place_is_used_after_statement(
            program,
            state.statement_nodes,
            call.statement_index,
            parameter.symbol,
            parameter.name.as_str(),
        ) && !call_site.is_some_and(|call_site| {
            intra_statement::place_is_used_after_call(
                program,
                state,
                call.statement_index,
                call_site,
                parameter.symbol,
                parameter.name.as_str(),
            )
        }) {
            continue;
        }
        append_if_suspension_forbidden(
            program,
            machine,
            parameter.type_reference,
            parameter.name.as_str(),
            call,
            claim_carry,
            checked_trees::SuspensionCrossingStorage::Parameter,
            checked_trees::SuspensionCrossingValueOrigin::Parameter {
                symbol: parameter.symbol,
                position,
            },
            crossing,
            diagnostics,
        );
    }
}

fn append_live_local_diagnostics(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    call: &checked_trees::BorrowCallFact,
    call_site: Option<&crate::CallSite<'_>>,
    claim_carry: &ClaimCarryContext<'_>,
    crossing: &mut CrossingAccumulator,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let parameter_count = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .count();
    let mut local_position = 0;
    for (definition_index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        if definition_index >= call.statement_index {
            break;
        }
        let typed_trees::statement::StatementNode::LocalData(local) = statement else {
            continue;
        };
        let position = local_position;
        local_position += 1;
        if !crate::borrow::place_is_used_after_statement(
            program,
            state.statement_nodes,
            call.statement_index,
            local.symbol,
            local.name.as_str(),
        ) && !call_site.is_some_and(|call_site| {
            intra_statement::place_is_used_after_call(
                program,
                state,
                call.statement_index,
                call_site,
                local.symbol,
                local.name.as_str(),
            )
        }) {
            continue;
        }
        append_if_suspension_forbidden(
            program,
            machine,
            local.type_reference,
            local.name.as_str(),
            call,
            claim_carry,
            checked_trees::SuspensionCrossingStorage::Local,
            checked_trees::SuspensionCrossingValueOrigin::Local {
                symbol: local.symbol,
                statement_index: definition_index,
                environment_position: parameter_count + position,
            },
            crossing,
            diagnostics,
        );
    }
}

fn append_if_suspension_forbidden(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    type_reference: typed_trees::types::TypeReferenceHandle,
    value_name: &str,
    call: &checked_trees::BorrowCallFact,
    claim_carry: &ClaimCarryContext<'_>,
    storage: checked_trees::SuspensionCrossingStorage,
    origin: checked_trees::SuspensionCrossingValueOrigin,
    crossing: &mut CrossingAccumulator,
    diagnostics: &mut Vec<Diagnostic>,
) {
    append_if_suspension_forbidden_with_type_parameters(
        program,
        program.machine_type_parameters(machine),
        type_reference,
        value_name,
        call,
        claim_carry,
        storage,
        origin,
        crossing,
        diagnostics,
    );
}

fn append_if_suspension_forbidden_with_type_parameters(
    program: &typed_trees::TypedTrees,
    type_parameters: &[typed_trees::data::TypeParameter],
    type_reference: typed_trees::types::TypeReferenceHandle,
    value_name: &str,
    call: &checked_trees::BorrowCallFact,
    claim_carry: &ClaimCarryContext<'_>,
    storage: checked_trees::SuspensionCrossingStorage,
    origin: checked_trees::SuspensionCrossingValueOrigin,
    crossing: &mut CrossingAccumulator,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let structural =
        validation::effective_type_carry_policy(program, type_parameters, type_reference);
    let claims = claim_carry.live_claim_identities(program, value_name);
    let policy = claim_carry.effective_policy(program, value_name, structural, &claims);
    crossing.effective = crossing.effective.intersect(policy);
    crossing
        .live_values
        .push(checked_trees::SuspensionCrossingLiveValueFact {
            type_reference,
            storage,
            origin,
            claims,
            effective: policy,
        });
    if policy.suspension == CarrySuspension::Allowed {
        return;
    }

    let target_name = crate::labels::symbol_name(program, call.target_symbol);
    let message = format!(
        "call to `{target_name}` may suspend while `{value_name}` remains live, but its effective policy is `{policy}`; consume the value before the call or use a suspension-safe carrier"
    );
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message == message)
    {
        return;
    }
    diagnostics.push(Diagnostic::error(message));
}
