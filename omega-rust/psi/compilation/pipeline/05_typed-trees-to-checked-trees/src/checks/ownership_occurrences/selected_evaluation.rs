//! Query-owned selection scratch. One entry and one completed sibling per
//! pending decision; each join runs once, without Cartesian path expansion.

use super::EvaluationBranch;
use diagnostics::Diagnostic;

struct Decision<Custody> {
    branch: EvaluationBranch,
    entry: Custody,
    sibling: Option<Custody>,
}

pub(in crate::checks) struct SelectedEvaluation<Custody> {
    decisions: Vec<Decision<Custody>>,
    completed: Vec<EvaluationBranch>,
}

impl<Custody: Clone> SelectedEvaluation<Custody> {
    pub(in crate::checks) fn new() -> Self {
        Self {
            decisions: Vec::new(),
            completed: Vec::new(),
        }
    }

    pub(in crate::checks) fn enter(
        &mut self,
        route: &[EvaluationBranch],
        current: &mut Custody,
        leave: impl FnMut(EvaluationBranch, &mut Custody) -> Result<(), Diagnostic>,
        join: impl FnMut(EvaluationBranch, &mut Custody, &Custody) -> Result<(), Diagnostic>,
    ) -> Result<(), Diagnostic> {
        self.enter_with(route, current, |_, _| Ok(()), leave, join)
    }

    pub(in crate::checks) fn enter_with(
        &mut self,
        route: &[EvaluationBranch],
        current: &mut Custody,
        mut begin: impl FnMut(EvaluationBranch, &mut Custody) -> Result<(), Diagnostic>,
        mut leave: impl FnMut(EvaluationBranch, &mut Custody) -> Result<(), Diagnostic>,
        mut join: impl FnMut(EvaluationBranch, &mut Custody, &Custody) -> Result<(), Diagnostic>,
    ) -> Result<(), Diagnostic> {
        let common = self
            .decisions
            .iter()
            .zip(route)
            .take_while(|(decision, branch)| decision.branch == **branch)
            .count();
        let sibling = self
            .decisions
            .get(common)
            .zip(route.get(common))
            .is_some_and(|(decision, branch)| decision.branch.same_decision(*branch));
        let retained = common + usize::from(sibling);
        while self.decisions.len() > retained {
            if let Some(decision) = self.decisions.last() {
                // Inner scopes finish before joining into their parent, so
                // an outer death observes every inner alternative.
                leave(decision.branch, current)?;
            }
            if let Some(decision) = self.decisions.pop() {
                if let Some(sibling) = decision.sibling {
                    join(decision.branch, current, &sibling)?;
                } else {
                    // A scalar fallback can have no permission events. Its
                    // remaining storage still enters the opposite decision
                    // before meeting the branch that moved a payload.
                    let mut sibling = decision.entry;
                    begin(decision.branch.opposite(), &mut sibling)?;
                    join(decision.branch, current, &sibling)?;
                }
                self.completed.push(decision.branch);
            }
        }
        if sibling {
            let decision = &mut self.decisions[common];
            if decision.sibling.is_some() {
                return Err(Diagnostic::error(
                    "permission occurrence revisits an already evaluated expression branch",
                ));
            }
            leave(decision.branch, current)?;
            decision.sibling = Some(std::mem::replace(current, decision.entry.clone()));
            decision.branch = route[common];
            begin(decision.branch, current)?;
        }
        for branch in &route[retained..] {
            if self
                .completed
                .iter()
                .any(|completed| completed.same_decision(*branch))
            {
                return Err(Diagnostic::error(
                    "permission occurrence follows the join of its expression branch",
                ));
            }
            let entry = current.clone();
            begin(*branch, current)?;
            self.decisions.push(Decision {
                branch: *branch,
                entry,
                sibling: None,
            });
        }
        Ok(())
    }
}
