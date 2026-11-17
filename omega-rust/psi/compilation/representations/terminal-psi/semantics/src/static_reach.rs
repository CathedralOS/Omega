//! Compose service reach from checked calls and exact static substitutions.
//! An open callback contributes a projection, never an empty service row.

use semantic_vocabulary::{MachineId, ServiceId};
use terminal_psi::static_reach::{
    StaticReachApplication, StaticReachArgument, StaticReachParameter, StaticReachSummary,
    StaticReachTerm,
};
use terminal_psi::{TerminalModule, UnoptimizedBehaviorOperation, UnoptimizedMachineBehavior};

type Result<T> = std::result::Result<T, &'static str>;

enum Contribution<'a> {
    Fixed(Vec<ServiceId>),
    Call {
        callee: usize,
        arguments: &'a [StaticReachArgument],
    },
}

fn union<T: Ord + Clone>(target: &mut Vec<T>, source: &[T]) {
    target.extend_from_slice(source);
    target.sort_unstable();
    target.dedup();
}

fn join(target: &mut StaticReachTerm, source: &StaticReachTerm) {
    union(&mut target.concrete, &source.concrete);
    union(&mut target.parameters, &source.parameters);
}

fn position(machines: &[MachineId], machine: MachineId) -> Result<usize> {
    machines
        .binary_search(&machine)
        .map_err(|_| "static reach names an absent machine")
}

fn substitute<'term>(
    term: &'term StaticReachTerm,
    arguments: &'term [StaticReachArgument],
    published: impl Fn(MachineId) -> Option<&'term StaticReachTerm>,
) -> Result<StaticReachTerm> {
    let mut answer = StaticReachTerm::default();
    let mut pending = vec![(term, arguments)];
    while let Some((term, arguments)) = pending.pop() {
        union(&mut answer.concrete, &term.concrete);
        for requirement in &term.parameters {
            match arguments
                .iter()
                .find(|argument| argument.requirement == *requirement)
            {
                Some(argument) => {
                    let selected = published(argument.selected)
                        .ok_or("selected callback has no published reach contract")?;
                    pending.push((selected, &argument.arguments));
                }
                None => union(&mut answer.parameters, &[*requirement]),
            }
        }
    }
    Ok(answer)
}

fn validate_arguments(
    machines: &[MachineId],
    parameters: &[StaticReachParameter],
    owner: MachineId,
    arguments: &[StaticReachArgument],
) -> Result<()> {
    let mut pending = vec![(owner, arguments)];
    while let Some((owner, arguments)) = pending.pop() {
        if arguments
            .windows(2)
            .any(|pair| pair[0].parameter >= pair[1].parameter)
        {
            return Err("static callback actuals repeat or reorder a parameter");
        }
        for argument in arguments {
            position(machines, argument.selected)?;
            if parameters
                .binary_search(&StaticReachParameter {
                    owner,
                    parameter: argument.parameter,
                    requirement: argument.requirement,
                })
                .is_err()
            {
                return Err("static callback actual has another originating requirement");
            }
            pending.push((argument.selected, &argument.arguments));
        }
    }
    Ok(())
}

/// This reads canonical calls, including cleanup calls. The caller checks each
/// supplied callback's refinement question separately before using the result.
pub fn derive<'bounds>(
    module: &TerminalModule,
    parameters: &[StaticReachParameter],
    applications: &[StaticReachApplication],
    abstract_reach: impl Fn(MachineId) -> Option<Option<&'bounds [ServiceId]>>,
) -> Result<Vec<StaticReachSummary>> {
    if module
        .machines
        .windows(2)
        .any(|pair| pair[0].id >= pair[1].id)
        || parameters.windows(2).any(|pair| pair[0] >= pair[1])
        || applications
            .windows(2)
            .any(|pair| (pair[0].caller, pair[0].site) >= (pair[1].caller, pair[1].site))
    {
        return Err("static reach coordinates are not strictly ordered");
    }
    let machines = module
        .machines
        .iter()
        .map(|machine| machine.id)
        .collect::<Vec<_>>();
    let mut projections = Vec::with_capacity(parameters.len());
    for parameter in parameters {
        position(&machines, parameter.owner)?;
        let requirement = &module.machines[position(&machines, parameter.requirement)?];
        if !requirement.blocks.is_empty() || requirement.declaration.is_some() {
            return Err("static callback projection has an executable requirement");
        }
        projections.push(parameter.requirement);
    }
    projections.sort_unstable();
    projections.dedup();
    for application in applications {
        position(&machines, application.caller)?;
        position(&machines, application.callee)?;
        validate_arguments(
            &machines,
            parameters,
            application.callee,
            &application.arguments,
        )?;
    }
    let mut rows = Vec::with_capacity(machines.len());
    let mut fixed = Vec::with_capacity(machines.len());
    for machine in &module.machines {
        let term = if projections.binary_search(&machine.id).is_ok() {
            Some(StaticReachTerm {
                concrete: Vec::new(),
                parameters: vec![machine.id],
            })
        } else if let Some(reach) = &machine.declared_service_reach {
            Some(StaticReachTerm {
                concrete: reach.clone(),
                parameters: Vec::new(),
            })
        } else if !machine.blocks.is_empty() {
            Some(StaticReachTerm::default())
        } else if let Some(reach) = abstract_reach(machine.id) {
            reach.map(|reach| StaticReachTerm {
                concrete: reach.to_vec(),
                parameters: Vec::new(),
            })
        } else {
            module
                .callables
                .machine_envelope(machine.id)
                .map(|envelope| StaticReachTerm {
                    concrete: envelope.service_reach.clone(),
                    parameters: Vec::new(),
                })
        };
        fixed.push(machine.blocks.is_empty() || machine.declared_service_reach.is_some());
        rows.push(term);
    }
    let mut contributions = Vec::with_capacity(machines.len());
    let mut predecessors = vec![Vec::new(); machines.len()];
    let mut used = vec![false; applications.len()];
    for (caller, machine) in module.machines.iter().enumerate() {
        let mut calls = Vec::new();
        for invocation in machine.module_invocations() {
            if projections.binary_search(&invocation.inputs.callee).is_ok()
                && invocation.static_reach_binding.is_none_or(|parameter| {
                    parameters
                        .binary_search(&StaticReachParameter {
                            owner: machine.id,
                            parameter,
                            requirement: invocation.inputs.callee,
                        })
                        .is_err()
                })
            {
                return Err("abstract callback call lost its exact originating parameter");
            }
            let application = applications
                .binary_search_by_key(&(machine.id, invocation.site), |row| (row.caller, row.site))
                .ok();
            let callee = position(&machines, invocation.inputs.callee)?;
            if let Some(contract) = invocation.inputs.required_contract {
                if application.is_some() {
                    return Err("a static substitution changed its required public contract");
                }
                let header = module
                    .invocation_machine_contract(invocation.inputs.callee, Some(contract))
                    .ok_or("static reach call has no exact public contract")?;
                calls.push(Contribution::Fixed(
                    header.published_service_ceiling.clone(),
                ));
            } else {
                let arguments = if let Some(index) = application {
                    let application = &applications[index];
                    if application.callee != invocation.inputs.callee {
                        return Err("static substitution names another called machine");
                    }
                    used[index] = true;
                    &application.arguments[..]
                } else {
                    &[]
                };
                let mut pending = arguments.iter().collect::<Vec<_>>();
                while let Some(argument) = pending.pop() {
                    predecessors[position(&machines, argument.selected)?].push(caller);
                    pending.extend(&argument.arguments);
                }
                predecessors[callee].push(caller);
                calls.push(Contribution::Call { callee, arguments });
            }
        }
        for invocation in machine.boundary_invocations() {
            let boundary = module
                .boundary_machine(invocation.boundary)
                .ok_or("static reach call has no exact boundary contract")?;
            calls.push(Contribution::Fixed(
                boundary.published_service_ceiling.clone(),
            ));
        }
        for invocation in machine.callable_invocations() {
            let contract = module
                .callables
                .contract(invocation.inputs.contract)
                .ok_or("static reach call has no exact runtime callable contract")?;
            calls.push(Contribution::Fixed(contract.envelope.service_reach.clone()));
        }
        for operation in UnoptimizedMachineBehavior::capture(machine).operations {
            if let UnoptimizedBehaviorOperation::InstructionService { service, .. } = operation {
                let mut reach = Vec::new();
                let mut pending = vec![service];
                while let Some(service) = pending.pop() {
                    if reach.binary_search(&service).is_ok() {
                        continue;
                    }
                    let index = module
                        .services
                        .binary_search_by_key(&service, |row| row.id)
                        .map_err(|_| "static reach instruction names an absent service")?;
                    union(&mut reach, &[service]);
                    pending.extend_from_slice(&module.services[index].parents);
                }
                calls.push(Contribution::Fixed(reach));
            }
        }
        contributions.push(calls);
    }
    if used.iter().any(|used| !used) {
        return Err("static callback substitution has no exact lowered invocation");
    }
    for callers in &mut predecessors {
        callers.sort_unstable();
        callers.dedup();
    }
    let mut pending = (0..machines.len()).collect::<Vec<_>>();
    let mut queued = vec![true; machines.len()];
    let mut cursor = 0;
    while cursor < pending.len() {
        let caller = pending[cursor];
        cursor += 1;
        queued[caller] = false;
        if fixed[caller] {
            continue;
        }
        let mut next = StaticReachTerm::default();
        for contribution in &contributions[caller] {
            match contribution {
                Contribution::Fixed(reach) => union(&mut next.concrete, reach),
                Contribution::Call { callee, arguments } => {
                    let callee = rows[*callee]
                        .as_ref()
                        .ok_or("open call reach has no originating static parameter")?;
                    join(
                        &mut next,
                        &substitute(callee, arguments, |selected| {
                            position(&machines, selected)
                                .ok()
                                .and_then(|position| rows[position].as_ref())
                        })?,
                    );
                }
            }
        }
        if rows[caller].as_ref() != Some(&next) {
            rows[caller] = Some(next);
            for &caller in &predecessors[caller] {
                if !queued[caller] {
                    queued[caller] = true;
                    pending.push(caller);
                }
            }
        }
    }
    Ok(machines
        .into_iter()
        .zip(rows)
        .filter_map(|(machine, term)| term.map(|term| StaticReachSummary { machine, term }))
        .collect())
}

/// Substitute one call without rebuilding the module-wide dependency graph.
/// The summaries have already been authenticated against those lowered calls.
pub fn applied_reach(
    summaries: &[StaticReachSummary],
    application: &StaticReachApplication,
) -> Result<StaticReachTerm> {
    let published = |id| {
        summaries
            .binary_search_by_key(&id, |row| row.machine)
            .ok()
            .map(|position| &summaries[position].term)
    };
    let callee = published(application.callee)
        .ok_or("static application has no published generic reach contract")?;
    substitute(callee, &application.arguments, published)
}

/// Commit portable parameter/member identities rather than allocation-local
/// machine IDs. Reordering calls cannot change this normalized interface.
pub fn template_commitment<'identity>(
    base: [u8; 32],
    fixed: &[ServiceId],
    projections: impl IntoIterator<Item = (u32, &'identity str)>,
    invocations: &[terminal_psi::TerminalCallableInvocation],
    service_identity: impl Fn(ServiceId) -> Option<&'identity str>,
) -> Result<[u8; 32]> {
    let mut services = fixed
        .iter()
        .map(|service| {
            service_identity(*service)
                .filter(|identity| !identity.is_empty())
                .ok_or("generic reach has no exact service identity")
        })
        .collect::<Result<Vec<_>>>()?;
    services.sort_unstable();
    services.dedup();
    let mut projections = projections.into_iter().collect::<Vec<_>>();
    if projections.iter().any(|(_, identity)| identity.is_empty()) {
        return Err("generic reach has no exact callback member identity");
    }
    projections.sort_unstable();
    projections.dedup();
    let mut digest = optimization::digest_encoder::DomainDigestEncoder::new(
        b"omega.generic-callable-reach-contract.v1\0",
    );
    digest.raw(&base);
    digest.len(services.len());
    for identity in services {
        digest.bytes(identity.as_bytes());
    }
    let mut entries = invocations
        .iter()
        .map(|entry| match entry {
            terminal_psi::TerminalCallableInvocation::Parameter(position) => Ok((0, *position, "")),
            terminal_psi::TerminalCallableInvocation::Service(service) => {
                service_identity(*service)
                    .filter(|identity| !identity.is_empty())
                    .map(|identity| (1, 0, identity))
                    .ok_or("generic invocation has no exact service identity")
            }
        })
        .collect::<Result<Vec<_>>>()?;
    entries.sort_unstable();
    entries.dedup();
    digest.len(entries.len());
    for (kind, position, identity) in entries {
        digest.raw(&[kind]);
        digest.raw(&position.to_le_bytes());
        digest.bytes(identity.as_bytes());
    }
    digest.len(projections.len());
    for (parameter, identity) in projections {
        digest.raw(&parameter.to_le_bytes());
        digest.bytes(identity.as_bytes());
    }
    Ok(digest.finish())
}
