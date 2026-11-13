//! Bind symbolic code inputs through actual invocations reached from entry.

use super::*;

/// Admitted callback/interrupt entries bound to their exact registrar occurrence.
pub type CallableInvocationRoots = BTreeMap<(MachineId, OperationId), BTreeSet<MachineId>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReachedCallableTarget {
    Declaration(MachineId),
    Callable(CallableValuePossibilities),
    /// The current operation has opaque code, not an absent call edge.
    Opaque,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReachableCallableInvocation {
    pub caller: MachineId,
    pub invocation: CallableInvocation,
    pub target: ReachedCallableTarget,
    pub selected_machines: BTreeSet<MachineId>,
}

impl ReachableCallableInvocation {
    pub fn targets_are_resolved(&self) -> bool {
        !self.selected_machines.is_empty()
            && match &self.target {
                ReachedCallableTarget::Declaration(_) => true,
                ReachedCallableTarget::Callable(values) => values.is_resolved(),
                ReachedCallableTarget::Opaque => false,
            }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReachableCallableInvocations {
    pub roots: BTreeSet<MachineId>,
    pub reached_machines: BTreeSet<MachineId>,
    pub invocations: BTreeMap<(MachineId, CallableInvocationSite), ReachableCallableInvocation>,
    pub inputs: BTreeMap<CallableFormalInput, CallableValuePossibilities>,
}

impl ReachableCallableInvocations {
    pub fn site(
        &self,
        caller: MachineId,
        site: CallableInvocationSite,
    ) -> Option<&ReachableCallableInvocation> {
        self.invocations.get(&(caller, site))
    }

    /// A missing summary row is allowed only for an authenticated source
    /// occurrence whose control-flow position cannot be reached from entry.
    pub fn checked_operation(
        &self,
        program: &CallableValueProgram<'_>,
        caller: MachineId,
        operation: OperationId,
    ) -> CallableValueResult<Option<&ReachableCallableInvocation>> {
        let site = CallableInvocationSite::Invocation(InvocationSite::Operation(operation));
        let source = program.body(caller)?.invocation(site)?;
        program.validate_occurrence(caller, source)?;
        if !self.reached_machines.contains(&caller) {
            return Err("callable summary has no established caller entry");
        }
        if let Some(reached) = self.site(caller, site) {
            if &reached.invocation != source {
                return Err("callable summary changed its exact source occurrence");
            }
            return Ok(Some(reached));
        }
        let flow = program.flow(caller)?;
        let reach = crate::descriptors::descriptor_results::payload_reachability::predecessors(
            flow,
            source.position,
        )
        .ok_or("callable source occurrence has unresolved control flow")?;
        if reach.contains(flow.entry) {
            return Err("callable summary lost a reached source occurrence");
        }
        Ok(None)
    }

    pub fn operation(
        &self,
        caller: MachineId,
        operation: OperationId,
    ) -> CallableValueResult<Option<&ReachableCallableInvocation>> {
        let mut matches = self
            .invocations
            .values()
            .filter(|row| row.caller == caller && row.invocation.operation == Some(operation));
        let first = matches.next();
        if matches.next().is_some() {
            return Err("reached callable invocation has an ambiguous operation association");
        }
        Ok(first)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ReachedEdge {
    caller: MachineId,
    site: CallableInvocationSite,
    callee: MachineId,
}

struct ReachedSolver<'a, 'program> {
    program: &'a CallableValueProgram<'program>,
    result: ReachableCallableInvocations,
    edges: BTreeSet<ReachedEdge>,
    queries: BTreeMap<CallableValueQuery, CallableValuePossibilities>,
    sites: BTreeMap<MachineId, Vec<CallableInvocation>>,
    bound_roots: &'a CallableInvocationRoots,
}

impl ReachedSolver<'_, '_> {
    fn raw(
        &mut self,
        query: CallableValueQuery,
    ) -> CallableValueResult<CallableValuePossibilities> {
        if let Some(value) = self.queries.get(&query) {
            return Ok(value.clone());
        }
        let (mut value, inputs) =
            resolve_reached_value(self.program, query.clone(), &self.result.inputs)?;
        for input in inputs {
            let values = self.input(input.clone());
            if !self.result.inputs.contains_key(&input) && !values.unresolved_ceilings.is_empty() {
                value.join(CallableValuePossibilities::ceiling(query.contract()));
            }
        }
        self.queries.insert(query, value.clone());
        Ok(value)
    }

    fn input(&mut self, input: CallableFormalInput) -> CallableValuePossibilities {
        if let Some(value) = self.result.inputs.get(&input) {
            return value.clone();
        }
        if let CallableParameter::Structural { position, path } = &input.parameter
            && self.result.inputs.keys().any(|previous| {
                matches!(&previous.parameter,
                    CallableParameter::Structural { position: p, path: previous_path }
                    if previous.machine == input.machine && previous.contract == input.contract
                        && p == position && previous_path.len() < path.len()
                        && path.ends_with(previous_path))
            })
        {
            // An unbounded recursive projection does not create an eligible
            // target roster. Its authored callable contract remains the ceiling.
            return CallableValuePossibilities::ceiling(input.contract);
        }
        let mut value = CallableValuePossibilities::default();
        if self.result.roots.contains(&input.machine) {
            value.inputs.insert(input.clone());
        }
        self.result.inputs.insert(input, value.clone());
        value
    }

    fn expand(&mut self, mut values: CallableValuePossibilities) -> CallableValuePossibilities {
        let inputs = std::mem::take(&mut values.inputs);
        for input in inputs {
            values.join(self.input(input));
        }
        values
    }

    fn call_sites(&mut self, machine: MachineId) -> CallableValueResult<Vec<CallableInvocation>> {
        if let Some(sites) = self.sites.get(&machine) {
            return Ok(sites.clone());
        }
        let body = self.program.body(machine)?;
        let flow = self.program.flow(machine)?;
        let occurrences = body
            .invocations
            .keys()
            .chain(body.invalid_invocations.keys())
            .copied()
            .collect::<BTreeSet<_>>();
        let mut sites = Vec::new();
        let mut operations = BTreeSet::new();
        for site in occurrences {
            let position = body
                .invocations
                .get(&site)
                .map(|call| call.position)
                .or_else(|| {
                    body.invalid_invocations
                        .get(&site)
                        .map(|(position, _)| *position)
                })
                .ok_or("reached callable occurrence lost its exact source cut")?;
            if !flow.flow.contains_key(&position) {
                return Err("reached callable invocation has a stale control-flow position");
            }
            let reach = crate::descriptors::descriptor_results::payload_reachability::predecessors(
                flow, position,
            )
            .ok_or("reached callable invocation has unresolved incoming control flow")?;
            if !reach.contains(flow.entry) {
                continue;
            }
            let invocation = body.invocation(site)?.clone();
            if let Some(ordinal) = invocation.successor {
                let successor = flow
                    .flow
                    .get(&position)
                    .and_then(|step| step.successors.get(ordinal))
                    .ok_or("callable cleanup lost its exact successor edge")?;
                let edge_reach =
                    crate::descriptors::descriptor_results::payload_reachability::predecessors(
                        flow,
                        successor.target,
                    )
                    .ok_or("callable cleanup edge has unresolved control flow")?;
                if !edge_reach.includes_edge(position, ordinal) {
                    continue;
                }
            }
            if let Some(operation) = invocation.operation
                && !operations.insert(operation)
            {
                return Err("reached callable invocations have ambiguous operation identities");
            }
            self.program.validate_occurrence(machine, &invocation)?;
            sites.push(invocation);
        }
        self.sites.insert(machine, sites.clone());
        Ok(sites)
    }

    fn evaluate_site(
        &mut self,
        caller: MachineId,
        invocation: &CallableInvocation,
    ) -> CallableValueResult<ReachedCallableTarget> {
        match invocation.target {
            CallableInvocationTarget::Machine(machine) => {
                self.program.validate_actuals(caller, machine, invocation)?;
                Ok(ReachedCallableTarget::Declaration(machine))
            }
            CallableInvocationTarget::Value(value, contract) => {
                let query = self.program.scalar_query(caller, value, contract)?;
                let values = self.raw(query)?;
                Ok(ReachedCallableTarget::Callable(self.expand(values)))
            }
            CallableInvocationTarget::Opaque => Ok(ReachedCallableTarget::Opaque),
        }
    }

    fn selected_machines(
        &self,
        caller: MachineId,
        invocation: &CallableInvocation,
        target: &ReachedCallableTarget,
    ) -> CallableValueResult<BTreeSet<MachineId>> {
        match target {
            ReachedCallableTarget::Declaration(machine) => Ok(BTreeSet::from([*machine])),
            ReachedCallableTarget::Callable(values) => {
                let CallableInvocationTarget::Value(_, contract) = invocation.target else {
                    return Err("reached callable selection lost its evaluated callee contract");
                };
                values
                    .descriptors
                    .iter()
                    .map(|id| {
                        let descriptor = self
                            .program
                            .catalog
                            .descriptor(*id)
                            .ok_or("reached callable descriptor is absent or stale")?;
                        if descriptor.contract != contract {
                            return Err(
                                "reached callable descriptor changes its evaluated contract",
                            );
                        }
                        self.program
                            .validate_actuals(caller, descriptor.machine, invocation)?;
                        Ok(descriptor.machine)
                    })
                    .collect()
            }
            ReachedCallableTarget::Opaque => Ok(BTreeSet::new()),
        }
    }

    fn retain_site(
        &mut self,
        caller: MachineId,
        invocation: CallableInvocation,
        target: ReachedCallableTarget,
    ) -> CallableValueResult<bool> {
        let key = (caller, invocation.site);
        let changed = match self.result.invocations.get_mut(&key) {
            Some(row) => {
                if row.invocation != invocation {
                    return Err("reached callable occurrence changed during its fixed point");
                }
                match (&mut row.target, target) {
                    (
                        ReachedCallableTarget::Callable(previous),
                        ReachedCallableTarget::Callable(values),
                    ) => previous.join(values),
                    (previous, current) if *previous == current => false,
                    _ => return Err("reached callable occurrence changed its target kind"),
                }
            }
            None => {
                self.result.invocations.insert(
                    key,
                    ReachableCallableInvocation {
                        caller,
                        invocation: invocation.clone(),
                        target,
                        selected_machines: BTreeSet::new(),
                    },
                );
                true
            }
        };
        let row = self
            .result
            .invocations
            .get(&key)
            .ok_or("reached callable occurrence lost its fixed-point row")?;
        let machines = self.selected_machines(caller, &invocation, &row.target)?;
        let mut changed = changed;
        for callee in machines {
            changed |= self
                .result
                .invocations
                .get_mut(&key)
                .ok_or("reached callable occurrence lost its selected machine association")?
                .selected_machines
                .insert(callee);
            changed |= self.edges.insert(ReachedEdge {
                caller,
                site: invocation.site,
                callee,
            });
            changed |= self.result.reached_machines.insert(callee);
        }
        for root in invocation
            .operation
            .and_then(|operation| self.bound_roots.get(&(caller, operation)))
            .into_iter()
            .flatten()
        {
            changed |= self.result.roots.insert(*root);
            changed |= self.result.reached_machines.insert(*root);
        }
        Ok(changed)
    }

    fn update_input(&mut self, input: &CallableFormalInput) -> CallableValueResult<bool> {
        let mut result = CallableValuePossibilities::default();
        if self.result.roots.contains(&input.machine) {
            result.inputs.insert(input.clone());
        }
        let incoming = self
            .edges
            .iter()
            .filter(|edge| edge.callee == input.machine)
            .cloned()
            .collect::<Vec<_>>();
        for edge in incoming {
            let invocation = self
                .result
                .invocations
                .get(&(edge.caller, edge.site))
                .ok_or("reached callable input lost its actual invocation")?
                .invocation
                .clone();
            self.program
                .validate_actuals(edge.caller, edge.callee, &invocation)?;
            let actual = self.program.input_actual(input, edge.caller, &invocation)?;
            let values = match actual {
                CallableActualValue::Value(query) => self.raw(query)?,
                CallableActualValue::Ceiling(contract) => {
                    CallableValuePossibilities::ceiling(contract)
                }
            };
            result.join(self.expand(values));
        }
        Ok(self
            .result
            .inputs
            .get_mut(input)
            .ok_or("reached callable input lost its fixed-point row")?
            .join(result))
    }
}

/// Summarize calls executable from the supplied authoritative entry roots.
/// Constructed descriptors retain code but never add invocation roots.
pub fn reachable_callable_invocations(
    program: &CallableValueProgram<'_>,
    roots: impl IntoIterator<Item = MachineId>,
) -> CallableValueResult<ReachableCallableInvocations> {
    reachable_callable_invocations_with_roots(program, roots, &CallableInvocationRoots::new())
}

pub fn reachable_callable_invocations_with_roots(
    program: &CallableValueProgram<'_>,
    roots: impl IntoIterator<Item = MachineId>,
    bound_roots: &CallableInvocationRoots,
) -> CallableValueResult<ReachableCallableInvocations> {
    let roots = roots.into_iter().collect::<BTreeSet<_>>();
    if roots.is_empty() {
        return Err("reachable callable analysis requires an authoritative entry root");
    }
    for root in &roots {
        program.body(*root)?;
        program.flow(*root)?;
    }
    for ((owner, operation), entries) in bound_roots {
        let body = program.body(*owner)?;
        let mut occurrences = body
            .invocations
            .values()
            .filter(|call| call.operation == Some(*operation));
        let occurrence = occurrences
            .next()
            .ok_or("callable entry root has no exact registrar invocation")?;
        if occurrences.next().is_some() || occurrence.boundary.is_none() {
            return Err("callable entry root has an ambiguous or non-boundary registrar");
        }
        program.validate_occurrence(*owner, occurrence)?;
        for entry in entries {
            program.body(*entry)?;
            program.flow(*entry)?;
        }
    }
    let mut solver = ReachedSolver {
        program,
        result: ReachableCallableInvocations {
            reached_machines: roots.clone(),
            roots,
            invocations: BTreeMap::new(),
            inputs: BTreeMap::new(),
        },
        edges: BTreeSet::new(),
        queries: BTreeMap::new(),
        sites: BTreeMap::new(),
        bound_roots,
    };
    loop {
        solver.queries.clear();
        let input_count = solver.result.inputs.len();
        let mut changed = false;
        let machines = solver
            .result
            .reached_machines
            .iter()
            .copied()
            .collect::<Vec<_>>();
        for machine in machines {
            for invocation in solver.call_sites(machine)? {
                let target = solver.evaluate_site(machine, &invocation)?;
                changed |= solver.retain_site(machine, invocation, target)?;
            }
        }
        let inputs = solver.result.inputs.keys().cloned().collect::<Vec<_>>();
        for input in inputs {
            changed |= solver.update_input(&input)?;
        }
        if changed || input_count != solver.result.inputs.len() {
            continue;
        }
        let bottoms = solver
            .result
            .inputs
            .iter()
            .filter(|(_, value)| value.is_bottom())
            .map(|(input, _)| input.clone())
            .collect::<Vec<_>>();
        if !bottoms.is_empty() {
            for input in bottoms {
                solver
                    .result
                    .inputs
                    .get_mut(&input)
                    .ok_or("reached callable cycle lost its input row")?
                    .join(CallableValuePossibilities::ceiling(input.contract));
            }
            continue;
        }
        break;
    }
    for row in solver.result.invocations.values_mut() {
        if let ReachedCallableTarget::Callable(values) = &mut row.target {
            let CallableInvocationTarget::Value(_, contract) = row.invocation.target else {
                return Err("reached callable result lost its exact contract");
            };
            if values.is_bottom() || !values.inputs.is_empty() {
                values.unresolved_ceilings.insert(contract);
            }
        }
    }
    Ok(solver.result)
}
