//! Check each crossing from a requester into a dependency against that use's own ceiling.
use diagnostics::Diagnostic;
use semantic_vocabulary::TargetMachineCallable as Callable;
use semantic_vocabulary::dependency_reach::{
    DependencyCallOrigin, DependencyCallOrigins, ExecutionReachPolicy, UnsafeAtom, UnsafeServices,
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

/// A caller can report the same service as a callee that is reachable through a
/// shorter path. Only an intrinsic or an actual boundary identifies the operation.
struct UnmetService<'a> {
    callable: Callable,
    service: &'a str,
    required: Option<UnsafeAtom>,
    intrinsic: Option<&'static str>,
    edge_withheld: bool,
}

impl UnmetService<'_> {
    fn names_unsafe_operation(&self) -> bool {
        self.required.is_some()
            && (self.intrinsic.is_some() || matches!(self.callable, Callable::Boundary(_)))
    }
}

mod call_paths;
pub use call_paths::{AdmittedUnsafeCall, AdmittingDependency};
use call_paths::{Paths, Via, call_path, complete_paths, path_notes};

#[derive(Clone, Copy)]
pub struct SourceReach<'a> {
    pub policy: &'a ExecutionReachPolicy,
    pub unsafe_services: &'a UnsafeServices,
    pub imported_calls: &'a DependencyCallOrigins,
}

pub enum Findings {
    Admission,
    PackageAudit,
}

/// Edges are on the actually invoked functions and their selected providers. A callback
/// descriptor or exported declaration alone does not spend a dependency's ceiling.
pub fn check(
    module: &TerminalModule,
    entry: Callable,
    implementations: &[CallableBoundaryImplementation],
    target: Option<&str>,
    source: SourceReach<'_>,
    findings: Findings,
) -> Result<Vec<AdmittedUnsafeCall>, Vec<Diagnostic>> {
    let SourceReach {
        policy,
        unsafe_services,
        imported_calls,
    } = source;
    let mut admitted_paths = Vec::new();
    let check_unsafe = !unsafe_services.is_empty()
        || module.machines.iter().any(|machine| {
            machine.operations().any(|op| {
                matches!(
                    op.kind,
                    terminal_psi::OperationKind::BorrowAt { .. }
                        | terminal_psi::OperationKind::UpgradeMutability { .. }
                )
            })
        });
    if policy.edges.is_empty() && !check_unsafe {
        return Ok(admitted_paths);
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
    if check_unsafe {
        root_paths.insert(entry, Via::Import(None));
        complete_paths(&calls, &mut root_paths, VecDeque::from([entry]));
    }
    let gates = std::iter::once(None)
        .filter(|_| check_unsafe)
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
        let mut refusal: Option<UnmetService<'_>> = None;
        'find_origin: for current in reached.into_iter().rev() {
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
            let mut intrinsic = Vec::new();
            if let Callable::Machine(id) = current {
                let machine = module
                    .machine(id)
                    .ok_or_else(|| error("missing reached machine"))?;
                for operation in machine.operations() {
                    if let Some((name, atoms)) =
                        terminal_semantics::storage::native_borrows::required_authority(
                            module, operation,
                        )
                        .map_err(error)?
                    {
                        for &atom in atoms {
                            let (service, _) = unsafe_services.iter().find(|(_, value)| **value == Some(atom))
                                .ok_or_else(|| error(format!("native operation `{name}` lacks its installed core {} service", atom.name())))?;
                            if !intrinsic.contains(&(service, name)) {
                                intrinsic.push((service, name));
                            }
                        }
                    }
                }
            }
            if matches!(findings, Findings::PackageAudit)
                && (!intrinsic.is_empty()
                    || matches!(current, Callable::Boundary(_))
                        && published.iter().any(|service| {
                            unsafe_services.get(*service).copied().flatten().is_some()
                        }))
            {
                admitted_paths.push(AdmittedUnsafeCall {
                    dependency: gate.map(|edge| AdmittingDependency {
                        requester: edge.requester,
                        target: edge.package,
                        alias: edge.alias.clone(),
                    }),
                    calls: call_path(&paths, &root_paths, current),
                });
            }
            let services = intrinsic
                .iter()
                .map(|(id, _)| id)
                .chain(published.iter())
                .chain(body);
            for &service in services {
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
                if !withheld_by_product && withheld_by_edge.is_none() {
                    continue;
                }
                let failure = UnmetService {
                    callable: current,
                    service: atom,
                    required: required.copied(),
                    intrinsic: intrinsic
                        .iter()
                        .find_map(|(id, name)| (*id == service).then_some(*name)),
                    edge_withheld: withheld_by_edge.is_some(),
                };
                if failure.names_unsafe_operation() {
                    refusal = Some(failure);
                    break 'find_origin;
                }
                if refusal
                    .as_ref()
                    .is_none_or(|prior| prior.required.is_none() && required.is_some())
                {
                    refusal = Some(failure);
                }
            }
        }
        if let Some(failure) = refusal {
            let notes = path_notes(
                module,
                policy,
                &headers,
                &paths,
                &root_paths,
                failure.callable,
            )?;
            if let Some(required) = failure.required {
                let subject = match gate {
                    Some(edge) if failure.edge_withheld => {
                        format!("dependency edge `{}` did not admit", edge.alias)
                    }
                    Some(edge) => format!("Build of dependency `{}` did not enable", edge.alias),
                    None => "Build of the root Product did not enable".to_owned(),
                };
                let mut diagnostics = error(format!("{subject} Unsafe + {}", required.name()));
                diagnostics.extend(notes);
                if let Some(name) = failure.intrinsic {
                    diagnostics.push(Diagnostic::note(format!("operation: |unsafe|::{name}")));
                } else if let Callable::Boundary(_) = failure.callable {
                    let boundary = application_identity::type_identity::callable_display_path(
                        name(module, headers[&failure.callable])?,
                    );
                    diagnostics.push(Diagnostic::note(format!("operation: {boundary}")));
                }
                diagnostics.push(Diagnostic::note("Omega is designed to be used without Unsafe. Performance is not a valid justification. Use checked Omega or checked inline assembly for performance-sensitive code."));
                diagnostics.push(Diagnostic::note("Unsafe exists only for extreme applications. Treat any use of Unsafe in a library as malicious."));
                return Err(diagnostics);
            }
            let edge = gate.ok_or_else(|| error("a service ceiling requires a dependency edge"))?;
            let mut diagnostics = error(format!(
                "dependency `{}` exceeds its reach ceiling with service `{}`",
                edge.alias, failure.service
            ));
            diagnostics.extend(notes);
            return Err(diagnostics);
        }
    }
    Ok(admitted_paths)
}
