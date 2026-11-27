//! Check each crossing from a requester into a dependency against that use's own ceiling.
use diagnostics::Diagnostic;
use semantic_vocabulary::TargetMachineCallable as Callable;
use semantic_vocabulary::dependency_reach::{
    DependencyCallOrigin, DependencyCallOrigins, ExecutionReachPolicy, UnsafeServices,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use terminal_psi::{ProofCallableHeader, TerminalModule};
use terminal_semantics::callable_values::{
    CallableBoundaryImplementation, reachable_callable_invocations,
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

enum Via {
    Import(Option<Callable>),
    Call(Callable),
}

type Paths = BTreeMap<Callable, Via>;

fn complete_paths(
    calls: &CallGraph<'_>,
    paths: &mut Paths,
    mut pending: VecDeque<Callable>,
) -> Vec<Callable> {
    let mut reached = Vec::new();
    while let Some(current) = pending.pop_front() {
        reached.push(current);
        if let Some(callees) = calls.get(&current) {
            for &callee in callees.keys() {
                if let std::collections::btree_map::Entry::Vacant(slot) = paths.entry(callee) {
                    slot.insert(Via::Call(current));
                    pending.push_back(callee);
                }
            }
        }
    }
    reached
}

fn diagnostic_path(
    module: &TerminalModule,
    headers: &BTreeMap<Callable, &ProofCallableHeader>,
    paths: &Paths,
    roots: &Paths,
    current: Callable,
) -> Result<String, Vec<Diagnostic>> {
    let mut path = Vec::new();
    let mut cursor = current;
    let mut predecessors = paths;
    loop {
        path.push(name(module, headers[&cursor])?);
        match predecessors[&cursor] {
            Via::Call(caller) => cursor = caller,
            Via::Import(Some(requester)) => {
                cursor = requester;
                if roots.is_empty() {
                    path.push(name(module, headers[&requester])?);
                    break;
                }
                predecessors = roots;
            }
            Via::Import(None) => break,
        }
    }
    path.reverse();
    Ok(path.join(" -> "))
}

/// Edges are on the actually invoked functions and their selected providers. A callback
/// descriptor or exported declaration alone does not spend a dependency's ceiling.
pub fn check(
    module: &TerminalModule,
    entry: Callable,
    implementations: &[CallableBoundaryImplementation],
    target: Option<&str>,
    policy: &ExecutionReachPolicy,
    unsafe_services: &UnsafeServices,
    imported_calls: &DependencyCallOrigins,
) -> Result<(), Vec<Diagnostic>> {
    if policy.edges.is_empty() && unsafe_services.is_empty() {
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
    // The call graph, the reached invocations and the body service rows all
    // read one copy of the module's callable values.
    let analyses = terminal_semantics::synchronous_calls::call_analyses(module).map_err(error)?;
    let graph = analyses
        .calls()
        .with_boundary_calls(
            implementations
                .iter()
                .map(|row| (row.boundary, row.machine)),
        )
        .map_err(error)?;
    let mut calls = CallGraph::new();
    calls.entry(entry).or_default();
    if entry != Callable::Machine(root) {
        record_call(&mut calls, entry, Callable::Machine(root), None);
    }
    let program = analyses
        .bound_callable_values(implementations)
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
    let derived =
        terminal_semantics::body_services::body_service_reaches_in(&analyses, implementations)
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
    for edge in &policy.edges {
        registered
            .entry((edge.requester, edge.package))
            .or_insert_with(BTreeSet::new)
            .insert(edge.alias.as_str());
    }
    // The Product itself must opt in even when no package edge is crossed.
    // Each actual dependency use must satisfy its edge and that package's own Build.
    let mut root_paths = Paths::new();
    if !unsafe_services.is_empty() {
        root_paths.insert(entry, Via::Import(None));
        complete_paths(&calls, &mut root_paths, VecDeque::from([entry]));
    }
    let gates = std::iter::once(None)
        .filter(|_| !unsafe_services.is_empty())
        .chain(policy.edges.iter().map(Some));
    for gate in gates {
        let mut pending = VecDeque::new();
        let mut paths = BTreeMap::new();
        if let Some(edge) = gate {
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
        } else {
            paths.insert(entry, Via::Import(None));
            pending.push_back(entry);
        }
        let reached = complete_paths(&calls, &mut paths, pending);
        // A callee's demand also appears in its callers' derived ceilings.
        // Diagnose the originating call before that propagated demand.
        for current in reached.into_iter().rev() {
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
            let services = published.iter().chain(body);
            // A rejected concrete child explains the operation even if its reporting parent
            // appears earlier in the service table.
            let (specific, other) =
                services.partition::<Vec<_>, _>(|id| unsafe_services.get(**id).is_some_and(Option::is_some));
            for &service in specific.into_iter().chain(other) {
                let atom = module
                    .service(service)
                    .and_then(|decl| module.identities.text(decl.identity))
                    .ok_or_else(|| error("dependency reach lost an exact service identity"))?;
                let required = unsafe_services.get(service).and_then(Option::as_ref);
                let product = gate.map(|edge| edge.package).or(policy.root);
                let withheld_by_product = required.is_some_and(|required| {
                    !policy
                        .enabled_unsafe
                        .get(&product)
                        .is_some_and(|enabled| enabled.contains(required))
                });
                let withheld_by_edge = gate.filter(|edge| !edge.max_reach.admits(atom));
                if withheld_by_product || withheld_by_edge.is_some() {
                    let path = diagnostic_path(module, &headers, &paths, &root_paths, current)?;
                    if let Some(required) = required {
                        let subject = match (gate, withheld_by_edge) {
                            (_, Some(edge)) => {
                                format!("dependency edge `{}` did not admit", edge.alias)
                            }
                            (Some(edge), None) => {
                                format!("Build of dependency `{}` did not enable", edge.alias)
                            }
                            (None, None) => "Build of the root Product did not enable".to_owned(),
                        };
                        return Err(error(format!(
                            "{subject} Unsafe + {} on {path}\nOmega is designed to be used without Unsafe. Performance is not a valid justification. Use checked Omega or checked inline assembly for performance-sensitive code.\n\nUnsafe exists only for extreme applications. Treat any use of Unsafe in a library as malicious.",
                            required.name()
                        )));
                    }
                    if let Some(edge) = withheld_by_edge {
                        return Err(error(format!(
                            "dependency `{}` exceeds its reach ceiling with service `{atom}` on {path}",
                            edge.alias
                        )));
                    }
                }
            }
        }
    }
    Ok(())
}
