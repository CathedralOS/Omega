//! A disposition settles one storage occurrence. Replacing that storage
//! withdraws its earlier disposal receipt; the new occurrence still owns its
//! selected normal-exit actions. This sequencing is shared by both body emitters.

use checked_trees::{
    CheckedNormalCleanupAction, CheckedUnitEffectOperationPlan, CheckedUnitStructuralParameterPlan,
};

pub(crate) fn outstanding_actions<'a>(
    actions: &'a [CheckedNormalCleanupAction],
    operations: &[CheckedUnitEffectOperationPlan],
    parameters: &[CheckedUnitStructuralParameterPlan],
    control: &checked_trees::CheckedCleanupControlPlans,
) -> Vec<&'a CheckedNormalCleanupAction> {
    let mut outstanding = vec![true; actions.len()];
    for operation in operations {
        for (action, retained) in actions.iter().zip(&mut outstanding) {
            match operation {
                CheckedUnitEffectOperationPlan::ContinuationCleanup {
                    cleanup_actions, ..
                } => {
                    if cleanup_actions.contains(action) {
                        *retained = false;
                    }
                }
                CheckedUnitEffectOperationPlan::ReleaseReference { owner_cleanups, .. }
                    if control
                        .reference_owner_cleanups
                        .span_or_empty(*owner_cleanups)
                        .iter()
                        .any(|owner| {
                            control
                                .actions
                                .span_or_empty(owner.actions)
                                .contains(action)
                        }) =>
                {
                    *retained = false;
                }
                CheckedUnitEffectOperationPlan::Store(store) => {
                    if store.window_place(parameters).is_some_and(|destination| {
                        action.source() == &destination.source
                            && action.path().starts_with(&destination.path)
                    }) {
                        *retained = true;
                    }
                }
                CheckedUnitEffectOperationPlan::ReleaseReference {
                    source,
                    path,
                    defer_to_exit: false,
                    ..
                } if path.is_empty()
                    && action.source() == source
                    && action.path().is_empty()
                    && !matches!(action, CheckedNormalCleanupAction::InvokeNominal(_)) =>
                {
                    *retained = false;
                }
                _ => {}
            }
        }
    }
    actions
        .iter()
        .zip(outstanding)
        .filter_map(|(action, retained)| retained.then_some(action))
        .collect()
}
