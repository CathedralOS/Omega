use super::*;
use crate::descriptors::descriptor_lifetimes::{
    DescriptorFlowSource, descriptor_location, descriptor_location_type, value_locations,
};
use arena::IdTable;
use semantic_vocabulary::{InvocationSite, OperationId};

pub(crate) mod flow;
use flow::{location_type, require_acyclic};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ValueIdentitySeed {
    Parameter(usize),
    Result(OperationId),
}

type QueryResult<T> = Result<T, &'static str>;
type IdentityFlow = BTreeMap<DescriptorPosition, DescriptorFlowStep>;

pub struct ValueIdentityAnalysis<'a> {
    module: &'a TerminalModule,
    body: &'a TerminalMachine,
    machine: &'a DescriptorMachine,
    operations: BTreeMap<DescriptorPosition, &'a Operation>,
    positions: IdTable<OperationId, Option<DescriptorPosition>>,
    terminal_positions: BTreeMap<InvocationSite, DescriptorPosition>,
    incoming: QueryResult<payload_reachability::IncomingFlow>,
    analysis: DescriptorAnalysis<'a>,
    cuts: BTreeMap<DescriptorPosition, QueryResult<payload_reachability::Reachability>>,
    flows: BTreeMap<DescriptorPosition, QueryResult<IdentityFlow>>,
    actuals:
        BTreeMap<(DescriptorPosition, StructuralArgument), QueryResult<DescriptorLocation>>,
    values: BTreeMap<(ValueIdentitySeed, DescriptorPosition), BTreeSet<DescriptorLocation>>,
    results:
        BTreeMap<(ValueIdentitySeed, InvocationSite, StructuralArgument), QueryResult<()>>,
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
        let mut terminal_positions = BTreeMap::new();
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
            let site = match block.terminator {
                Terminator::InvokeNoNormal { edge, .. } => {
                    Some(InvocationSite::InvokeNoNormal(edge))
                }
                Terminator::ExternalCompletion { edge, .. } => {
                    Some(InvocationSite::ExternalCompletion(edge))
                }
                _ => None,
            };
            if let Some(site) = site {
                let position = (
                    block.id,
                    u32::try_from(block.operations.len())
                        .map_err(|_| "operation index overflow")?,
                );
                if terminal_positions.insert(site, position).is_some() {
                    return Err("value identity terminal invocation is ambiguous");
                }
            }
        }
        position_rows.sort_unstable_by_key(|(identity, _)| *identity);
        if position_rows.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err("value identity operation is ambiguous");
        }
        let positions = if let Some((first, last)) =
            position_rows.first().zip(position_rows.last())
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
            terminal_positions,
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

    pub fn require(
        &mut self,
        seed: ValueIdentitySeed,
        observation: OperationId,
        actual: &StructuralArgument,
    ) -> QueryResult<()> {
        self.require_at_invocation(seed, InvocationSite::Operation(observation), actual)
    }

    pub fn require_at_invocation(
        &mut self,
        seed: ValueIdentitySeed,
        observation: InvocationSite,
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

    fn observation_position(&self, site: InvocationSite) -> Option<DescriptorPosition> {
        match site {
            InvocationSite::Operation(operation) => self.operation_position(operation),
            InvocationSite::ExternalCompletion(_) | InvocationSite::InvokeNoNormal(_) => {
                self.terminal_positions.get(&site).copied()
            }
            InvocationSite::Cleanup { .. } => None,
        }
    }

    fn query(
        &mut self,
        seed: ValueIdentitySeed,
        invocation: InvocationSite,
        actual: &StructuralArgument,
    ) -> QueryResult<()> {
        // Several cleanup actions share one terminator coordinate.
        // This identity walk has no action-prefix state, so the
        // pre-terminator value cannot prove an unchanged-value claim.
        if matches!(invocation, InvocationSite::Cleanup { .. }) {
            return Err("unchanged-value proof at an ordered cleanup action is not served");
        }
        let observation = self
            .observation_position(invocation)
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
                    return Err(
                        "value identity result has not completed before its observation",
                    );
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
                                .filter(|(ordinal, _)| {
                                    relevant.includes_edge(*position, *ordinal)
                                })
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
