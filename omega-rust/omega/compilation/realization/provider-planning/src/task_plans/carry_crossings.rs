//! Activation carry crossings, their validation, and the plan-facing
//! crossing/carry values translated from the exact checked carry facts.

use diagnostics::Diagnostic;
use language_semantics::{CarryCpu, CarryHostThread, CarryPolicy, CarrySuspension};
use task_plans::{
    ActivationCarryObligations, CanonicalSuspensionCrossing, LiveCarryDemand, LiveCarryPlaceId,
    LiveCarryStorage, LiveCarryTypeId,
};

pub(crate) fn exact_activation_wide_carry<'program>(
    check_facts: &'program checked_trees::CheckFacts,
    machine: symbols::SymbolHandle,
    machine_name: &str,
) -> Result<&'program checked_trees::MachineActivationCarryFact, Vec<Diagnostic>> {
    let mut matches = check_facts
        .carry
        .activation_wide_carry
        .iter()
        .filter(|fact| fact.machine == machine);
    let fact = matches.next().ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "task activation target `{machine_name}` has no exact activation-wide CPU/thread carry envelope"
        ))]
    })?;
    if matches.next().is_some() {
        return Err(vec![Diagnostic::error(format!(
            "task activation target `{machine_name}` has duplicate exact activation-wide CPU/thread carry envelopes"
        ))]);
    }
    if !fact.analysis_complete {
        return Err(vec![Diagnostic::error(format!(
            "task activation target `{machine_name}` has incomplete activation-wide CPU/thread carry analysis"
        ))]);
    }
    Ok(fact)
}

pub(crate) fn validate_activation_carry_crossing(
    typed: &checked_trees::typed_trees::TypedTrees,
    check_facts: &checked_trees::CheckFacts,
    crossing: &checked_trees::SuspensionCrossingCarryFact,
) -> Result<(), Vec<Diagnostic>> {
    let mut machines = typed
        .machines()
        .iter()
        .filter(|machine| machine.symbol == crossing.machine);
    let machine = machines.next().ok_or_else(|| {
        vec![Diagnostic::error(
            "task activation carry crossing must name an exact typed machine",
        )]
    })?;
    if machines.next().is_some() {
        return Err(vec![Diagnostic::error(
            "task activation carry crossing machine must resolve uniquely",
        )]);
    }
    let mut states = typed
        .machine_states(machine)
        .iter()
        .filter(|state| state.symbol == crossing.state);
    let state = states.next().ok_or_else(|| {
        vec![Diagnostic::error(
            "task activation carry crossing state must belong to its exact typed machine",
        )]
    })?;
    if states.next().is_some() {
        return Err(vec![Diagnostic::error(
            "task activation carry crossing state must resolve uniquely within its machine",
        )]);
    }
    if typed
        .statement_table
        .statement_at(state, crossing.statement_index)
        .is_none()
    {
        return Err(vec![Diagnostic::error(
            "task activation carry crossing statement must belong to its exact typed state",
        )]);
    }

    let mut flow_states = check_facts.flow.control.states.iter().filter(|(_, flow)| {
        flow.machine_symbol == crossing.machine && flow.state_symbol == crossing.state
    });
    let flow_state = flow_states.next().map(|(_, flow)| flow).ok_or_else(|| {
        vec![Diagnostic::error(
            "task activation carry crossing must name one exact checked flow state",
        )]
    })?;
    if flow_states.next().is_some() {
        return Err(vec![Diagnostic::error(
            "task activation carry crossing must name exactly one checked flow state",
        )]);
    }
    let calls = check_facts
        .flow
        .control
        .calls
        .span(flow_state.calls)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "task activation carry crossing flow state must retain an exact valid call span",
            )]
        })?;
    let mut calls = calls.iter().filter(|call| {
        call.statement_index == crossing.statement_index
            && call.call_ordinal == crossing.call_ordinal
    });
    let call = calls.next().ok_or_else(|| {
        vec![Diagnostic::error(
            "task activation carry crossing must name one exact checked flow call",
        )]
    })?;
    if calls.next().is_some() {
        return Err(vec![Diagnostic::error(
            "task activation carry crossing must name exactly one checked flow call",
        )]);
    }
    if call.target_symbol != crossing.target {
        return Err(vec![Diagnostic::error(
            "task activation carry crossing must retain its exact checked call target",
        )]);
    }
    let mut targets = typed.machines().iter().flat_map(|machine| {
        typed
            .machine_states(machine)
            .iter()
            .filter(|state| state.symbol == crossing.target)
    });
    targets.next().ok_or_else(|| {
        vec![Diagnostic::error(
            "task activation carry crossing target must name an exact typed state",
        )]
    })?;
    if targets.next().is_some() {
        return Err(vec![Diagnostic::error(
            "task activation carry crossing target must resolve to exactly one typed state",
        )]);
    }
    if !call.suspension.direct_may_suspend && !call.suspension.transitive_may_suspend {
        return Err(vec![Diagnostic::error(
            "task activation carry crossing must retain a may-suspend checked call",
        )]);
    }
    Ok(())
}

pub(crate) fn carry_obligations(policy: CarryPolicy) -> ActivationCarryObligations {
    ActivationCarryObligations {
        preserve_cpu: policy.cpu == CarryCpu::Origin,
        preserve_host_thread: policy.host_thread == CarryHostThread::Origin,
    }
}

pub(crate) fn canonical_suspension_crossing(
    typed: &checked_trees::typed_trees::TypedTrees,
    crossing: &checked_trees::SuspensionCrossingCarryFact,
) -> Result<CanonicalSuspensionCrossing, Vec<Diagnostic>> {
    let mut live_carry = Vec::with_capacity(crossing.live_values.len());
    for live in &crossing.live_values {
        live_carry.push(live_carry_demand(typed, crossing, live)?);
    }
    Ok(CanonicalSuspensionCrossing {
        identity: checked_trees::canonical_suspension_crossing_id(typed, crossing).ok_or_else(
            || {
                vec![Diagnostic::error(
                    "task activation carry crossing source identity must resolve exactly",
                )]
            },
        )?,
        suspension_allowed: crossing.effective.suspension == CarrySuspension::Allowed,
        preserve_cpu: crossing.effective.cpu == CarryCpu::Origin,
        preserve_host_thread: crossing.effective.host_thread == CarryHostThread::Origin,
        live_carry,
    })
}

/// Translate one checked live value at a suspension crossing into the
/// plan's suspension-safe-loan roster row: the exact place, its checked
/// type, the storage class holding it across the park, the complete live
/// claim identities attached to it, and the four-axis demand it carries.
fn live_carry_demand(
    typed: &checked_trees::typed_trees::TypedTrees,
    crossing: &checked_trees::SuspensionCrossingCarryFact,
    live: &checked_trees::SuspensionCrossingLiveValueFact,
) -> Result<LiveCarryDemand, Vec<Diagnostic>> {
    Ok(LiveCarryDemand {
        place: super::normalized_id(
            live_place_identity(typed, crossing, live),
            LiveCarryPlaceId::from_normalized_identity,
        )?,
        ty: super::normalized_id(
            live_type_identity(typed, live),
            LiveCarryTypeId::from_normalized_identity,
        )?,
        storage: match live.storage {
            checked_trees::SuspensionCrossingStorage::Parameter => LiveCarryStorage::Parameter,
            checked_trees::SuspensionCrossingStorage::Local => LiveCarryStorage::Local,
            checked_trees::SuspensionCrossingStorage::CallArgument => {
                LiveCarryStorage::CallArgument
            }
        },
        claims: live
            .claims
            .iter()
            .map(|claim| live_claim_identity(typed, claim))
            .collect::<Result<_, _>>()?,
        effective: live.effective,
    })
}

/// Stable identity of one exact live place at one crossing. The crossing
/// coordinate distinguishes the same source place observed at different
/// crossings and gives `CallArgument` ordinals — which share no symbol —
/// their exact coordinate.
fn live_place_identity(
    typed: &checked_trees::typed_trees::TypedTrees,
    crossing: &checked_trees::SuspensionCrossingCarryFact,
    live: &checked_trees::SuspensionCrossingLiveValueFact,
) -> u64 {
    let mut hash = crate::fnv1a::Fnv1a::new();
    hash.byte(0x4c);
    hash.string_terminated(&typed.symbols.display_path(crossing.machine, "::"));
    hash.string_terminated(&typed.symbols.display_path(crossing.state, "::"));
    hash.usize(crossing.statement_index);
    hash.usize(crossing.call_ordinal);
    match live.origin {
        checked_trees::SuspensionCrossingValueOrigin::Parameter { symbol, position } => {
            hash.byte(2);
            hash.string_terminated(&typed.symbols.display_path(symbol, "::"));
            hash.usize(position);
        }
        checked_trees::SuspensionCrossingValueOrigin::Local {
            symbol,
            statement_index,
            environment_position,
        } => {
            hash.byte(3);
            hash.string_terminated(&typed.symbols.display_path(symbol, "::"));
            hash.usize(statement_index);
            hash.usize(environment_position);
        }
        checked_trees::SuspensionCrossingValueOrigin::CallArgument { position } => {
            hash.byte(4);
            hash.usize(position);
        }
    }
    hash.finish_or(1)
}

/// Stable identity of a live value's checked type: the canonical type
/// identity string is already the normalized semantic coordinate.
fn live_type_identity(
    typed: &checked_trees::typed_trees::TypedTrees,
    live: &checked_trees::SuspensionCrossingLiveValueFact,
) -> u64 {
    let mut hash = crate::fnv1a::Fnv1a::new();
    hash.byte(0x54);
    hash.string_terminated(&typed.normalized_type_identity_text(live.type_reference));
    hash.finish_or(1)
}

/// Stable identity of one live claim attached to a crossing place. The
/// normalized coordinate commits to the claim's exact checked provenance —
/// establishing machine, state, event source, and ordinal — so a plan
/// frontier distinguishes loans rather than counting them.
fn live_claim_identity(
    typed: &checked_trees::typed_trees::TypedTrees,
    claim: &language_semantics::PermissionClaimIdentity,
) -> Result<semantic_vocabulary::ClaimId, Vec<Diagnostic>> {
    let language_semantics::PermissionClaimIdentity::Established {
        machine_symbol,
        state_symbol,
        source,
        ordinal,
    } = *claim
    else {
        return Err(vec![Diagnostic::error(
            "task activation carry crossing retains a live claim whose identity never resolved",
        )]);
    };
    let mut hash = crate::fnv1a::Fnv1a::new();
    hash.byte(0x43);
    hash.string_terminated(&typed.symbols.display_path(machine_symbol, "::"));
    hash.string_terminated(&typed.symbols.display_path(state_symbol, "::"));
    match source {
        language_semantics::PermissionEventSource::StateEntry => hash.byte(1),
        language_semantics::PermissionEventSource::Statement { statement_index } => {
            hash.byte(2);
            hash.usize(statement_index);
        }
        language_semantics::PermissionEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => {
            hash.byte(3);
            hash.usize(statement_index);
            hash.usize(call_ordinal);
            hash.string_terminated(&typed.symbols.display_path(target_symbol, "::"));
        }
        language_semantics::PermissionEventSource::StateExit => hash.byte(4),
        language_semantics::PermissionEventSource::InstructionExitDisposition {
            statement_index,
        } => {
            hash.byte(6);
            hash.usize(statement_index);
        }
        language_semantics::PermissionEventSource::InstructionDisposition { statement_index } => {
            hash.byte(5);
            hash.usize(statement_index);
        }
    }
    hash.u64(u64::from(ordinal));
    semantic_vocabulary::ClaimId::new(hash.finish_or(1)).ok_or_else(|| {
        vec![Diagnostic::error(
            "task activation carry crossing claim identity resolved to the reserved zero \
             coordinate",
        )]
    })
}
