//! Recover invocation identities from executable Psi. An omitted claim cannot
//! hide a cycle, including a cycle through a selected cleanup or trait member.
use super::{ComputedRankError, error};
use omega_psi::*;
use std::collections::BTreeSet;

pub(super) struct Call<'a> {
    pub block: BlockId,
    pub at: CallSite,
    pub target: usize,
    pub arguments: &'a [CallArgument],
    pub site: &'a Site,
}

fn target(
    unit: &LoweredPackage,
    callee: &CallTarget,
    site: &Site,
) -> Result<Option<usize>, ComputedRankError> {
    let id = match callee {
        CallTarget::Machine(id) => *id,
        CallTarget::Member(slot) => match &unit
            .member_calls
            .get(*slot)
            .ok_or_else(|| error(site, "unknown member in rank call graph"))?
            .target
        {
            MemberRealization::Conformance { machine, .. } => *machine,
            MemberRealization::Boundary { .. } => return Ok(None),
        },
        CallTarget::Runtime { .. } => return Ok(None),
    };
    let i = usize::try_from(id.get() - 1)
        .ok()
        .filter(|i| unit.machines.get(*i).is_some_and(|m| m.id == id))
        .ok_or_else(|| error(site, "unknown local target in rank call graph"))?;
    Ok(Some(i))
}

pub(super) fn calls<'a>(
    unit: &'a LoweredPackage,
    m: &'a LoweredMachine,
) -> Result<Vec<Call<'a>>, ComputedRankError> {
    let b = &m.body;
    let fallback = &b.diagnostics.body;
    let mut found = Vec::new();
    for block in &b.blocks {
        let mut record = |at, callee: &CallTarget, args: &'a [CallArgument], site: &'a Site| -> Result<(), ComputedRankError> {
            if let Some(target) = target(unit, callee, site)? {
                found.push(Call {
                    block: block.id,
                    at,
                    target,
                    arguments: args,
                    site,
                });
            }
            Ok(())
        };
        let mut cleanups = Vec::new();
        for op in b
            .operations
            .get(block.operations.clone())
            .ok_or_else(|| error(fallback, "invalid operation range in rank call graph"))?
        {
            let site = b.diagnostics.operations.get(op.id.index()).unwrap_or(fallback);
            match &op.kind {
                OperationKind::Call { callee, arguments, .. } => record(CallSite::Operation(op.id), callee, arguments, site)?,
                OperationKind::StructuralStore {
                    disposition: StructuralStoreDisposition::Dispose(list),
                    ..
                } => cleanups.push((CleanupList::Store(op.id), list.as_slice(), site)),
                _ => {}
            }
        }
        let edge_site = |id: EdgeId| b.diagnostics.edges.get(id.get() - 1).unwrap_or(fallback);
        match &block.terminator {
            BlockTerminator::TerminalCall {
                id,
                callee,
                arguments,
                cleanup,
                ..
            } => {
                record(CallSite::Edge(*id), callee, arguments, edge_site(*id))?;
                cleanups.push((CleanupList::Edge(*id), cleanup, edge_site(*id)));
            }
            BlockTerminator::Return { id, cleanup, .. } => cleanups.push((CleanupList::Edge(*id), cleanup, edge_site(*id))),
            BlockTerminator::Jump(e) => cleanups.push((CleanupList::Edge(e.id), &e.cleanup, edge_site(e.id))),
            BlockTerminator::Conditional { when_true, when_false, .. } => {
                for e in [when_true, when_false] {
                    cleanups.push((CleanupList::Edge(e.id), &e.cleanup, edge_site(e.id)));
                }
            }
            BlockTerminator::ExternalExit(_) => {}
        }
        for (list, rows, site) in cleanups {
            for (action, row) in rows.iter().enumerate() {
                if let CleanupAction::Invoke { target, .. } = row {
                    record(CallSite::Cleanup(CleanupSite { list, action }), &CallTarget::Machine(*target), &[], site)?;
                }
            }
        }
    }
    Ok(found)
}

pub(super) fn reachable(
    graph: &[Vec<usize>],
    start: usize,
) -> BTreeSet<usize> {
    let mut seen = BTreeSet::new();
    let mut pending = vec![start];
    while let Some(at) = pending.pop() {
        if seen.insert(at) {
            pending.extend(&graph[at]);
        }
    }
    seen
}

pub(super) fn acyclic(graph: &[Vec<usize>]) -> bool {
    let mut degree = vec![0; graph.len()];
    for edges in graph {
        for &dest in edges {
            degree[dest] += 1;
        }
    }
    let mut stack = (0..graph.len()).filter(|&i| degree[i] == 0).collect::<Vec<_>>();
    let mut visited = 0;
    while let Some(i) = stack.pop() {
        visited += 1;
        for &to in &graph[i] {
            degree[to] -= 1;
            if degree[to] == 0 {
                stack.push(to);
            }
        }
    }
    visited == graph.len()
}
