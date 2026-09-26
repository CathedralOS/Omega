//! Activation carry crossings, their validation, and the plan-facing
//! crossing/carry values translated from the exact checked carry facts.

use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use language_semantics::{CarryCpu, CarryHostThread, CarryPolicy, CarrySuspension};
use task_plans::{
    ActivationCarryObligations, CanonicalSuspensionCrossing, LiveCarryDemand, LiveCarryPlaceId,
    LiveCarryStorage, LiveCarryTypeId,
};

pub(crate) struct ActivationCarryCrossings<'program> {
    pub(crate) subtree: Vec<&'program checked_trees::SuspensionCrossingCarryFact>,
}

pub(crate) fn exact_activation_wide_carry<'program>(
    program: &'program CheckedTrees,
    machine: symbols::SymbolHandle,
    machine_name: &str,
) -> Result<&'program checked_trees::MachineActivationCarryFact, Vec<Diagnostic>> {
    let mut matches = program
        .facts
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

pub(crate) fn activation_carry_crossings(
    program: &CheckedTrees,
    root: symbols::SymbolHandle,
) -> Result<ActivationCarryCrossings<'_>, Vec<Diagnostic>> {
    let subtree_machines = exact_activation_carry_subtree(program, root)?;
    let subtree = program
        .facts
        .carry
        .suspension_crossings
        .iter()
        .filter(|crossing| subtree_machines.contains(&crossing.machine))
        .collect::<Vec<_>>();
    let mut coordinates = Vec::new();
    for crossing in &subtree {
        validate_activation_carry_crossing(program, crossing)?;
        let coordinate = (
            crossing.machine,
            crossing.state,
            crossing.statement_index,
            crossing.call_ordinal,
        );
        if coordinates.contains(&coordinate) {
            return Err(vec![Diagnostic::error(
                "task activation carry crossings must retain one row per exact call coordinate",
            )]);
        }
        coordinates.push(coordinate);
    }
    Ok(ActivationCarryCrossings { subtree })
}

pub(crate) fn exact_activation_carry_subtree(
    program: &CheckedTrees,
    root: symbols::SymbolHandle,
) -> Result<Vec<symbols::SymbolHandle>, Vec<Diagnostic>> {
    let mut machines = vec![root];
    let mut cursor = 0;
    while cursor < machines.len() {
        let machine_symbol = machines[cursor];
        cursor += 1;

        let mut typed_machines = program
            .machines()
            .iter()
            .filter(|machine| machine.symbol == machine_symbol);
        typed_machines.next().ok_or_else(|| {
            vec![Diagnostic::error(
                "task activation carry topology must name an exact typed machine",
            )]
        })?;
        if typed_machines.next().is_some() {
            return Err(vec![Diagnostic::error(
                "task activation carry topology machine must resolve uniquely",
            )]);
        }

        let mut topologies = program
            .facts
            .carry
            .machine_topologies
            .iter()
            .filter(|(_, topology)| topology.machine == machine_symbol);
        let topology = topologies
            .next()
            .map(|(_, topology)| topology)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "task activation carry topology must retain one exact row per reached machine",
                )]
            })?;
        if topologies.next().is_some() {
            return Err(vec![Diagnostic::error(
                "task activation carry topology must retain exactly one row per reached machine",
            )]);
        }
        let fields = program
            .facts
            .carry
            .contained_fields
            .span(topology.fields)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "task activation carry topology must retain an exact valid field span",
                )]
            })?;
        let mut field_symbols = Vec::new();
        for field in fields {
            if !field.field.is_valid() || !field.data.is_valid() || !field.type_reference.is_valid()
            {
                return Err(vec![Diagnostic::error(
                    "task activation carry topology fields must retain nonempty exact coordinates",
                )]);
            }
            if field_symbols.contains(&field.field) {
                return Err(vec![Diagnostic::error(
                    "task activation carry topology fields must be unique within their machine",
                )]);
            }
            field_symbols.push(field.field);
            let targets = program
                .facts
                .carry
                .contained_targets
                .span(field.targets)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "task activation carry topology field must retain an exact valid target span",
                    )]
                })?;
            if targets.is_empty() {
                return Err(vec![Diagnostic::error(
                    "task activation carry topology field must retain at least one exact target",
                )]);
            }
            let mut field_targets = Vec::new();
            for target in targets {
                if field_targets.contains(&target.machine) {
                    return Err(vec![Diagnostic::error(
                        "task activation carry topology field targets must be unique",
                    )]);
                }
                field_targets.push(target.machine);
                let mut typed_targets = program
                    .machines()
                    .iter()
                    .filter(|machine| machine.symbol == target.machine);
                typed_targets.next().ok_or_else(|| {
                    vec![Diagnostic::error(
                        "task activation carry topology target must name an exact typed machine",
                    )]
                })?;
                if typed_targets.next().is_some() {
                    return Err(vec![Diagnostic::error(
                        "task activation carry topology target must resolve uniquely",
                    )]);
                }
                if !machines.contains(&target.machine) {
                    machines.push(target.machine);
                }
            }
        }
    }
    Ok(machines)
}

pub(crate) fn validate_activation_carry_crossing(
    program: &CheckedTrees,
    crossing: &checked_trees::SuspensionCrossingCarryFact,
) -> Result<(), Vec<Diagnostic>> {
    let mut machines = program
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
    let mut states = program
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
    if program
        .statement_table
        .statements(state.statement_nodes)
        .get(crossing.statement_index)
        .is_none()
    {
        return Err(vec![Diagnostic::error(
            "task activation carry crossing statement must belong to its exact typed state",
        )]);
    }

    let mut flow_states = program
        .facts
        .flow
        .control
        .states
        .iter()
        .filter(|(_, flow)| {
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
    let calls = program
        .facts
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
    let mut targets = program.machines().iter().flat_map(|machine| {
        program
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
    program: &CheckedTrees,
    crossing: &checked_trees::SuspensionCrossingCarryFact,
) -> Result<CanonicalSuspensionCrossing, Vec<Diagnostic>> {
    let mut live_carry = Vec::with_capacity(crossing.live_values.len());
    for live in &crossing.live_values {
        live_carry.push(live_carry_demand(program, crossing, live)?);
    }
    Ok(CanonicalSuspensionCrossing {
        identity: checked_trees::canonical_suspension_crossing_id(program, crossing).ok_or_else(
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
    program: &CheckedTrees,
    crossing: &checked_trees::SuspensionCrossingCarryFact,
    live: &checked_trees::SuspensionCrossingLiveValueFact,
) -> Result<LiveCarryDemand, Vec<Diagnostic>> {
    Ok(LiveCarryDemand {
        place: super::normalized_id(
            live_place_identity(program, crossing, live),
            LiveCarryPlaceId::from_normalized_identity,
        )?,
        ty: super::normalized_id(
            live_type_identity(program, live),
            LiveCarryTypeId::from_normalized_identity,
        )?,
        storage: match live.storage {
            checked_trees::SuspensionCrossingStorage::Persistent => LiveCarryStorage::Persistent,
            checked_trees::SuspensionCrossingStorage::Parameter => LiveCarryStorage::Parameter,
            checked_trees::SuspensionCrossingStorage::Local => LiveCarryStorage::Local,
            checked_trees::SuspensionCrossingStorage::CallArgument => {
                LiveCarryStorage::CallArgument
            }
        },
        claims: live
            .claims
            .iter()
            .map(|claim| live_claim_identity(program, claim))
            .collect::<Result<_, _>>()?,
        effective: live.effective,
    })
}

/// Stable identity of one exact live place at one crossing. The crossing
/// coordinate distinguishes the same source place observed at different
/// crossings and gives `CallArgument` ordinals — which share no symbol —
/// their exact coordinate.
fn live_place_identity(
    program: &CheckedTrees,
    crossing: &checked_trees::SuspensionCrossingCarryFact,
    live: &checked_trees::SuspensionCrossingLiveValueFact,
) -> u64 {
    let mut hash = super::StableHash::new();
    hash.byte(0x4c);
    hash.string(&program.typed.symbols.display_path(crossing.machine, "::"));
    hash.string(&program.typed.symbols.display_path(crossing.state, "::"));
    hash.usize(crossing.statement_index);
    hash.usize(crossing.call_ordinal);
    match live.origin {
        checked_trees::SuspensionCrossingValueOrigin::Persistent { symbol } => {
            hash.byte(1);
            hash.string(&program.typed.symbols.display_path(symbol, "::"));
        }
        checked_trees::SuspensionCrossingValueOrigin::Parameter { symbol, position } => {
            hash.byte(2);
            hash.string(&program.typed.symbols.display_path(symbol, "::"));
            hash.usize(position);
        }
        checked_trees::SuspensionCrossingValueOrigin::Local {
            symbol,
            statement_index,
            environment_position,
        } => {
            hash.byte(3);
            hash.string(&program.typed.symbols.display_path(symbol, "::"));
            hash.usize(statement_index);
            hash.usize(environment_position);
        }
        checked_trees::SuspensionCrossingValueOrigin::CallArgument { position } => {
            hash.byte(4);
            hash.usize(position);
        }
    }
    hash.finish()
}

/// Stable identity of a live value's checked type: the canonical type
/// identity string is already the normalized semantic coordinate.
fn live_type_identity(
    program: &CheckedTrees,
    live: &checked_trees::SuspensionCrossingLiveValueFact,
) -> u64 {
    let mut hash = super::StableHash::new();
    hash.byte(0x54);
    hash.string(
        program
            .normalized_type_identity(live.type_reference)
            .as_str(),
    );
    hash.finish()
}

/// Stable identity of one live claim attached to a crossing place. The
/// normalized coordinate commits to the claim's exact checked provenance —
/// establishing machine, state, event source, and ordinal — so a plan
/// frontier distinguishes loans rather than counting them.
fn live_claim_identity(
    program: &CheckedTrees,
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
    let mut hash = super::StableHash::new();
    hash.byte(0x43);
    hash.string(&program.typed.symbols.display_path(machine_symbol, "::"));
    hash.string(&program.typed.symbols.display_path(state_symbol, "::"));
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
            hash.string(&program.typed.symbols.display_path(target_symbol, "::"));
        }
        language_semantics::PermissionEventSource::StateExit => hash.byte(4),
    }
    hash.u64(u64::from(ordinal));
    semantic_vocabulary::ClaimId::new(hash.finish()).ok_or_else(|| {
        vec![Diagnostic::error(
            "task activation carry crossing claim identity resolved to the reserved zero \
             coordinate",
        )]
    })
}
