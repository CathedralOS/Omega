//! Cut recurrent call paths at fresh, arbitrary invocation entries.
//!
//! DFS backedge targets intersect every directed cycle. Starting each target
//! independently prevents a first iteration's values from becoming an invariant.
//! Every independently justified entry boundary is retained regardless of
//! traversal order: open callable inputs and authored cycle contracts cannot
//! inherit the known caller's facts. DFS cuts only the remaining cycles.
//! Start from actual group entries before disconnected members so ordinary
//! guarded loops keep their natural entry as the recurring proof boundary.

#[derive(Clone, Copy, PartialEq, Eq)]
enum VisitState {
    Unvisited,
    Active,
    Complete,
}

pub(super) fn entries(
    successors: &[Vec<usize>],
    needed: &[bool],
    declared_entries: &[usize],
    independent_entries: &[usize],
) -> Vec<usize> {
    let mut visits = vec![VisitState::Unvisited; successors.len()];
    let mut selected = vec![false; successors.len()];
    let mut independent_boundary = vec![false; successors.len()];
    for &entry in independent_entries {
        if needed[entry] {
            independent_boundary[entry] = true;
            selected[entry] = true;
        }
    }
    let mut pending = Vec::new();
    for start in independent_entries
        .iter()
        .chain(declared_entries)
        .copied()
        .chain(0..successors.len())
    {
        if !needed[start] || visits[start] != VisitState::Unvisited {
            continue;
        }
        visits[start] = VisitState::Active;
        pending.push((start, 0));
        while let Some(&(current, next)) = pending.last() {
            if next == successors[current].len() {
                visits[current] = VisitState::Complete;
                pending.pop();
                continue;
            }
            let destination = successors[current][next];
            // The frame remains active while its next successor is examined.
            pending.last_mut().expect("active traversal frame").1 += 1;
            if !needed[destination] || independent_boundary[destination] {
                continue;
            }
            match visits[destination] {
                VisitState::Unvisited => {
                    visits[destination] = VisitState::Active;
                    pending.push((destination, 0));
                }
                VisitState::Active => selected[destination] = true,
                VisitState::Complete => {}
            }
        }
    }
    selected
        .into_iter()
        .enumerate()
        .filter_map(|(index, selected)| selected.then_some(index))
        .collect()
}
