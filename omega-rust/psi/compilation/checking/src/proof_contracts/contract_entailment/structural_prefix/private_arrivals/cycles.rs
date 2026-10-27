//! Cut recurrent call paths at fresh, arbitrary invocation entries.
//!
//! DFS backedge targets intersect every directed cycle. Starting each target
//! independently prevents a first iteration's values from becoming an invariant.
//! Start from actual group entries before disconnected members so ordinary
//! guarded loops keep their natural entry as the recurring proof boundary.

pub(super) fn entries(
    successors: &[Vec<usize>],
    needed: &[bool],
    declared_entries: &[usize],
) -> Vec<usize> {
    let mut colors = vec![0u8; successors.len()];
    let mut selected = vec![false; successors.len()];
    let mut pending = Vec::new();
    for start in declared_entries.iter().copied().chain(0..successors.len()) {
        if !needed[start] || colors[start] != 0 {
            continue;
        }
        colors[start] = 1;
        pending.push((start, 0));
        while let Some(&(current, next)) = pending.last() {
            if next == successors[current].len() {
                colors[current] = 2;
                pending.pop();
                continue;
            }
            let destination = successors[current][next];
            // The frame remains active while its next successor is examined.
            pending.last_mut().expect("active traversal frame").1 += 1;
            if !needed[destination] {
                continue;
            }
            match colors[destination] {
                0 => {
                    colors[destination] = 1;
                    pending.push((destination, 0));
                }
                1 => selected[destination] = true,
                _ => {}
            }
        }
    }
    selected
        .into_iter()
        .enumerate()
        .filter_map(|(index, selected)| selected.then_some(index))
        .collect()
}
