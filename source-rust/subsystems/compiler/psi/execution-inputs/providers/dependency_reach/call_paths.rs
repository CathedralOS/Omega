//! The call path witnessed by Product admission, including the specific incoming
//! registration when a dependency ceiling was checked.
use super::{CallGraph, name};
use diagnostics::Diagnostic;
use semantic_vocabulary::{
    PackageKeyIdentity, TargetMachineCallable as Callable, dependency_reach::ExecutionReachPolicy,
};
use std::collections::{BTreeMap, VecDeque};
use terminal_psi::{ProofCallableHeader, TerminalModule};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedUnsafeCall {
    pub dependency: Option<AdmittingDependency>,
    pub calls: Vec<Callable>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittingDependency {
    pub requester: Option<PackageKeyIdentity>,
    pub target: PackageKeyIdentity,
    pub alias: String,
}

pub(super) enum Via {
    Import(Option<Callable>),
    Call(Callable),
}

pub(super) type Paths = BTreeMap<Callable, Via>;

pub(super) fn complete_paths(
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

pub(super) fn call_path(paths: &Paths, roots: &Paths, current: Callable) -> Vec<Callable> {
    let mut path = Vec::new();
    let mut cursor = current;
    let mut predecessors = paths;
    loop {
        path.push(cursor);
        match predecessors[&cursor] {
            Via::Call(caller) => cursor = caller,
            Via::Import(Some(requester)) => {
                cursor = requester;
                if roots.is_empty() {
                    path.push(requester);
                    break;
                }
                predecessors = roots;
            }
            Via::Import(None) => break,
        }
    }
    path.reverse();
    path
}

pub(super) fn path_notes(
    module: &TerminalModule,
    policy: &ExecutionReachPolicy,
    headers: &BTreeMap<Callable, &ProofCallableHeader>,
    paths: &Paths,
    roots: &Paths,
    current: Callable,
) -> Result<Vec<Diagnostic>, Vec<Diagnostic>> {
    let path = call_path(paths, roots, current);
    let mut packages = path
        .iter()
        .filter_map(|callable| headers[callable].package)
        .collect::<Vec<_>>();
    packages.dedup();
    let packages = packages
        .iter()
        .filter_map(|package| policy.package_names.get(package).map(String::as_str))
        .collect::<Vec<_>>();
    let calls = path
        .into_iter()
        .map(|callable| {
            name(module, headers[&callable])
                .map(application_identity::type_identity::callable_display_path)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut notes = Vec::new();
    if !packages.is_empty() {
        notes.push(Diagnostic::note(format!(
            "dependency path: {}",
            packages.join(" -> ")
        )));
    }
    notes.push(Diagnostic::note(format!(
        "call path: {}",
        calls.join(" -> ")
    )));
    Ok(notes)
}
