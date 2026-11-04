//! A disposition settles one storage occurrence. Replacing that storage
//! withdraws its earlier disposal receipt; the new occurrence still owns its
//! selected normal-exit actions. This sequencing is shared by both body emitters.

use checked_trees::{
    CheckedNormalCleanupAction, CheckedStoreLeaf, CheckedStoreRoot, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralArgumentSourcePlan as SourcePlan, CheckedUnitStructuralParameterPlan,
};
use symbols::SymbolHandle;

/// Match names the body actually publishes; this grants no storage availability.
/// A rebind withdraws the old association rather than following its copied value.
struct CurrentHomes {
    locals: Vec<(SymbolHandle, Option<u32>)>,
}

impl CurrentHomes {
    fn new(operations: &[CheckedUnitEffectOperationPlan]) -> Self {
        let mut locals = operations
            .iter()
            .filter_map(|operation| {
                let result = operation.structural_result_binding()?;
                result
                    .local
                    .is_named()
                    .then_some((result.local.symbol, None))
            })
            .collect::<Vec<_>>();
        locals.sort_unstable_by_key(|(symbol, _)| (symbol.arena_index(), symbol.generation()));
        locals.dedup_by_key(|(symbol, _)| *symbol);
        Self { locals }
    }

    fn local(&self, symbol: SymbolHandle) -> Option<usize> {
        self.locals
            .binary_search_by_key(
                &(symbol.arena_index(), symbol.generation()),
                |(symbol, _)| (symbol.arena_index(), symbol.generation()),
            )
            .ok()
    }

    fn source_local(&self, source: &SourcePlan) -> Option<usize> {
        let SourcePlan::StructuralLocal { symbol } = source else {
            return None;
        };
        self.local(*symbol)
    }

    fn binding(&self, local: Option<usize>) -> Option<u32> {
        self.locals.get(local?).and_then(|(_, binding)| *binding)
    }

    fn observe(&mut self, operation: &CheckedUnitEffectOperationPlan) {
        if let Some(result) = operation.structural_result_binding()
            && result.local.is_named()
            && let Some(local) = self.local(result.local.symbol)
        {
            self.locals[local].1 = Some(result.binding_ordinal);
        }
        if let CheckedUnitEffectOperationPlan::Store(store) = operation
            && matches!(store.destination.leaf, CheckedStoreLeaf::OwnedRebind { .. })
            && let CheckedStoreRoot::Local { symbol } = store.destination.root
            && let Some(local) = self.local(symbol)
        {
            self.locals[local].1 = None;
        }
    }

    fn same_storage(
        &self,
        left: &SourcePlan,
        left_local: Option<usize>,
        right: &SourcePlan,
        right_local: Option<usize>,
    ) -> bool {
        if left == right {
            return true;
        }
        match (left, right) {
            (
                SourcePlan::StructuralResult { binding_ordinal },
                SourcePlan::StructuralLocal { .. },
            ) => self.binding(right_local) == Some(*binding_ordinal),
            (
                SourcePlan::StructuralLocal { .. },
                SourcePlan::StructuralResult { binding_ordinal },
            ) => self.binding(left_local) == Some(*binding_ordinal),
            _ => false,
        }
    }
}

pub(crate) fn outstanding_actions<'a>(
    actions: &'a [CheckedNormalCleanupAction],
    operations: &[CheckedUnitEffectOperationPlan],
    parameters: &[CheckedUnitStructuralParameterPlan],
    control: &checked_trees::CheckedCleanupControlPlans,
) -> Vec<&'a CheckedNormalCleanupAction> {
    if actions.is_empty() {
        return Vec::new();
    }
    let mut homes = CurrentHomes::new(operations);
    let action_locals = actions
        .iter()
        .map(|action| homes.source_local(action.source()))
        .collect::<Vec<_>>();
    let mut outstanding = vec![true; actions.len()];
    for operation in operations {
        homes.observe(operation);
        let destination = operation
            .store()
            .and_then(|store| store.window_place(parameters));
        let destination_local = destination
            .as_ref()
            .and_then(|destination| homes.source_local(&destination.source));
        let release_local = match operation {
            CheckedUnitEffectOperationPlan::ReleaseReference { source, .. } => {
                homes.source_local(source)
            }
            _ => None,
        };
        for ((action, local), retained) in actions.iter().zip(&action_locals).zip(&mut outstanding)
        {
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
                CheckedUnitEffectOperationPlan::Store(_) => {
                    if destination.as_ref().is_some_and(|destination| {
                        homes.same_storage(
                            action.source(),
                            *local,
                            &destination.source,
                            destination_local,
                        ) && action.path().starts_with(&destination.path)
                    }) {
                        *retained = true;
                    }
                }
                CheckedUnitEffectOperationPlan::ReleaseReference {
                    source,
                    path,
                    case_conditions,
                    select_evaluated_storage,
                    defer_to_exit: false,
                    ..
                } if path.is_empty()
                    && case_conditions.is_empty()
                    && !select_evaluated_storage
                    && homes.same_storage(action.source(), *local, source, release_local)
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
