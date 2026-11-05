//! Read a contract match after substituting its exact inputs and result.
//! Selection follows authored order. An unknown earlier test cannot be skipped
//! to use a later promise, and saved payload aliases denote this one subject.

use super::super::conditional_guarantees::{ConditionalGuarantees, ObservedCondition};
use super::super::normal_returns::CapturedResultEquation;
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
        operand: impl FnMut(&Self, ExpressionHandle) -> Option<StructuralTerm>,
    ) -> Option<StructuralTerm> {
        let program = self.program;
        if !crate::expression_has_closed_value_captures(program, expression) {
            return None;
        }
        self.bound_declaration_match_term(expression, dispatch, scope, parameters, operand)
    }

    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn bound_match_condition(
        &self,
        dispatch: &TableMatchExpression,
        subject: &StructuralTerm,
        pattern: MatchPattern,
        scope: SymbolHandle,
        parameters: &[StateParameter],
        operand: &mut impl FnMut(&Self, ExpressionHandle) -> Option<StructuralTerm>,
    ) -> Option<Option<ObservedCondition>> {
        Some(match pattern {
            MatchPattern::Wildcard => None,
            MatchPattern::Destructure(pattern) => Some(self.bound_observed_condition(
                pattern.condition,
                scope,
                parameters,
                operand,
            )?),
            MatchPattern::Value(pattern) => {
                let reference = crate::parameter_expression_result_type_reference(
                    self.program,
                    scope,
                    parameters,
                    dispatch.subject,
                );
                Some(
                    if let Some((definition, variant)) =
                        super::super::cases::applied_case_classifier(
                            self.program,
                            pattern,
                            reference,
                        )
                    {
                        ObservedCondition::Case {
                            subject: subject.clone(),
                            data: definition.symbol,
                            case: variant.symbol,
                        }
                    } else {
                        ObservedCondition::Equal(subject.clone(), operand(self, pattern)?)
                    },
                )
            }
        })
    }

    fn bound_observed_condition(
        &self,
        expression: ExpressionHandle,
        scope: SymbolHandle,
        parameters: &[StateParameter],
        operand: &mut impl FnMut(&Self, ExpressionHandle) -> Option<StructuralTerm>,
    ) -> Option<ObservedCondition> {
        use typed_trees::expression::BinaryOperator;
        let program = self.program;
        if let Some(binary) = program.expression_table.binary_node(expression) {
            if crate::proof_contracts::bound_expression_meaning::has_exact_declaration_case_membership_meaning(
                program, scope, parameters, expression, binary,
            ) {
                let reference = crate::parameter_expression_result_type_reference(
                    program, scope, parameters, binary.left,
                );
                let (definition, variant) =
                    super::super::cases::applied_case_classifier(program, binary.right, reference)?;
                return Some(ObservedCondition::Case {
                    subject: operand(self, binary.left)?,
                    data: definition.symbol,
                    case: variant.symbol,
                });
            }
            if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
                let left =
                    self.bound_observed_condition(binary.left, scope, parameters, operand)?;
                let mut selected = self.clone();
                left.assume(&mut selected, binary.operator == BinaryOperator::And);
                let right =
                    selected.bound_observed_condition(binary.right, scope, parameters, operand)?;
                return Some(ObservedCondition::Connective {
                    operator: binary.operator,
                    left: Box::new(left),
                    right: Box::new(right),
                });
            }
        }
        Some(ObservedCondition::Predicate(operand(self, expression)?))
    }

    /// Retain each arm's public consequences under its exact ordered guards.
    /// Hypothetical tags permit forming that arm's projections, never publishing
    /// them unconditionally. Nested matches extend guards instead of multiplying
    /// execution paths or replaying a callee body.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn capture_match_guarantees(
        &self,
        expression: ExpressionHandle,
        dispatch: &TableMatchExpression,
        scope: SymbolHandle,
        parameters: &[StateParameter],
        callable: SymbolHandle,
        bindings: &[(SymbolHandle, StructuralTerm)],
        type_bindings: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
        result: Option<&StructuralTerm>,
        established_equations: &mut Vec<(StructuralTerm, StructuralTerm)>,
        established_identities: &mut Vec<(StructuralTerm, StructuralTerm)>,
        conditional: &mut Vec<ConditionalGuarantees>,
        depth: usize,
    ) -> Option<()> {
        if !crate::expression_has_closed_value_captures(self.program, expression) {
            return None;
        }
        let mut operand = |judge: &Self, operand| {
            judge.bound_contract_term(
                operand,
                scope,
                parameters,
                callable,
                bindings,
                type_bindings,
                result,
                depth + 1,
            )
        };
        let subject = operand(self, dispatch.subject)?;
        let mut remaining = self.clone();
        if dispatch.capture.is_valid() {
            let capture = self
                .program
                .expression_table
                .value_capture(dispatch.capture);
            if capture.definition != expression || capture.subject != dispatch.subject {
                return None;
            }
            remaining.bind_formed_match_subjects(&[(capture.value_anchor, subject.clone())]);
        }
        let mut preceding = Vec::new();
        for arm in self.program.expression_table.match_arms(dispatch.arms) {
            let condition = remaining.bound_match_condition(
                dispatch,
                &subject,
                arm.pattern,
                scope,
                parameters,
                &mut operand,
            )?;
            let mut selected = remaining.clone();
            let mut conditions = preceding.clone();
            if let Some(condition) = &condition {
                condition.assume(&mut selected, true);
                conditions.push((condition.clone(), true));
            }
            let mut equations = Vec::new();
            let mut identities = Vec::new();
            let mut nested = Vec::new();
            super::capture_guarantees(
                &selected,
                arm.value,
                scope,
                parameters,
                callable,
                bindings,
                type_bindings,
                result,
                &mut equations,
                &mut identities,
                &mut nested,
                depth + 1,
            );
            if conditions.iter().all(|(condition, holds)| {
                matches!(
                    (condition.judgment(self), holds),
                    (StructuralJudgment::Proven, true) | (StructuralJudgment::Refuted, false)
                )
            }) {
                // A known selection is already an unconditional consequence in
                // this caller, including for erased mathematical applications.
                established_equations.extend(equations);
                established_identities.extend(identities);
            } else {
                let equations = equations
                    .into_iter()
                    .map(CapturedResultEquation::public)
                    .chain(
                        identities
                            .into_iter()
                            .map(CapturedResultEquation::data_identity),
                    )
                    .collect::<Vec<_>>();
                if !equations.is_empty() {
                    conditional.push(ConditionalGuarantees {
                        conditions: conditions.clone(),
                        equations,
                    });
                }
            }
            for mut guarantee in nested {
                let mut guards = conditions.clone();
                guards.append(&mut guarantee.conditions);
                guarantee.conditions = guards;
                conditional.push(guarantee);
            }
            if let Some(condition) = condition {
                condition.assume(&mut remaining, false);
                preceding.push((condition, false));
            } else {
                break;
            }
        }
        Some(())
    }
}
