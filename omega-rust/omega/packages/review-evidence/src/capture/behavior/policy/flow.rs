use super::rejected;
use crate::capture::PackageReviewInput;
use crate::capture::semantics::declarations::nominal_identity;
use crate::capture::semantics::facts::exactly_one;
use crate::record::PackagePolicyCapabilityFlow;
use checked_trees::{CheckedTrees, MachineServiceReachRows, RealizedMachineContractEnvelope};
use diagnostics::Diagnostic;
use std::cell::OnceCell;
use symbols::{SymbolHandle, SymbolMap};
use typed_trees::machine::Machine;

/// Invocation-local raw associations, never a cached policy or authority verdict.
/// The immutable supplied program stays separate from compiler custody.
pub(crate) struct CapabilityFlowProjection<'input, 'program> {
    compilation: &'input PackageReviewInput<'program>,
    rows: OnceCell<FlowRows<'program>>,
}

impl<'input, 'program> CapabilityFlowProjection<'input, 'program> {
    pub(crate) fn new(compilation: &'input PackageReviewInput<'program>) -> Self {
        Self {
            compilation,
            rows: OnceCell::new(),
        }
    }

    /// Disclose the authority exercised anywhere in this checked machine call
    /// closure, independently of whether an acquired value escapes to its caller.
    /// The service-reach topology is machine-wide, as is capability inference;
    /// this is not a claim about dynamically feasible execution paths.
    pub(crate) fn reachable_capability_flows(
        &self,
        root: &RealizedMachineContractEnvelope,
    ) -> Result<Vec<PackagePolicyCapabilityFlow>, Vec<Diagnostic>> {
        let compilation = self.compilation;
        let rows = self.rows.get_or_init(|| FlowRows::new(compilation.program));
        let mut pending = vec![root.machine];
        let mut cursor = 0;
        let mut projected = Vec::new();
        while cursor < pending.len() {
            let symbol = pending[cursor];
            cursor += 1;
            let machine = exactly_one(
                rows.machines.get(&symbol).into_iter().flatten().copied(),
                "reachable capability closure",
                "machine declaration",
            )?;
            let envelope = exactly_one(
                rows.envelopes.get(&symbol).into_iter().flatten().copied(),
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
                rows.reaches.get(&symbol).into_iter().flatten().copied(),
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
                    let mut owners = rows.state_owners(call.target_state);
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
}

pub(crate) fn capability_flows(
    compilation: &PackageReviewInput<'_>,
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

/// Complete raw rosters in original order. Duplicate/invalid symbols remain
/// data for the original per-use uniqueness and owner checks.
struct FlowRows<'program> {
    machines: SymbolMap<Vec<&'program Machine>>,
    envelopes: SymbolMap<Vec<&'program RealizedMachineContractEnvelope>>,
    reaches: SymbolMap<Vec<&'program MachineServiceReachRows>>,
    owners: SymbolMap<Vec<(usize, &'program Machine)>>,
}

impl<'program> FlowRows<'program> {
    fn new(program: &'program CheckedTrees) -> Self {
        let mut rows = Self {
            machines: SymbolMap::default(),
            envelopes: SymbolMap::default(),
            reaches: SymbolMap::default(),
            owners: SymbolMap::default(),
        };
        for (position, machine) in program.machines().iter().enumerate() {
            rows.machines
                .entry(machine.symbol)
                .or_default()
                .push(machine);
            for state in program.machine_states(machine) {
                let owners = rows.owners.entry(state.symbol).or_default();
                // A machine row is one owner even if its state roster repeats
                // the symbol. Distinct rows with identical machine IDs remain.
                if owners
                    .last()
                    .is_none_or(|(previous, _)| *previous != position)
                {
                    owners.push((position, machine));
                }
            }
        }
        for envelope in &program.facts.contract_plans.realized_envelopes {
            rows.envelopes
                .entry(envelope.machine)
                .or_default()
                .push(envelope);
        }
        for reach in program.facts.service_reaches.machines() {
            rows.reaches.entry(reach.machine).or_default().push(reach);
        }
        rows
    }

    fn state_owners(&self, state: SymbolHandle) -> impl Iterator<Item = &'program Machine> + '_ {
        self.owners
            .get(&state)
            .into_iter()
            .flatten()
            .filter_map(move |(_, machine)| state.is_valid().then_some(*machine))
    }
}

#[cfg(test)]
mod row_tests;
