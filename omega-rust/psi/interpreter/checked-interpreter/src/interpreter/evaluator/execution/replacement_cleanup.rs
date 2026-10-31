//! Retain old-value receivers before the RHS; execute their selected actions
//! after materialization and before writing the captured destination. The
//! producer carries both the displaced home and its path below that destination.
//! Reconstructing a move/store neighborhood here would lose whole-array leaves
//! and force selector replay. Cleanup target identity still comes exclusively
//! from checked death selection, never from the runtime value's type.

use crate::interpreter::evaluator::{Cell, EvalResult, Evaluator, Frame, Halt};
use checked_trees::CheckedUnitStructuralPathSegment;
use checked_trees::{
    CheckedNormalCleanupAction, CheckedUnitCleanupBoundary, CheckedUnitEffectOperationPlan,
    CheckedUnitNominalAffineCleanupPlan,
};
use symbols::SymbolHandle;

pub(super) struct ReplacementCleanup<'program> {
    action: &'program CheckedUnitNominalAffineCleanupPlan,
    receiver: Cell,
    path: &'program [CheckedUnitStructuralPathSegment],
}

/// Physical assignment storage, captured before evaluating the RHS. A window
/// keeps its backing and extent rather than allocating a synthetic array owner.
pub(super) enum ReplacementStorage {
    Cell(Cell),
    ArrayWindow {
        carrier: Cell,
        start: usize,
        end: usize,
    },
}

impl<'program> Evaluator<'program> {
    pub(super) fn cleanup_operations(
        &self,
        machine: SymbolHandle,
        state: SymbolHandle,
    ) -> &'program [CheckedUnitEffectOperationPlan] {
        let Some(checked) = self.checked else {
            return &[];
        };
        let plans = &checked.facts.flow.terminal_unit_effects;
        if let Some(plan) = plans
            .for_machine(machine)
            .filter(|plan| plan.state == state)
        {
            return &plan.operations;
        }
        plans
            .composed_for_machine(machine)
            .and_then(|plan| plan.state(state))
            .map_or(&[], |state| state.operations.as_slice())
    }

    pub(super) fn capture_replacement_cleanup(
        &self,
        statement_index: usize,
        destination: Option<ReplacementStorage>,
        frame: &Frame,
    ) -> EvalResult<Vec<ReplacementCleanup<'program>>> {
        let mut prepared = Vec::new();
        for operation in self.cleanup_operations(frame.machine_symbol, frame.state_symbol) {
            let CheckedUnitEffectOperationPlan::ContinuationCleanup {
                boundary:
                    CheckedUnitCleanupBoundary::Replacement {
                        statement_index: selected_statement,
                        old_value,
                        receiver_path,
                        ..
                    },
                cleanup_actions,
                selected_regions,
            } = operation
            else {
                continue;
            };
            if *selected_statement as usize != statement_index {
                continue;
            }
            if !selected_regions.is_empty() {
                return Err(Halt::Unsupported(
                    "unimplemented: checked-tree interpreter selected owner cleanup".into(),
                ));
            }
            for action in cleanup_actions {
                // Discard and home retirement have no nominal runtime body.
                // The checked interpreter does not invent a hook for either.
                let CheckedNormalCleanupAction::InvokeNominal(action) = action else {
                    continue;
                };
                let path = action
                    .path
                    .strip_prefix(old_value.path.as_slice())
                    .filter(|_| action.source == old_value.source)
                    .ok_or_else(|| {
                        Halt::Unsupported(
                            "replacement cleanup lost its selected old-value home".into(),
                        )
                    })?;
                let storage = destination.as_ref().ok_or_else(|| {
                    Halt::Unsupported("replacement cleanup has no retained record storage".into())
                })?;
                let (receiver, path) = match storage {
                    ReplacementStorage::Cell(receiver) => {
                        let receiver = self.normal_cleanup_cell(receiver.clone(), receiver_path)?;
                        (receiver, path)
                    }
                    ReplacementStorage::ArrayWindow {
                        carrier,
                        start,
                        end,
                    } => {
                        // Relative leaf coordinates are local to the captured
                        // window, not the containing array's zero origin.
                        let (first, tail, remaining) =
                            if let Some((first, tail)) = receiver_path.split_first() {
                                (first, tail, path)
                            } else {
                                let (first, tail) = path.split_first().ok_or_else(|| {
                                    Halt::Unsupported(
                                        "array window cleanup has no element receiver".into(),
                                    )
                                })?;
                                (first, &[][..], tail)
                            };
                        let CheckedUnitStructuralPathSegment::FixedIndex(index) = first else {
                            return Err(Halt::Unsupported(
                                "array window cleanup lost its relative element".into(),
                            ));
                        };
                        let index = usize::try_from(*index)
                            .ok()
                            .filter(|index| *index < end - start)
                            .ok_or_else(|| {
                                Halt::Trap("array window cleanup element is out of bounds".into())
                            })?;
                        let receiver = self.element_cell(carrier, start + index)?;
                        let receiver = self.normal_cleanup_cell(receiver, tail)?;
                        (receiver, remaining)
                    }
                };
                prepared.push(ReplacementCleanup {
                    action,
                    receiver,
                    path,
                });
            }
        }
        Ok(prepared)
    }

    pub(super) fn execute_replacement_cleanup(
        &mut self,
        prepared: Vec<ReplacementCleanup<'program>>,
    ) -> EvalResult<()> {
        for cleanup in prepared {
            // Capture the dying owner before the RHS, but select children in
            // action order: a parent's hook may change the child's storage.
            // This is a projection of retained old storage, not selector replay.
            let receiver = self.normal_cleanup_cell(cleanup.receiver, cleanup.path)?;
            self.execute_nominal_cleanup_receiver(cleanup.action, receiver)?;
        }
        Ok(())
    }
}
