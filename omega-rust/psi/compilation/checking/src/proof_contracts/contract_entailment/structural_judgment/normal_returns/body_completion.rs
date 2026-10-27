//! Ordinary operations and terminal transfers observe the same executed body.
//! Operand evaluation has already completed here. Bind those snapshots once,
//! traverse the selected checked body, then return its current observations to
//! the caller. This grants neither mathematical denotation nor a recursive IH.

use super::super::super::citations::CitationFacts;
use super::super::super::structural_case_arms::{
    call_entries::body_can_be_observed, recognize_structural_state_leaves,
};
use super::*;

impl<'program> StructuralJudge<'program> {
    pub(super) fn completed_body_paths(
        &self,
        expression: ExpressionHandle,
        actuals: Option<&[StructuralTerm]>,
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
        writes: &NormalizedWriteFrame,
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
        // Public completion evidence and executed-body observations describe
        // the same occurrence. Capture its contract against incoming actuals,
        // then refine that occurrence with the independently observed body.
        let mut caller = self.clone();
        let post_storage =
            caller.capture_normal_call(expression, actuals, environment, scope, writes);
        let result = caller
            .normal_results
            .iter()
            .find(|(source, _)| *source == expression)
            .map(|(_, value)| value.clone());
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
        let mut diagnostics = Vec::new();
        let arms = recognize_structural_state_leaves(
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
            &mut 0,
            &mut None,
            CitationFacts::default(),
        )?;
        if !diagnostics.is_empty() {
            return None;
        }
        arms.into_iter()
            .map(|arm| {
                let mut judge = arm
                    .normal_judge
                    .complete_invocation(&caller, scope.1, &subjects, writes)?;
                if let Some(result) = &result {
                    judge.assume_normal_equation(result.clone(), arm.value.clone());
                }
                for (coordinate, value) in post_storage.iter().flatten() {
                    judge.retain_storage_value(coordinate.clone(), value.clone(), false);
                }
                std::sync::Arc::make_mut(&mut judge.normal_results)
                    .push((expression, arm.value.clone()));
                Some(NormalReturnPath::observed(arm.value, judge))
            })
            .collect()
    }
}
