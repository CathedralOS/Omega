//! Merge independently selected partitions in the declared cleanup order.
//! Only retained action paths are visited. Their exact typed coordinates join
//! the same canonical ordering authority used to check hook prerequisites.
use super::actions::checked_path_segments;
use checked_trees::{CheckedNormalCleanupAction, CheckedUnitStructuralArgumentSourcePlan};

pub(super) fn merge(
    program: &dyn validation::CheckingInput,
    state: symbols::SymbolHandle,
    statement: usize,
    root: facts::PlaceRoot,
    actions: &mut [CheckedNormalCleanupAction],
    source: &CheckedUnitStructuralArgumentSourcePlan,
) -> Option<()> {
    let reference = validation::canonical_place_type_reference(
        program,
        state,
        statement,
        &validation::CanonicalPlace {
            root,
            segments: Vec::new(),
        },
    )?;
    let positions = actions
        .iter()
        .enumerate()
        .filter_map(|(position, action)| (action.source() == source).then_some(position))
        .collect::<Vec<_>>();
    let mut selected = positions
        .iter()
        .map(|position| {
            let action = actions[*position].clone();
            let path = checked_path_segments(program, state, statement, reference, action.path())?;
            let key = crate::flow::cleanup_path_order(program, &path)?;
            Some((key, action))
        })
        .collect::<Option<Vec<_>>>()?;
    selected.sort_by(|left, right| left.0.cmp(&right.0));
    for (position, (_, action)) in positions.into_iter().zip(selected) {
        actions[position] = action;
    }
    Some(())
}
