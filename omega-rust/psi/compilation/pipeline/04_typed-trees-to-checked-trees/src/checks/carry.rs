//! Suspension demands from live values and their actual loan backing.
//!
//! Flow owns the invocation epoch and current loan roster. Retain its graph,
//! then close continuation liveness over backing dependencies before weighing
//! type floors and resource permissions. Parameters and locals share one
//! source-coordinate table, independent of lowering's sparse operand lanes.
//! Policy judgment follows that closure. Complete stored-leaf coverage joins
//! the checked enclosing-invocation resources to retained owner storage; an
//! incoming referent or incomplete capture still lacks that lifetime join.
//! Canonical lowering carries these demands, not physical preservation receipts.
//! CPU/thread affinity remains conservative and parking still owes stable backing.

use diagnostics::Diagnostic;
use facts::QualificationEvidence;
use language_semantics::{CarryPolicy, CarrySuspension};

mod activation;
mod intra_statement;
mod loans;

/// One source environment coordinate, prepared once per state. Both direct
/// liveness and loan backing use this same namespace; neither reconstructs a
/// local from its name or from a sparse scalar/structural operand lane.
struct StateCarryValue<'program> {
    symbol: symbols::SymbolHandle,
    type_reference: typed_trees::types::TypeReferenceHandle,
    name: &'program str,
    storage: checked_trees::SuspensionCrossingStorage,
    origin: checked_trees::SuspensionCrossingValueOrigin,
}

impl StateCarryValue<'_> {
    fn available_before(&self, statement_index: usize) -> bool {
        match self.origin {
            checked_trees::SuspensionCrossingValueOrigin::Parameter { .. } => true,
            checked_trees::SuspensionCrossingValueOrigin::Local {
                statement_index: definition_index,
                ..
            } => definition_index < statement_index,
            checked_trees::SuspensionCrossingValueOrigin::CallArgument { .. } => false,
        }
    }
}

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
        program: &dyn ::validation::CheckingInput,
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
                if !fact.payload.is_carry_claim() {
                    continue;
                }
                let permission = fact.payload.carry_permission();
                let Some(place) = fact.place.as_place() else {
                    continue;
                };
                if self.semantic.place_label(program, place) != value_name {
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
        program: &dyn ::validation::CheckingInput,
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
            let label = facts::canonical_place_label_from_parts(
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
    obligations: Vec<PendingCarryObligation>,
}

/// Retention closes before policy judgment. A descriptor's conservative type
/// fence and a resource's independent floor must not erase one another.
struct PendingCarryObligation {
    name: String,
    ordinary: CarryPolicy,
    borrow_independent: CarryPolicy,
    requires_borrow_preservation: bool,
}

impl Default for CrossingAccumulator {
    fn default() -> Self {
        Self {
            effective: CarryPolicy::PERMISSIVE,
            live_values: Vec::new(),
            obligations: Vec::new(),
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
    program: &dyn ::validation::CheckingInput,
    facts: &mut checked_trees::CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    facts.carry.claim_policies = derive_claim_carry_policies(facts);
    let activation_wide_carry =
        activation::build_machine_activation_carry_facts(program, &facts.semantic);
    facts.carry.activation_wide_carry = activation_wide_carry;
    let mut diagnostics = Vec::new();
    let mut suspension_crossings = Vec::new();
    let mut suspension_loans = arena::Arena::new();
    let mut suspension_value_loans = arena::Arena::new();
    let mut loan_resources = None;

    for state_borrows in facts.borrow.states.iter().map(|(_, state)| state) {
        let Some(machine) = program.machine_by_symbol(state_borrows.machine_symbol) else {
            continue;
        };
        let Some(state) = program.state_in_machine(machine, state_borrows.state_symbol) else {
            continue;
        };
        let source_values = state_carry_values(program, state);
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

            let call_site = validation::find_call_site(
                program,
                machine.symbol,
                state.symbol,
                call.statement_index,
                call.call_ordinal,
            );
            let mut crossing = CrossingAccumulator::default();
            let resources =
                loan_resources.get_or_insert_with(|| loans::resource_bindings(&facts.borrow));
            let borrows = match loans::retain_call_frontier(
                program,
                &facts.borrow,
                &facts.flow,
                state_borrows,
                call_flow,
                &source_values,
                resources,
                &mut suspension_loans,
            ) {
                Ok(frontier) => frontier,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
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

            collect_call_carried_values(
                program,
                machine,
                call,
                call_site.as_ref(),
                &claim_carry,
                &mut crossing,
            );
            append_live_value_carry_demands(
                program,
                machine,
                state,
                call,
                call_site.as_ref(),
                &source_values,
                &facts.borrow,
                suspension_loans.span_mut_or_empty(borrows.loans),
                &claim_carry,
                &mut crossing,
            );
            for live in &mut crossing.live_values {
                live.borrow_frontier = loans::value_frontier(
                    program,
                    &facts.borrow,
                    &facts.flow,
                    &suspension_loans,
                    borrows.loans,
                    borrows.operands,
                    live,
                    &mut suspension_value_loans,
                );
            }
            resolve_crossing_carry(
                program,
                &facts.borrow,
                &suspension_loans,
                &suspension_value_loans,
                state_borrows,
                call,
                borrows,
                &mut crossing,
                &mut diagnostics,
            );
            suspension_crossings.push(checked_trees::SuspensionCrossingCarryFact {
                machine: machine.symbol,
                state: state.symbol,
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
                target: call.target_symbol,
                effective: crossing.effective,
                live_values: crossing.live_values,
                borrows,
            });
        }
    }

    facts.carry.suspension_crossings = suspension_crossings;
    facts.carry.suspension_loans = suspension_loans;
    facts.carry.suspension_value_loans = suspension_value_loans;

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
        if !fact.payload.is_carry_claim() {
            continue;
        }
        let permission = fact.payload.carry_permission();
        let Some(place) = fact.place.as_place() else {
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

fn collect_call_carried_values(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    call: &checked_trees::BorrowCallFact,
    call_site: Option<&validation::CallSite<'_>>,
    claim_carry: &ClaimCarryContext<'_>,
    crossing: &mut CrossingAccumulator,
) {
    let Some(call_site) = call_site else {
        return;
    };
    let Some(parameters) = validation::call_target_parameters(program, call.target_symbol) else {
        return;
    };
    // An applied public signature may retain the caller's symbolic operands.
    // Its own telescope is closed, but those operands still have the caller's
    // checked structural bounds. Keep both exact binder scopes; neither an
    // unrelated same-shaped type nor a value qualification supplies a floor.
    let mut type_parameters = program.machine_type_parameters(machine).to_vec();
    for parameter in validation::call_target_type_parameters(program, call.target_symbol) {
        if !type_parameters
            .iter()
            .any(|existing| existing.symbol == parameter.symbol)
        {
            type_parameters.push(parameter.clone());
        }
    }
    let arguments = validation::call_site_argument_expressions(program, call_site);
    for (position, (parameter, argument)) in parameters.iter().zip(arguments).enumerate() {
        let display_name = program.expression_table.display_name(*argument);
        collect_value_carry_obligation(
            program,
            &type_parameters,
            parameter.type_reference,
            &display_name,
            claim_carry,
            checked_trees::SuspensionCrossingStorage::CallArgument,
            checked_trees::SuspensionCrossingValueOrigin::CallArgument { position },
            crossing,
        );
    }
}

fn state_carry_values<'program>(
    program: &'program dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
) -> Vec<StateCarryValue<'program>> {
    let parameters = program.state_parameters(state);
    let mut values = parameters
        .iter()
        .enumerate()
        .map(|(position, parameter)| StateCarryValue {
            symbol: parameter.symbol,
            type_reference: parameter.type_reference,
            name: parameter.name.as_str(),
            storage: checked_trees::SuspensionCrossingStorage::Parameter,
            origin: checked_trees::SuspensionCrossingValueOrigin::Parameter {
                symbol: parameter.symbol,
                position,
            },
        })
        .collect::<Vec<_>>();
    let mut local_position = 0;
    for (statement_index, statement) in program
        .statement_table
        .indexed_statements(state.statement_nodes)
    {
        let Some(local) = statement.as_local_data() else {
            continue;
        };
        values.push(StateCarryValue {
            symbol: local.symbol,
            type_reference: local.type_reference,
            name: local.name.as_str(),
            storage: checked_trees::SuspensionCrossingStorage::Local,
            origin: checked_trees::SuspensionCrossingValueOrigin::Local {
                symbol: local.symbol,
                statement_index,
                environment_position: parameters.len() + local_position,
            },
        });
        local_position += 1;
    }
    values
}

fn append_live_value_carry_demands(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    call: &checked_trees::BorrowCallFact,
    call_site: Option<&validation::CallSite<'_>>,
    source_values: &[StateCarryValue<'_>],
    borrow: &checked_trees::BorrowFacts,
    frontier_loans: &mut [checked_trees::SuspensionCrossingLoanFact],
    claim_carry: &ClaimCarryContext<'_>,
    crossing: &mut CrossingAccumulator,
) {
    let mut live_symbols = Vec::new();
    for value in source_values
        .iter()
        .filter(|value| value.available_before(call.statement_index))
    {
        if crate::borrow::place_is_used_after_statement(
            program,
            state.statement_nodes,
            call.statement_index,
            value.symbol,
            value.name,
        ) || call_site.is_some_and(|call_site| {
            intra_statement::place_is_used_after_call(
                program,
                state,
                call.statement_index,
                call_site,
                value.symbol,
                value.name,
            )
        }) {
            live_symbols.push(value.symbol);
        }
    }
    loans::retain_backing_values(borrow, frontier_loans, &mut live_symbols);
    for value in source_values {
        if !value.available_before(call.statement_index) || !live_symbols.contains(&value.symbol) {
            continue;
        }
        collect_live_value_carry_obligation(
            program,
            machine,
            value.type_reference,
            value.name,
            claim_carry,
            value.storage,
            value.origin,
            crossing,
        );
    }
}

fn collect_live_value_carry_obligation(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    type_reference: typed_trees::types::TypeReferenceHandle,
    value_name: &str,
    claim_carry: &ClaimCarryContext<'_>,
    storage: checked_trees::SuspensionCrossingStorage,
    origin: checked_trees::SuspensionCrossingValueOrigin,
    crossing: &mut CrossingAccumulator,
) {
    collect_value_carry_obligation(
        program,
        program.machine_type_parameters(machine),
        type_reference,
        value_name,
        claim_carry,
        storage,
        origin,
        crossing,
    );
}

fn collect_value_carry_obligation(
    program: &dyn ::validation::CheckingInput,
    type_parameters: &[typed_trees::data::TypeParameter],
    type_reference: typed_trees::types::TypeReferenceHandle,
    value_name: &str,
    claim_carry: &ClaimCarryContext<'_>,
    storage: checked_trees::SuspensionCrossingStorage,
    origin: checked_trees::SuspensionCrossingValueOrigin,
    crossing: &mut CrossingAccumulator,
) {
    let requirements =
        validation::effective_type_carry_requirements(program, type_parameters, type_reference);
    let claims = claim_carry.live_claim_identities(program, value_name);
    let policy =
        claim_carry.effective_policy(program, value_name, requirements.structural, &claims);
    crossing.obligations.push(PendingCarryObligation {
        name: value_name.to_owned(),
        ordinary: policy,
        borrow_independent: claim_carry.effective_policy(
            program,
            value_name,
            requirements.borrow_independent,
            &claims,
        ),
        requires_borrow_preservation: requirements.requires_borrow_preservation,
    });
    crossing
        .live_values
        .push(checked_trees::SuspensionCrossingLiveValueFact {
            type_reference,
            storage,
            origin,
            claims,
            effective: policy,
            borrow_frontier: checked_trees::SuspensionValueBorrowFrontier::Unresolved,
        });
}

fn resolve_crossing_carry(
    program: &dyn ::validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    loans: &arena::Arena<checked_trees::SuspensionCrossingLoanFact>,
    value_loans: &arena::Arena<arena::Handle<checked_trees::SuspensionCrossingLoanFact>>,
    state: &checked_trees::StateBorrowFact,
    call: &checked_trees::BorrowCallFact,
    frontier: checked_trees::SuspensionCrossingBorrowFrontier,
    crossing: &mut CrossingAccumulator,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let lifetime_demands = crossing
        .live_values
        .iter()
        .map(|live| {
            loans::retained_lifetime_demand(
                borrow,
                loans,
                value_loans,
                state,
                frontier,
                live,
                &crossing.live_values,
            )
        })
        .collect::<Vec<_>>();
    crossing.effective = CarryPolicy::PERMISSIVE;
    let target_name = crate::labels::symbol_name(program, call.target_symbol);
    for ((live, obligation), lifetime) in crossing
        .live_values
        .iter_mut()
        .zip(&crossing.obligations)
        .zip(lifetime_demands)
    {
        let missing_borrow_obligation = obligation
            .requires_borrow_preservation
            .then(|| lifetime.as_ref().err().copied())
            .flatten();
        let policy = if obligation.requires_borrow_preservation {
            match lifetime {
                Ok(demand) => obligation.borrow_independent.intersect(demand),
                Err(_) => obligation.ordinary.intersect(CarryPolicy::STRICT),
            }
        } else {
            obligation.ordinary
        };
        live.effective = policy;
        crossing.effective = crossing.effective.intersect(policy);
        if policy.suspension == CarrySuspension::Allowed {
            continue;
        }

        let value_name = &obligation.name;
        let mut message = if missing_borrow_obligation.is_some()
            && obligation.ordinary.suspension == CarrySuspension::Allowed
        {
            format!(
                "call to `{target_name}` may suspend while `{value_name}` may retain borrowed storage, but its backing lifetime, access restrictions and address preservation have no checked per-value join; carry permission on the descriptor does not establish loan preservation"
            )
        } else {
            format!(
                "call to `{target_name}` may suspend while `{value_name}` remains live, but its effective policy is `{policy}`; consume the value before the call or use a suspension-safe carrier"
            )
        };
        if let Some(reason) = missing_borrow_obligation {
            message.push_str("; missing retained-loan obligation: ");
            message.push_str(reason);
        }
        if diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == message)
        {
            continue;
        }
        diagnostics.push(Diagnostic::error(message));
    }
}
