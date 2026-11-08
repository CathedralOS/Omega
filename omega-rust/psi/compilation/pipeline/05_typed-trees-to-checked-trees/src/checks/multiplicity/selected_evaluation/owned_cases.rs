//! Keep branch cleanup tied to the source value and storage version selected.

use super::place_bindings::SelectionSite;
use crate::checks::multiplicity::cleanup_dispositions::RemainingCustody;
use crate::checks::ownership_occurrences::{EvaluationBranch, StatementOccurrence};
use arena::HandleSpan;
use checked_trees::{FlowControlFacts, FlowOwnedCaseSelection};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

type ArmKey = (u32, u32, u32, u32);

struct PreparedCases {
    paths: Vec<(symbols::SymbolHandle, validation::CanonicalPlace)>,
    by_variant: BTreeMap<(u32, u32), usize>,
}

struct PreparedOwner {
    root: facts::PlaceRoot,
    path: HandleSpan<facts::PlaceSegment>,
    cases: Arc<PreparedCases>,
}

struct Selection {
    row: FlowOwnedCaseSelection,
    cases: Arc<PreparedCases>,
    selected: usize,
}

pub(super) struct OwnedCaseSelections {
    arms: BTreeMap<ArmKey, Option<Selection>>,
}

impl OwnedCaseSelections {
    pub(super) fn new(
        program: &dyn validation::CheckingInput,
        control: &FlowControlFacts,
        site: SelectionSite,
        occurrences: &[StatementOccurrence],
    ) -> Self {
        let mut arms = BTreeMap::new();
        let Some(state) = control
            .unique_state_by_symbol(site.state)
            .filter(|state| state.machine_symbol == site.machine)
        else {
            return Self { arms };
        };
        let statements = state.statements;
        if control.statements.span(statements).is_none() {
            return Self { arms };
        }
        let mut visited = BTreeSet::new();
        for branch in occurrences.iter().flat_map(|occurrence| &occurrence.route) {
            let expression = branch.expression();
            if !expression.is_valid()
                || !branch.source_arm().is_valid()
                || !visited.insert((expression.arena_index(), expression.generation()))
            {
                continue;
            }
            control.with_owned_case_selections(expression, |rows| {
                let mut owner: Option<PreparedOwner> = None;
                for &handle in rows {
                    let row = *control.owned_case_selections.get(handle);
                    if row.machine_symbol != site.machine
                        || row.state_symbol != site.state
                        || row.statement_index != site.statement
                        || row.evaluation != site.evaluation
                    {
                        continue;
                    }
                    let Some(offset) = row
                        .statement
                        .arena_index()
                        .checked_sub(statements.start().arena_index())
                    else {
                        continue;
                    };
                    if statements.handles().nth(offset as usize) != Some(row.statement) {
                        continue;
                    }
                    let statement = control.statements.get(row.statement);
                    if statement.statement_index != site.statement {
                        continue;
                    }
                    if control
                        .owned_case_selections
                        .span(statement.owned_case_selections)
                        .is_none()
                    {
                        continue;
                    }
                    let Some(offset) = handle
                        .arena_index()
                        .checked_sub(statement.owned_case_selections.start().arena_index())
                    else {
                        continue;
                    };
                    if statement
                        .owned_case_selections
                        .handles()
                        .nth(offset as usize)
                        != Some(handle)
                    {
                        continue;
                    }
                    let key = (
                        expression.arena_index(),
                        expression.generation(),
                        row.source_arm.arena_index(),
                        row.source_arm.generation(),
                    );
                    let selected = (|| {
                        if !control.operands.is_valid(row.subject)
                            || !control.storage_snapshots.is_valid(row.captured_storage)
                            || !control.storage_snapshots.is_valid(row.selected_storage)
                        {
                            return None;
                        }
                        let subject = control.operands.get(row.subject);
                        if subject.state_symbol != site.state
                            || subject.statement_index != site.statement
                            || subject.evaluation != site.evaluation
                        {
                            return None;
                        }
                        let cases = match &owner {
                            Some(prepared)
                                if prepared.root == row.root && prepared.path == row.path =>
                            {
                                Arc::clone(&prepared.cases)
                            }
                            _ => {
                                let path = control.operand_referent_segments.span(row.path)?;
                                let place = validation::CanonicalPlace {
                                    root: row.root,
                                    segments: path.to_vec(),
                                };
                                let paths = RemainingCustody::owned_case_paths(
                                    program,
                                    site.state,
                                    site.statement,
                                    &place,
                                )?;
                                let by_variant = paths
                                    .iter()
                                    .enumerate()
                                    .map(|(ordinal, (variant, _))| {
                                        ((variant.arena_index(), variant.generation()), ordinal)
                                    })
                                    .collect();
                                let cases = Arc::new(PreparedCases { paths, by_variant });
                                owner = Some(PreparedOwner {
                                    root: row.root,
                                    path: row.path,
                                    cases: Arc::clone(&cases),
                                });
                                cases
                            }
                        };
                        let selected = *cases
                            .by_variant
                            .get(&(row.variant.arena_index(), row.variant.generation()))?;
                        Some(Selection {
                            row,
                            cases,
                            selected,
                        })
                    })();
                    arms.entry(key)
                        .and_modify(|previous: &mut Option<Selection>| {
                            let identical = previous.as_ref().zip(selected.as_ref()).is_some_and(
                                |(left, right)| {
                                    left.row == right.row
                                        && left.selected == right.selected
                                        && Arc::ptr_eq(&left.cases, &right.cases)
                                },
                            );
                            if !identical {
                                *previous = None;
                            }
                        })
                        .or_insert(selected);
                }
            });
        }
        Self { arms }
    }

    pub(super) fn observe(&self, branch: EvaluationBranch, remaining: &mut RemainingCustody) {
        let expression = branch.expression();
        let arm = branch.source_arm();
        let key = (
            expression.arena_index(),
            expression.generation(),
            arm.arena_index(),
            arm.generation(),
        );
        let Some(Some(selection)) = self.arms.get(&key) else {
            return;
        };
        if branch.selected() {
            for (ordinal, (_, path)) in selection.cases.paths.iter().enumerate() {
                if ordinal != selection.selected {
                    remaining.exclude_absent_case(path);
                }
            }
        } else if selection.row.miss_excludes_case {
            // Only a total case test can show that this payload is absent.
            // A failed guard still leaves that case's cleanup obligations live.
            remaining.exclude_absent_case(&selection.cases.paths[selection.selected].1);
        }
    }
}
