//! Bind symbolic code inputs through actual invocations reached from entry.

use super::*;

/// Admitted callback/interrupt entries bound to their exact registrar occurrence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallableInvocationRoot {
    pub caller: MachineId,
    pub operation: OperationId,
    pub entries: Vec<MachineId>,
}

pub type CallableInvocationRoots = Vec<CallableInvocationRoot>;

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
    pub selected_machines: Vec<MachineId>,
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
    pub roots: Vec<MachineId>,
    pub reached_machines: Vec<MachineId>,
    pub invocations: Vec<ReachableCallableInvocation>,
    pub inputs: Vec<(CallableFormalInput, CallableValuePossibilities)>,
}

impl ReachableCallableInvocations {
    pub fn site(
        &self,
        caller: MachineId,
        site: CallableInvocationSite,
    ) -> Option<&ReachableCallableInvocation> {
        self.invocations
            .binary_search_by_key(&(caller, site), |row| (row.caller, row.invocation.site))
            .ok()
            .map(|position| &self.invocations[position])
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
        if self.reached_machines.binary_search(&caller).is_err() {
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
            .iter()
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
    edges: Vec<ReachedEdge>,
    queries: Vec<(CallableValueQuery, CallableValuePossibilities)>,
    sites: Vec<Option<Vec<CallableInvocation>>>,
    bound_roots: &'a CallableInvocationRoots,
}

impl ReachedSolver<'_, '_> {
    fn raw(
        &mut self,
        query: CallableValueQuery,
    ) -> CallableValueResult<CallableValuePossibilities> {
        let position = match self.queries.binary_search_by(|(key, _)| key.cmp(&query)) {
            Ok(position) => return Ok(self.queries[position].1.clone()),
            Err(position) => position,
        };
        let (mut value, inputs) =
            resolve_reached_value(self.program, query.clone(), &self.result.inputs)?;
        for input in inputs {
            let values = self.input(input.clone());
            if input_position(&self.result.inputs, &input).is_err()
                && !values.unresolved_ceilings.is_empty()
            {
                value.join(CallableValuePossibilities::ceiling(query.contract()));
            }
        }
        self.queries.insert(position, (query, value.clone()));
        Ok(value)
    }

    fn input(&mut self, input: CallableFormalInput) -> CallableValuePossibilities {
        let position = match input_position(&self.result.inputs, &input) {
            Ok(position) => return self.result.inputs[position].1.clone(),
            Err(position) => position,
        };
        if let CallableParameter::Structural { position, path } = &input.parameter
            && self.result.inputs.iter().any(|(previous, _)| {
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
        if self.result.roots.binary_search(&input.machine).is_ok() {
            insert_ordered(&mut value.inputs, input.clone());
        }
        self.result.inputs.insert(position, (input, value.clone()));
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
        let index = self
            .program
            .bodies
            .binary_search_by_key(&machine, |(id, _)| *id)
            .map_err(|_| "callable provenance has no exact machine body")?;
        if let Some(sites) = &self.sites[index] {
            return Ok(sites.clone());
        }
        let body = self.program.body(machine)?;
        let flow = self.program.flow(machine)?;
        let mut sites = Vec::new();
        let mut operations = Vec::new();
        for occurrence in &body.invocations {
            let site = occurrence.site;
            let position = occurrence.position;
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
                && !insert_ordered(&mut operations, operation)
            {
                return Err("reached callable invocations have ambiguous operation identities");
            }
            self.program.validate_occurrence(machine, &invocation)?;
            sites.push(invocation);
        }
        self.sites[index] = Some(sites.clone());
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
    ) -> CallableValueResult<Vec<MachineId>> {
        match target {
            ReachedCallableTarget::Declaration(machine) => Ok(vec![*machine]),
            ReachedCallableTarget::Callable(values) => {
                let CallableInvocationTarget::Value(_, contract) = invocation.target else {
                    return Err("reached callable selection lost its evaluated callee contract");
                };
                let mut machines = Vec::with_capacity(values.descriptors.len());
                for id in &values.descriptors {
                    let descriptor = self
                        .program
                        .catalog
                        .descriptor(*id)
                        .ok_or("reached callable descriptor is absent or stale")?;
                    if descriptor.contract != contract {
                        return Err("reached callable descriptor changes its evaluated contract");
                    }
                    self.program
                        .validate_actuals(caller, descriptor.machine, invocation)?;
                    insert_ordered(&mut machines, descriptor.machine);
                }
                Ok(machines)
            }
            ReachedCallableTarget::Opaque => Ok(Vec::new()),
        }
    }

    fn retain_site(
        &mut self,
        caller: MachineId,
        invocation: CallableInvocation,
        target: ReachedCallableTarget,
    ) -> CallableValueResult<bool> {
        let key = (caller, invocation.site);
        let (position, mut changed) = match self
            .result
            .invocations
            .binary_search_by_key(&key, |row| (row.caller, row.invocation.site))
        {
            Ok(position) => {
                let row = &mut self.result.invocations[position];
                if row.invocation != invocation {
                    return Err("reached callable occurrence changed during its fixed point");
                }
                let changed = match (&mut row.target, target) {
                    (
                        ReachedCallableTarget::Callable(previous),
                        ReachedCallableTarget::Callable(values),
                    ) => previous.join(values),
                    (previous, current) if *previous == current => false,
                    _ => return Err("reached callable occurrence changed its target kind"),
                };
                (position, changed)
            }
            Err(position) => {
                self.result.invocations.insert(
                    position,
                    ReachableCallableInvocation {
                        caller,
                        invocation: invocation.clone(),
                        target,
                        selected_machines: Vec::new(),
                    },
                );
                (position, true)
            }
        };
        let machines = self.selected_machines(
            caller,
            &invocation,
            &self.result.invocations[position].target,
        )?;
        for callee in machines {
            changed |= insert_ordered(
                &mut self.result.invocations[position].selected_machines,
                callee,
            );
            changed |= insert_ordered(
                &mut self.edges,
                ReachedEdge {
                    caller,
                    site: invocation.site,
                    callee,
                },
            );
            changed |= insert_ordered(&mut self.result.reached_machines, callee);
        }
        if let Some(operation) = invocation.operation
            && let Ok(index) = self
                .bound_roots
                .binary_search_by_key(&(caller, operation), |row| (row.caller, row.operation))
        {
            for root in &self.bound_roots[index].entries {
                changed |= insert_ordered(&mut self.result.roots, *root);
                changed |= insert_ordered(&mut self.result.reached_machines, *root);
            }
        }
        Ok(changed)
    }

    fn update_input(&mut self, input: &CallableFormalInput) -> CallableValueResult<bool> {
        let mut result = CallableValuePossibilities::default();
        if self.result.roots.binary_search(&input.machine).is_ok() {
            insert_ordered(&mut result.inputs, input.clone());
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
                .site(edge.caller, edge.site)
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
        let position = input_position(&self.result.inputs, input)
            .map_err(|_| "reached callable input lost its fixed-point row")?;
        Ok(self.result.inputs[position].1.join(result))
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
    let mut roots = roots.into_iter().collect::<Vec<_>>();
    roots.sort_unstable();
    roots.dedup();
    if roots.is_empty() {
        return Err("reachable callable analysis requires an authoritative entry root");
    }
    for root in &roots {
        program.body(*root)?;
        program.flow(*root)?;
    }
    if bound_roots
        .windows(2)
        .any(|pair| (pair[0].caller, pair[0].operation) >= (pair[1].caller, pair[1].operation))
    {
        return Err("callable entry roots have repeated or unordered registrar occurrences");
    }
    for root in bound_roots {
        let (owner, operation, entries) = (root.caller, root.operation, &root.entries);
        if entries.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err("callable entry roots repeat or reorder selected entries");
        }
        let body = program.body(owner)?;
        let mut occurrences = body
            .invocations
            .iter()
            .filter_map(|row| row.value.as_ref().ok())
            .filter(|call| call.operation == Some(operation));
        let occurrence = occurrences
            .next()
            .ok_or("callable entry root has no exact registrar invocation")?;
        if occurrences.next().is_some() || occurrence.boundary.is_none() {
            return Err("callable entry root has an ambiguous or non-boundary registrar");
        }
        program.validate_occurrence(owner, occurrence)?;
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
            invocations: Vec::new(),
            inputs: Vec::new(),
        },
        edges: Vec::new(),
        queries: Vec::new(),
        sites: vec![None; program.bodies.len()],
        bound_roots,
    };
    loop {
        solver.queries.clear();
        let input_count = solver.result.inputs.len();
        let mut changed = false;
        let machines = solver.result.reached_machines.clone();
        for machine in machines {
            for invocation in solver.call_sites(machine)? {
                let target = solver.evaluate_site(machine, &invocation)?;
                changed |= solver.retain_site(machine, invocation, target)?;
            }
        }
        let inputs = solver
            .result
            .inputs
            .iter()
            .map(|(input, _)| input.clone())
            .collect::<Vec<_>>();
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
                let position = input_position(&solver.result.inputs, &input)
                    .map_err(|_| "reached callable cycle lost its input row")?;
                solver.result.inputs[position]
                    .1
                    .join(CallableValuePossibilities::ceiling(input.contract));
            }
            continue;
        }
        break;
    }
    for row in solver.result.invocations.iter_mut() {
        if let ReachedCallableTarget::Callable(values) = &mut row.target {
            let CallableInvocationTarget::Value(_, contract) = row.invocation.target else {
                return Err("reached callable result lost its exact contract");
            };
            if values.is_bottom() || !values.inputs.is_empty() {
                insert_ordered(&mut values.unresolved_ceilings, contract);
            }
        }
    }
    Ok(solver.result)
}
