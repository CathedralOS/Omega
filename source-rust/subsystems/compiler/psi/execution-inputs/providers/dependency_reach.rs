//! Check each crossing from a requester into a dependency against that use's own ceiling.
use diagnostics::Diagnostic;
use semantic_vocabulary::TargetMachineCallable as Callable;
use semantic_vocabulary::dependency_reach::{
    DependencyCallOrigin, DependencyCallOrigins, DependencyServiceEdge,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use terminal_psi::{ProofCallableHeader, TerminalModule};
use terminal_semantics::callable_values::{
    CallableBoundaryImplementation, reachable_callable_invocations,
    terminal_callable_values_with_boundary_implementations,
};

fn error(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::error(message)]
}

fn name<'a>(
    module: &'a TerminalModule,
    header: &ProofCallableHeader,
) -> Result<&'a str, Vec<Diagnostic>> {
    header
        .callable
        .as_ref()
        .ok()
        .and_then(|identity| module.identities.text(*identity))
        .ok_or_else(|| error("dependency reach lost the callable's canonical identity"))
}

type CallGraph<'source> =
    BTreeMap<Callable, BTreeMap<Callable, BTreeSet<Option<&'source DependencyCallOrigin>>>>;

fn record_call<'source>(
    calls: &mut CallGraph<'source>,
    caller: Callable,
    target: Callable,
    origin: Option<&'source BTreeSet<DependencyCallOrigin>>,
) {
    let uses = calls.entry(caller).or_default().entry(target).or_default();
    match origin {
        Some(choices) if !choices.is_empty() => uses.extend(choices.iter().map(Some)),
        _ => {
            uses.insert(None);
        }
    }
}

/// Edges are on the actually invoked functions and their selected providers. A callback
/// descriptor or exported declaration alone does not spend a dependency's ceiling.
pub fn check(
    module: &TerminalModule,
    entry: Callable,
    implementations: &[CallableBoundaryImplementation],
    target: Option<&str>,
    edges: &[DependencyServiceEdge],
    imported_calls: &DependencyCallOrigins,
) -> Result<(), Vec<Diagnostic>> {
    if edges.is_empty() {
        return Ok(());
    }
    let root = match entry {
        Callable::Machine(id) => id,
        Callable::Boundary(id) => implementations
            .iter()
            .find(|row| row.boundary == id)
            .map(|row| row.machine)
            .ok_or_else(|| {
                error("dependency reach needs the installed body of the package entry")
            })?,
    };
    let graph = terminal_semantics::synchronous_calls::SynchronousCallGraph::new(module, None)
        .and_then(|graph| {
            graph.with_boundary_calls(
                implementations
                    .iter()
                    .map(|row| (row.boundary, row.machine)),
            )
        })
        .map_err(error)?;
    let mut calls = CallGraph::new();
    calls.entry(entry).or_default();
    if entry != Callable::Machine(root) {
        record_call(&mut calls, entry, Callable::Machine(root), None);
    }
    let program = terminal_callable_values_with_boundary_implementations(module, implementations)
        .map_err(error)?;
    let mut roots = BTreeSet::from([root]);
    let mut traversed = BTreeSet::new();
    let reached = loop {
        let reached =
            reachable_callable_invocations(&program, roots.iter().copied()).map_err(error)?;
        let new = reached
            .reached_machines
            .iter()
            .copied()
            .filter(|machine| traversed.insert(*machine))
            .collect::<Vec<_>>();
        if new.is_empty()
            || graph
                .scalar_evaluation_roots(&new, target)
                .map_err(error)?
                .is_empty()
        {
            break reached;
        }
        let prior = roots.len();
        // Scalar evaluation is part of the requesting function's use. Keep that
        // edge even when the same exact scalar is also evaluated by another package.
        for requester in new {
            for scalar in graph
                .scalar_evaluation_roots(&[requester], target)
                .map_err(error)?
            {
                record_call(&mut calls, Callable::Machine(requester), scalar, None);
                match scalar {
                    Callable::Machine(machine) => {
                        roots.insert(machine);
                    }
                    Callable::Boundary(boundary) => {
                        if let Some(body) = implementations
                            .iter()
                            .find(|body| body.boundary == boundary)
                        {
                            roots.insert(body.machine);
                            record_call(&mut calls, scalar, Callable::Machine(body.machine), None);
                        }
                    }
                }
            }
        }
        if roots.len() == prior {
            break reached;
        }
    };
    for row in reached.invocations {
        let caller = Callable::Machine(row.caller);
        // An indirect call selects a value delivered to this machine. Its namespace
        // cannot authorize the selected body on the supplier's behalf.
        let direct = row.invocation.boundary.is_some()
            || matches!(
                row.target,
                terminal_semantics::callable_values::ReachedCallableTarget::Declaration(_)
            );
        let site = direct
            .then(|| imported_calls.get(&(row.caller, row.invocation.site)))
            .flatten();
        let source = if let Some(id) = row.invocation.boundary {
            let boundary = Callable::Boundary(id);
            record_call(&mut calls, caller, boundary, site);
            boundary
        } else {
            caller
        };
        for machine in row.selected_machines {
            let target = Callable::Machine(machine);
            // Provider installation selects the implementation of a called boundary.
            record_call(
                &mut calls,
                source,
                target,
                if source == caller { site } else { None },
            );
        }
    }
    let derived = terminal_semantics::body_services::body_service_reaches(module, implementations)
        .map_err(error)?;
    let scope = &module.scalars.scalar_applications.proof_scope;
    let declarations = scope
        .callable_headers
        .iter()
        .map(|(_, header)| (header.declaration, header))
        .collect::<BTreeMap<_, _>>();
    let mut headers = BTreeMap::new();
    for &callable in calls.keys().chain(calls.values().flat_map(BTreeMap::keys)) {
        if let std::collections::btree_map::Entry::Vacant(slot) = headers.entry(callable) {
            let header = scope
                .callable_declaration(callable)
                .and_then(|symbol| declarations.get(&symbol))
                .copied()
                .ok_or_else(|| error("dependency reach lost a callable's checked package"))?;
            slot.insert(header);
        }
    }
    let mut registered = BTreeMap::new();
    for edge in edges {
        registered
            .entry((edge.requester, edge.package))
            .or_insert_with(BTreeSet::new)
            .insert(edge.alias.as_str());
    }
    for edge in edges {
        let mut pending = VecDeque::new();
        // Keep a single predecessor for each invocation. Diagnostics reconstruct just the
        // failing path, including its requester, without duplicating paths at every branch.
        enum Via {
            Import(Option<Callable>),
            Call(Callable),
        }
        let mut paths = BTreeMap::new();
        if headers[&entry].package == Some(edge.package) {
            paths.insert(entry, Via::Import(None));
            pending.push_back(entry);
        }
        for (&requester, callees) in &calls {
            let owner = headers[&requester].package;
            if owner == Some(edge.package) {
                continue;
            }
            for (&callee, uses) in callees {
                if headers[&callee].package != Some(edge.package) {
                    continue;
                }
                // A forwarded descriptor can enter here through another requester even
                // if the current machine happens to import the very same package itself.
                if let Some(aliases) = registered.get(&(owner, edge.package)) {
                    let exact = uses.iter().all(|choice| {
                        choice.is_some_and(|choice| {
                            choice.package == edge.package
                                && aliases.contains(choice.alias.as_str())
                        })
                    });
                    let selected = uses
                        .iter()
                        .flatten()
                        .any(|choice| choice.alias == edge.alias);
                    if exact && (owner != edge.requester || !selected) {
                        continue;
                    }
                }
                if let std::collections::btree_map::Entry::Vacant(slot) = paths.entry(callee) {
                    slot.insert(Via::Import(Some(requester)));
                    pending.push_back(callee);
                }
            }
        }
        while let Some(current) = pending.pop_front() {
            let (published, body): (&[_], &[_]) = match current {
                Callable::Machine(id) => {
                    let machine = module
                        .machine(id)
                        .ok_or_else(|| error("dependency reach lost an invoked machine"))?;
                    (
                        &machine.published_service_ceiling,
                        derived.get(id).map_err(error)?,
                    )
                }
                Callable::Boundary(id) => {
                    let boundary = module
                        .boundary_machine(id)
                        .ok_or_else(|| error("dependency reach lost a called boundary"))?;
                    if boundary.service_reach_is_installation_bound
                        && implementations
                            .iter()
                            .any(|provider| provider.boundary == id)
                    {
                        (&[], &[])
                    } else {
                        (&boundary.published_service_ceiling, &[])
                    }
                }
            };
            for &service in published.iter().chain(body) {
                let atom = module
                    .service(service)
                    .and_then(|decl| module.identities.text(decl.identity))
                    .ok_or_else(|| error("dependency reach lost an exact service identity"))?;
                if !edge.max_reach.admits(atom) {
                    let mut path = Vec::new();
                    let mut at = current;
                    loop {
                        path.push(name(module, headers[&at])?);
                        match paths[&at] {
                            Via::Call(previous) => at = previous,
                            Via::Import(previous) => {
                                if let Some(previous) = previous {
                                    path.push(name(module, headers[&previous])?);
                                }
                                break;
                            }
                        }
                    }
                    path.reverse();
                    return Err(error(format!(
                        "dependency `{}` exceeds its reach ceiling with service `{atom}` on {}",
                        edge.alias,
                        path.join(" -> ")
                    )));
                }
            }
            if let Some(callees) = calls.get(&current) {
                for &callee in callees.keys() {
                    if let std::collections::btree_map::Entry::Vacant(slot) = paths.entry(callee) {
                        slot.insert(Via::Call(current));
                        pending.push_back(callee);
                    }
                }
            }
        }
    }
    Ok(())
}
