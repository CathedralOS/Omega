use super::rejected;
use crate::capture::semantics::declarations::nominal_identity;
use crate::capture::semantics::facts::exactly_one;
use crate::record::PackagePolicyCapabilityFlow;
use checked_trees::RealizedMachineContractEnvelope;
use compiler::CheckedCompilation;
use diagnostics::Diagnostic;

/// Disclose the authority exercised anywhere in this checked machine call
/// closure, independently of whether an acquired value escapes to its caller.
/// The service-reach topology is machine-wide, as is capability inference;
/// this is not a claim about dynamically feasible execution paths.
pub(crate) fn reachable_capability_flows(
    compilation: &CheckedCompilation,
    root: &RealizedMachineContractEnvelope,
) -> Result<Vec<PackagePolicyCapabilityFlow>, Vec<Diagnostic>> {
    let mut pending = vec![root.machine];
    let mut cursor = 0;
    let mut projected = Vec::new();
    while cursor < pending.len() {
        let symbol = pending[cursor];
        cursor += 1;
        let machine = exactly_one(
            compilation
                .machines()
                .iter()
                .filter(|machine| machine.symbol == symbol),
            "reachable capability closure",
            "machine declaration",
        )?;
        let envelope = exactly_one(
            compilation
                .facts
                .contract_plans
                .realized_envelopes
                .iter()
                .filter(|envelope| envelope.machine == symbol),
            "reachable capability closure",
            "realized contract envelope",
        )?;
        if symbol == root.machine && envelope != root {
            return Err(rejected(
                "reachable capability root differs from its checked envelope",
            ));
        }
        projected.extend(capability_flows(compilation, envelope)?);
        let graph = &compilation.facts.service_reaches;
        let reached = exactly_one(
            graph
                .machines()
                .iter()
                .filter(|fact| fact.machine == symbol),
            "reachable capability closure",
            "service-reach machine",
        )?;
        let states = compilation.machine_states(machine);
        let checked_states = graph.states_for(reached);
        if states.len() != checked_states.len() {
            return Err(rejected(
                "reachable machine has a different checked state roster",
            ));
        }
        for state in states {
            let checked_state = exactly_one(
                checked_states
                    .iter()
                    .filter(|fact| fact.state == state.symbol),
                "reachable capability closure",
                "service-reach state",
            )?;
            for call in graph.calls_for(checked_state) {
                // Requirements and machine parameters have signature symbols,
                // but no concrete machine body. Do not fabricate closure edges
                // for them or infer an implementation from a rendered name.
                let mut owners = compilation.machines().iter().filter(|candidate| {
                    call.target_state.is_valid()
                        && compilation
                            .machine_states(candidate)
                            .iter()
                            .any(|state| state.symbol == call.target_state)
                });
                let owner = owners.next();
                if owners.next().is_some()
                    || owner.map(|machine| machine.symbol)
                        != call
                            .target_machine
                            .is_valid()
                            .then_some(call.target_machine)
                {
                    return Err(rejected(
                        "reachable call does not identify one exact state owner",
                    ));
                }
                if let Some(owner) = owner
                    && !pending.contains(&owner.symbol)
                {
                    pending.push(owner.symbol);
                }
            }
        }
        if envelope
            .capabilities
            .iter()
            .any(|flow| !states.iter().any(|state| state.symbol == flow.state_symbol))
        {
            return Err(rejected(
                "reachable capability fact belongs to a different state owner",
            ));
        }
    }
    projected.sort_by(PackagePolicyCapabilityFlow::compare_canonical);
    projected.dedup();
    Ok(projected)
}

pub(crate) fn capability_flows(
    compilation: &CheckedCompilation,
    envelope: &RealizedMachineContractEnvelope,
) -> Result<Vec<PackagePolicyCapabilityFlow>, Vec<Diagnostic>> {
    let actual = compilation
        .facts
        .capabilities
        .flows()
        .filter(|flow| flow.machine_symbol == envelope.machine)
        .copied()
        .collect::<Vec<_>>();
    if actual != envelope.capabilities {
        return Err(rejected(
            "capability flow collection differs from its exact checked envelope",
        ));
    }
    let mut projected = actual
        .iter()
        .map(|flow| {
            Ok(PackagePolicyCapabilityFlow {
                capability: nominal_identity(compilation, flow.capability_symbol)?,
                kind: flow.kind,
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    projected.sort_by(PackagePolicyCapabilityFlow::compare_canonical);
    projected.dedup();
    Ok(projected)
}
