//! Recursive calls transport a finite set of captured return candidates.
//! Empty construction seeds are private: publication also needs complete
//! equations and independent reference coverage or proved non-returning control.

use super::{Captures, CheckedReturnedReferencePlan, SymbolHandle, completions::ReturnCoverage};
use checked_trees::CheckFacts;
use std::cell::{Cell, RefCell};

#[derive(Default)]
pub(super) struct EquationObservations {
    dependencies: RefCell<Vec<SymbolHandle>>,
    completions: RefCell<Vec<ReturnCoverage>>,
    nonreturning: Cell<bool>,
}

impl EquationObservations {
    pub(super) fn require(&self, state: SymbolHandle) {
        let mut dependencies = self.dependencies.borrow_mut();
        if !dependencies.contains(&state) {
            dependencies.push(state);
        }
    }

    pub(super) fn completed(&self, completion: ReturnCoverage) {
        for dependency in completion.dependencies() {
            self.require(dependency);
        }
        self.completions.borrow_mut().push(completion);
    }

    pub(super) fn nonreturning(&self) {
        self.nonreturning.set(true);
    }
}

struct ReturnEquation {
    state: SymbolHandle,
    sources: Vec<CheckedReturnedReferencePlan>,
    dependencies: Vec<SymbolHandle>,
    callers: Vec<SymbolHandle>,
    completions: Vec<ReturnCoverage>,
    nonreturning: bool,
    candidates: Option<Vec<CheckedReturnedReferencePlan>>,
    productive: bool,
}

#[derive(Default)]
pub(super) struct ReturnRelations {
    rows: Vec<ReturnEquation>,
}

fn key(state: SymbolHandle) -> (u32, u32) {
    (state.arena_index(), state.generation())
}

impl ReturnRelations {
    pub(super) fn productive(&self, state: SymbolHandle) -> bool {
        self.position(state)
            .is_some_and(|position| self.rows[position].productive)
    }

    fn position(&self, state: SymbolHandle) -> Option<usize> {
        self.rows
            .binary_search_by_key(&key(state), |row| key(row.state))
            .ok()
    }

    /// Only the equation builder reads an unfinished approximation. Every
    /// missing dependency is queued, and no answer escapes before closure.
    pub(super) fn approximation(
        &self,
        state: SymbolHandle,
        observed: &EquationObservations,
    ) -> Vec<CheckedReturnedReferencePlan> {
        observed.require(state);
        match self.position(state) {
            Some(position) => self.rows[position].sources.clone(),
            None => Vec::new(),
        }
    }

    fn edges(&self) -> Option<Vec<Vec<usize>>> {
        self.rows
            .iter()
            .map(|row| {
                row.dependencies
                    .iter()
                    .map(|state| self.position(*state))
                    .collect()
            })
            .collect()
    }

    fn completion_established(&self, position: usize) -> bool {
        self.rows[position].completions.iter().any(|completion| {
            completion.established(|state| {
                self.position(state)
                    .is_some_and(|index| self.rows[index].productive)
            })
        })
    }
}

/// One immutable-facts question owns all construction scratch. Dependency
/// discovery follows captured-return queries and their actual value inputs.
/// Unrelated states and discarded sibling arms are outside this question.
pub(super) fn complete<T>(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    question: impl Fn(&Captures<'_>) -> Option<T>,
) -> Option<T> {
    let calls = super::completion_calls::CompletionCalls::new(&facts.flow.control);
    let mut relations = ReturnRelations::default();
    let mut answer = None;
    let mut question_dependencies = Vec::new();
    let mut question_coverage = Vec::new();
    let mut question_nonreturning = false;
    let mut question_dirty = true;
    let mut pending = Vec::new();
    loop {
        let mut evaluated = Vec::new();
        let mut required = Vec::new();
        for state in std::mem::take(&mut pending) {
            let position = relations.position(state)?;
            let row = &relations.rows[position];
            let observed = EquationObservations::default();
            let captures = Captures {
                program,
                facts,
                calls: &calls,
                relations: Some(&relations),
                observed: Some(&observed),
            };
            let sources = captures
                .returned(program.state_by_symbol(row.state)?, &mut Vec::new())
                .or_else(|| refusal("captured return equation is unavailable", Some(row.state)))?;
            // Equations only add possible origins. Losing a previously proved
            // alternative is unavailable data, never permission to narrow it.
            if row.sources.iter().any(|source| !sources.contains(source)) {
                return refusal(
                    "captured return alternatives are not monotone",
                    Some(row.state),
                );
            }
            let dependencies = observed.dependencies.into_inner();
            required.extend(dependencies.iter().copied());
            evaluated.push(EvaluatedEquation {
                position,
                sources,
                dependencies,
                completions: observed.completions.into_inner(),
                nonreturning: observed.nonreturning.get(),
            });
        }
        if question_dirty {
            question_dirty = false;
            let observed = EquationObservations::default();
            answer = Some(
                question(&Captures {
                    program,
                    facts,
                    calls: &calls,
                    relations: Some(&relations),
                    observed: Some(&observed),
                })
                .or_else(|| refusal("completed return question is unavailable", None))?,
            );
            // Direct root-return questions need their own latest coverage;
            // they need not have registered a state equation for that root.
            question_coverage = observed.completions.into_inner();
            question_nonreturning = observed.nonreturning.get();
            for dependency in observed.dependencies.into_inner() {
                required.push(dependency);
                if !question_dependencies.contains(&dependency) {
                    question_dependencies.push(dependency);
                    question_dirty = true;
                }
            }
        }
        let mut affected = Vec::new();
        let mut new_edges = Vec::new();
        let mut evaluated_states = Vec::with_capacity(evaluated.len());
        for evaluated in evaluated {
            let row = &mut relations.rows[evaluated.position];
            evaluated_states.push(row.state);
            let mut changed = false;
            // Alternative order is not execution order. Keep first discovery
            // order so a recursive parameter permutation cannot oscillate.
            for source in evaluated.sources {
                if !row.sources.contains(&source) {
                    row.sources.push(source);
                    changed = true;
                }
            }
            if evaluated.nonreturning && !row.nonreturning {
                row.nonreturning = true;
                changed = true;
            }
            for completion in evaluated.completions {
                if !row.completions.contains(&completion) {
                    row.completions.push(completion);
                    changed = true;
                }
            }
            for dependency in evaluated.dependencies {
                if !row.dependencies.contains(&dependency) {
                    row.dependencies.push(dependency);
                    new_edges.push((dependency, row.state));
                    changed = true;
                }
            }
            if changed {
                affected.push(row.state);
            }
        }
        required.sort_by_key(|state| key(*state));
        required.dedup();
        let new_states = required
            .into_iter()
            .filter(|state| relations.position(*state).is_none())
            .collect::<Vec<_>>();
        let membership_changed = !new_states.is_empty();
        for state in new_states {
            pending.push(state);
            relations.rows.push(ReturnEquation {
                state,
                sources: Vec::new(),
                dependencies: Vec::new(),
                callers: Vec::new(),
                completions: Vec::new(),
                nonreturning: false,
                candidates: None,
                productive: false,
            });
            affected.push(state);
        }
        if membership_changed {
            relations.rows.sort_by_key(|row| key(row.state));
        }
        let graph_changed = !new_edges.is_empty();
        for (dependency, caller) in new_edges {
            let position = relations.position(dependency)?;
            let callers = &mut relations.rows[position].callers;
            if !callers.contains(&caller) {
                callers.push(caller);
            }
        }
        if graph_changed {
            let edges = relations.edges()?;
            // Paths around a recursive component may not grow by unrolling it.
            // Its outputs must belong to the finite actual returned-loan frontier.
            // Acyclic transport keeps the existing composed projection behavior.
            for component in
                crate::machine_calls::graph_components::strongly_connected_components(&edges)
            {
                let cyclic = component.len() > 1
                    || component
                        .first()
                        .is_some_and(|position| edges[*position].contains(position));
                if !cyclic {
                    continue;
                }
                for position in component {
                    let row = &mut relations.rows[position];
                    if row.candidates.is_none() {
                        row.candidates =
                            Some(candidates(program, facts, row.state).or_else(|| {
                                refusal(
                                    "recursive returned-loan frontier is unavailable",
                                    Some(row.state),
                                )
                            })?);
                    }
                    let candidates = row.candidates.as_ref()?;
                    if let Some(source) = row
                        .sources
                        .iter()
                        .find(|source| !candidates.contains(source))
                    {
                        if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                            eprintln!(
                                "CHECKSITE captured-return-equation state={:?} refusal=recursive-source-outside-capture source={source:?} candidates={candidates:?}",
                                row.state,
                            );
                        }
                        return None;
                    }
                }
            }
        }
        // Existing cyclic components need only check changed source rosters.
        // Their candidate frontiers were prepared when the cycle was found.
        for state in &affected {
            let row = &relations.rows[relations.position(*state)?];
            if let Some(candidates) = &row.candidates
                && row
                    .sources
                    .iter()
                    .any(|source| !candidates.contains(source))
            {
                return refusal(
                    "recursive source is outside its captured frontier",
                    Some(*state),
                );
            }
        }
        // Completion can become established without adding an origin. Only
        // rows whose equations ran need checking; dependent rows are dirtied
        // below when a child's outcome changes.
        let newly_productive = evaluated_states
            .into_iter()
            .filter_map(|state| {
                let position = relations.position(state)?;
                (!relations.rows[position].productive && relations.completion_established(position))
                    .then_some(position)
            })
            .collect::<Vec<_>>();
        for position in newly_productive {
            relations.rows[position].productive = true;
            affected.push(relations.rows[position].state);
        }
        affected.sort_by_key(|state| key(*state));
        affected.dedup();
        // Only dependent questions run again. A newly established empty
        // result is a change too, even though it added no origin candidate.
        for state in &affected {
            let row = &relations.rows[relations.position(*state)?];
            pending.extend(row.callers.iter().copied());
        }
        pending.sort_by_key(|state| key(*state));
        pending.dedup();
        if question_dependencies.iter().any(|state| {
            affected
                .binary_search_by_key(&key(*state), |state| key(*state))
                .is_ok()
        }) {
            question_dirty = true;
        }
        if question_dirty || !pending.is_empty() {
            continue;
        }
        if let Some(row) = relations
            .rows
            .iter()
            .find(|row| !row.productive && !row.nonreturning)
        {
            return refusal(
                "recursive return has no independently covered value boundary",
                Some(row.state),
            );
        }
        if !question_nonreturning
            && !question_coverage.is_empty()
            && !question_coverage.iter().any(|coverage| {
                coverage.established(|state| {
                    relations
                        .position(state)
                        .is_some_and(|position| relations.rows[position].productive)
                })
            })
        {
            return refusal(
                "root return has no independently covered value boundary",
                None,
            );
        }
        return answer;
    }
}

struct EvaluatedEquation {
    position: usize,
    sources: Vec<CheckedReturnedReferencePlan>,
    dependencies: Vec<SymbolHandle>,
    completions: Vec<ReturnCoverage>,
    nonreturning: bool,
}

fn refusal<T>(reason: &'static str, state: Option<SymbolHandle>) -> Option<T> {
    if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
        eprintln!("CHECKSITE captured-return-equation state={state:?} refusal={reason}");
    }
    None
}

/// Candidate identity comes from finite checked captures, not a recursive
/// expansion of caller paths. This is an upper bound only: each equation
/// must still establish its exact sources through its completed operands.
fn candidates(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    state: SymbolHandle,
) -> Option<Vec<CheckedReturnedReferencePlan>> {
    let state = program.state_by_symbol(state)?;
    if !crate::normal_completion_possible(program, state)? {
        return Some(Vec::new());
    }
    let calls = super::completion_calls::CompletionCalls::new(&facts.flow.control);
    let captures = Captures {
        program,
        facts,
        calls: &calls,
        relations: None,
        observed: None,
    };
    let borrowed = facts.borrow.unique_state_borrow_by_symbol(state.symbol)?;
    let loans = facts.borrow.returned_loans.span(borrowed.returned_loans)?;
    let mut output = Vec::new();
    for leaf in captures.reference_leaves(state)? {
        let owner_path = crate::corresponding_loan_owner_path(
            program,
            state.return_type,
            &super::loan_owner_path(program, state.return_type, &leaf.path)?,
        )?;
        for loan in loans.iter().filter(|loan| {
            loan.lineage != checked_trees::BorrowLoanLineage::LifetimeDependency
                && captures.returned_loan_owner_path(state, loan).as_deref()
                    == Some(owner_path.as_slice())
        }) {
            if leaf.source.access.borrow_access_kind()? != loan.kind {
                return None;
            }
            for source in captures.return_origin(
                state,
                loan,
                &leaf,
                facts.borrow.access_segments.span(loan.segments)?,
                loans,
            )? {
                let candidate = CheckedReturnedReferencePlan {
                    path: leaf.path.clone(),
                    source,
                };
                if !output.contains(&candidate) {
                    output.push(candidate);
                    if output.len() > super::super::MAX_REFERENCE_ORIGIN_LEAVES {
                        return None;
                    }
                }
            }
        }
    }
    Some(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use checked_trees::{FlowOperandCompletion, FlowOperandFact};

    #[test]
    fn captured_return_question_checks_direct_root_before_publication() {
        let program = typed_trees::TypedTrees::default();
        let facts = CheckFacts::default();
        let selected_without_value = FlowOperandFact {
            completion: FlowOperandCompletion::Selected(arena::HandleSpan::empty()),
            ..FlowOperandFact::default()
        };
        let missing =
            ReturnCoverage::captured(&facts.flow.control, &selected_without_value, |_| {
                Some(Vec::new())
            })
            .expect("the missing alternative is represented, not silently removed");
        let result = complete(&program, &facts, |captures| {
            captures.observed?.completed(missing.clone());
            Some(7)
        });
        // No state row or dependency was registered. The root's own absent
        // selected value must nevertheless prevent its answer escaping.
        assert_eq!(result, None);

        let empty = FlowOperandFact {
            completion: FlowOperandCompletion::Constructed(arena::HandleSpan::empty()),
            ..FlowOperandFact::default()
        };
        let covered = ReturnCoverage::captured(&facts.flow.control, &empty, |_| Some(Vec::new()))
            .expect("the formed empty constructor has complete coverage");
        assert_eq!(
            complete(&program, &facts, |captures| {
                captures.observed?.completed(covered.clone());
                Some(7)
            }),
            Some(7)
        );
        // A query that reads only a public/registered relation supplies no
        // separate root-return coverage and retains its existing behavior.
        assert_eq!(complete(&program, &facts, |_| Some(7)), Some(7));
    }

    #[test]
    fn captured_return_question_preserves_proved_nonreturning_root() {
        let program = typed_trees::TypedTrees::default();
        let facts = CheckFacts::default();
        assert_eq!(
            complete(&program, &facts, |captures| {
                // Production records this only after normal_completion_possible
                // has proved the root cannot return normally.
                captures.observed?.nonreturning();
                Some(Vec::<CheckedReturnedReferencePlan>::new())
            }),
            Some(Vec::new())
        );
    }
}
