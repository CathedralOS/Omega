//! Derive conservative body reach independently of a named machine's authored
//! ceiling. Known bodies compose through their actual invocation edges; an
//! unresolved code value contributes its complete retained callable contract.

use crate::callable_values::{CallableInvocationTarget, terminal_callable_values};
use semantic_vocabulary::{MachineId, ServiceId};
use terminal_psi::{TerminalModule, UnoptimizedBehaviorOperation, UnoptimizedMachineBehavior};

type Result<T> = std::result::Result<T, &'static str>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyServiceReaches {
    machines: Vec<MachineId>,
    rows: Vec<Vec<ServiceId>>,
}
impl BodyServiceReaches {
    pub fn get(&self, machine: MachineId) -> Result<&[ServiceId]> {
        let position = self
            .machines
            .binary_search(&machine)
            .map_err(|_| "body service summary has no such machine")?;
        Ok(&self.rows[position])
    }
}

pub fn body_service_reaches(module: &TerminalModule) -> Result<BodyServiceReaches> {
    if module
        .machines
        .windows(2)
        .any(|rows| rows[0].id >= rows[1].id)
        || module
            .services
            .windows(2)
            .any(|rows| rows[0].id >= rows[1].id)
    {
        return Err("body service summary requires canonical machine and service coordinates");
    }
    let machines = module
        .machines
        .iter()
        .map(|machine| machine.id)
        .collect::<Vec<_>>();
    let machine_position = |machine| {
        machines
            .binary_search(&machine)
            .map_err(|_| "body service edge names an absent machine")
    };
    let service_position = |service| {
        module
            .services
            .binary_search_by_key(&service, |row| row.id)
            .map_err(|_| "body service row names an absent service")
    };
    let mut parent_positions = Vec::with_capacity(module.services.len());
    for service in &module.services {
        if service.parents.windows(2).any(|rows| rows[0] >= rows[1]) {
            return Err("body service inheritance repeats or reorders a parent");
        }
        parent_positions.push(
            service
                .parents
                .iter()
                .map(|parent| service_position(*parent))
                .collect::<Result<Vec<_>>>()?,
        );
    }
    let mut colors = vec![0u8; module.services.len()];
    for root in 0..module.services.len() {
        let mut pending = vec![(root, false)];
        while let Some((position, leaving)) = pending.pop() {
            if leaving {
                colors[position] = 2;
                continue;
            }
            match colors[position] {
                1 => return Err("body service inheritance has a cycle"),
                2 => continue,
                _ => {}
            }
            colors[position] = 1;
            pending.push((position, true));
            pending.extend(
                parent_positions[position]
                    .iter()
                    .rev()
                    .map(|parent| (*parent, false)),
            );
        }
    }
    let validate_row = |row: &[ServiceId]| -> Result<()> {
        if row.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err("body service contract row is not canonical");
        }
        for service in row {
            let position = service_position(*service)?;
            if module.services[position]
                .parents
                .iter()
                .any(|parent| row.binary_search(parent).is_err())
            {
                return Err("body service contract row omits an inherited parent");
            }
        }
        Ok(())
    };
    for boundary in &module.boundary_machines {
        validate_row(&boundary.published_service_ceiling)?;
    }
    for contract in &module.callables.contracts {
        validate_row(&contract.envelope.service_reach)?;
    }
    let program = terminal_callable_values(module)?;
    let mut rows = vec![Vec::new(); machines.len()];
    let mut predecessors = vec![Vec::new(); machines.len()];
    for (position, machine) in module.machines.iter().enumerate() {
        let body = program
            .bodies
            .get(&machine.id)
            .ok_or("body service summary lost its exact invocation source")?;
        if let Some((_, (_, reason))) = body.invalid_invocations.iter().next() {
            return Err(*reason);
        }
        for invocation in body.invocations.values() {
            program.validate_occurrence(machine.id, invocation)?;
            match invocation.target {
                CallableInvocationTarget::Machine(callee) => {
                    program.validate_actuals(machine.id, callee, invocation)?;
                    let callee = machine_position(callee)?;
                    predecessors[callee].push(position);
                }
                CallableInvocationTarget::Value(_, contract) => {
                    let contract = module
                        .callables
                        .contract(contract)
                        .ok_or("body service invocation names a missing callable contract")?;
                    union(&mut rows[position], &contract.envelope.service_reach);
                }
                CallableInvocationTarget::Opaque => {
                    let boundary=invocation.boundary.and_then(|id|module.boundary_machine(id)).ok_or("body service invocation has opaque code without a retained boundary contract")?;
                    if boundary.scalar_parameters.len() != invocation.scalar_arguments.len()
                        || boundary.structural_parameters.len()
                            != invocation.structural_arguments.len()
                        || !boundary
                            .scalar_parameters
                            .iter()
                            .zip(&invocation.scalar_arguments)
                            .all(|(expected, actual)| {
                                body.scalar_types.get(actual) == Some(expected)
                            })
                        || !boundary
                            .structural_parameters
                            .iter()
                            .zip(&invocation.structural_actual_types)
                            .all(|(expected, actual)| {
                                (expected.structural_type, expected.access) == *actual
                            })
                    {
                        return Err(
                            "body service boundary actuals disagree with its complete formal telescope",
                        );
                    }
                    union(&mut rows[position], &boundary.published_service_ceiling);
                }
            }
        }
        // This established operation classifier also includes instruction
        // services that are not expressed as calls in the scalar graph.
        for operation in UnoptimizedMachineBehavior::capture(machine).operations {
            if let UnoptimizedBehaviorOperation::InstructionService { service, .. } = operation {
                let mut pending = vec![service_position(service)?];
                let mut retained = vec![false; module.services.len()];
                while let Some(service) = pending.pop() {
                    if retained[service] {
                        continue;
                    }
                    retained[service] = true;
                    union(&mut rows[position], &[module.services[service].id]);
                    pending.extend_from_slice(&parent_positions[service]);
                }
            }
        }
    }
    for row in &mut predecessors {
        row.sort_unstable();
        row.dedup();
    }
    let mut pending = rows
        .iter()
        .enumerate()
        .filter_map(|(position, row)| (!row.is_empty()).then_some(position))
        .collect::<Vec<_>>();
    let mut queued = rows.iter().map(|row| !row.is_empty()).collect::<Vec<_>>();
    let mut cursor = 0;
    while cursor < pending.len() {
        let callee = pending[cursor];
        cursor += 1;
        queued[callee] = false;
        let contribution = rows[callee].clone();
        for &caller in &predecessors[callee] {
            if union(&mut rows[caller], &contribution) && !queued[caller] {
                queued[caller] = true;
                pending.push(caller);
            }
        }
    }
    Ok(BodyServiceReaches { machines, rows })
}

fn union(target: &mut Vec<ServiceId>, source: &[ServiceId]) -> bool {
    let mut changed = false;
    for service in source {
        if let Err(position) = target.binary_search(service) {
            target.insert(position, *service);
            changed = true;
        }
    }
    changed
}
