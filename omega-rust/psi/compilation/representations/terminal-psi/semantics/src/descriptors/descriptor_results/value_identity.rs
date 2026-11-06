//! Prove unchanged value transport from one specified formal or result.
//!
//! Backing aliases locate values; they do not establish their identity. Only
//! explicit moves/copies and complete simultaneous arrivals transport it here.
//! Consumers authenticate the seed and compose actual-to-formal calls. Ordinary
//! call results have no inferred identity, and this query grants no authority.

use super::*;
use crate::descriptors::descriptor_lifetimes::{
    DescriptorFlowSource, descriptor_location, descriptor_location_type, value_locations,
};
use semantic_vocabulary::{IdTable, OperationId};

mod flow;
use flow::{location_type, require_acyclic};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ValueIdentitySeed {
    /// Index in the selected machine's structural formal roster.
    Parameter(usize),
    /// The structural result of this exact completed operation.
    Result(OperationId),
}

type QueryResult<T> = Result<T, &'static str>;
type IdentityFlow = BTreeMap<DescriptorPosition, DescriptorFlowStep>;

/// One body's immutable indexes, descriptor replay and demand-driven identity
/// observations. Keep this owner throughout a root traversal; different seeds
/// and actuals reuse the same pre-operation snapshots and adapted flow.
pub struct ValueIdentityAnalysis<'a> {
    module: &'a TerminalModule,
    body: &'a TerminalMachine,
    machine: &'a DescriptorMachine,
    operations: BTreeMap<DescriptorPosition, &'a Operation>,
    // A body can retain gaps in module-wide operation IDs after pruning.
    positions: IdTable<OperationId, Option<DescriptorPosition>>,
    incoming: QueryResult<payload_reachability::IncomingFlow>,
    analysis: DescriptorAnalysis<'a>,
    cuts: BTreeMap<DescriptorPosition, QueryResult<payload_reachability::Reachability>>,
    flows: BTreeMap<DescriptorPosition, QueryResult<IdentityFlow>>,
    actuals: BTreeMap<(DescriptorPosition, StructuralArgument), QueryResult<DescriptorLocation>>,
    values: BTreeMap<(ValueIdentitySeed, DescriptorPosition), BTreeSet<DescriptorLocation>>,
    results: BTreeMap<(ValueIdentitySeed, OperationId, StructuralArgument), QueryResult<()>>,
}

impl<'a> ValueIdentityAnalysis<'a> {
    pub fn new(
        module: &'a TerminalModule,
        reference_graph: &'a [DescriptorMachine],
        identity: MachineId,
    ) -> QueryResult<Self> {
        let body = exactly_one(module.machines.iter(), |body| body.id == identity)
            .ok_or("value identity machine is absent or ambiguous")?;
        let machine = exactly_one(reference_graph.iter(), |body| body.id == identity)
            .ok_or("value identity reference graph is absent or ambiguous")?;
        let mut operations = BTreeMap::new();
        let mut position_rows = Vec::new();
        for block in &body.blocks {
            for (index, operation) in block.operations.iter().enumerate() {
                let position = (
                    block.id,
                    u32::try_from(index).map_err(|_| "operation index overflow")?,
                );
                if operations.insert(position, operation).is_some() {
                    return Err("value identity operation is ambiguous");
                }
                position_rows.push((operation.id, position));
            }
        }
        position_rows.sort_unstable_by_key(|(identity, _)| *identity);
        if position_rows.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err("value identity operation is ambiguous");
        }
        let positions = if let Some((first, last)) = position_rows.first().zip(position_rows.last())
        {
            let first = first.0.get();
            let length = last
                .0
                .get()
                .checked_sub(first)
                .and_then(|distance| distance.checked_add(1))
                .and_then(|length| usize::try_from(length).ok())
                .ok_or("value identity operation span cannot be indexed")?;
            let mut slots = Vec::new();
            slots
                .try_reserve_exact(length)
                .map_err(|_| "value identity operation span cannot be allocated")?;
            slots.resize(length, None);
            for (operation, position) in position_rows {
                let offset = usize::try_from(operation.get() - first)
                    .map_err(|_| "value identity operation cannot be indexed")?;
                slots[offset] = Some(position);
            }
            IdTable::from_rows(slots.into_iter().enumerate().map(|(offset, position)| {
                let identity = OperationId::new(first + offset as u64)
                    .expect("validated nonzero operation interval");
                (identity, position)
            }))
            .map_err(|_| "value identity operation interval is inconsistent")?
        } else {
            IdTable::default()
        };
        Ok(Self {
            module,
            body,
            machine,
            operations,
            positions,
            incoming: payload_reachability::incoming_flow(&machine.flow)
                .ok_or("value identity has incomplete incoming control flow"),
            analysis: DescriptorAnalysis::with_reference_slots(
                &module.structural_types,
                reference_graph,
            ),
            cuts: BTreeMap::new(),
            flows: BTreeMap::new(),
            actuals: BTreeMap::new(),
            values: BTreeMap::new(),
            results: BTreeMap::new(),
        })
    }

    /// Require this actual immediately before the operation to retain the
    /// specified value. Types alone never suffice. Failures are cached too.
    pub fn require(
        &mut self,
        seed: ValueIdentitySeed,
        observation: OperationId,
        actual: &StructuralArgument,
    ) -> QueryResult<()> {
        let key = (seed, observation, actual.clone());
        if let Some(result) = self.results.get(&key) {
            return *result;
        }
        let result = self.query(seed, observation, actual);
        self.results.insert(key, result);
        result
    }

    fn operation_position(&self, operation: OperationId) -> Option<DescriptorPosition> {
        self.positions.get(operation).copied().flatten()
    }

    fn query(
        &mut self,
        seed: ValueIdentitySeed,
        operation: OperationId,
        actual: &StructuralArgument,
    ) -> QueryResult<()> {
        let observation = self
            .operation_position(operation)
            .ok_or("value identity observation is absent")?;
        let (establishment, value, at_entry) = match seed {
            ValueIdentitySeed::Parameter(index) => {
                let formal = self
                    .body
                    .structural_parameters
                    .get(index)
                    .ok_or("value identity structural formal is absent")?;
                (
                    self.machine.entry,
                    DescriptorLocation::whole(formal.place),
                    true,
                )
            }
            ValueIdentitySeed::Result(operation) => {
                let position = self
                    .operation_position(operation)
                    .ok_or("value identity result producer is absent")?;
                let result = self.operations[&position]
                    .result
                    .as_structural()
                    .ok_or("value identity seed has no structural result")?;
                if position == observation {
                    return Err("value identity result has not completed before its observation");
                }
                (position, DescriptorLocation::whole(result.place), false)
            }
        };
        self.prepare_cut(observation);
        let relevant = self.cuts[&observation].as_ref().map_err(|reason| *reason)?;
        if !relevant.contains(establishment) || !relevant.contains(self.machine.entry) {
            return Err("value identity seed does not precede this observation");
        }
        let actual_key = (observation, actual.clone());
        if !self.actuals.contains_key(&actual_key) {
            let observed = self.normalize_actual(observation, actual);
            self.actuals.insert(actual_key.clone(), observed);
        }
        let observed = self.actuals[&actual_key]
            .as_ref()
            .map_err(|reason| *reason)?;
        if location_type(self.module, self.machine, &value)?
            != location_type(self.module, self.machine, observed)?
        {
            return Err("value identity seed and actual have different retained types");
        }
        if !self.flows.contains_key(&observation) {
            let adapted = flow::adapt(
                self.module,
                self.machine,
                &self.operations,
                &mut self.analysis,
                self.cuts[&observation].as_ref().map_err(|reason| *reason)?,
                observation,
            );
            self.flows.insert(observation, adapted);
        }
        let flow = self.flows[&observation]
            .as_ref()
            .map_err(|reason| *reason)?;
        let known = self.values.entry((seed, observation)).or_insert_with(|| {
            value_locations(flow, establishment, &value, at_entry)
                .remove(&observation)
                .unwrap_or_default()
        });
        if !known.contains(observed) {
            return Err(
                "actual value is not the specified unchanged value; transformations and ordinary call returns supply no identity",
            );
        }
        Ok(())
    }

    fn normalize_actual(
        &mut self,
        observation: DescriptorPosition,
        actual: &StructuralArgument,
    ) -> QueryResult<DescriptorLocation> {
        let observed = descriptor_location(
            &self.module.structural_types,
            actual.place,
            *self
                .machine
                .place_types
                .get(&actual.place)
                .ok_or("value identity actual type is absent")?,
            &actual.path,
            None,
        )
        .ok_or("value identity actual has an unresolved projection")?;
        self.analysis
            .location(self.machine.id, observation, &observed)
            .ok_or("value identity actual has ambiguous or unavailable backing")
    }

    fn prepare_cut(&mut self, observation: DescriptorPosition) {
        if self.cuts.contains_key(&observation) {
            return;
        }
        let cut = (|| {
            let incoming = self.incoming.as_ref().map_err(|reason| *reason)?;
            let relevant = payload_reachability::predecessors_with_incoming(
                self.machine,
                incoming,
                observation,
            )
            .ok_or("value identity has incomplete incoming control flow")?;
            // Reject renewal before cutting the observation's successors; a
            // loop through the observation must not become a first-use proof.
            let incoming = self
                .machine
                .flow
                .iter()
                .filter(|(position, _)| relevant.contains(**position))
                .map(|(position, step)| {
                    (
                        *position,
                        DescriptorFlowStep {
                            successors: step
                                .successors
                                .iter()
                                .enumerate()
                                .filter(|(ordinal, _)| relevant.includes_edge(*position, *ordinal))
                                .map(|(_, edge)| edge.clone())
                                .collect(),
                            ..Default::default()
                        },
                    )
                })
                .collect();
            require_acyclic(&incoming, self.machine.entry)?;
            Ok(relevant)
        })();
        self.cuts.insert(observation, cut);
    }
}

/// Convenience for one observation. Repeated consumers should keep a
/// `ValueIdentityAnalysis` instead. The graph must be this module's current
/// Terminal reference graph. Neither route grants ownership or authority.
pub fn require_unchanged_value(
    module: &TerminalModule,
    reference_graph: &[DescriptorMachine],
    identity: MachineId,
    seed: ValueIdentitySeed,
    observation: OperationId,
    actual: &StructuralArgument,
) -> Result<(), &'static str> {
    ValueIdentityAnalysis::new(module, reference_graph, identity)?.require(
        seed,
        observation,
        actual,
    )
}
