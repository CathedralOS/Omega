//! Iterative exact-key DFS: each package enters the stack once.

use super::super::{
    CanonicalDependencySourceSelection, CanonicalSourceClosureSubjectError as Error, usage::Budget,
};
use super::closure::selected_edges;
use crate::resolution::graph::ResolvedSourceIdentity;

pub(super) fn validate(
    packages: &[ResolvedSourceIdentity],
    edges: &[CanonicalDependencySourceSelection],
    root: usize,
    budget: &mut Budget,
) -> Result<(), Error> {
    let mut states = budget.reserve::<u8>(packages.len())?;
    states.resize(packages.len(), 0);
    let mut stack = budget.reserve::<(usize, usize)>(packages.len())?;
    states[root] = 1;
    stack.push((root, 0));
    while let Some((package, next)) = stack.last_mut() {
        let selected = selected_edges(edges, &packages[*package]);
        if let Some(edge) = selected.get(*next) {
            *next += 1;
            let target = packages
                .binary_search_by(|source| source.key().cmp(edge.selected.key()))
                .map_err(|_| invalid())?;
            match states[target] {
                1 => return Err(invalid()),
                2 => {}
                _ => {
                    states[target] = 1;
                    stack.push((target, 0));
                }
            }
        } else {
            states[*package] = 2;
            stack.pop();
        }
    }
    if states.contains(&0) {
        return Err(invalid());
    }
    Ok(())
}

fn invalid() -> Error {
    Error::new("source-closure subject does not form one closed reachable acyclic graph")
}
