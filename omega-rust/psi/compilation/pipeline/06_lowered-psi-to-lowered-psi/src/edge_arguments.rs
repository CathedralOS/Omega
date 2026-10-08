use semantic_vocabulary::ValueId;

/// Drop the listed old positions from one edge's scalar arguments.
pub(crate) fn drop_removed_positions(arguments: &mut Vec<ValueId>, removed: Option<&Vec<usize>>) {
    let Some(removed) = removed else { return };
    let mut position = 0;
    arguments.retain(|_| {
        let retained = !removed.contains(&position);
        position += 1;
        retained
    });
}
