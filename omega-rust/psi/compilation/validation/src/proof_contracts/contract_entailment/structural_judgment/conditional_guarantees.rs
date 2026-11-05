//! Completed promises whose conditions are not yet known to the caller.
//!
//! Conditions and consequences name captured values, never mutable source names.
//! Keep them outside the ordinary equation ledger until every guard is established
//! independently. Selecting one arm cannot import another arm's payload facts.

use super::normal_returns::CapturedResultEquation;
use super::{StructuralJudge, StructuralJudgment, StructuralTerm};
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees::expression::BinaryOperator;

#[derive(Clone, Debug)]
pub(super) enum ObservedCondition {
    Case {
        subject: StructuralTerm,
        data: SymbolHandle,
        case: SymbolHandle,
    },
    Predicate(StructuralTerm),
    Equal(StructuralTerm, StructuralTerm),
    Connective {
        operator: BinaryOperator,
        left: Box<Self>,
        right: Box<Self>,
    },
}

impl ObservedCondition {
    fn same_condition(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Case {
                    subject,
                    data,
                    case,
                },
                Self::Case {
                    subject: other_subject,
                    data: other_data,
                    case: other_case,
                },
            ) => {
                data == other_data
                    && case == other_case
                    && subject.same_resolution_payload(other_subject)
            }
            (Self::Predicate(value), Self::Predicate(other)) => {
                value.same_resolution_payload(other)
            }
            (Self::Equal(left, right), Self::Equal(other_left, other_right)) => {
                left.same_resolution_payload(other_left)
                    && right.same_resolution_payload(other_right)
            }
            (
                Self::Connective {
                    operator,
                    left,
                    right,
                },
                Self::Connective {
                    operator: other_operator,
                    left: other_left,
                    right: other_right,
                },
            ) => {
                operator == other_operator
                    && left.same_condition(other_left)
                    && right.same_condition(other_right)
            }
            _ => false,
        }
    }

    pub(super) fn judgment(&self, judge: &StructuralJudge<'_>) -> StructuralJudgment {
        match self {
            Self::Case {
                subject,
                data,
                case,
            } => judge.judge_case(subject.clone(), *data, *case),
            Self::Predicate(value) => judge.judge_observed_predicate(value.clone(), 0),
            Self::Equal(left, right) => {
                judge.judge_equation(judge.resolve(left.clone()), judge.resolve(right.clone()), 0)
            }
            Self::Connective {
                operator,
                left,
                right,
            } => {
                // The right observation was formed under the left test's
                // selecting polarity. Do not inspect a payload on another arm.
                match (operator, left.judgment(judge)) {
                    (BinaryOperator::And, StructuralJudgment::Refuted) => {
                        StructuralJudgment::Refuted
                    }
                    (BinaryOperator::Or, StructuralJudgment::Proven) => StructuralJudgment::Proven,
                    (BinaryOperator::And, StructuralJudgment::Proven)
                    | (BinaryOperator::Or, StructuralJudgment::Refuted) => right.judgment(judge),
                    _ => StructuralJudgment::Unknown,
                }
            }
        }
    }

    /// Used only in a detached trial. Its consequences must keep this condition
    /// as a guard when they leave the trial; this is not ambient call evidence.
    pub(super) fn assume(&self, judge: &mut StructuralJudge<'_>, holds: bool) {
        match self {
            Self::Case {
                subject,
                data,
                case,
            } => judge.intake_observed_case(subject.clone(), *data, *case, holds),
            Self::Predicate(value) => judge.intake_equation(
                value.clone(),
                super::value_predicates::boolean_term(holds),
                0,
            ),
            Self::Equal(left, right) if holds => {
                judge.intake_equation(left.clone(), right.clone(), 0)
            }
            Self::Connective {
                operator,
                left,
                right,
            } if (*operator == BinaryOperator::And && holds)
                || (*operator == BinaryOperator::Or && !holds) =>
            {
                left.assume(judge, holds);
                right.assume(judge, holds);
            }
            // A false equality or an unresolved disjunction does not choose
            // a particular value/arm. The complete condition still guards it.
            _ => {}
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct ConditionalGuarantees {
    pub(super) conditions: Vec<(ObservedCondition, bool)>,
    pub(super) equations: Vec<CapturedResultEquation>,
}

impl ConditionalGuarantees {
    pub(super) fn same_guarantee(&self, other: &Self) -> bool {
        self.conditions.len() == other.conditions.len()
            && self.conditions.iter().zip(&other.conditions).all(
                |((condition, holds), (other, other_holds))| {
                    holds == other_holds && condition.same_condition(other)
                },
            )
            && self.equations.len() == other.equations.len()
            && self
                .equations
                .iter()
                .zip(&other.equations)
                .all(|(equation, other)| equation.same_guarantee(other))
    }
}

impl StructuralJudge<'_> {
    pub(super) fn retain_conditional_guarantees(&mut self, guarantees: Vec<ConditionalGuarantees>) {
        if guarantees.is_empty() {
            return;
        }
        Arc::make_mut(&mut self.conditional_guarantees).extend(guarantees);
        self.activate_conditional_guarantees();
    }

    pub(super) fn activate_conditional_guarantees(&mut self) {
        if self.case_facts_only {
            return;
        }
        while !self.conditional_guarantees.is_empty() {
            // Temporarily detach pending promises. Consequence intake can add
            // facts but cannot recursively activate this same roster.
            let pending = std::mem::take(&mut self.conditional_guarantees);
            let mut ready = Vec::new();
            for (ordinal, guarantee) in pending.iter().enumerate() {
                let established = guarantee.conditions.iter().all(|(condition, holds)| {
                    matches!(
                        (condition.judgment(self), holds),
                        (StructuralJudgment::Proven, true) | (StructuralJudgment::Refuted, false)
                    )
                });
                if established {
                    ready.push(ordinal);
                }
            }
            if ready.is_empty() {
                self.conditional_guarantees = pending;
                return;
            }
            let mut selected = ready.into_iter().peekable();
            let mut remaining = Vec::new();
            for (ordinal, guarantee) in pending.iter().enumerate() {
                if selected.peek() == Some(&ordinal) {
                    selected.next();
                    self.intake_normal_call_equations(guarantee.equations.clone());
                } else {
                    remaining.push(guarantee.clone());
                }
            }
            // Intake may have completed additional semantic applications.
            remaining.extend(self.conditional_guarantees.iter().cloned());
            self.conditional_guarantees = Arc::new(remaining);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use numerics::bignum::BigInt;
    use typed_trees::TypedTrees;

    fn named(name: &str) -> StructuralTerm {
        StructuralTerm::Variable(name.to_owned())
    }

    fn integer(value: i64) -> StructuralTerm {
        StructuralTerm::Integer(BigInt::from_i64(value))
    }

    fn payload_judgment(judge: &StructuralJudge<'_>) -> StructuralJudgment {
        judge.judge_equation(judge.resolve(named("payload")), integer(1), 0)
    }

    fn implication(guard: &str, output: &str) -> ConditionalGuarantees {
        ConditionalGuarantees {
            conditions: vec![(ObservedCondition::Equal(named(guard), integer(1)), true)],
            equations: vec![CapturedResultEquation::public((named(output), integer(1)))],
        }
    }

    #[test]
    fn unknown_condition_neither_establishes_itself_nor_escapes_a_branch() {
        let program = TypedTrees::default();
        let mut pending = StructuralJudge::from_declaration_requires(
            &program,
            SymbolHandle::invalid(),
            &[],
            &[],
            true,
        );
        pending.retain_conditional_guarantees(vec![implication("selected", "payload")]);
        assert!(matches!(
            payload_judgment(&pending),
            StructuralJudgment::Unknown
        ));
        let mut selected = pending.clone();
        selected.intake_equation(named("selected"), integer(1), 0);
        assert!(matches!(
            payload_judgment(&selected),
            StructuralJudgment::Proven
        ));
        let mut other = pending.clone();
        other.intake_equation(named("selected"), integer(0), 0);
        assert!(matches!(
            payload_judgment(&other),
            StructuralJudgment::Unknown
        ));
        assert!(matches!(
            payload_judgment(&pending),
            StructuralJudgment::Unknown
        ));
        assert!(!selected.has_no_new_effects_or_guarantees_since(&pending));
    }

    #[test]
    fn consequence_cannot_supply_its_own_guard() {
        let program = TypedTrees::default();
        let mut judge = StructuralJudge::from_declaration_requires(
            &program,
            SymbolHandle::invalid(),
            &[],
            &[],
            true,
        );
        judge.retain_conditional_guarantees(vec![implication("payload", "payload")]);
        assert!(matches!(
            payload_judgment(&judge),
            StructuralJudgment::Unknown
        ));
        assert_eq!(judge.conditional_guarantees.len(), 1);
    }

    #[test]
    fn independently_established_guards_activate_a_chain_once() {
        let program = TypedTrees::default();
        let mut judge = StructuralJudge::from_declaration_requires(
            &program,
            SymbolHandle::invalid(),
            &[],
            &[],
            true,
        );
        judge.retain_conditional_guarantees(vec![
            implication("middle", "payload"),
            implication("selected", "middle"),
        ]);
        judge.intake_equation(named("selected"), integer(1), 0);
        assert!(matches!(
            payload_judgment(&judge),
            StructuralJudgment::Proven
        ));
        assert!(judge.conditional_guarantees.is_empty());
        let recorded = judge.captured_result_equations.len();
        judge.activate_conditional_guarantees();
        assert_eq!(judge.captured_result_equations.len(), recorded);
    }
}
