//! Advisory warnings for authored branches that can never run.
//!
//! A branch such as `count < 5` under `requires count >= 10` contradicts the
//! facts in force before it. The language allows obligations on that branch
//! to close by contradiction elimination; this module only tells the author.
//! Proof acceptance never depends on it: the warning is computed from the same
//! narrowed facts the checker uses, after the fact, and every obligation inside
//! the branch is still checked on its own. Reporting runs once per statement
//! from ordinary validation, never from a speculative analysis whose caller
//! reads any diagnostic as a failed proof.
//!
//! Only the branches a statement selects directly are judged: transition arms,
//! and match arms in a statement's own value or in an arm of such a match. A
//! match nested inside another expression may follow calls that change the
//! facts, so it stays unreported rather than judged against stale facts.

use crate::proof_contracts::arithmetic_domains::{
    self, ValueEnvironment, contradicting_premises, dispatch_arm_environments,
};
use diagnostics::Diagnostic;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, MatchPattern};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

/// Report every directly selected branch of `statement` that the facts in
/// `environment`, the facts in force before the statement, rule out.
pub(super) fn warn(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement: &StatementNode,
    environment: &ValueEnvironment,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let report = Report {
        program,
        machine,
        state,
    };
    match statement {
        StatementNode::Expression(expression) => {
            report.matches(*expression, environment, diagnostics);
        }
        StatementNode::LocalData(local) => {
            report.matches(local.initial_value, environment, diagnostics);
        }
        StatementNode::Transition(transition) => {
            report.transition(transition, environment, diagnostics);
        }
        _ => {}
    }
}

struct Report<'a> {
    program: &'a dyn crate::CheckingInput,
    machine: &'a Machine,
    state: &'a State,
}

impl Report<'_> {
    fn transition(
        &self,
        transition: &typed_trees::statement::TableTransition,
        environment: &ValueEnvironment,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let arms = [
            (transition.target, true, "`true` arm"),
            (transition.continuation, false, "fall-back arm"),
        ];
        for (target, positive, arm) in arms {
            if !target.is_valid() {
                continue;
            }
            let TransitionGuardNode::When(guard) = transition.guard else {
                // An unconditional transition selects its only arm.
                self.target(target, environment, diagnostics);
                continue;
            };
            let narrowed = if positive {
                arithmetic_domains::guard_narrowed_environment(
                    self.program,
                    self.machine,
                    Some(self.state),
                    &transition.guard,
                    environment,
                )
            } else {
                arithmetic_domains::fall_through_narrowed_environment(
                    self.program,
                    self.machine,
                    Some(self.state),
                    &transition.guard,
                    environment,
                )
            };
            match contradicting_premises(self.program, self.machine, environment, &narrowed) {
                Some(premises) => diagnostics.push(self.warning(
                    &format!("the {arm} of a transition"),
                    "guard",
                    &premises,
                    guard,
                )),
                None => self.target(target, &narrowed, diagnostics),
            }
        }
    }

    fn target(
        &self,
        target: typed_trees::statement::TransitionTargetHandle,
        environment: &ValueEnvironment,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if let TransitionTargetNode::Value(value) =
            self.program.statement_table.transition_target(target)
        {
            self.matches(*value, environment, diagnostics);
        }
    }

    fn matches(
        &self,
        expression: ExpressionHandle,
        environment: &ValueEnvironment,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let ExpressionNode::Match(dispatch) = self.program.expression_table.expression(expression)
        else {
            return;
        };
        let arms = self.program.expression_table.match_arms(dispatch.arms);
        let environments = dispatch_arm_environments(
            self.program,
            self.machine,
            Some(self.state),
            expression,
            dispatch,
            environment,
        );
        for (position, (arm, selected)) in arms.iter().zip(&environments).enumerate() {
            match contradicting_premises(self.program, self.machine, environment, selected) {
                Some(premises) => {
                    let name = match arm.pattern {
                        MatchPattern::Value(pattern) => {
                            match self.program.expression_table.boolean_node(pattern) {
                                Some(value) => format!("`{value}` arm"),
                                None => format!("arm {}", position + 1),
                            }
                        }
                        _ => format!("arm {}", position + 1),
                    };
                    diagnostics.push(self.warning(
                        &format!("the {name} of a match"),
                        "pattern",
                        &premises,
                        arm.value,
                    ));
                }
                None => self.matches(arm.value, selected, diagnostics),
            }
        }
    }

    fn warning(
        &self,
        branch: &str,
        selector: &str,
        premises: &[String],
        location: ExpressionHandle,
    ) -> Diagnostic {
        let reason = if premises.is_empty() {
            format!("its {selector} can never select it")
        } else {
            format!(
                "its {selector} contradicts the facts in force here: {}",
                premises.join(", ")
            )
        };
        Diagnostic::warning(format!(
            "unreachable branch: {branch} in machine `{}` state `{}` can never run; {reason}",
            self.machine.name, self.state.name,
        ))
        .with_source_span(self.program.expression_table.source_span(location))
    }
}
