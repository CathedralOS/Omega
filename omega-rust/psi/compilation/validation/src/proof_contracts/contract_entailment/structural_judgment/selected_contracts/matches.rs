//! Read a contract match after substituting its exact inputs and result.
//! Selection follows authored order. An unknown earlier test cannot be skipped
//! to use a later promise, and saved payload aliases denote this one subject.

use super::{StructuralJudge, StructuralJudgment, StructuralTerm};
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, MatchPattern, TableMatchExpression};
use typed_trees::signature::StateParameter;

impl StructuralJudge<'_> {
    pub(super) fn bound_match_term(
        &self,
        expression: ExpressionHandle,
        dispatch: &TableMatchExpression,
        scope: SymbolHandle,
        parameters: &[StateParameter],
        mut operand: impl FnMut(&Self, ExpressionHandle) -> Option<StructuralTerm>,
    ) -> Option<StructuralTerm> {
        let program = self.program;
        if !crate::expression_has_closed_value_captures(program, expression) {
            return None;
        }
        let subject = operand(self, dispatch.subject)?;
        let mut selected = self.clone();
        if dispatch.capture.is_valid() {
            let capture = program.expression_table.value_capture(dispatch.capture);
            if capture.definition != expression || capture.subject != dispatch.subject {
                return None;
            }
            selected.bind_formed_match_subjects(&[(capture.value_anchor, subject.clone())]);
        }
        for arm in program.expression_table.match_arms(dispatch.arms) {
            let judgment = match arm.pattern {
                MatchPattern::Wildcard => StructuralJudgment::Proven,
                MatchPattern::Destructure(pattern) => {
                    let predicate = operand(&selected, pattern.condition)?;
                    selected.judge_observed_predicate(predicate, 0)
                }
                MatchPattern::Value(pattern) => {
                    let reference = crate::parameter_expression_result_type_reference(
                        program,
                        scope,
                        parameters,
                        dispatch.subject,
                    );
                    if let Some((definition, variant)) =
                        super::super::cases::applied_case_classifier(program, pattern, reference)
                    {
                        selected.judge_case(subject.clone(), definition.symbol, variant.symbol)
                    } else {
                        let pattern = operand(&selected, pattern)?;
                        selected.judge_equation(subject.clone(), pattern, 0)
                    }
                }
            };
            match judgment {
                StructuralJudgment::Proven => return operand(&selected, arm.value),
                StructuralJudgment::Refuted => {}
                StructuralJudgment::Unknown => return None,
            }
        }
        // A non-exhaustive dispatch has no value on the unmatched path.
        None
    }
}
