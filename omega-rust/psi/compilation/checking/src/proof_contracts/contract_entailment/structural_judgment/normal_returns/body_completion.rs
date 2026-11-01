//! Ordinary operations and terminal transfers observe the same executed body.
//! Operand evaluation has already completed here. Bind those snapshots once,
//! traverse the selected checked body, then return its current observations to
//! the caller. This grants neither mathematical denotation nor a recursive IH.

use super::super::super::citations::CitationFacts;
use super::super::super::structural_case_arms::{
    StructuralArrivalLog, call_entries::body_can_be_observed, is_subordinate_machine,
    recognize_structural_state_leaves,
};
use super::*;

impl<'program> StructuralJudge<'program> {
    /// Complete a non-calling terminal arm or an implicit fallthrough. Unit
    /// closes one normal path just as a scalar result does; it is not an
    /// unsupported transfer that can invalidate observations of other arms.
    pub(in crate::proof_contracts::contract_entailment) fn terminal_completion_paths(
        &self,
        target: &typed_trees::statement::TransitionTargetNode,
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
    ) -> Option<Vec<NormalReturnPath<'program>>> {
        match target {
            typed_trees::statement::TransitionTargetNode::Value(expression) => {
                self.normal_return_paths(*expression, environment, scope)
            }
            typed_trees::statement::TransitionTargetNode::Terminal
                if matches!(
                    self.program.type_reference_table.type_reference(scope.2),
                    typed_trees::types::TypeReferenceNode::Unit
                ) =>
            {
                Some(vec![NormalReturnPath::observed(
                    StructuralTerm::Opaque("()".to_owned()),
                    self.clone(),
                )])
            }
            // An unresolved named invocation is not a completion, and Unit
            // cannot supply a result whose declared carrier is non-Unit.
            _ => None,
        }
    }

    pub(super) fn observed_body_paths(
        &self,
        expression: ExpressionHandle,
        actuals: Option<&[StructuralTerm]>,
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
        writes: &NormalizedWriteFrame,
        public_capture: Option<(
            &call_completion::PreparedNormalCall<'program, '_>,
            &super::super::selected_contracts::PreparedSelectedGuarantees<'_>,
        )>,
    ) -> Option<Vec<NormalReturnPath<'program>>> {
        if !self.requires_value_preservation() {
            return None;
        }
        let program = self.program;
        let call = program.expression_table.call_node(expression)?;
        let SelectedCallContract::Concrete(callee, entry) =
            selected_call_contract(program, scope.0.symbol, call)?
        else {
            return None;
        };
        let classification = crate::proof_only_classification(program);
        if classification.is_proof_machine(program, callee)
            || call_has_mathematical_denotation(program, scope.0.symbol, call)
            || entry.symbol == scope.1.symbol
            || self.body_invocations.contains(&entry.symbol)
            || !is_subordinate_machine(program, self.machine_symbol, callee.symbol)
            || !body_can_be_observed(program, callee, &classification, true)
            || !independent_call_application(
                program,
                self.machine_symbol,
                callee.symbol,
                typed_trees::references::StaticArgumentCall::Expression(expression, call),
            )
        {
            return None;
        }
        let subjects = self
            .frame_resolver()?
            .expression_entry_reference_subjects(
                scope.0,
                expression,
                program.state_parameters(entry),
            )
            .unwrap_or_default();
        // Preparation already froze the call inputs. Body admission remains
        // independent of whether its optional public capture was available.
        let caller = self;
        let (incoming, captured) =
            caller
                .clone()
                .bind_invocation(entry, actuals?, &subjects, environment, scope)?;
        let theorem = program.machine(self.machine_symbol)?;
        let parameter_names = self
            .declaration_parameters
            .iter()
            .map(|parameter| parameter.name.to_string())
            .collect::<Vec<_>>();
        let integer_result =
            crate::expression_result_type_reference(program, scope.0, scope.1, expression)
                .and_then(|reference| program.primitive_type_reference(reference))
                .filter(|primitive| primitive.integer_bounds().is_some());
        let mut diagnostics = Vec::new();
        // This operation reads the selected body directly, rather than through
        // application unfolding or a returned-field correspondence query.
        captured.record_returned_field_dependency(callee.symbol);
        let arms = phase_profile::scope("05.bc.recognize", || {
            recognize_structural_state_leaves(
                program,
                theorem,
                &captured,
                &classification,
                &mut diagnostics,
                program.machine_states(callee),
                entry,
                &parameter_names,
                incoming,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                true,
                &mut Vec::new(),
                &mut StructuralArrivalLog::new(),
                usize::MAX,
                &mut 0,
                &mut [],
                CitationFacts::default(),
            )
        })?;
        if !diagnostics.is_empty() {
            return None;
        }
        arms.into_iter()
            .map(|arm| {
                // Freeze scalar metadata while this completed result still
                // has its actual callee environment; leaving scope reads no home.
                let result = call_completion::CompletedBodyResult {
                    integer_range: integer_result
                        .and_then(|_| arm.normal_judge.completed_integer_range_recipe(&arm.value)),
                    value: arm.value,
                };
                let mut judge = arm
                    .normal_judge
                    .complete_invocation(caller, scope.1, &subjects, writes)?;
                let completed = public_capture.and_then(|(prepared, guarantees)| {
                    judge.capture_prepared_normal_call(prepared, guarantees, Some(&result), scope)
                });
                if let Some(completed) = completed {
                    if let Some(captured) = completed.result {
                        judge.assume_normal_equation(captured, result.value.clone());
                    }
                    for storage in completed.post_storage {
                        judge.install_post_call_storage(scope.1, storage);
                    }
                }
                std::sync::Arc::make_mut(&mut judge.normal_results)
                    .push((expression, result.value.clone()));
                Some(NormalReturnPath::observed(result.value, judge))
            })
            .collect()
    }
}
