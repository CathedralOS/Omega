//! One observation cut for predicates and content equations on completed values.
//!
//! The prefix owner supplies only dominating premises. The normal-return owner
//! then captures the selected expression in authored operand order, independently
//! of later siblings or statements. Consumers judge that captured value; they
//! cannot import a machine's eventual guarantees as premises at an earlier cut.

use super::citations::CitationFacts;
use super::structural_case_arms::structural_arm_judge;
use super::structural_judgment::{StructuralJudge, StructuralTerm};
use super::structural_prefix::{
    StructuralPrefixSite, StructuralPrefixVisitor, observe_runtime_state_prefix,
};
use typed_trees::expression::ExpressionHandle;
use typed_trees::types::TypeReferenceHandle;
use typed_trees::{machine::Machine, state::State};

/// Visit every reachable completion. None means the prefix was unavailable;
/// false means that the callbacks do not cover all reachable completions.
/// Partial observations may refute a predicate, but cannot establish one.
pub(super) fn observe_completed_values(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
    observe: impl FnMut(&StructuralJudge<'_>, StructuralTerm),
) -> Option<bool> {
    let mut visitor = CompletedObservation {
        site: StructuralPrefixSite {
            state: state.symbol,
            statement_index,
            in_transition_target: super::structural_prefix::expression_is_transition_target(
                program,
                state,
                statement_index,
                expression,
            ),
        },
        expression,
        reference,
        observe,
        complete: true,
        observed: false,
    };
    observe_runtime_state_prefix(program, machine, state, &mut visitor)?;
    Some(visitor.complete && visitor.observed)
}

struct CompletedObservation<Observer> {
    site: StructuralPrefixSite,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
    observe: Observer,
    complete: bool,
    observed: bool,
}

impl<Observer> StructuralPrefixVisitor for CompletedObservation<Observer>
where
    Observer: FnMut(&StructuralJudge<'_>, StructuralTerm),
{
    fn site(&self) -> StructuralPrefixSite {
        self.site
    }

    fn visit<'program>(
        &mut self,
        program: &'program dyn crate::CheckingInput,
        machine: &Machine,
        judge: &StructuralJudge<'program>,
        environment: &[(String, StructuralTerm)],
        hypotheses: &[(String, StructuralTerm)],
        equations: &[(StructuralTerm, StructuralTerm)],
        guarantees: &CitationFacts,
    ) {
        let mut judge = structural_arm_judge(program, judge, hypotheses, equations);
        if judge.hypotheses_contradictory {
            return;
        }
        guarantees.intake(&mut judge);
        let Some(state) = program.state_in_machine(machine, self.site.state) else {
            self.complete = false;
            return;
        };
        let Some(outcomes) = judge.completed_statement_paths(
            self.site.statement_index,
            self.expression,
            environment,
            (machine, state, self.reference),
        ) else {
            self.complete = false;
            return;
        };
        self.observed |= !outcomes.is_empty();
        for outcome in outcomes {
            (self.observe)(
                &outcome.judge,
                outcome.judge.completed_value_identity(outcome.value),
            );
        }
    }
}
